//! The `sim` benchmarks' inputs, built the same way on every target, and the
//! one call each measures. `tests/cases.rs` checks them everywhere.

use eng_core::PlayerCommand;
use eng_sim::{Sim, SimConfig};
use game_testbed_sim::{ROOM, TICK_RATE};
use kit_fps::MoveState;

/// Ticks walked before the measured one, so the hull is moving and the
/// measured tick is a steady walking tick, not the first.
pub const WARMUP_TICKS: u32 = 20;

/// The seed of both sims: the dot's start comes from it.
pub const SEED: u64 = 1;

/// A one-player sim walking, and the commands for its next tick.
pub struct Walking {
    pub sim: Sim,
    pub commands: [PlayerCommand; 1],
}

/// One player and [`SEED`], at the testbed's tick rate.
pub fn config() -> SimConfig {
    SimConfig {
        tick_rate: TICK_RATE,
        players: 1,
        seed: SEED,
    }
}

/// The command for the tick after `sim`'s: full forward along `yaw`.
fn walk(sim: &Sim, yaw: u16) -> [PlayerCommand; 1] {
    [PlayerCommand {
        forward: 127,
        view_yaw: yaw,
        ..PlayerCommand::idle(sim.tick().next())
    }]
}

/// `sim` after [`WARMUP_TICKS`] ticks walking along `yaw`, with the commands
/// for the next.
fn warmed(mut sim: Sim, yaw: u16) -> Walking {
    for _ in 0..WARMUP_TICKS {
        sim.step(&walk(&sim, yaw)).expect("a one-player command");
    }
    let commands = walk(&sim, yaw);
    Walking { sim, commands }
}

/// The dot map with one player, walking toward the far side of the room
/// from where the seed put it (east from the west half, west from the east
/// half), so the measured tick touches no wall.
pub fn dots_walking() -> Walking {
    let sim = game_testbed_sim::build(config()).expect("the dot map builds");
    let (_, state) = sim
        .world()
        .storage::<MoveState>()
        .iter()
        .next()
        .expect("one dot");
    let middle = (ROOM.min.x + ROOM.max.x) / 2.0;
    let yaw = if state.pos.x < middle { 0 } else { 0x8000 };
    warmed(sim, yaw)
}

/// The room with one player, walking east from spawn S0 (1.5, 4.5) toward
/// the pillar, which it does not reach.
pub fn room_walking() -> Walking {
    warmed(room_build(config()), 0)
}

/// One tick: what `sim::tick::*` measure.
pub fn tick(walking: Walking) -> Walking {
    let Walking { mut sim, commands } = walking;
    sim.step(&commands).expect("commands for the next tick");
    Walking { sim, commands }
}

/// `game_testbed_sim::room::build`: what `sim::build::room_build` measures.
pub fn room_build(config: SimConfig) -> Sim {
    game_testbed_sim::room::build(config).expect("the room builds")
}
