//! The `collision` benchmarks' inputs, in the room's grid, built the same
//! way on every target, and the one call each measures. `tests/cases.rs`
//! checks them everywhere.

use eng_collision_api::{BoxHull, CollisionWorld, Trace};
use eng_collision_grid::Grid;
use eng_core::{PlayerCommand, Tick};
use eng_math::{Vec3, WorldPos};
use game_testbed_sim::TICK_RATE;
use game_testbed_sim::room::layout;
use kit_fps::{GROUND_PROBE, GroundOutcome, MoveState, ground_move};

/// The room's collision grid, as the room builds it.
pub fn room_grid() -> Grid {
    Grid::new(layout::walls(), layout::slabs()).expect("the room's grid builds")
}

/// One trace of the player's hull through the room.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceCase {
    pub start: WorldPos,
    pub end: WorldPos,
    pub hull: BoxHull,
}

/// The hull's centre standing on the hall's floor (z = 0).
const STANDING: f64 = 0.875;

/// One tick's walk and a little more (0.1 m) east into the hall's east
/// wall (x = 12), from 5 cm in front of where the hull meets it: it stops
/// at x = 11.75.
pub const INTO_WALL: TraceCase = TraceCase {
    start: WorldPos::new(11.70, 4.5, STANDING),
    end: WorldPos::new(11.80, 4.5, STANDING),
    hull: layout::HULL,
};

/// 11 m across the hall, west to east and a little north, past the pillar
/// and south of the step, through a dozen cells, hitting nothing.
pub const ACROSS_HALL: TraceCase = TraceCase {
    start: WorldPos::new(0.5, 2.0, STANDING),
    end: WorldPos::new(11.5, 3.5, STANDING),
    hull: layout::HULL,
};

/// `kit-fps`'s ground check at spawn S0: [`GROUND_PROBE`] down, which meets
/// the floor at once.
pub const GROUND_PROBE_CASE: TraceCase = TraceCase {
    start: WorldPos::new(1.5, 4.5, STANDING),
    end: WorldPos::new(1.5, 4.5, STANDING - GROUND_PROBE),
    hull: layout::HULL,
};

/// One trace: what `collision::trace::*` measure.
pub fn trace<C: CollisionWorld + ?Sized>(world: &C, case: &TraceCase) -> Trace {
    world.trace(case.start, case.end, &case.hull)
}

/// One tick of `ground_move` in the room, from `state` with `cmd`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundCase {
    pub state: MoveState,
    pub cmd: PlayerCommand,
}

/// Full forward along `yaw`, a 16-bit command angle.
const fn forward(yaw: u16) -> PlayerCommand {
    PlayerCommand {
        forward: 127,
        view_yaw: yaw,
        ..PlayerCommand::idle(Tick(1))
    }
}

/// Walking east at 3 m/s on the open floor south of the pillar.
pub const WALK: GroundCase = GroundCase {
    state: MoveState {
        pos: WorldPos::new(2.0, 4.5, STANDING),
        vel: Vec3::new(3.0, 0.0, 0.0),
    },
    cmd: forward(0),
};

/// Walking north at 3.5 m/s into the 0.2 m step's south edge (y = 5.25) at
/// x = 9.3, as `room-walk` does: the flat slide meets the step's side, and
/// the stepped one ends on top of it.
pub const STEP_UP: GroundCase = GroundCase {
    state: MoveState {
        pos: WorldPos::new(9.3, 4.97, STANDING),
        vel: Vec3::new(0.0, 3.5, 0.0),
    },
    // A quarter turn: north.
    cmd: forward(0x4000),
};

/// One tick of `ground_move` with the room's tuning: what
/// `collision::movement::*` measure.
pub fn ground<C: CollisionWorld + ?Sized>(world: &C, case: &GroundCase) -> GroundOutcome {
    // The sim's own tick length (`SimInfo::dt`).
    let dt = 1.0 / TICK_RATE as f32;
    ground_move(
        &case.state,
        &case.cmd,
        &layout::MOVE,
        &layout::GROUND,
        dt,
        world,
    )
}
