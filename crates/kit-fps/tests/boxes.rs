//! The test-only box world keeps the collision contract
//! (`eng_collision_api::conformance`), so `ground_move`'s tests can trust it.

#[path = "support/boxes.rs"]
mod boxes;

use boxes::{Boxes, course, fallbacks, solid};
use eng_collision_api::{BoxHull, CollisionWorld, conformance};
use eng_math::{Vec3, WorldPos};
use proptest::prelude::*;

const HULL: BoxHull = BoxHull {
    half_extents: Vec3::new(0.25, 0.25, 0.875),
};

fn at(x: f64, y: f64, z: f64) -> WorldPos {
    WorldPos::new(x, y, z)
}

#[test]
fn a_hit_stops_exactly_on_the_face() {
    let world = Boxes(vec![solid([2.0, -1.0, -1.0], [3.0, 1.0, 1.0])]);
    let tr = world.trace(at(0.0, 0.0, 0.0), at(4.0, 0.0, 0.0), &HULL);
    assert_eq!(tr.fraction, 0.4375);
    assert_eq!(tr.end, at(1.75, 0.0, 0.0));
    assert_eq!(tr.normal, Vec3::new(-1.0, 0.0, 0.0));
    conformance::check(&world, at(0.0, 0.0, 0.0), at(4.0, 0.0, 0.0), &HULL);
}

#[test]
fn touching_is_free_and_sliding_along_a_face_does_not_catch() {
    let world = Boxes(vec![solid([0.0, 0.0, -1.0], [4.0, 4.0, 0.0])]);
    let on = at(1.0, 1.0, 0.875);
    assert!(!world.overlaps(on, &HULL));
    assert!(world.overlaps(at(1.0, 1.0, 0.875f64.next_down()), &HULL));
    let along = world.trace(on, at(3.0, 2.0, 0.875), &HULL);
    assert!(!along.hit() && !along.start_solid, "{along:?}");
    let down = world.trace(on, at(1.0, 1.0, 0.5), &HULL);
    assert_eq!((down.fraction, down.end, down.normal), (0.0, on, Vec3::Z));
}

#[test]
fn ties_go_to_the_lower_box_then_the_lower_axis() {
    let east = solid([2.0, -5.0, -1.0], [3.0, 5.0, 1.0]);
    let north = solid([-5.0, 2.0, -1.0], [5.0, 3.0, 1.0]);
    let (from, to) = (at(0.0, 0.0, 0.0), at(4.0, 4.0, 0.0));
    let x_first = Boxes(vec![east, north]).trace(from, to, &HULL);
    let y_first = Boxes(vec![north, east]).trace(from, to, &HULL);
    assert_eq!(x_first.normal, Vec3::new(-1.0, 0.0, 0.0));
    assert_eq!(y_first.normal, Vec3::new(0.0, -1.0, 0.0));
    assert_eq!(
        (x_first.end, y_first.end),
        (at(1.75, 1.75, 0.0), at(1.75, 1.75, 0.0))
    );
    let corner = Boxes(vec![solid([2.0, 2.0, -1.0], [3.0, 3.0, 1.0])]);
    assert_eq!(
        corner.trace(from, to, &HULL).normal,
        Vec3::new(-1.0, 0.0, 0.0)
    );
}

#[test]
fn a_start_inside_solid_does_not_move() {
    let world = course();
    let tr = world.trace(at(1.0, 1.0, 0.5), at(2.0, 1.0, 0.5), &HULL);
    assert!(tr.start_solid);
    assert_eq!((tr.fraction, tr.end), (0.0, at(1.0, 1.0, 0.5)));
}

proptest! {
    // Starts inside the course's solids are rejected (about 1 in 6), so a
    // large PROPTEST_CASES needs more than the default 1,024 rejects.
    #![proptest_config(ProptestConfig {
        max_global_rejects: 1 << 16,
        ..ProptestConfig::default()
    })]

    #[test]
    fn the_course_keeps_the_contract(
        x in 0.25f64..15.75,
        y in 0.25f64..11.75,
        z in prop_oneof![Just(0.875), Just(1.275), 0.875f64..2.625],
        dx in prop_oneof![Just(0.0), -12.0f64..12.0],
        dy in prop_oneof![Just(0.0), -12.0f64..12.0],
        dz in prop_oneof![Just(0.0), -4.0f64..4.0],
    ) {
        let world = course();
        let start = at(x, y, z);
        prop_assume!(!world.overlaps(start, &HULL));
        let before = fallbacks();
        conformance::check(&world, start, at(x + dx, y + dy, z + dz), &HULL);
        prop_assert_eq!(fallbacks(), before, "an inexact sweep fell back");
    }
}
