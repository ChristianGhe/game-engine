//! Rules every [`CollisionWorld`] must follow. Each implementation's tests
//! call [`check`] with probes from its own geometry.

use crate::{BoxHull, CollisionWorld};
use eng_math::WorldPos;

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
        assert!(into <= 0.0, "normal {:?} faces along the motion", tr.normal);
    } else {
        assert_eq!(tr.end, end, "an unobstructed trace must reach its end");
    }
    let again = world.trace(start, end, hull);
    assert_eq!(again, tr, "traces must be deterministic");
}
