//! `ground_move` (spec 6.11, rung 2): gravity, step-up, step-down and ground
//! snapping, checked exactly in worlds of solid boxes.

#[path = "support/boxes.rs"]
mod boxes;

use boxes::{Boxes, course, solid};
use eng_collision_api::{BoxHull, BoxRoom, CollisionWorld, Trace};
use eng_core::{PlayerCommand, Tick};
use eng_math::{Vec3, WorldPos, clamp_f64};
use eng_schema_core::{Schema, from_bytes, to_bytes};
use kit_fps::{GroundConfig, GroundOutcome, MoveConfig, MoveState, ground_move, pmove};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// Rung 2's room hull: 0.5 m x 0.5 m x 1.75 m.
const MOVE: MoveConfig = MoveConfig {
    max_speed: 4.0,
    accel: 10.0,
    friction: 6.0,
    stop_speed: 1.0,
    hull: BoxHull {
        half_extents: Vec3::new(0.25, 0.25, 0.875),
    },
};
const GROUND: GroundConfig = GroundConfig::DEFAULT;
const DT: f32 = 1.0 / 60.0;
/// The hull's half height: its centre's height above the floor it stands on.
const H: f64 = 0.875;

fn cmd(forward: i8, side: i8, yaw: u16) -> PlayerCommand {
    PlayerCommand {
        forward,
        side,
        view_yaw: yaw,
        ..PlayerCommand::idle(Tick(1))
    }
}

fn east() -> PlayerCommand {
    cmd(127, 0, 0)
}

fn tick<C: CollisionWorld + ?Sized>(world: &C, s: &MoveState, c: &PlayerCommand) -> GroundOutcome {
    ground_move(s, c, &MOVE, &GROUND, DT, world)
}

fn state(pos: [f64; 3], vel: [f32; 3]) -> MoveState {
    MoveState {
        pos: WorldPos::new(pos[0], pos[1], pos[2]),
        vel: Vec3::new(vel[0], vel[1], vel[2]),
    }
}

#[test]
fn ground_config_is_schema_v1_with_the_plan_defaults() {
    assert_eq!(GroundConfig::default(), GROUND);
    assert_eq!(
        (GROUND.gravity, GROUND.step_height, GROUND.snap_distance),
        (9.81, 0.4, 0.4)
    );
    assert_eq!(GROUND.min_floor_normal_z, 0.7);
    assert_eq!(
        (GroundConfig::NAME, GroundConfig::VERSION),
        ("kit_fps.GroundConfig", 1)
    );
    let bytes = to_bytes(&GROUND);
    assert_eq!(bytes.len(), 16);
    assert_eq!(from_bytes::<GroundConfig>(&bytes).unwrap(), GROUND);
    // pmove's types keep their names and versions.
    assert_eq!(
        (MoveState::NAME, MoveState::VERSION),
        ("kit_fps.MoveState", 1)
    );
    assert_eq!(
        (MoveConfig::NAME, MoveConfig::VERSION),
        ("kit_fps.MoveConfig", 1)
    );
}

/// The 8 m x 6 m x 3 m room as boxes: a floor at z = 0, walls and a ceiling.
fn walled_room() -> Boxes {
    Boxes(vec![
        solid([-1.0, -1.0, -1.0], [9.0, 7.0, 0.0]),
        solid([-1.0, -1.0, 0.0], [0.0, 7.0, 3.0]),
        solid([8.0, -1.0, 0.0], [9.0, 7.0, 3.0]),
        solid([0.0, -1.0, 0.0], [8.0, 0.0, 3.0]),
        solid([0.0, 6.0, 0.0], [8.0, 7.0, 3.0]),
        solid([-1.0, -1.0, 3.0], [9.0, 7.0, 4.0]),
    ])
}

const ROOM: BoxRoom = BoxRoom {
    min: WorldPos::new(0.0, 0.0, 0.0),
    max: WorldPos::new(8.0, 6.0, 3.0),
};

/// Runs `pmove` and `ground_move` side by side from a hull on the floor.
fn same_as_pmove<C: CollisionWorld>(
    world: &C,
    start: MoveState,
    commands: &[(i8, i8, u16)],
) -> Result<(), TestCaseError> {
    let (mut flat, mut grounded) = (start, start);
    for &(f, s, yaw) in commands {
        let c = cmd(f, s, yaw);
        let want = pmove(&flat, &c, &MOVE, DT, world);
        let got = tick(world, &grounded, &c);
        let bits = |m: &MoveState| {
            let (p, v) = (m.pos, m.vel);
            [
                p.x.to_bits(),
                p.y.to_bits(),
                v.x.to_bits().into(),
                v.y.to_bits().into(),
            ]
        };
        prop_assert_eq!(bits(&got.state), bits(&want.state), "{:?}", got.state);
        prop_assert_eq!(got.state.pos.z.to_bits(), H.to_bits(), "z moved");
        prop_assert_eq!(got.state.vel.z, 0.0);
        prop_assert_eq!(got.planes(), want.planes());
        prop_assert!(got.on_ground);
        (flat, grounded) = (want.state, got.state);
    }
    Ok(())
}

proptest! {
    #[test]
    fn flat_ground_gives_pmoves_result_bit_for_bit(
        x in 0.25f64..7.75,
        y in 0.25f64..5.75,
        vx in -6.0f32..6.0,
        vy in -6.0f32..6.0,
        commands in prop::collection::vec((any::<i8>(), any::<i8>(), any::<u16>()), 1..150),
    ) {
        let start = state([x, y, H], [vx, vy, 0.0]);
        same_as_pmove(&ROOM, start, &commands)?;
        same_as_pmove(&walled_room(), start, &commands)?;
    }
}

#[test]
fn in_the_air_velocity_changes_first_then_position() {
    let world = walled_room();
    let mut s = state([4.0, 3.0, 2.0], [1.5, 0.0, 0.0]);
    let mut air_ticks = 0;
    loop {
        let out = tick(&world, &s, &cmd(0, 0, 0));
        if out.on_ground {
            assert_eq!((out.state.pos.z, out.state.vel.z), (H, 0.0), "landed");
            break;
        }
        // Semi-implicit Euler: the new velocity moves the hull this tick.
        let vz = s.vel.z - GROUND.gravity * DT;
        let z = s.pos.z + f64::from(vz) * f64::from(DT);
        assert_eq!(out.state.vel.z.to_bits(), vz.to_bits(), "tick {air_ticks}");
        assert_eq!(out.state.pos.z.to_bits(), z.to_bits(), "tick {air_ticks}");
        // No friction in the air.
        assert_eq!(out.state.vel.x, 1.5);
        assert_eq!(out.state.pos.x, s.pos.x + 1.5 * f64::from(DT));
        s = out.state;
        air_ticks += 1;
    }
    // From 1.125 m up, 28 ticks in the air; position first would take 29.
    assert_eq!(air_ticks, 28);
}

/// A floor at z = 0, and from x = 2 a step whose top is at `top`.
fn step(top: f64) -> Boxes {
    Boxes(vec![
        solid([-1.0, -1.0, -1.0], [2.0, 3.0, 0.0]),
        solid([2.0, -1.0, -1.0], [8.0, 3.0, top]),
    ])
}

/// A hull 2 cm short of the riser, at full speed towards it.
fn before_riser() -> MoveState {
    state([1.73, 1.0, H], [4.0, 0.0, 0.0])
}

#[test]
fn a_step_up_to_the_step_height_is_climbed_within_one_tick() {
    for top in [0.2, 0.4, f64::from(GROUND.step_height)] {
        let out = tick(&step(top), &before_riser(), &east());
        assert_eq!(out.state.pos.z, top + H, "top {top}: {out:?}");
        assert_eq!(out.state.pos.x, 1.73 + 4.0 * f64::from(DT), "top {top}");
        assert_eq!(out.state.vel, Vec3::new(4.0, 0.0, 0.0), "top {top}");
        assert!(out.on_ground, "top {top}");
    }
}

#[test]
fn a_taller_step_blocks() {
    let world = step(0.41);
    let mut s = before_riser();
    for _ in 0..60 {
        let out = tick(&world, &s, &east());
        assert_eq!((out.state.pos.x, out.state.pos.z), (1.75, H), "{out:?}");
        assert_eq!(out.planes(), [Vec3::new(-1.0, 0.0, 0.0)]);
        assert!(out.on_ground);
        s = out.state;
    }
}

/// A floor at z = 0, and for x < 4 a ledge `depth` above it.
fn ledge(depth: f64) -> Boxes {
    Boxes(vec![
        solid([-1.0, -1.0, -1.0], [4.0, 3.0, depth]),
        solid([4.0, -1.0, -1.0], [12.0, 3.0, 0.0]),
    ])
}

#[test]
fn a_drop_within_the_snap_distance_stays_on_the_ground() {
    for depth in [0.1, 0.3, 0.4, f64::from(GROUND.snap_distance)] {
        let world = ledge(depth);
        let mut s = state([3.0, 1.0, depth + H], [4.0, 0.0, 0.0]);
        while s.pos.x < 5.0 {
            let out = tick(&world, &s, &east());
            assert!(out.on_ground, "depth {depth}: off the ground at {out:?}");
            s = out.state;
        }
        assert_eq!((s.pos.z, s.vel.z), (H, 0.0), "depth {depth}");
    }
}

#[test]
fn a_deeper_drop_leaves_the_ground_then_falls_under_gravity() {
    for depth in [0.4005, 0.41, 1.0, 2.5] {
        let world = ledge(depth);
        let mut s = state([3.0, 1.0, depth + H], [4.0, 0.0, 0.0]);
        let mut out = tick(&world, &s, &east());
        while out.on_ground {
            s = out.state;
            out = tick(&world, &s, &east());
        }
        // The tick it walks off: no snap, and no gravity yet.
        assert!(out.state.pos.x > 4.25, "depth {depth}: {out:?}");
        assert_eq!((out.state.pos.z, out.state.vel.z), (depth + H, 0.0));
        let mut falling = 0;
        loop {
            s = out.state;
            out = tick(&world, &s, &east());
            if out.on_ground {
                break;
            }
            let vz = s.vel.z - GROUND.gravity * DT;
            assert_eq!(out.state.vel.z.to_bits(), vz.to_bits(), "depth {depth}");
            let z = s.pos.z + f64::from(vz) * f64::from(DT);
            assert_eq!(out.state.pos.z.to_bits(), z.to_bits(), "depth {depth}");
            assert_eq!(out.state.vel.x, 4.0, "depth {depth}: friction in the air");
            falling += 1;
        }
        assert_eq!(
            (out.state.pos.z, out.state.vel.z),
            (H, 0.0),
            "depth {depth}"
        );
        assert!(falling > 10, "depth {depth}: {falling} ticks");
    }
}

#[test]
fn a_falling_hull_that_brushes_a_ledge_lands_on_it() {
    // Feet 0.2 m below the ledge's top, falling and moving west towards it.
    let s = state([4.3, 1.0, 0.8 + H], [-4.0, 0.0, -1.0]);
    let out = tick(&ledge(1.0), &s, &cmd(127, 0, 0x8000));
    assert_eq!(out.state.pos.z, 1.0 + H, "{out:?}");
    assert!(out.state.pos.x < 4.25 && out.on_ground, "{out:?}");
    assert_eq!(out.state.vel.z, 0.0);
}

#[test]
fn a_hull_on_the_ground_keeps_no_velocity_into_the_floor() {
    let world = walled_room();
    let sinking = tick(&world, &state([4.0, 3.0, H], [2.0, 1.0, -3.0]), &east());
    let level = tick(&world, &state([4.0, 3.0, H], [2.0, 1.0, 0.0]), &east());
    assert_eq!(sinking, level);
    assert!(sinking.planes().is_empty(), "the floor is not a plane hit");
}

#[test]
fn a_hull_on_the_ground_moving_up_is_not_snapped_down() {
    let out = tick(
        &walled_room(),
        &state([4.0, 3.0, H], [0.0, 0.0, 1.0]),
        &cmd(0, 0, 0),
    );
    assert_eq!(out.state.pos.z, H + f64::from(DT));
    assert!(!out.on_ground);
}

/// Solid below a plane through the origin with unit normal `n`, since boxes
/// have no slopes. A hit stops at contact, and only a hull more than
/// [`SKIN`] inside counts as overlapping.
struct Slope(Vec3);

/// The f32 velocity `pmove` clips against a tilted plane is still a little
/// into it, and its slide zeroes the velocity when it hits the same plane
/// twice. A 1 µm skin keeps that rounding from stopping the hull, so this
/// test sees only the floor rule.
const SKIN: f64 = 1e-6;

impl Slope {
    /// The hull's distance from the plane; negative inside.
    fn gap(&self, at: WorldPos, hull: &BoxHull) -> f64 {
        let (n, h) = (self.0, hull.half_extents);
        let reach = f64::from(n.x.abs() * h.x + n.y.abs() * h.y + n.z.abs() * h.z);
        f64::from(n.x) * at.x + f64::from(n.y) * at.y + f64::from(n.z) * at.z - reach
    }

    /// A hull resting on the plane at (x, 0).
    fn resting(&self, x: f64) -> MoveState {
        let base = state([x, 0.0, 0.0], [0.0; 3]);
        let lift = -self.gap(base.pos, &MOVE.hull) / f64::from(self.0.z);
        state([x, 0.0, lift], [0.0; 3])
    }
}

impl CollisionWorld for Slope {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let (g0, g1) = (self.gap(start, hull), self.gap(end, hull));
        if g0 < -SKIN || g1 >= -SKIN {
            let solid = g0 < -SKIN;
            return Trace {
                fraction: if solid { 0.0 } else { 1.0 },
                end: if solid { start } else { end },
                normal: Vec3::ZERO,
                start_solid: solid,
            };
        }
        let d = WorldPos::new(end.x - start.x, end.y - start.y, end.z - start.z);
        let mut t = clamp_f64(g0 / (g0 - g1), 0.0, 1.0f64.next_down());
        loop {
            let p = WorldPos::new(start.x + d.x * t, start.y + d.y * t, start.z + d.z * t);
            if !self.overlaps(p, hull) {
                return Trace {
                    fraction: t,
                    end: p,
                    normal: self.0,
                    start_solid: false,
                };
            }
            t *= 0.5;
        }
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        self.gap(at, hull) < -SKIN
    }
}

#[test]
fn a_walkable_slope_holds_the_hull_and_a_steeper_one_does_not() {
    // Normal z 0.8 (about 37 degrees) is floor; 0.6 (about 53) is not.
    let gentle = Slope(Vec3::new(-0.6, 0.0, 0.8));
    let start = gentle.resting(1.0);
    let mut s = start;
    for _ in 0..60 {
        let out = tick(&gentle, &s, &cmd(0, 0, 0));
        assert!(out.on_ground && out.state.vel == Vec3::ZERO, "{out:?}");
        s = out.state;
    }
    assert!(
        s.pos.distance(start.pos) < 1e-6,
        "crept from {start:?} to {s:?}"
    );

    let steep = Slope(Vec3::new(-0.8, 0.0, 0.6));
    let start = steep.resting(1.0);
    let mut s = start;
    for _ in 0..60 {
        let out = tick(&steep, &s, &cmd(0, 0, 0));
        assert!(
            !out.on_ground && !steep.overlaps(out.state.pos, &MOVE.hull),
            "{out:?}"
        );
        s = out.state;
    }
    // It slides down the slope, towards -x.
    assert!(
        s.pos.x < start.pos.x - 1.0 && s.pos.z < start.pos.z - 1.0,
        "{s:?}"
    );
}

proptest! {
    // Starts inside the course's solids are rejected (about 1 in 5), so a
    // large PROPTEST_CASES needs more than the default 1,024 rejects.
    #![proptest_config(ProptestConfig {
        max_global_rejects: 1 << 16,
        ..ProptestConfig::default()
    })]

    #[test]
    fn the_hull_never_ends_inside_solid(
        x in 0.25f64..15.75,
        y in 0.25f64..11.75,
        lift in prop_oneof![Just(0.0), 0.0f64..1.5],
        extra in prop::collection::vec(
            (0.0f64..16.0, 0.0f64..12.0, 0.0f64..2.5, 0.05f64..3.0, 0.05f64..3.0, 0.05f64..1.5),
            0..8,
        ),
        moves in prop::collection::vec(
            (any::<i8>(), any::<i8>(), any::<u16>(), 1usize..30),
            1..40,
        ),
    ) {
        let mut world = course();
        let start = state([x, y, H + lift], [0.0; 3]);
        prop_assume!(!world.overlaps(start.pos, &MOVE.hull));
        let mut added = Boxes(
            extra
                .iter()
                .map(|&(bx, by, bz, w, d, h)| solid([bx, by, bz], [bx + w, by + d, bz + h]))
                .collect(),
        );
        added.0.retain(|b| !Boxes(vec![*b]).overlaps(start.pos, &MOVE.hull));
        world.0.extend(added.0);
        let mut s = start;
        for (f, side, yaw, ticks) in moves {
            for _ in 0..ticks {
                let out = tick(&world, &s, &cmd(f, side, yaw));
                prop_assert!(
                    !world.overlaps(out.state.pos, &MOVE.hull),
                    "inside solid: {:?} from {:?}",
                    out.state,
                    s
                );
                prop_assert!(out.state.pos.is_finite() && out.state.vel.is_finite());
                s = out.state;
            }
        }
    }
}
