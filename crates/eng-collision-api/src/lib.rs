//! The collision contract (spec 6.11): traces and overlaps of a hull against
//! the world, in the style of Quake 3's `bg_pmove`.
//!
//! Level 1's real implementation (`eng-collision-grid`) arrives at rung 2.
//! Until then [`BoxRoom`], the inside of one axis-aligned room, stands in.
//! Every implementation must pass [`conformance::check`].
#![no_std]
#![forbid(unsafe_code)]

pub mod conformance;
mod room;

pub use room::BoxRoom;

use eng_math::{Vec3, WorldPos};
use eng_schema_core::Schema;

/// A vertical, axis-aligned box hull centred on its position.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "eng.collision.BoxHull", version = 1)]
pub struct BoxHull {
    pub half_extents: Vec3,
}

/// The result of sweeping a hull from `start` towards `end`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trace {
    /// How far the hull got, from 0 (blocked at once) to 1 (no hit).
    pub fraction: f64,
    /// Where the hull stopped. Never inside solid.
    pub end: WorldPos,
    /// The surface normal at the hit, or zero when nothing was hit.
    pub normal: Vec3,
    /// The hull started inside solid; it did not move.
    pub start_solid: bool,
}

impl Trace {
    pub fn hit(&self) -> bool {
        self.fraction < 1.0
    }
}

/// Something a hull can be swept against.
pub trait CollisionWorld {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace;

    /// Whether a hull at `at` would be inside solid.
    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool;
}
