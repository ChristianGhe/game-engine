//! No pass-through, checked without sampling (rung 2, T23): the hull,
//! shrunk by [`THROUGH`] on every side, must be free on the whole path from a
//! trace's start to its stop. Included by `#[path]` from `../contract.rs`
//! and `../found.rs`.
//!
//! Three checks, each catching what another may not:
//! - the shrunk hull's own trace from the start to the stop hits nothing.
//!   Exact for any thickness of solid, but it is the grid's sweep again: a
//!   rule that lets the hull through a solid lets the shrunk hull through
//!   too. It finds a solid passed by a wrong choice among the hits (a stop
//!   at a later one), not one the sweep itself skips;
//! - the path clipped against each primitive grown by the shrunk hull, in
//!   plain arithmetic that shares none of the sweep's rules (no graze, slide,
//!   sliver or nudge): where any of it is left, `overlaps` at its midpoint
//!   must be false. The grown primitive is the intersection of slabs on X,
//!   Y, Z and each edge's normal (the Minkowski sum of a box and a convex
//!   prism has no other faces), so a solid of any thickness is found;
//! - `overlaps` at [`SAMPLES`] evenly spaced points from the start to the
//!   hit fraction, which needs neither of the above to be right.
//!
//! [`THROUGH`] is far deeper than any rounding the sweep allows (`GRAZE`,
//! `NOISE`, slivers), and far shallower than any solid the grid stores.

use eng_collision_api::{BoxHull, CollisionWorld, Trace};
use eng_collision_grid::{Grid, Point2};
use eng_math::{Vec3, WorldPos, max_f64, min_f64};

/// How far inside solid a point on the way may be (m).
pub const THROUGH: f32 = 1.0e-6;
/// The samples of the third check.
pub const SAMPLES: u32 = 32;

/// `hull` shrunk by [`THROUGH`] on every side.
pub fn shrunk(hull: &BoxHull) -> BoxHull {
    let h = hull.half_extents;
    BoxHull {
        half_extents: Vec3::new(h.x - THROUGH, h.y - THROUGH, h.z - THROUGH),
    }
}

/// Panics, naming the check, if `tr` (`world`'s trace of `hull` from `from`
/// to `to`, from a free start) passes through solid before its stop.
pub fn assert_no_pass_through(
    world: &Grid,
    from: WorldPos,
    to: WorldPos,
    tr: &Trace,
    hull: &BoxHull,
) {
    let small = shrunk(hull);
    let again = world.trace(from, tr.end, &small);
    assert!(
        !again.hit() && !again.start_solid,
        "the trace from {from:?} to {to:?} ({tr:?}) passes through solid: the hull shrunk by {THROUGH} m, traced to the stop, hits {again:?}"
    );
    if let Some(p) = solid_on_path(world, from, tr.end, &small) {
        panic!(
            "the trace from {from:?} to {to:?} ({tr:?}) passes through solid: the path to the stop, clipped against the primitives, is inside solid at {p:?}"
        );
    }
    for i in 0..=SAMPLES {
        let t = tr.fraction * f64::from(i) / f64::from(SAMPLES);
        let p = WorldPos::new(
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t,
            from.z + (to.z - from.z) * t,
        );
        assert!(
            !world.overlaps(p, &small),
            "the trace from {from:?} to {to:?} ({tr:?}) passes through solid at sample {p:?}"
        );
    }
}

/// A point on the segment from `a` to `b` where `hull` is inside solid, as
/// `world.overlaps` says, found by clipping the segment against each
/// primitive grown by `hull`; `None` if `overlaps` is false at the midpoint
/// of every clipped piece.
pub fn solid_on_path(world: &Grid, a: WorldPos, b: WorldPos, hull: &BoxHull) -> Option<WorldPos> {
    let h = hull.half_extents;
    let (hx, hy, hz) = (f64::from(h.x), f64::from(h.y), f64::from(h.z));
    let walls = world
        .walls()
        .iter()
        .map(|w| (vec![w.a, w.b], w.bottom, w.top));
    let slabs = world
        .slabs()
        .iter()
        .map(|s| (s.points.clone(), s.bottom, s.top));
    for (points, bottom, top) in walls.chain(slabs) {
        // The piece of the segment, as fractions `(t0, t1)`, strictly inside
        // the grown primitive on every axis clipped so far.
        let (mut t0, mut t1) = (0.0, 1.0);
        let mut clip = |pa: f64, pb: f64, lo: f64, hi: f64| {
            let v = pb - pa;
            if v == 0.0 {
                if !(lo < pa && pa < hi) {
                    t1 = f64::NEG_INFINITY;
                }
            } else {
                let (u, w) = ((lo - pa) / v, (hi - pa) / v);
                let (enter, leave) = if v > 0.0 { (u, w) } else { (w, u) };
                t0 = max_f64(t0, enter);
                t1 = min_f64(t1, leave);
            }
        };
        // An edge normal (not of unit length) and its projections; X and Y
        // are the normals (1, 0) and (0, 1).
        let mut normals = vec![(1.0, 0.0), (0.0, 1.0)];
        for (i, p) in points.iter().enumerate() {
            let q = points[(i + 1) % points.len()];
            normals.push((p.y - q.y, q.x - p.x));
        }
        for (nx, ny) in normals {
            let project = |p: &Point2| nx * p.x + ny * p.y;
            let lo = points.iter().map(project).fold(f64::INFINITY, min_f64);
            let hi = points.iter().map(project).fold(f64::NEG_INFINITY, max_f64);
            let r = nx.abs() * hx + ny.abs() * hy;
            clip(nx * a.x + ny * a.y, nx * b.x + ny * b.y, lo - r, hi + r);
        }
        clip(a.z, b.z, bottom - hz, top + hz);
        if t0 < t1 {
            let t = (t0 + t1) * 0.5;
            let p = WorldPos::new(
                a.x + (b.x - a.x) * t,
                a.y + (b.y - a.y) * t,
                a.z + (b.z - a.z) * t,
            );
            if world.overlaps(p, hull) {
                return Some(p);
            }
        }
    }
    None
}
