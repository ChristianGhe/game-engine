//! A small plain-data ECS world (spec 6.5).
//!
//! - Entities are generational ids from a deterministic free list.
//! - Components, resources and events are plain-data [`Schema`] types, each
//!   registered under its stable name. Components are stored densely per type
//!   in sparse sets; iteration follows dense order, which is part of the state.
//! - Systems are plain functions (see `eng-sim`). Structural changes made
//!   while iterating go through [`Deferred`] and apply at the stage's sync point.
//! - The whole world can be encoded canonically ([`World::visit_state`]),
//!   restored ([`World::decode_state`]) and dumped for agents
//!   ([`World::visit_dump`]).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod deferred;
mod state;
mod storage;
mod world;

pub use deferred::Deferred;
pub use eng_schema_core::Schema;
pub use storage::Storage;
pub use world::World;

use eng_core::GenId;

/// An entity id. Stale ids (after despawn) never match a live entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Schema)]
#[schema(name = "eng.ecs.Entity", version = 1)]
pub struct Entity {
    pub index: u32,
    pub generation: u32,
}

impl From<GenId> for Entity {
    fn from(id: GenId) -> Self {
        Self {
            index: id.index,
            generation: id.generation,
        }
    }
}

impl From<Entity> for GenId {
    fn from(e: Entity) -> Self {
        Self {
            index: e.index,
            generation: e.generation,
        }
    }
}

impl core::fmt::Display for Entity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}v{}", self.index, self.generation)
    }
}
