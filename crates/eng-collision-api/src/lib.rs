//! The collision contract (spec 6.11): traces and overlaps of a hull against
//! the world, in the style of Quake 3's `bg_pmove`.
//!
//! Level 1's implementation is `eng-collision-grid` (rung 2). [`BoxRoom`],
//! the inside of one axis-aligned room, is the trivial one. Every
//! implementation must pass [`conformance::check`].
//!
//! **Traces stop at contact** (ADR-0024). Touching is free: a hull whose
//! boundary meets solid, with none of its inside in it, does not overlap. A
//! blocked trace ends free and in contact with what it hit:
//!
//! - **on an axis** (the hit's normal is ±X, ±Y or ±Z), exactly on the
//!   boundary the world's own [`CollisionWorld::overlaps`] uses, so the rest
//!   of the motion hits at once and moves nothing;
//! - **off the axes**, within [`CONTACT_GAP`] of the face, measured along
//!   the hit's normal.
//!
//! `kit-fps`'s ground check relies on it: a hull that lands stands, and its
//! next probe down hits.
#![no_std]
#![forbid(unsafe_code)]

pub mod conformance;
mod room;

pub use room::BoxRoom;

use eng_math::{Vec3, WorldPos};
use eng_schema_core::Schema;

/// How far from a face off the axes a blocked trace may stop, measured along
/// the hit's normal (m): 2^-20 m, about 1 µm (ADR-0024). A face square to an
/// axis allows no gap at all.
///
/// It bounds what rounding leaves: a sweep's few f64 operations at room
/// scale err by a few times f64's spacing, 2^-43 m at 1 km.
pub const CONTACT_GAP: f64 = 1.0 / 1_048_576.0;

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
    /// Where the hull stopped. Never inside solid. After a hit, in contact
    /// with what it hit: exactly on the boundary for a normal on an axis,
    /// within [`CONTACT_GAP`] along the normal otherwise (ADR-0024).
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
    /// Sweeps `hull` from `start` towards `end`. A blocked trace stops in
    /// contact with what it hit, and the rest of the motion, traced from
    /// there, hits again (ADR-0024; see the crate docs).
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace;

    /// Whether a hull at `at` would be inside solid. A hull that only
    /// touches solid is not.
    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool;
}
