//! Level 1's collision (spec 6.11, rung 2 decision 2): a uniform grid of 1 m
//! cells over static primitives, [`Wall`]s (vertical segments with a z
//! range) and [`Slab`]s (horizontal convex polygons with a z range, for
//! floors, ceilings, steps and pillars), built once from a map's sim data.
//! [`Grid`] implements `eng-collision-api`'s `CollisionWorld` and passes its
//! conformance suite.
//!
//! **Solid and free.** A hull is inside a primitive when its open box meets
//! the primitive. By the separating-axis theorem that is: strictly inside on
//! every axis of the primitive, X, Y, Z and the unit normal of each edge off
//! the X and Y axes (rounded to f32; a wall has one, a slab one per such
//! edge). On each axis the primitive's projection, grown by the hull's, is
//! an open interval `(lo, hi)`: on X it is `(min_x - hx, max_x + hx)`, off
//! the axes `(min - r, max + r)` with `r = |nx| hx + |ny| hy`. A hull on the
//! boundary only touches, and is free. The f32-rounded edge normals make each
//! solid a superset of the authored one by about 4e-8 times the edge's
//! length, so the contact promise holds against the grid's own geometry, and
//! against the authored face only within `CONTACT_GAP` for edges up to about
//! 20 m.
//!
//! **A trace**, from `s` to `e`, in f64 with IEEE basic operations only:
//!
//! 1. Each primitive is swept on each axis, projecting `s` and `e` to `ps`
//!    and `pe` (`v = pe - ps`). Moving up, it enters at `(lo - ps) / v`
//!    (never, if `ps >= hi` or `pe <= lo`; at once if `ps > lo`) and leaves at
//!    `(hi - ps) / v` (never, if `pe < hi`); moving down, the mirror image;
//!    still, it is inside throughout or never. The hull meets the primitive
//!    if the latest entry is before the earliest exit; the latest entry's
//!    axis is the hit's (ties to X, Y, Z, then the edges in order), its
//!    fraction clamped to `[0, 1)`.
//! 2. **A graze is not a hit.** A hull inside the primitive for less than
//!    [`GRAZE`] of travel, on the motion's longest axis, only passes an edge
//!    or a corner, and the primitive does not block, unless `e` is inside it.
//!    Nor is a motion along an edge off the axes that moves along its normal
//!    by no more than rounding ([`NOISE`]): the hull slides along that face,
//!    and its entry there is rounding, unless `e` is inside.
//! 3. **The stop** at fraction `t` is `s + (e - s) t` on each axis. On a hit
//!    on X, Y or Z, the hit axis is set to its face, `lo` or `hi`, exactly
//!    the boundary `overlaps` uses (ADR-0024); so is each other axis of X, Y
//!    and Z entered during the motion that rounding left short of its face
//!    (a corner). Off the axes the stop is within rounding of the face. The
//!    normal is the hit axis's, facing against the motion on it.
//! 4. **The rest of the motion**, swept from the stop against the same
//!    primitive, must meet it again, or the hit is a graze as in step 2. If
//!    it meets an edge off the axes later than at once, the hull's corner is
//!    at a corner of the primitive and that edge is what blocks, within
//!    rounding: the hit becomes the edge's. If a hit on X, Y or Z leaves the
//!    stop moved one ulp towards `e` (on every axis where they differ, or on
//!    the hit axis alone, as the conformance check does) still free, the
//!    solid ahead is thinner than an ulp, or a wedge at a vertex contact. The
//!    hit becomes that of the latest entered edge whose projection is nearer
//!    its entry face than its exit face; an edge the stop touches at its
//!    entry face within `4 * NOISE` per meter counts as entered at once (a
//!    vertex contact). With no such edge, the hit is a graze only if the
//!    solid is a sliver by depth: midway through the rest's overlap, the hull
//!    is inside by less than 2^-40 m per meter of coordinate on some axis.
//! 5. **The first hit wins:** the smallest fraction, ties to the lower
//!    primitive index (the walls in order, then the slabs). A start inside
//!    anything is `start_solid`.
//! 6. **Never inside.** If rounding leaves the stop inside primitives met at
//!    the same time, the stop moves onto the face through which the motion
//!    last enters each of them, where that face is on X, Y or Z (the lower
//!    index first on each axis). If it is still inside, the stop on the hit
//!    face moves on along the path by `ε`, doubling up to [`NUDGE_STEPS`]
//!    times; then the fraction backs off along the path by `t·ε`, doubling up
//!    to [`BACKOFF_STEPS`] times, to a free point, which is set on the hit
//!    face if that is free too. The start is free, so this ends there at the
//!    latest. A stop off the hit face would break the contact rule; since
//!    the vertex-contact rule of step 4, no test has seen one (two runs of 3
//!    million aimed proptest cases).
//!
//! The grid only finds candidates: the primitives listed in the cells under
//! the hull's swept bounds, visited in cell order (row-major, south to north,
//! west to east), then index order, each once. Since the winner is chosen by
//! (fraction, index), the order changes nothing, and the brute-force oracle
//! (`tests/oracle.rs`, every primitive, no grid) gives the same `Trace` bits.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod grid;
mod shape;
mod sweep;

pub use grid::Grid;
pub use shape::{Point2, Slab, Wall};

use core::fmt;

/// The largest coordinate a primitive may have, on any axis (m): 2^20 m,
/// about 1,000 km, where f64's spacing is 2^-32 m.
pub const MAX_COORD: f64 = 1_048_576.0;

/// The most cells a grid may have.
pub const MAX_CELLS: u64 = 1 << 22;

/// The most corners a slab may have.
pub const MAX_CORNERS: usize = 64;

/// The most (cell, primitive) entries a grid may hold.
pub const MAX_ENTRIES: u64 = 1 << 24;

/// How many times a stop that rounding left inside solid backs off (step 6
/// of a trace), enough to reach the start from any fraction.
pub const BACKOFF_STEPS: usize = 64;

/// How many times a stop that rounding left inside solid moves on along the
/// path before it backs off (step 6), from 2^-52 doubling: at most 2^-36 of
/// the motion.
pub const NUDGE_STEPS: usize = 16;

/// The least travel inside a primitive that counts as a hit (m), on the
/// motion's longest axis: 2^-24 m, about 60 nm (step 2 of a trace). Less
/// is a graze past an edge or a corner. It is far above rounding (2^-32 m
/// at [`MAX_COORD`]) and far below `CONTACT_GAP`.
pub const GRAZE: f64 = 1.0 / 16_777_216.0;

/// The rounding allowed per meter of coordinate in a projection onto an
/// axis off X and Y (step 2): 2^-48, a few ulps. A motion along such an
/// axis by no more than `(|x| + |y| + 1) * NOISE` at each end only slides.
pub const NOISE: f64 = 1.0 / 281_474_976_710_656.0;

/// How far beyond the hull's swept bounds the grid looks for candidates (m),
/// so rounding cannot hide one.
const MARGIN: f64 = 1.0 / 1024.0;

/// Why [`Grid::new`] refused a map. `primitive` counts the walls, then the
/// slabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridError {
    /// A coordinate that is not finite, or beyond [`MAX_COORD`].
    Coordinate { primitive: usize },
    /// A bottom above its top.
    Heights { primitive: usize },
    /// A wall whose ends are the same point.
    ZeroLength { primitive: usize },
    /// A slab with fewer than three corners or more than [`MAX_CORNERS`], or
    /// not strictly convex.
    NotConvex { primitive: usize },
    /// More than [`MAX_CELLS`] cells.
    TooLarge { cells: u64 },
    /// More than [`MAX_ENTRIES`] cell entries.
    TooDense { entries: u64 },
    /// More primitives than a `u32` can count.
    TooMany { primitives: usize },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinate { primitive } => write!(
                f,
                "primitive {primitive}: a coordinate is not finite or beyond {MAX_COORD} m"
            ),
            Self::Heights { primitive } => write!(f, "primitive {primitive}: bottom above top"),
            Self::ZeroLength { primitive } => {
                write!(f, "primitive {primitive}: a wall's ends are the same point")
            }
            Self::NotConvex { primitive } => write!(
                f,
                "primitive {primitive}: a slab needs 3 to {MAX_CORNERS} corners, strictly convex"
            ),
            Self::TooLarge { cells } => {
                write!(f, "the grid would have {cells} cells (max {MAX_CELLS})")
            }
            Self::TooDense { entries } => {
                write!(
                    f,
                    "the grid would hold {entries} entries (max {MAX_ENTRIES})"
                )
            }
            Self::TooMany { primitives } => write!(f, "{primitives} primitives is too many"),
        }
    }
}

impl core::error::Error for GridError {}
