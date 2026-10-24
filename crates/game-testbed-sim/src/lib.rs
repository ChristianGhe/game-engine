//! The testbed game's simulation: the content engine CI exercises.
//!
//! Rung 1a, "headless dot": one dot per player in an 8 m by 6 m room, moved
//! by [`PlayerCommand`](eng_core::PlayerCommand)s through `kit-fps`. A dot's
//! start position comes from the gameplay random stream, so the seed matters.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;

use eng_collision_api::{BoxHull, BoxRoom};
use eng_core::PlayerSlot;
use eng_ecs::{Entity, World};
use eng_math::{Vec3, WorldPos};
use eng_schema_core::Schema;
use eng_sim::{
    GameplayRng, ScheduleError, Sim, SimBuilder, SimConfig, SimInfo, Stage, System, TickInputs,
};
use kit_fps::{MoveConfig, MoveState, pmove};

pub const TICK_RATE: u32 = 60;

pub const ROOM: BoxRoom = BoxRoom {
    min: WorldPos::new(0.0, 0.0, 0.0),
    max: WorldPos::new(8.0, 6.0, 2.0),
};

pub const MOVE: MoveConfig = MoveConfig {
    max_speed: 4.0,
    accel: 10.0,
    friction: 6.0,
    stop_speed: 1.0,
    hull: BoxHull {
        half_extents: Vec3::new(0.25, 0.25, 0.5),
    },
};

/// Marks a player's dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "testbed.Dot", version = 1)]
pub struct Dot;

/// A dot's move was clipped against a wall this tick.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "testbed.WallBump", version = 1)]
pub struct WallBump {
    pub entity: Entity,
    pub normal: Vec3,
}

/// Builds the simulation for a session.
pub fn build(config: SimConfig) -> Result<Sim, ScheduleError> {
    let mut builder = SimBuilder::new(config);
    let world = builder.world_mut();
    world.register_component::<Dot>();
    world.register_component::<PlayerSlot>();
    world.register_component::<MoveState>();
    world.register_event::<WallBump>();
    world.insert_resource(ROOM);
    world.insert_resource(MOVE);
    for player in 0..config.players {
        let rng = &mut world.resource_mut::<GameplayRng>().0;
        let x = 1.0 + 6.0 * rng.next_f64();
        let y = 1.0 + 4.0 * rng.next_f64();
        let dot = world.spawn();
        world.insert(dot, Dot);
        world.insert(dot, PlayerSlot(player));
        world.insert(
            dot,
            MoveState {
                pos: WorldPos::new(x, y, 1.0),
                vel: Vec3::ZERO,
            },
        );
    }
    builder.add_system(System::new("testbed.move_dots", Stage::Update, move_dots));
    builder.build()
}

fn move_dots(world: &mut World) {
    let dt = world.resource::<SimInfo>().dt();
    let room = *world.resource::<BoxRoom>();
    let config = *world.resource::<MoveConfig>();
    let inputs = world.resource::<TickInputs>().clone();
    let mut bumps = Vec::new();
    let (slots, states) = world.storage_pair_mut::<PlayerSlot, MoveState>();
    for (dot, state) in states.iter_mut() {
        let Some(cmd) = slots.get(dot).and_then(|slot| inputs.for_player(*slot)) else {
            continue;
        };
        let outcome = pmove(state, cmd, &config, dt, &room);
        *state = outcome.state;
        bumps.extend(outcome.planes().iter().map(|&normal| WallBump {
            entity: dot,
            normal,
        }));
    }
    for bump in bumps {
        world.send_event(bump);
    }
}
