//! Pooling (ADR-0019): which texture each transient is drawn into.
//! A transient lives from the first pass that uses it to the last, in the
//! order the passes run. Transients of one format, size and usage share a
//! texture when their lifetimes do not overlap.
//!
//! This is the plan, on the CPU. The textures themselves belong to the
//! targets ([`crate::Targets::transient_textures`]), which make them on the
//! first frame that needs them and keep them for every frame after.

use std::collections::BTreeMap;

use super::{Aliases, Format, GraphError, PassDecl, Resource, Transient};

/// One texture of a graph's pool ([`crate::Graph::pool`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PooledTexture {
    pub format: Format,
    /// The targets' size divided by this, rounding up.
    pub divisor: u32,
    /// Whether a pass reads it, so that it is bound as a texture as well as
    /// drawn into. Every transient of a sorted graph is read; one of a
    /// [`crate::Graph::new`] graph may not be.
    pub sampled: bool,
    /// The transients drawn into it, in the order the frame uses them. Their
    /// lifetimes do not overlap.
    pub transients: Vec<&'static str>,
}

/// A graph's pool: its textures, and each transient's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    textures: Vec<PooledTexture>,
    /// Each transient's texture, by name.
    assigned: BTreeMap<&'static str, usize>,
}

/// When a transient is in use: the first and last passes, by running order,
/// that read or write it.
struct Lifetime {
    transient: Transient,
    first: usize,
    last: usize,
    sampled: bool,
}

impl Plan {
    /// Plans the transients of `passes`, in running order, each resource
    /// drawn where `aliases` says (a bypassed pass's write takes its input's
    /// texture). Each texture is given to the transients in the order they
    /// start, the lowest-numbered free texture of the right kind first,
    /// which gives each kind as few textures as its busiest moment needs.
    /// The declarations have passed [`check_transients`].
    pub(crate) fn new(passes: &[PassDecl], aliases: &Aliases) -> Self {
        let mut lifetimes: BTreeMap<&'static str, Lifetime> = BTreeMap::new();
        for (at, pass) in passes.iter().enumerate() {
            let reads = pass.reads.iter().map(|r| (r, true));
            let writes = pass.writes.iter().map(|r| (r, false));
            for (&resource, read) in reads.chain(writes) {
                let Resource::Transient(transient) = aliases.of(resource) else {
                    continue;
                };
                let lifetime = lifetimes.entry(transient.name).or_insert(Lifetime {
                    transient,
                    first: at,
                    last: at,
                    sampled: false,
                });
                lifetime.last = at;
                lifetime.sampled |= read;
            }
        }
        let mut lifetimes: Vec<Lifetime> = lifetimes.into_values().collect();
        lifetimes.sort_by_key(|l| (l.first, l.transient.name));

        let mut plan = Self::default();
        // For each texture, the last pass that uses it so far.
        let mut busy_until: Vec<usize> = Vec::new();
        for lifetime in lifetimes {
            let Lifetime {
                transient,
                first,
                last,
                sampled,
            } = lifetime;
            let free = plan
                .textures
                .iter()
                .zip(&busy_until)
                .position(|(t, &busy)| {
                    t.format == transient.format
                        && t.divisor == transient.divisor
                        && t.sampled == sampled
                        && busy < first
                });
            let index = free.unwrap_or_else(|| {
                plan.textures.push(PooledTexture {
                    format: transient.format,
                    divisor: transient.divisor,
                    sampled,
                    transients: Vec::new(),
                });
                busy_until.push(last);
                plan.textures.len() - 1
            });
            plan.textures[index].transients.push(transient.name);
            busy_until[index] = last;
            plan.assigned.insert(transient.name, index);
        }
        plan
    }

    pub(crate) fn textures(&self) -> &[PooledTexture] {
        &self.textures
    }

    /// The index in [`Plan::textures`] of the texture `transient` is drawn
    /// into.
    pub(crate) fn texture(&self, transient: &Transient) -> Option<usize> {
        self.assigned.get(transient.name).copied()
    }
}

/// Transients of one name agree, none is named like a built-in resource,
/// and none divides the targets' size by 0.
pub(super) fn check_transients<'a>(
    resources: impl Iterator<Item = &'a Resource>,
) -> Result<(), GraphError> {
    let mut seen: BTreeMap<&'static str, Transient> = BTreeMap::new();
    for resource in resources {
        let &Resource::Transient(transient) = resource else {
            continue;
        };
        let built_in = [Resource::Scene, Resource::Output, Resource::Surface];
        if built_in.iter().any(|r| r.name() == transient.name) {
            return Err(GraphError::TransientName {
                name: transient.name,
            });
        }
        if transient.divisor == 0 {
            return Err(GraphError::Divisor {
                name: transient.name,
            });
        }
        if *seen.entry(transient.name).or_insert(transient) != transient {
            return Err(GraphError::TransientMismatch {
                name: transient.name,
            });
        }
    }
    Ok(())
}
