//! `ground_move`'s choice between the flat and the stepped move (spec 6.11,
//! rung 2): the stepped one is kept only if its push-down lands on floor
//! within the rise and it travels strictly further, by squared horizontal
//! distance, whichever way the hull walks. Gaps that mutation testing found
//! (rung 2, T19 follow-up).

// Its `course` is `ground.rs`'s; these worlds are small ones of their own.
#[path = "support/boxes.rs"]
#[allow(dead_code)]
mod boxes;

use boxes::{Boxes, solid};
use eng_collision_api::{BoxHull, CollisionWorld};
use eng_collision_grid::{Grid, Point2, Slab};
use eng_core::{PlayerCommand, Tick};
use eng_math::{Vec3, WorldPos};
use kit_fps::{GroundConfig, GroundOutcome, MoveConfig, MoveState, ground_move, pmove};

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
fn a_step_is_climbed_whichever_way_the_hull_walks() {
    // The kept move is the one that travels further, so the travel must
    // measure both axes, whatever the signs of the motion and the position.
    // A floor, and a 0.2 m step whose riser is 2 cm ahead of the hull.
    let floor = solid([0.0, 0.0, -1.0], [10.0, 10.0, 0.0]);
    let ways = [
        (
            0x0000,
            [1.0, 0.0],
            solid([5.27, 0.0, -1.0], [9.0, 10.0, 0.2]),
        ),
        (
            0x4000,
            [0.0, 1.0],
            solid([0.0, 5.27, -1.0], [10.0, 9.0, 0.2]),
        ),
        (
            0x8000,
            [-1.0, 0.0],
            solid([1.0, 0.0, -1.0], [4.73, 10.0, 0.2]),
        ),
        (
            0xc000,
            [0.0, -1.0],
            solid([0.0, 1.0, -1.0], [10.0, 4.73, 0.2]),
        ),
    ];
    for (yaw, way, step) in ways {
        let world = Boxes(vec![floor, step]);
        let s = state([5.0, 5.0, H], [4.0 * way[0], 4.0 * way[1], 0.0]);
        let out = tick(&world, &s, &cmd(127, 0, yaw));
        assert_eq!(out.state.pos.z, 0.2 + H, "yaw {yaw:#06x}: {out:?}");
        let along = f64::from(way[0]) * (out.state.pos.x - 5.0)
            + f64::from(way[1]) * (out.state.pos.y - 5.0);
        assert!(along > 0.05, "yaw {yaw:#06x}: only {along} m on: {out:?}");
        assert!(out.on_ground, "yaw {yaw:#06x}");
    }
}

#[test]
fn a_raised_move_that_travels_less_is_dropped() {
    // A beam the hull walks under on the floor, but runs into when raised by
    // the step height: the stepped move lands back on the floor 2 cm on, the
    // flat one goes further, and the flat one is kept.
    let world = Boxes(vec![
        solid([-1.0, -1.0, -1.0], [8.0, 3.0, 0.0]),
        solid([1.27, -1.0, 1.9], [8.0, 3.0, 3.0]),
    ]);
    let s = state([1.0, 1.0, H], [3.0, 0.0, 0.0]);
    let out = tick(&world, &s, &east());
    let flat = pmove(&s, &east(), &MOVE, DT, &world);
    assert_eq!(out.state, flat.state);
    assert!(out.state.pos.x > 1.04, "{out:?}");
    assert!(out.planes().is_empty() && out.on_ground, "{out:?}");
}

#[test]
fn a_tie_in_travel_keeps_the_flat_move() {
    // In a corner between a rail low enough to step onto (east) and a beam
    // the hull fits under only on the floor (north), a diagonal move slides
    // north along the rail on the floor, and east along the beam when raised
    // onto the rail: the same travel, by symmetry. The flat move is kept.
    let world = Boxes(vec![
        solid([-1.0, -1.0, -1.0], [4.0, 4.0, 0.0]),
        solid([1.25, -1.0, 0.0], [4.0, 4.0, 0.2]),
        solid([-1.0, 1.25, 1.9], [4.0, 4.0, 3.0]),
    ]);
    let s = state([1.0, 1.0, H], [2.0, 2.0, 0.0]);
    let idle = cmd(0, 0, 0);
    let out = tick(&world, &s, &idle);
    let flat = pmove(&s, &idle, &MOVE, DT, &world);
    assert_eq!(out.state, flat.state);
    assert_eq!(out.planes(), [Vec3::new(-1.0, 0.0, 0.0)]);
    assert_eq!(out.state.pos.x, 1.0, "{out:?}");
    assert!(out.state.pos.y > 1.0 && out.on_ground, "{out:?}");
}

#[test]
fn a_step_up_lands_no_lower_than_it_rose() {
    // A fast hull clears a rail whose far side drops 1 m into a pit. The
    // stepped move's push-down reaches only the rise plus the probe, so it
    // finds no floor, and the hull stops at the rail instead of dropping in.
    // With every surface counted as floor (`min_floor_normal_z` 0), a miss's
    // zero normal passes the floor rule, and only the push-down's own hit
    // tells it apart.
    let world = Boxes(vec![
        solid([-1.0, -1.0, -1.0], [2.0, 3.0, 0.0]),
        solid([2.0, -1.0, -1.0], [2.05, 3.0, 0.3]),
        solid([2.05, -1.0, -2.0], [8.0, 3.0, -1.0]),
    ]);
    let s = state([1.73, 1.0, H], [40.0, 0.0, 0.0]);
    let all_floor = GroundConfig {
        min_floor_normal_z: 0.0,
        ..GROUND
    };
    for ground in [GROUND, all_floor] {
        let out = ground_move(&s, &east(), &MOVE, &ground, DT, &world);
        let flat = pmove(&s, &east(), &MOVE, DT, &world);
        assert_eq!(out.state, flat.state, "{ground:?}");
        assert_eq!((out.state.pos.x, out.state.pos.z), (1.75, H), "{ground:?}");
        assert!(out.on_ground, "{ground:?}");
    }
}

#[test]
fn a_hull_resting_on_the_floor_keeps_the_sign_of_a_zero_coordinate() {
    // Standing still at x = -0 on a grid floor: the step-down's trace hits
    // at once, and a trace's stop is `s + d * t`, which turns -0 into +0. A
    // hit at fraction 0 is no drop, so the hull stays where it is, bit for
    // bit (the state hash sees the sign of a zero).
    let floor = Slab {
        points: [(-4.0, -4.0), (4.0, -4.0), (4.0, 4.0), (-4.0, 4.0)]
            .map(|(x, y)| Point2::new(x, y))
            .to_vec(),
        bottom: -0.5,
        top: 0.0,
    };
    let world = Grid::new(vec![], vec![floor]).unwrap();
    let s = state([-0.0, 1.0, H], [0.0; 3]);
    let out = tick(&world, &s, &cmd(0, 0, 0));
    assert_eq!(out.state.pos.x.to_bits(), (-0.0f64).to_bits(), "{out:?}");
    assert_eq!(out.state, s);
    assert!(out.on_ground);
}
