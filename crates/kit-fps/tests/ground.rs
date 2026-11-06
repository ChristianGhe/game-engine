//! `ground_move` (spec 6.11, rung 2): gravity, step-up, step-down and ground
//! snapping, checked exactly in worlds of solid boxes. Slopes are in
//! `slopes.rs`.

#[path = "support/boxes.rs"]
mod boxes;

use boxes::{Boxes, course, fallbacks, solid};
use eng_collision_api::{BoxHull, BoxRoom, CollisionWorld};
use eng_core::{PlayerCommand, Tick};
use eng_math::{Vec3, WorldPos};
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
        let before = fallbacks();
        same_as_pmove(&walled_room(), start, &commands)?;
        prop_assert_eq!(fallbacks(), before, "an inexact sweep fell back");
    }
}

#[test]
fn in_the_air_velocity_changes_first_then_position() {
    let world = walled_room();
    let mut s = state([4.0, 3.0, 2.0], [1.5, 0.0, 0.0]);
    let mut air_ticks = 0;
    loop {
        // Full stick north-east: in the air it changes nothing.
        let out = tick(&world, &s, &cmd(127, 127, 0x2000));
        if out.on_ground {
            assert_eq!((out.state.pos.z, out.state.vel.z), (H, 0.0), "landed");
            break;
        }
        // Semi-implicit Euler: the new velocity moves the hull this tick.
        let vz = s.vel.z - GROUND.gravity * DT;
        let z = s.pos.z + f64::from(vz) * f64::from(DT);
        assert_eq!(out.state.vel.z.to_bits(), vz.to_bits(), "tick {air_ticks}");
        assert_eq!(out.state.pos.z.to_bits(), z.to_bits(), "tick {air_ticks}");
        // No friction and no air control.
        assert_eq!((out.state.vel.x, out.state.vel.y), (1.5, 0.0));
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

#[test]
fn a_fast_hull_steps_over_a_low_rail_onto_floor_at_its_own_height() {
    // The stepped move clears the 5 cm rail in one tick, so its push-down
    // lands on the floor at exactly the start's height, which the push finds
    // only by looking one probe past the rise.
    let world = Boxes(vec![
        solid([-1.0, -1.0, -1.0], [8.0, 3.0, 0.0]),
        solid([2.0, -1.0, 0.0], [2.05, 3.0, 0.3]),
    ]);
    let out = tick(&world, &state([1.73, 1.0, H], [40.0, 0.0, 0.0]), &east());
    assert!(out.state.pos.x > 2.05 + 0.25, "{out:?}");
    assert_eq!(out.state.pos.z, H);
    assert!(out.on_ground);
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
fn a_falling_hull_does_not_step_onto_a_ledge() {
    // Feet 0.2 m below the ledge's top, falling and moving west into its side:
    // only a hull on the ground steps up, so this one slides down the side.
    let s = state([4.3, 1.0, 0.8 + H], [-4.0, 0.0, -1.0]);
    let out = tick(&ledge(1.0), &s, &cmd(127, 0, 0x8000));
    assert_eq!(out.state.pos.x, 4.25, "{out:?}");
    assert!(out.state.pos.z < s.pos.z && !out.on_ground, "{out:?}");
    assert_eq!(out.planes(), [Vec3::new(1.0, 0.0, 0.0)]);
}

#[test]
fn a_hull_on_the_ground_has_no_vertical_velocity() {
    // Nothing lifts a hull off the floor (no jumping), and nothing pushes it in.
    let world = walled_room();
    let level = tick(&world, &state([4.0, 3.0, H], [2.0, 1.0, 0.0]), &east());
    for vz in [-3.0, 1.0] {
        let out = tick(&world, &state([4.0, 3.0, H], [2.0, 1.0, vz]), &east());
        assert_eq!(out, level, "vel.z {vz}");
    }
    assert!(level.on_ground && level.planes().is_empty(), "{level:?}");
    assert_eq!((level.state.pos.z, level.state.vel.z), (H, 0.0));
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
        let before = fallbacks();
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
        // The box world's fallback guarantees "never inside" by itself; it
        // must not have been needed, or this test proves nothing.
        prop_assert_eq!(fallbacks(), before, "an inexact sweep fell back");
    }
}
