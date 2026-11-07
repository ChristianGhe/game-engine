//! Ordering for [`super::Graph::sorted`] (ADR-0019): the checks on
//! the declarations, then a topological sort over reads, writes and
//! continues, in which the smallest name runs first among the passes free
//! to run.
//!
//! Every check walks the passes in name order, so the error, like the order,
//! does not depend on the order of declaration.

use std::collections::{BTreeMap, BTreeSet};

use super::{Aliases, GraphError, PassNode, Resource, Target, bypass, pool};

/// Passes in the order they run, before culling.
pub(super) struct Sorted {
    /// Indexes into the declarations, in running order.
    pub(super) order: Vec<usize>,
    /// For each declaration, those it runs directly after: the pass it
    /// continues, and the last writer of each resource it reads.
    pub(super) after: Vec<BTreeSet<usize>>,
}

/// Checks the declarations and sorts all of them, as declared: no pass is
/// switched off and no resource takes another's texture.
pub(super) fn sort(nodes: &[PassNode], target: Target) -> Result<Sorted, GraphError> {
    let mut by_name: Vec<usize> = (0..nodes.len()).collect();
    by_name.sort_by_key(|&i| nodes[i].name);
    if let Some(pair) = by_name
        .windows(2)
        .find(|pair| nodes[pair[0]].name == nodes[pair[1]].name)
    {
        return Err(GraphError::DuplicateName {
            pass: nodes[pair[0]].name,
        });
    }
    let named = names(nodes);
    for &i in &by_name {
        own_checks(&nodes[i], target)?;
    }
    for &i in &by_name {
        continued(nodes, &named, i)?;
    }
    bypass::check_shapes(nodes, &by_name)?;
    pool::check_transients(
        by_name
            .iter()
            .flat_map(|&i| nodes[i].reads.iter().chain(nodes[i].writes)),
    )?;
    bypass::check_formats(nodes, &by_name)?;
    let written: BTreeSet<Resource> = nodes
        .iter()
        .flat_map(|node| node.writes.iter().copied())
        .collect();
    for &i in &by_name {
        if let Some(&resource) = nodes[i].reads.iter().find(|r| !written.contains(r)) {
            return Err(GraphError::Unwritten {
                pass: nodes[i].name,
                resource,
            });
        }
    }
    if !written.contains(&Resource::Output) {
        return Err(GraphError::OutputNotWritten);
    }
    if target == Target::Surface && !written.contains(&Resource::Surface) {
        return Err(GraphError::SurfaceNotWritten);
    }
    arrange(nodes, &vec![true; nodes.len()], &Aliases::default())
}

/// Sorts the `live` passes, each resource seen as the one whose texture it
/// takes (`aliases`). After [`sort`] has passed, the only errors left are
/// two writers of one resource and a cycle among the declarations; with
/// passes bypassed, neither can arise anew.
pub(super) fn arrange(
    nodes: &[PassNode],
    live: &[bool],
    aliases: &Aliases,
) -> Result<Sorted, GraphError> {
    let named = names(nodes);
    let mut by_name: Vec<usize> = (0..nodes.len()).filter(|&i| live[i]).collect();
    by_name.sort_by_key(|&i| nodes[i].name);
    // Each resource's writers, in name order.
    let mut writers: BTreeMap<Resource, Vec<usize>> = BTreeMap::new();
    for &i in &by_name {
        for &resource in nodes[i].writes {
            writers.entry(aliases.of(resource)).or_default().push(i);
        }
    }
    let mut last: BTreeMap<Resource, usize> = BTreeMap::new();
    for (&resource, chain) in &writers {
        if let Some(end) = chain_end(nodes, &named, resource, chain)? {
            last.insert(resource, end);
        }
    }
    let mut after = vec![BTreeSet::new(); nodes.len()];
    for &i in &by_name {
        let node = &nodes[i];
        if let Some(&continued) = node.continues.and_then(|name| named.get(name)) {
            after[i].insert(continued);
        }
        for &resource in node.reads {
            if let Some(&writer) = last.get(&aliases.of(resource)) {
                after[i].insert(writer);
            }
        }
    }
    let order = topological(nodes, &after, live)?;
    Ok(Sorted { order, after })
}

fn names(nodes: &[PassNode]) -> BTreeMap<&'static str, usize> {
    nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.name, i))
        .collect()
}

/// The rules [`super::Graph::new`] applies to each pass that do not depend
/// on the order: the surface offscreen and the output as the surface; and
/// at least one write, none twice.
fn own_checks(node: &PassNode, target: Target) -> Result<(), GraphError> {
    let uses_surface = node
        .reads
        .iter()
        .chain(node.writes)
        .any(|r| *r == Resource::Surface);
    if uses_surface && target == Target::Offscreen {
        return Err(GraphError::SurfaceOffscreen { pass: node.name });
    }
    if node.writes.contains(&Resource::Output) && node.writes.contains(&Resource::Surface) {
        return Err(GraphError::OutputIsSurface { pass: node.name });
    }
    if node.writes.is_empty() {
        return Err(GraphError::WritesNothing { pass: node.name });
    }
    for (k, resource) in node.writes.iter().enumerate() {
        if node.writes[..k].contains(resource) {
            return Err(GraphError::WritesTwice {
                pass: node.name,
                resource: *resource,
            });
        }
    }
    Ok(())
}

/// The pass that `nodes[i]` continues exists, writes everything `nodes[i]`
/// writes, and may not be bypassed.
fn continued(
    nodes: &[PassNode],
    named: &BTreeMap<&'static str, usize>,
    i: usize,
) -> Result<(), GraphError> {
    let node = &nodes[i];
    let Some(continues) = node.continues else {
        return Ok(());
    };
    let Some(&j) = named.get(continues) else {
        return Err(GraphError::UnknownContinued {
            pass: node.name,
            continues,
        });
    };
    if let Some(&resource) = node.writes.iter().find(|r| !nodes[j].writes.contains(r)) {
        return Err(GraphError::ContinuesOther {
            pass: node.name,
            resource,
            continues,
        });
    }
    if nodes[j].bypassable && j != i {
        return Err(GraphError::ContinuesBypassable {
            pass: node.name,
            continues,
        });
    }
    Ok(())
}

/// The last of the passes that write `resource` (`chain`, in name order),
/// which its readers run after: one starts it, and each other continues
/// another, which no third pass that writes it continues. `None` if every
/// writer continues another, which is a cycle that the sort reports.
fn chain_end(
    nodes: &[PassNode],
    named: &BTreeMap<&'static str, usize>,
    resource: Resource,
    chain: &[usize],
) -> Result<Option<usize>, GraphError> {
    let two = |first: usize, second: usize| GraphError::TwoWriters {
        resource,
        first: nodes[first].name,
        second: nodes[second].name,
    };
    let starts: Vec<usize> = chain
        .iter()
        .copied()
        .filter(|&i| nodes[i].continues.is_none())
        .collect();
    if let [first, second, ..] = starts[..] {
        return Err(two(first, second));
    }
    // continued_by[c] = the pass that continues c, among those writing
    // `resource`. A continued pass writes all its continuer writes.
    let mut continued_by: BTreeMap<usize, usize> = BTreeMap::new();
    for &i in chain {
        let Some(&c) = nodes[i].continues.and_then(|name| named.get(name)) else {
            continue;
        };
        if let Some(&other) = continued_by.get(&c) {
            return Err(two(other, i));
        }
        continued_by.insert(c, i);
    }
    // From the start, each pass continues the one before, so the walk ends:
    // a pass met twice would continue two passes.
    Ok(starts.first().map(|&start| {
        let mut end = start;
        while let Some(&next) = continued_by.get(&end) {
            end = next;
        }
        end
    }))
}

/// Kahn's sort of the `live` passes, taking the ready pass with the
/// smallest name each time.
fn topological(
    nodes: &[PassNode],
    after: &[BTreeSet<usize>],
    live: &[bool],
) -> Result<Vec<usize>, GraphError> {
    let mut waiting: Vec<usize> = after.iter().map(BTreeSet::len).collect();
    let mut before: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (i, earlier) in after.iter().enumerate() {
        for &j in earlier {
            before[j].push(i);
        }
    }
    let mut ready: BTreeSet<(&'static str, usize)> = (0..nodes.len())
        .filter(|&i| live[i] && waiting[i] == 0)
        .map(|i| (nodes[i].name, i))
        .collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some((_, i)) = ready.pop_first() {
        order.push(i);
        for &j in &before[i] {
            waiting[j] -= 1;
            if waiting[j] == 0 {
                ready.insert((nodes[j].name, j));
            }
        }
    }
    if order.len() == live.iter().filter(|&&l| l).count() {
        Ok(order)
    } else {
        Err(GraphError::Cycle {
            passes: cycle(nodes, after, &waiting),
        })
    }
}

/// One cycle among the passes the sort could not place (`waiting` above 0),
/// each running after the one before it, from the smallest name. Each such
/// pass waits for another such pass, so walking from one to the pass it
/// waits for (the smallest name first) never stops, and comes back to a pass
/// it has met.
fn cycle(nodes: &[PassNode], after: &[BTreeSet<usize>], waiting: &[usize]) -> Vec<&'static str> {
    let smallest_stuck = |candidates: &mut dyn Iterator<Item = usize>| {
        candidates
            .filter(|&i| waiting[i] > 0)
            .min_by_key(|&i| nodes[i].name)
    };
    let Some(mut at) = smallest_stuck(&mut (0..nodes.len())) else {
        return Vec::new();
    };
    let mut walk: Vec<usize> = Vec::new();
    while !walk.contains(&at) {
        walk.push(at);
        match smallest_stuck(&mut after[at].iter().copied()) {
            Some(next) => at = next,
            None => break,
        }
    }
    let from = walk.iter().position(|&i| i == at).unwrap_or(0);
    // The walk goes from a pass to one it runs after; a cycle reads the
    // other way.
    let mut passes: Vec<&'static str> = walk[from..].iter().rev().map(|&i| nodes[i].name).collect();
    if let Some(first) = (0..passes.len()).min_by_key(|&k| passes[k]) {
        passes.rotate_left(first);
    }
    passes
}
