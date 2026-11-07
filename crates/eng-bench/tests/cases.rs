//! Each benchmark's input does what its name says, on every target: the
//! benchmarks themselves run only on Linux (`benches/`), and a count is
//! worth comparing only while it measures the same thing. Both benches'
//! `cases.rs` are included here as they are there.

#[path = "../../eng-collision-grid/tests/oracle/brute.rs"]
mod brute;
#[path = "../benches/collision/cases.rs"]
mod collision;
#[path = "../benches/sim/cases.rs"]
mod sim;

use brute::Brute;
use collision::{ACROSS_HALL, GROUND_PROBE_CASE, INTO_WALL, STEP_UP, TraceCase, WALK};
use eng_collision_api::CollisionWorld;
use eng_math::Vec3;
use game_testbed_sim::WallBump;
use game_testbed_sim::room::layout::{self, STEP_TOP};
use kit_fps::MoveState;
use sim::{WARMUP_TICKS, Walking};

/// The hull's centre above the floor it stands on.
const H: f64 = 0.875;

fn player(walking: &Walking) -> MoveState {
    let (_, state) = walking
        .sim
        .world()
        .storage::<MoveState>()
        .iter()
        .next()
        .expect("one player");
    *state
}

/// A measured tick that moves the walker, bumps nothing and keeps it at
/// its height.
fn walks_in_the_open(before: Walking) {
    assert_eq!(before.sim.tick().0, u64::from(WARMUP_TICKS));
    let from = player(&before);
    assert!(from.vel.x != 0.0, "moving after the warm-up: {from:?}");
    let after = sim::tick(before);
    let to = player(&after);
    assert_eq!(after.sim.tick().0, u64::from(WARMUP_TICKS) + 1);
    assert!(to.pos.x != from.pos.x, "{from:?} -> {to:?}");
    assert_eq!(to.pos.z, from.pos.z, "stays on its floor");
    assert!(
        after.sim.world().events::<WallBump>().is_empty(),
        "the measured tick touches nothing"
    );
}

#[test]
fn the_dot_walks_in_the_open() {
    walks_in_the_open(sim::dots_walking());
}

#[test]
fn the_room_walker_walks_in_the_open() {
    walks_in_the_open(sim::room_walking());
}

#[test]
fn the_room_builds_with_its_sim_data() {
    let sim = sim::room_build(sim::config());
    assert!(sim.has_sim_data());
    assert_eq!(sim.config().players, 1);
}

fn grid_trace(case: &TraceCase) -> eng_collision_api::Trace {
    collision::trace(&collision::room_grid(), case)
}

#[test]
fn the_trace_into_the_wall_stops_on_it() {
    let tr = grid_trace(&INTO_WALL);
    assert!(!tr.start_solid);
    assert!(tr.hit() && tr.fraction > 0.0, "{tr:?}");
    assert_eq!(tr.normal, Vec3::new(-1.0, 0.0, 0.0));
    assert_eq!(tr.end.x, layout::HALL_X - 0.25, "in contact with x = 12");
}

#[test]
fn the_trace_across_the_hall_hits_nothing() {
    let tr = grid_trace(&ACROSS_HALL);
    assert!(!tr.start_solid && !tr.hit(), "{tr:?}");
    assert_eq!(tr.end, ACROSS_HALL.end);
    let (dx, dy) = (
        ACROSS_HALL.end.x - ACROSS_HALL.start.x,
        ACROSS_HALL.end.y - ACROSS_HALL.start.y,
    );
    assert!(dx * dx + dy * dy > 100.0, "more than 10 m");
}

#[test]
fn the_ground_probe_meets_the_floor_at_once() {
    let tr = grid_trace(&GROUND_PROBE_CASE);
    assert!(!tr.start_solid);
    assert_eq!(tr.fraction, 0.0);
    assert_eq!(tr.normal, Vec3::new(0.0, 0.0, 1.0));
}

/// The brute-force world, measured beside the grid, gives the same traces.
#[test]
fn the_brute_force_world_gives_the_grids_traces() {
    let grid = collision::room_grid();
    let brute = Brute::new(&layout::walls(), &layout::slabs());
    for case in [INTO_WALL, ACROSS_HALL, GROUND_PROBE_CASE] {
        assert_eq!(
            collision::trace(&brute, &case),
            collision::trace(&grid, &case),
            "{case:?}"
        );
    }
}

#[test]
fn ground_move_walks_on_the_flat() {
    let out = collision::ground(&collision::room_grid(), &WALK);
    assert!(out.on_ground && out.planes().is_empty(), "{out:?}");
    assert!(out.state.pos.x > WALK.state.pos.x);
    assert_eq!(out.state.pos.z, H);
}

#[test]
fn ground_move_climbs_the_step() {
    let out = collision::ground(&collision::room_grid(), &STEP_UP);
    assert!(out.on_ground, "{out:?}");
    assert_eq!(out.state.pos.z, STEP_TOP + H, "on top of the step");
    assert!(out.state.pos.y + 0.25 > 5.25, "over its south edge");
}

/// The grid's `CollisionWorld` is what the benchmarks call through
/// `collision::trace`, a generic: the same trace as calling it directly.
#[test]
fn the_trace_helper_is_the_worlds_own() {
    let grid = collision::room_grid();
    let c = INTO_WALL;
    assert_eq!(
        collision::trace(&grid, &c),
        grid.trace(c.start, c.end, &c.hull)
    );
}
