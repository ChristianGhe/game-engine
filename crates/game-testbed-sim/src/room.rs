//! The room (rung 2 plan, "The room"; decisions 1 to 4): a first-person
//! hall with a platform and its step, a ledge, a pillar, an alcove and a
//! skylight, walked over by `kit-fps`'s [`ground_move`] against an
//! [`eng_collision_grid::Grid`]. [`layout`] gives every size.
//!
//! **Sim data** (ADR-0023). The room's geometry is constant resources,
//! inserted before the sim is built and outside its per-tick state, hash
//! and snapshots: the [`Grid`] (its walls, slabs and index), the
//! [`RoomMap`] (regions, spawns, things) and the [`MoveConfig`] and
//! [`GroundConfig`] tuning. Their content is the sim's `sim_data_hash`, and
//! their names and schemas are in its protocol.
//!
//! **The state:** one entity per player, in slot order from entity 0, with
//! a [`PlayerSlot`], a [`MoveState`] (the hull's centre) and a [`View`];
//! then one per thing, with a [`Thing`], and a [`Turning`] for the marker.
//!
//! **Each tick** (both in `Update`, independent, so their names order
//! them): `testbed.room.move_players` moves each player with a command by
//! [`ground_move`], keeps the command's view angles, and reports every
//! plane a move was clipped against as a [`WallBump`]; and
//! `testbed.room.turn_things` sets each turning thing's facing from the
//! tick.

pub mod layout;

use alloc::vec::Vec;

use eng_collision_grid::{Grid, Point2, Slab, Wall};
use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_ecs::{Entity, World};
use eng_math::{BAngle, Vec3, WorldPos};
use eng_schema_core::Schema;
use eng_sim::{
    ScheduleError, Sim, SimBuilder, SimConfig, SimInfo, SimTick, Stage, System, TickInputs,
};
use kit_fps::{GroundConfig, GroundOutcome, MoveConfig, MoveState, ground_move};

use crate::WallBump;

/// The system that moves the players.
pub const MOVE_PLAYERS: &str = "testbed.room.move_players";
/// The system that turns the things.
pub const TURN_THINGS: &str = "testbed.room.turn_things";

/// The room's regions, spawns and things: its sim data beside the
/// [`Grid`].
#[derive(Debug, Clone, PartialEq, Schema)]
#[schema(name = "testbed.RoomMap", version = 1)]
pub struct RoomMap {
    pub regions: Vec<Region>,
    /// Player `p` starts at spawn `p % spawns.len()`.
    pub spawns: Vec<Spawn>,
    pub things: Vec<ThingSpawn>,
}

impl RoomMap {
    /// The index of the region holding the point `(x, y)`, edges included:
    /// the last listed that holds it, or `None` outside them all.
    pub fn region_at(&self, x: f64, y: f64) -> Option<usize> {
        self.regions
            .iter()
            .rposition(|r| r.min.x <= x && x <= r.max.x && r.min.y <= y && y <= r.max.y)
    }
}

/// An area of the floor plan with its own light: a rectangle from `min` to
/// `max` (m). Where regions overlap, the later one holds the point.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "testbed.Region", version = 1)]
pub struct Region {
    pub min: Point2,
    pub max: Point2,
    /// A Doom light level, 0 to 255; the renderer maps it to lux.
    pub light: u8,
    /// Its ceiling is open to the sky.
    pub sky: bool,
}

/// Where a player starts: its feet, on the floor, and its yaw.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "testbed.Spawn", version = 1)]
pub struct Spawn {
    pub feet: WorldPos,
    pub yaw: BAngle,
}

/// What a thing is, for its sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "testbed.ThingKind", version = 1)]
pub enum ThingKind {
    Marker,
    Crate,
    Barrel,
}

/// Where a thing starts, and how it turns.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "testbed.ThingSpawn", version = 1)]
pub struct ThingSpawn {
    pub kind: ThingKind,
    /// The point it stands on (m).
    pub feet: WorldPos,
    /// Its facing at tick 0.
    pub facing: BAngle,
    /// Ticks per full turn, counter-clockwise; 0 stands still.
    pub turn_ticks: u32,
}

/// A thing in the room (decision 4): what it is, the point it stands on and
/// its facing. Things do not block.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "testbed.Thing", version = 1)]
pub struct Thing {
    pub kind: ThingKind,
    pub feet: WorldPos,
    pub facing: BAngle,
}

/// A thing that turns: its facing at tick 0, and the ticks per full turn
/// (at least 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "testbed.Turning", version = 1)]
pub struct Turning {
    pub from: BAngle,
    pub ticks: u32,
}

impl Turning {
    /// The facing at `tick`: `from` plus `(tick mod ticks) / ticks` of a
    /// turn, rounded down, in integers. Every `ticks / 8` ticks, when that
    /// is whole, it is exactly an eighth of a turn on.
    pub fn at(&self, tick: Tick) -> BAngle {
        let ticks = u64::from(self.ticks.max(1));
        // Below 2^32 times 2^32 / ticks: no overflow, and the quotient fits.
        let on = ((tick.0 % ticks) << 32) / ticks;
        BAngle(self.from.0.wrapping_add(on as u32))
    }
}

/// Where a player looks: the view angles of its last command, its spawn's
/// yaw and level before the first. A first-person camera reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "testbed.View", version = 1)]
pub struct View {
    pub yaw: BAngle,
    pub pitch: BAngle,
}

impl View {
    /// A command's view angles.
    pub fn of(cmd: &PlayerCommand) -> Self {
        Self {
            yaw: BAngle::from_u16(cmd.view_yaw),
            pitch: BAngle::from_u16(cmd.view_pitch),
        }
    }
}

/// Everything the room is built from: its sim data before the grid is
/// built.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomData {
    pub walls: Vec<Wall>,
    pub slabs: Vec<Slab>,
    pub map: RoomMap,
    pub movement: MoveConfig,
    pub ground: GroundConfig,
}

impl RoomData {
    /// The room as [`layout`] describes it.
    pub fn room() -> Self {
        Self {
            walls: layout::walls(),
            slabs: layout::slabs(),
            map: layout::map(),
            movement: layout::MOVE,
            ground: layout::GROUND,
        }
    }
}

/// Builds the room's simulation for a session: the testbed's `room` map.
/// Any player count builds; players beyond the spawns share them.
pub fn build(config: SimConfig) -> Result<Sim, ScheduleError> {
    build_with(config, RoomData::room())
}

/// Builds a room from `data`, as [`build`] does from the room's own.
///
/// # Panics
/// With `[E-MAP]` if the grid refuses the walls and slabs, or there are no
/// spawns: the room's own data is tested.
pub fn build_with(config: SimConfig, data: RoomData) -> Result<Sim, ScheduleError> {
    let grid = Grid::new(data.walls, data.slabs)
        .unwrap_or_else(|e| panic!("[E-MAP] the room's grid refused its data: {e}"));
    assert!(
        !data.map.spawns.is_empty(),
        "[E-MAP] the room has no spawns"
    );
    let mut builder = SimBuilder::new(config);
    let world = builder.world_mut();
    world.register_component::<PlayerSlot>();
    world.register_component::<MoveState>();
    world.register_component::<View>();
    world.register_component::<Thing>();
    world.register_component::<Turning>();
    world.register_event::<WallBump>();
    let half_height = f64::from(data.movement.hull.half_extents.z);
    for player in 0..config.players {
        let spawn = data.map.spawns[usize::from(player) % data.map.spawns.len()];
        let e = world.spawn();
        world.insert(e, PlayerSlot(player));
        world.insert(
            e,
            MoveState {
                pos: WorldPos::new(spawn.feet.x, spawn.feet.y, spawn.feet.z + half_height),
                vel: Vec3::ZERO,
            },
        );
        world.insert(
            e,
            View {
                yaw: spawn.yaw,
                pitch: BAngle::ZERO,
            },
        );
    }
    for thing in &data.map.things {
        let e = world.spawn();
        world.insert(
            e,
            Thing {
                kind: thing.kind,
                feet: thing.feet,
                facing: thing.facing,
            },
        );
        if thing.turn_ticks > 0 {
            world.insert(
                e,
                Turning {
                    from: thing.facing,
                    ticks: thing.turn_ticks,
                },
            );
        }
    }
    world.insert_constant(grid);
    world.insert_constant(data.map);
    world.insert_constant(data.movement);
    world.insert_constant(data.ground);
    builder.add_system(System::new(MOVE_PLAYERS, Stage::Update, move_players));
    builder.add_system(System::new(TURN_THINGS, Stage::Update, turn_things));
    builder.build()
}

fn move_players(world: &mut World) {
    let dt = world.resource::<SimInfo>().dt();
    let moved: Vec<(Entity, GroundOutcome, View)> = {
        let grid = world.constant::<Grid>();
        let movement = world.constant::<MoveConfig>();
        let ground = world.constant::<GroundConfig>();
        let inputs = world.resource::<TickInputs>();
        let slots = world.storage::<PlayerSlot>();
        world
            .storage::<MoveState>()
            .iter()
            .filter_map(|(e, state)| {
                let cmd = slots.get(e).and_then(|slot| inputs.for_player(*slot))?;
                let outcome = ground_move(state, cmd, movement, ground, dt, grid);
                Some((e, outcome, View::of(cmd)))
            })
            .collect()
    };
    let mut bumps = Vec::new();
    for (e, outcome, view) in moved {
        *world
            .get_mut::<MoveState>(e)
            .expect("a player has a move state") = outcome.state;
        *world.get_mut::<View>(e).expect("a player has a view") = view;
        bumps.extend(
            outcome
                .planes()
                .iter()
                .map(|&normal| WallBump { entity: e, normal }),
        );
    }
    for bump in bumps {
        world.send_event(bump);
    }
}

fn turn_things(world: &mut World) {
    let tick = world.resource::<SimTick>().0;
    let facings: Vec<(Entity, BAngle)> = world
        .storage::<Turning>()
        .iter()
        .map(|(e, turning)| (e, turning.at(tick)))
        .collect();
    for (e, facing) in facings {
        world.get_mut::<Thing>(e).expect("a turning thing").facing = facing;
    }
}
