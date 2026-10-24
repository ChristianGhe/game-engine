use eng_collision_api::{BoxHull, BoxRoom, CollisionWorld, conformance};
use eng_math::{Vec3, WorldPos};
use proptest::prelude::*;

const ROOM: BoxRoom = BoxRoom {
    min: WorldPos::new(0.0, 0.0, 0.0),
    max: WorldPos::new(8.0, 6.0, 3.0),
};
const HULL: BoxHull = BoxHull {
    half_extents: Vec3::new(0.25, 0.25, 0.5),
};

#[test]
fn free_motion_reaches_the_end() {
    let tr = ROOM.trace(
        WorldPos::new(1.0, 1.0, 1.0),
        WorldPos::new(2.0, 1.5, 1.0),
        &HULL,
    );
    assert_eq!(tr.fraction, 1.0);
    assert_eq!(tr.end, WorldPos::new(2.0, 1.5, 1.0));
}

#[test]
fn hit_stops_exactly_at_the_wall() {
    let tr = ROOM.trace(
        WorldPos::new(7.0, 3.0, 1.0),
        WorldPos::new(9.0, 3.0, 1.0),
        &HULL,
    );
    assert_eq!(tr.fraction, 0.375);
    assert_eq!(tr.end, WorldPos::new(7.75, 3.0, 1.0));
    assert_eq!(tr.normal, Vec3::new(-1.0, 0.0, 0.0));
    let along = ROOM.trace(tr.end, WorldPos::new(7.75, 5.0, 1.0), &HULL);
    assert!(
        !along.start_solid && !along.hit(),
        "sliding along the wall is free"
    );
}

#[test]
fn start_solid_does_not_move() {
    let tr = ROOM.trace(
        WorldPos::new(-1.0, 3.0, 1.0),
        WorldPos::new(1.0, 3.0, 1.0),
        &HULL,
    );
    assert!(tr.start_solid);
    assert_eq!(tr.fraction, 0.0);
}

proptest! {
    #[test]
    fn conformance_holds(
        sx in 0.25f64..7.75, sy in 0.25f64..5.75,
        dx in -20.0f64..20.0, dy in -20.0f64..20.0,
    ) {
        let start = WorldPos::new(sx, sy, 1.0);
        let end = WorldPos::new(sx + dx, sy + dy, 1.0);
        conformance::check(&ROOM, start, end, &HULL);
    }
}

#[test]
fn a_hit_one_ulp_past_the_wall_still_reports_a_hit() {
    // Found in review: a long move ending one ULP past the wall used to round
    // the fraction to 1.0 while clamping the end.
    let room = BoxRoom {
        min: WorldPos::new(-1000.0, 0.0, 0.0),
        max: WorldPos::new(8.0, 6.0, 3.0),
    };
    let start = WorldPos::new(-999.75, 3.0, 1.0);
    let end = WorldPos::new(7.75f64.next_up(), 3.0, 1.0);
    let tr = room.trace(start, end, &HULL);
    assert!(tr.hit() && tr.fraction < 1.0, "{tr:?}");
    assert_eq!(tr.end.x, 7.75);
    conformance::check(&room, start, end, &HULL);
}
