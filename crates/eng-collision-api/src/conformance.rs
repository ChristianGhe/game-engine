//! Rules every [`CollisionWorld`] must follow. Each implementation's tests
//! call [`check`] with probes from its own geometry.
//!
//! Besides the basic rules, a blocked trace must end in contact (ADR-0024):
//! [`check`] traces the rest of the motion from the hit's end, which must hit
//! again without starting in solid. Then, for a normal on an axis, the rest
//! hits at fraction 0 and leaves the end where it is, and one ulp further
//! towards the end point is inside solid. For a normal off the axes, the rest
//! moves the hull at most [`CONTACT_GAP`] along the normal, and the normal
//! may face along the motion by at most that much: a trace that slides along
//! a face off the axes, starting within rounding of it, can seem to leave it.

use crate::{BoxHull, CONTACT_GAP, CollisionWorld, Trace};
use eng_math::{Vec3, WorldPos};

/// Checks one trace. `start` must be a position where the hull is free.
///
/// # Panics
/// With a description of the first rule broken.
pub fn check<C: CollisionWorld + ?Sized>(
    world: &C,
    start: WorldPos,
    end: WorldPos,
    hull: &BoxHull,
) {
    assert!(
        !world.overlaps(start, hull),
        "probe start {start:?} must be free"
    );
    let tr = world.trace(start, end, hull);
    assert!(!tr.start_solid, "free start reported as start_solid");
    assert!(
        (0.0..=1.0).contains(&tr.fraction),
        "fraction {} outside [0, 1]",
        tr.fraction
    );
    assert!(
        !world.overlaps(tr.end, hull),
        "trace from {start:?} to {end:?} ended inside solid at {:?}",
        tr.end
    );
    if tr.hit() {
        let len = tr.normal.length();
        assert!(
            (len - 1.0).abs() < 1e-5,
            "a hit needs a unit normal, got {:?}",
            tr.normal
        );
        let travel = WorldPos::new(end.x - start.x, end.y - start.y, end.z - start.z);
        let into = f64::from(tr.normal.x) * travel.x
            + f64::from(tr.normal.y) * travel.y
            + f64::from(tr.normal.z) * travel.z;
        // Off the axes a motion along the face, from within rounding of it,
        // may seem to leave it by a rounding error; a hull within
        // CONTACT_GAP of a face is in contact with it all the same.
        let along = if axis_of(tr.normal).is_some() {
            0.0
        } else {
            CONTACT_GAP
        };
        assert!(
            into <= along,
            "normal {:?} faces along the motion ({into:e} m)",
            tr.normal
        );
        contact(world, start, end, hull, &tr);
    } else {
        assert_eq!(tr.end, end, "an unobstructed trace must reach its end");
    }
    let again = world.trace(start, end, hull);
    assert_eq!(again, tr, "traces must be deterministic");
}

/// ADR-0024: a blocked trace `tr` from `start` to `end` ends in contact.
fn contact<C: CollisionWorld + ?Sized>(
    world: &C,
    start: WorldPos,
    end: WorldPos,
    hull: &BoxHull,
    tr: &Trace,
) {
    let at = tr.end;
    let rest = world.trace(at, end, hull);
    assert!(
        !rest.start_solid,
        "the trace from {start:?} to {end:?} stopped at {at:?}, where the rest of the motion starts in solid"
    );
    assert!(
        rest.hit(),
        "the trace from {start:?} to {end:?} stopped at {at:?}, short of contact: the rest of the motion does not hit"
    );
    match axis_of(tr.normal) {
        Some(axis) => {
            assert!(
                rest.fraction == 0.0 && rest.end == at,
                "the trace from {start:?} to {end:?} hit a face on an axis but stopped at {at:?}, off its boundary: the rest of the motion moves to {:?} (fraction {})",
                rest.end,
                rest.fraction
            );
            // Free at `at` and solid one ulp on is what "the boundary the
            // world's own `overlaps` uses" means. Stepping every moving axis
            // keeps an exact corner hit, which touches along an edge only,
            // overlapping; stepping the hit axis alone keeps a hit on an edge
            // the hull is sliding off overlapping.
            let on = ulp_towards(at, end, None);
            let alone = ulp_towards(at, end, Some(axis));
            assert!(
                world.overlaps(on, hull) || world.overlaps(alone, hull),
                "the trace from {start:?} to {end:?} stopped at {at:?}, but one ulp further on is still free: it stopped short of the boundary `overlaps` uses"
            );
        }
        None => {
            let n = tr.normal;
            let gap = f64::from(n.x) * (rest.end.x - at.x)
                + f64::from(n.y) * (rest.end.y - at.y)
                + f64::from(n.z) * (rest.end.z - at.z);
            assert!(
                gap * gap <= CONTACT_GAP * CONTACT_GAP,
                "the trace from {start:?} to {end:?} stopped at {at:?}, {gap:e} m from a face off the axes along its normal {n:?}, more than CONTACT_GAP"
            );
        }
    }
}

/// The axis of a normal that is ±1 on one axis and 0 on the others.
fn axis_of(n: Vec3) -> Option<usize> {
    let c = [n.x, n.y, n.z];
    let zeros = c.iter().filter(|&&v| v == 0.0).count();
    let axis = c.iter().position(|&v| v == 1.0 || v == -1.0)?;
    (zeros == 2).then_some(axis)
}

/// `at` moved one ulp towards `end` on `only` that axis, or on every axis
/// where they differ.
fn ulp_towards(at: WorldPos, end: WorldPos, only: Option<usize>) -> WorldPos {
    let step = |axis: usize, x: f64, to: f64| {
        if only.is_some_and(|o| o != axis) {
            x
        } else if to > x {
            x.next_up()
        } else if to < x {
            x.next_down()
        } else {
            x
        }
    };
    WorldPos::new(
        step(0, at.x, end.x),
        step(1, at.y, end.y),
        step(2, at.z, end.z),
    )
}
