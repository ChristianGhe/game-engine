use eng_collision_api::{BoxHull, BoxRoom, CollisionWorld, Trace, conformance};
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

proptest! {
    #[test]
    fn conformance_holds_in_three_dimensions(
        sx in 0.25f64..7.75, sy in 0.25f64..5.75, sz in 0.5f64..2.5,
        dx in prop_oneof![Just(0.0), -20.0f64..20.0],
        dy in prop_oneof![Just(0.0), -20.0f64..20.0],
        dz in prop_oneof![Just(0.0), -6.0f64..6.0],
    ) {
        let start = WorldPos::new(sx, sy, sz);
        let end = WorldPos::new(sx + dx, sy + dy, sz + dz);
        conformance::check(&ROOM, start, end, &HULL);
    }
}

#[test]
fn a_hit_into_a_corner_ends_in_contact_with_both_walls() {
    let (start, end) = (WorldPos::new(7.0, 5.0, 1.0), WorldPos::new(9.0, 7.0, 1.0));
    let tr = ROOM.trace(start, end, &HULL);
    assert_eq!(tr.end, WorldPos::new(7.75, 5.75, 1.0));
    assert_eq!(tr.normal, Vec3::new(-1.0, 0.0, 0.0), "ties go to X");
    conformance::check(&ROOM, start, end, &HULL);
}

/// Traces against the room shrunk by a skin on every side, overlaps against
/// the room itself: a world that stops short and blocks within its skin.
struct Skin(f64);

impl CollisionWorld for Skin {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let s = self.0;
        let inner = BoxRoom {
            min: WorldPos::new(s, s, s),
            max: WorldPos::new(8.0 - s, 6.0 - s, 3.0 - s),
        };
        inner.trace(start, end, hull)
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        ROOM.overlaps(at, hull)
    }
}

#[test]
#[should_panic(expected = "one ulp further on is still free")]
fn a_world_that_stops_a_skin_short_fails_the_check() {
    let start = WorldPos::new(7.0, 3.0, 1.0);
    conformance::check(&Skin(1e-9), start, WorldPos::new(9.0, 3.0, 1.0), &HULL);
}

/// Every hit reported at the start, as `kit-fps`'s box world once fell back
/// to: free, but nowhere near the wall.
struct HitAtStart;

impl CollisionWorld for HitAtStart {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let tr = ROOM.trace(start, end, hull);
        if tr.hit() && !tr.start_solid {
            return Trace {
                fraction: 0.0,
                end: start,
                ..tr
            };
        }
        tr
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        ROOM.overlaps(at, hull)
    }
}

#[test]
#[should_panic(expected = "one ulp further on is still free")]
fn a_world_that_blocks_at_the_start_fails_the_check() {
    let start = WorldPos::new(7.0, 3.0, 1.0);
    conformance::check(&HitAtStart, start, WorldPos::new(9.0, 3.0, 1.0), &HULL);
}

/// The room, with every hit's normal replaced by this one.
struct Turned(Vec3);

impl CollisionWorld for Turned {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let mut tr = ROOM.trace(start, end, hull);
        if tr.hit() && !tr.start_solid {
            tr.normal = self.0;
        }
        tr
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        ROOM.overlaps(at, hull)
    }
}

/// East into the east wall: 2 m of travel along +X.
fn east_into_the_wall(world: &Turned) {
    let start = WorldPos::new(7.0, 3.0, 1.0);
    conformance::check(world, start, WorldPos::new(9.0, 3.0, 1.0), &HULL);
}

#[test]
#[should_panic(expected = "faces along the motion")]
fn a_normal_on_an_axis_facing_along_the_motion_fails_the_check() {
    east_into_the_wall(&Turned(Vec3::new(1.0, 0.0, 0.0)));
}

#[test]
#[should_panic(expected = "faces along the motion")]
fn a_normal_off_the_axes_facing_along_the_motion_by_more_than_contact_gap_fails() {
    // 2^-20 along +X over 2 m: 2^-19 m along the motion.
    east_into_the_wall(&Turned(Vec3::new(1.0 / 1_048_576.0, 1.0, 0.0)));
}

#[test]
fn a_normal_off_the_axes_may_face_along_the_motion_by_less_than_contact_gap() {
    // 2^-22 along +X over 2 m: 2^-21 m, within CONTACT_GAP.
    east_into_the_wall(&Turned(Vec3::new(1.0 / 4_194_304.0, 1.0, 0.0)));
}

#[test]
#[should_panic(expected = "faces along the motion")]
fn a_normal_on_z_facing_along_a_fall_fails_the_check() {
    // Falling onto the floor, with the floor's normal turned to face down.
    let start = WorldPos::new(4.0, 3.0, 2.0);
    let floor_down = Turned(Vec3::new(0.0, 0.0, -1.0));
    conformance::check(&floor_down, start, WorldPos::new(4.0, 3.0, -1.0), &HULL);
}

/// Solid below a plane through `point` whose unit normal `n` is off every
/// axis (a slope), for a box hull. A trace that travels more than 1 mm to the
/// plane stops `short` of contact, measured along the normal; a shorter one
/// stops in contact. So the first trace of a probe stops short, and the
/// rest of its motion, traced from there, closes the gap.
struct Slope {
    n: Vec3,
    point: WorldPos,
    short: f64,
}

impl Slope {
    /// The hull's distance from the plane along the normal; negative inside.
    fn gap(&self, at: WorldPos, hull: &BoxHull) -> f64 {
        let (n, h) = (self.n, hull.half_extents);
        let reach = f64::from(n.x.abs() * h.x + n.y.abs() * h.y + n.z.abs() * h.z);
        f64::from(n.x) * (at.x - self.point.x)
            + f64::from(n.y) * (at.y - self.point.y)
            + f64::from(n.z) * (at.z - self.point.z)
            - reach
    }
}

impl CollisionWorld for Slope {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let (g0, g1) = (self.gap(start, hull), self.gap(end, hull));
        if g0 < 0.0 || g1 >= 0.0 {
            return Trace {
                fraction: if g0 < 0.0 { 0.0 } else { 1.0 },
                end: if g0 < 0.0 { start } else { end },
                normal: Vec3::ZERO,
                start_solid: g0 < 0.0,
            };
        }
        let at = |t: f64| {
            WorldPos::new(
                start.x + (end.x - start.x) * t,
                start.y + (end.y - start.y) * t,
                start.z + (end.z - start.z) * t,
            )
        };
        let mut t = g0 / (g0 - g1);
        if start.distance(at(t)) > 1e-3 {
            t = (g0 - self.short) / (g0 - g1);
        }
        while self.overlaps(at(t), hull) {
            t = t.next_down();
        }
        Trace {
            fraction: t,
            end: at(t),
            normal: self.n,
            start_solid: false,
        }
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        self.gap(at, hull) < 0.0
    }
}

/// Straight down the slope's normal onto it, from 2 m off.
fn down_the_normal(short: f64) {
    // Every component of the normal, and of the point it passes through, is
    // off zero, so every term of the gap counts.
    let n = Vec3::new(0.48, 0.6, 0.64);
    let point = WorldPos::new(3.0, 4.0, 1.0);
    let slope = Slope { n, point, short };
    let start = point.add_scaled(n, 2.0);
    conformance::check(&slope, start, point.add_scaled(n, -2.0), &HULL);
}

#[test]
#[should_panic(expected = "more than CONTACT_GAP")]
fn a_world_that_stops_short_of_a_slope_by_more_than_contact_gap_fails_the_check() {
    down_the_normal(2.0 * eng_collision_api::CONTACT_GAP);
}

#[test]
fn a_world_that_stops_within_contact_gap_of_a_slope_passes_the_check() {
    down_the_normal(0.5 * eng_collision_api::CONTACT_GAP);
}

/// Solid where `x < 0` and `y > 1`, for a hull of any size reduced to its
/// centre: the corner of a wedge. A trace from `x < 0` that crosses `y = 1`
/// stops there, on the face of Y, keeping its x.
struct Wedge;

impl CollisionWorld for Wedge {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        if self.overlaps(start, hull) {
            return Trace {
                fraction: 0.0,
                end: start,
                normal: Vec3::ZERO,
                start_solid: true,
            };
        }
        if start.x < 0.0 && start.y <= 1.0 && end.y > 1.0 {
            return Trace {
                fraction: (1.0 - start.y) / (end.y - start.y),
                end: WorldPos::new(start.x, 1.0, start.z),
                normal: Vec3::new(0.0, -1.0, 0.0),
                start_solid: false,
            };
        }
        Trace {
            fraction: 1.0,
            end,
            normal: Vec3::ZERO,
            start_solid: false,
        }
    }

    fn overlaps(&self, at: WorldPos, _: &BoxHull) -> bool {
        at.x < 0.0 && at.y > 1.0
    }
}

#[test]
fn a_hit_on_y_beside_a_wedge_thinner_than_an_ulp_is_solid_one_ulp_on_along_y() {
    // The hull stops on the face of Y the least subnormal west of the
    // wedge's edge, moving north-east. One ulp on along both axes is past the
    // edge and free; one ulp on along Y alone is solid, which the check
    // accepts (the hit's axis is Y, not X).
    let start = WorldPos::new(-f64::from_bits(1), 0.0, 0.0);
    conformance::check(&Wedge, start, WorldPos::new(1.0, 2.0, 0.0), &HULL);
}

/// Traces against the room with its east wall moved in by a skin, overlaps
/// against the room itself: a world that stops short of the east wall only,
/// so the hull can run into it touching the floor or the ceiling.
struct ShortOfEast(f64);

impl CollisionWorld for ShortOfEast {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let inner = BoxRoom {
            min: ROOM.min,
            max: WorldPos::new(ROOM.max.x - self.0, ROOM.max.y, ROOM.max.z),
        };
        inner.trace(start, end, hull)
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        ROOM.overlaps(at, hull)
    }
}

/// East into the east wall at height `z`: the check's ulp step must leave
/// the axes the motion does not move where they are.
fn east_short_of_the_wall_at(z: f64) {
    let start = WorldPos::new(7.0, 3.0, z);
    let end = WorldPos::new(9.0, 3.0, z);
    conformance::check(&ShortOfEast(1e-9), start, end, &HULL);
}

#[test]
#[should_panic(expected = "one ulp further on is still free")]
fn a_world_that_stops_short_fails_the_check_with_the_hull_on_the_floor() {
    east_short_of_the_wall_at(0.5);
}

#[test]
#[should_panic(expected = "one ulp further on is still free")]
fn a_world_that_stops_short_fails_the_check_with_the_hull_under_the_ceiling() {
    east_short_of_the_wall_at(2.5);
}
