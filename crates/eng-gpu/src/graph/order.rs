//! Ordering for [`super::Graph::sorted`] (ADR-0019, proposed): the checks on
//! the declarations, then a topological sort over reads, writes and
//! continues, in which the smallest name runs first among the passes free
//! to run.
//!
//! Every check walks the passes in name order, so the error, like the order,
//! does not depend on the order of declaration.

use std::collections::{BTreeMap, BTreeSet};

use super::{GraphError, PassNode, Resource, Target, pool};

/// The declared passes in the order they run, before culling.
pub(super) struct Sorted {
    /// Indexes into the declarations, in running order.
    pub(super) order: Vec<usize>,
    /// For each declaration, those it runs directly after: the pass it
    /// continues, and the last writer of each resource it reads.
    pub(super) after: Vec<BTreeSet<usize>>,
}

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
    let named: BTreeMap<&'static str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.name, i))
        .collect();
    for &i in &by_name {
        own_checks(&nodes[i], target)?;
    }
    for &i in &by_name {
        continued(nodes, &named, i)?;
    }
    pool::check_transients(
        by_name
            .iter()
            .flat_map(|&i| nodes[i].reads.iter().chain(nodes[i].writes)),
    )?;

    // Each resource's writers, in name order.
    let mut writers: BTreeMap<Resource, Vec<usize>> = BTreeMap::new();
    for &i in &by_name {
        for &resource in nodes[i].writes {
            writers.entry(resource).or_default().push(i);
        }
    }
    for &i in &by_name {
        if let Some(&resource) = nodes[i].reads.iter().find(|r| !writers.contains_key(r)) {
            return Err(GraphError::Unwritten {
                pass: nodes[i].name,
                resource,
            });
        }
    }
    let mut last: BTreeMap<Resource, usize> = BTreeMap::new();
    for (&resource, chain) in &writers {
        if let Some(end) = chain_end(nodes, &named, resource, chain)? {
            last.insert(resource, end);
        }
    }
    if !writers.contains_key(&Resource::Output) {
        return Err(GraphError::OutputNotWritten);
    }
    if target == Target::Surface && !writers.contains_key(&Resource::Surface) {
        return Err(GraphError::SurfaceNotWritten);
    }

    let mut after = vec![BTreeSet::new(); nodes.len()];
    for (i, node) in nodes.iter().enumerate() {
        if let Some(&continued) = node.continues.and_then(|name| named.get(name)) {
            after[i].insert(continued);
        }
        for resource in node.reads {
            if let Some(&writer) = last.get(resource) {
                after[i].insert(writer);
            }
        }
    }
    let order = topological(nodes, &after)?;
    Ok(Sorted { order, after })
}

/// The rules [`super::Graph::new`] applies to each pass that do not depend
/// on the order: the surface offscreen, the output as the surface, and one
/// write.
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
    if node.writes.len() != 1 {
        return Err(GraphError::Writes {
            pass: node.name,
            count: node.writes.len(),
        });
    }
    Ok(())
}

/// The pass that `nodes[i]` continues exists and writes what it writes. Each
/// pass writes one resource by now.
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
    if nodes[j].writes != node.writes {
        return Err(GraphError::ContinuesOther {
            pass: node.name,
            writes: node.writes[0],
            continues,
            continued_writes: nodes[j].writes[0],
        });
    }
    Ok(())
}

/// The last of the passes that write `resource` (`chain`, in name order),
/// which its readers run after: one starts it, and each other continues
/// another, which no third pass continues. `None` if every writer continues
/// another, which is a cycle that the sort reports.
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
    // continued_by[c] = the pass that continues c.
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

/// Kahn's sort, taking the ready pass with the smallest name each time.
fn topological(nodes: &[PassNode], after: &[BTreeSet<usize>]) -> Result<Vec<usize>, GraphError> {
    let mut waiting: Vec<usize> = after.iter().map(BTreeSet::len).collect();
    let mut before: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (i, earlier) in after.iter().enumerate() {
        for &j in earlier {
            before[j].push(i);
        }
    }
    let mut ready: BTreeSet<(&'static str, usize)> = (0..nodes.len())
        .filter(|&i| waiting[i] == 0)
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
    if order.len() == nodes.len() {
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
