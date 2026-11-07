//! Bypassing for [`super::Graph::sorted_without`] (rung 2 plan, decision 5;
//! ADR-0019): a switched-off pass that may be bypassed is removed,
//! and its write takes the texture of its input, its first read, so its
//! readers read the input. A built-in resource keeps its own texture: with
//! the palette pass switched off, `toned` is drawn into `output`.

use std::collections::BTreeMap;

use super::{Format, GraphError, PassNode, Resource};

/// The resource whose texture each bypassed resource takes; any other
/// resource keeps its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Aliases(BTreeMap<Resource, Resource>);

impl Aliases {
    /// The resource whose texture `resource` is drawn into.
    pub(crate) fn of(&self, resource: Resource) -> Resource {
        self.0.get(&resource).copied().unwrap_or(resource)
    }
}

/// Which passes are switched off, and the textures their writes take.
pub(super) struct Bypass {
    /// By declaration.
    pub(super) off: Vec<bool>,
    pub(super) aliases: Aliases,
}

/// A bypassable pass reads at least one resource, writes exactly one, and
/// continues no pass; that none continues it is checked with `continues`.
pub(super) fn check_shapes(nodes: &[PassNode], by_name: &[usize]) -> Result<(), GraphError> {
    for &i in by_name {
        let node = &nodes[i];
        if !node.bypassable {
            continue;
        }
        let reason = if node.reads.is_empty() {
            "it reads nothing, so it has no input to pass on"
        } else if node.writes.len() != 1 {
            "it writes more than one resource, and only one can take its input's texture"
        } else if node.continues.is_some() {
            "it continues another pass, whose targets it would leave half drawn"
        } else {
            continue;
        };
        return Err(GraphError::Unbypassable {
            pass: node.name,
            reason,
        });
    }
    Ok(())
}

/// Each bypassable pass's write has its input's format and size, so that it
/// can take the input's texture. Checked whether or not the pass is switched
/// off, so that switching it off cannot fail later.
pub(super) fn check_formats(nodes: &[PassNode], by_name: &[usize]) -> Result<(), GraphError> {
    for &i in by_name {
        let node = &nodes[i];
        if !node.bypassable {
            continue;
        }
        let (input, output) = (node.reads[0], node.writes[0]);
        if texture(input) != texture(output) {
            return Err(GraphError::BypassMismatch {
                pass: node.name,
                input,
                output,
            });
        }
    }
    Ok(())
}

/// A resource's texture format and divisor; the surface's format is none of
/// [`Format`]'s.
fn texture(resource: Resource) -> (Option<Format>, u32) {
    match resource {
        Resource::Scene => (Some(Format::Rgba16Float), 1),
        Resource::Output => (Some(Format::Rgba8UnormSrgb), 1),
        Resource::Surface => (None, 1),
        Resource::Transient(transient) => (Some(transient.format), transient.divisor),
    }
}

/// The passes `off` names, which must be declared and bypassable, and the
/// texture each of their writes takes. Called after the declarations have
/// passed [`super::order::sort`], so the bypasses form no cycle.
pub(super) fn plan(nodes: &[PassNode], off: &[&'static str]) -> Result<Bypass, GraphError> {
    let mut names = off.to_vec();
    names.sort_unstable();
    names.dedup();
    let mut switched = vec![false; nodes.len()];
    // parent[w] = the input whose texture a bypassed pass's write w takes.
    let mut parent: BTreeMap<Resource, Resource> = BTreeMap::new();
    for name in names {
        let Some(i) = nodes.iter().position(|node| node.name == name) else {
            return Err(GraphError::UnknownSwitchedOff { pass: name });
        };
        if !nodes[i].bypassable {
            return Err(GraphError::NotBypassable { pass: name });
        }
        switched[i] = true;
        parent.insert(nodes[i].writes[0], nodes[i].reads[0]);
    }
    // Resources joined by bypasses share one texture: their root's, the
    // input no bypassed pass writes, unless a built-in resource is among
    // them. Bypasses join only resources of one format and size, and the
    // built-in resources' differ, so there is at most one.
    let root = |mut resource: Resource| {
        // At most one step per bypass, even if they formed a loop.
        for _ in 0..parent.len() {
            match parent.get(&resource) {
                Some(&up) => resource = up,
                None => break,
            }
        }
        resource
    };
    let mut classes: BTreeMap<Resource, Vec<Resource>> = BTreeMap::new();
    for (&write, &input) in &parent {
        for member in [write, input] {
            let class = classes.entry(root(member)).or_default();
            if !class.contains(&member) {
                class.push(member);
            }
        }
    }
    let mut aliases = BTreeMap::new();
    for (root, members) in classes {
        let texture = members
            .iter()
            .copied()
            .find(|r| !matches!(r, Resource::Transient(_)))
            .unwrap_or(root);
        for member in members {
            if member != texture {
                aliases.insert(member, texture);
            }
        }
    }
    Ok(Bypass {
        off: switched,
        aliases: Aliases(aliases),
    })
}
