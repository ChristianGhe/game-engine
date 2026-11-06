//! Culling for [`super::Graph::sorted`] (ADR-0019, proposed): a pass runs only
//! if what it writes reaches the output or the surface, through the passes
//! that read it or continue it.

use std::collections::BTreeSet;

use super::{Aliases, PassNode, Resource};

/// For each declaration, whether it runs: it is `live` (not bypassed), and
/// it writes the output or the surface (perhaps through a bypassed pass's
/// write, `aliases`), or a pass that runs comes after it (`after`, from the
/// sort).
pub(super) fn kept(
    nodes: &[PassNode],
    after: &[BTreeSet<usize>],
    live: &[bool],
    aliases: &Aliases,
) -> Vec<bool> {
    let mut keep = vec![false; nodes.len()];
    let mut pending: Vec<usize> = (0..nodes.len())
        .filter(|&i| {
            live[i]
                && nodes[i]
                    .writes
                    .iter()
                    .any(|&r| matches!(aliases.of(r), Resource::Output | Resource::Surface))
        })
        .collect();
    while let Some(i) = pending.pop() {
        if !keep[i] {
            keep[i] = true;
            pending.extend(after[i].iter().copied());
        }
    }
    keep
}
