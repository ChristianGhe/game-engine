//! The room's sim data (rung 2 criterion 5; ADR-0023): the room's
//! snapshot fits a datagram; its geometry is outside the per-tick state,
//! its hash and its snapshots; its `sim_data_hash` changes with the room's
//! data only; the dot map has none; and the `[sim]` budgets hold. The
//! probe's side (recordings, reports, stream headers, dumps) is in
//! `game-testbed/tests/room.rs`.

use eng_collision_grid::{Grid, Point2};
use eng_core::{PlayerCommand, Tick};
use eng_net_transport::MAX_DATAGRAM;
use eng_net_wire::{Message, Snapshot};
use eng_schema_core::to_bytes;
use eng_sim::{Sim, SimConfig};
use game_testbed_sim::room::{self, RoomData, RoomMap};
use game_testbed_sim::{TICK_RATE, build};

fn config(players: u8, seed: u64) -> SimConfig {
    SimConfig {
        tick_rate: TICK_RATE,
        players,
        seed,
    }
}

fn room_sim(players: u8, seed: u64) -> Sim {
    room::build(config(players, seed)).unwrap()
}

/// One tick of every player running at `yaw` (16-bit).
fn step(sim: &mut Sim, forward: i8, yaw: u16) {
    let tick = sim.tick().next();
    let cmds: Vec<PlayerCommand> = (0..sim.config().players)
        .map(|_| PlayerCommand {
            forward,
            view_yaw: yaw,
            ..PlayerCommand::idle(tick)
        })
        .collect();
    sim.step(&cmds).unwrap();
}

/// The `Snapshot` message the session sends for the sim's state.
fn snapshot_message_len(sim: &Sim) -> usize {
    Message::Snapshot(Snapshot {
        tick: sim.tick(),
        state: sim.snapshot(),
    })
    .encode()
    .unwrap()
    .len()
}

/// The canonical bytes of the room's constants, as the sim-data hash reads
/// them.
fn sim_data_len(sim: &Sim) -> usize {
    let w = sim.world();
    to_bytes(w.constant::<Grid>()).len()
        + to_bytes(w.constant::<RoomMap>()).len()
        + to_bytes(w.constant::<kit_fps::GroundConfig>()).len()
        + to_bytes(w.constant::<kit_fps::MoveConfig>()).len()
}

#[test]
fn the_rooms_snapshot_fits_a_datagram() {
    // One player: at tick 0, and after ticks with a command in the state.
    let mut sim = room_sim(1, 1);
    let at_start = snapshot_message_len(&sim);
    for _ in 0..30 {
        step(&mut sim, 127, 0xe000);
    }
    let moving = snapshot_message_len(&sim);
    assert!(at_start <= moving, "{at_start} {moving}");
    assert!(moving <= 700, "{moving} bytes, of {MAX_DATAGRAM}");
    // A player for each of the four spawns fits too, moving.
    let mut sim = room_sim(4, 1);
    step(&mut sim, 127, 0);
    let four = snapshot_message_len(&sim);
    assert!(four <= MAX_DATAGRAM, "{four}");
    // Its sim data alone would not.
    assert!(sim_data_len(&sim) > MAX_DATAGRAM, "{}", sim_data_len(&sim));
}

#[test]
fn sim_data_is_outside_the_state_its_hash_and_snapshots() {
    // The same room with other sim data that no player reaches: a region's
    // light, and a pillar beyond the walls.
    let mut data = RoomData::room();
    data.map.regions[0].light = 10;
    let mut far = data.slabs[5].clone();
    for p in &mut far.points {
        *p = Point2::new(p.x + 100.0, p.y);
    }
    data.slabs.push(far);
    let mut a = room_sim(2, 3);
    let mut b = room::build_with(config(2, 3), data).unwrap();
    assert_ne!(a.sim_data_hash(), b.sim_data_hash());
    assert_eq!(a.protocol_version(), b.protocol_version());
    for tick in 0..60 {
        assert_eq!(a.state_hash(), b.state_hash(), "tick {tick}");
        assert_eq!(a.snapshot(), b.snapshot(), "tick {tick}");
        assert_eq!(a.state_len(), b.state_len(), "tick {tick}");
        step(&mut a, 127, 0x1000);
        step(&mut b, 127, 0x1000);
    }
    // A snapshot restores into a sim with other constants: it carries none.
    let mut c = room_sim(2, 3);
    c.restore(&a.snapshot()).unwrap();
    assert_eq!(c.state_hash(), a.state_hash());
    assert_eq!(c.sim_data_hash(), room_sim(1, 0).sim_data_hash());
    // Nor does the state hold the grid's bytes.
    let grid = to_bytes(a.world().constant::<Grid>());
    let state = a.state_bytes();
    assert!(state.len() < grid.len(), "{} {}", state.len(), grid.len());
    assert!(!state.windows(64).any(|w| grid.windows(64).any(|g| g == w)));
}

#[test]
fn the_sim_data_hash_changes_with_the_rooms_data_only() {
    let hash = room_sim(1, 1).sim_data_hash();
    assert_ne!(hash, 0);
    for players in [1, 2, 4, 9] {
        for seed in [0, 1, 7, u64::MAX] {
            let mut sim = room_sim(players, seed);
            assert_eq!(sim.sim_data_hash(), hash, "{players} {seed}");
            for _ in 0..10 {
                step(&mut sim, 100, 0x3000);
            }
            assert_eq!(sim.sim_data_hash(), hash, "after ticks");
        }
    }
    // Each part of the data moves it, each to its own hash.
    type Edit = fn(&mut RoomData);
    let edits: [(&str, Edit); 9] = [
        ("a wall's top", |d| d.walls[0].top = 3.5),
        ("a slab's corner", |d| d.slabs[5].points[0].x = 5.25),
        ("a region's light", |d| d.map.regions[3].light = 254),
        ("a region's sky", |d| d.map.regions[1].sky = true),
        ("a spawn's yaw", |d| d.map.spawns[2].yaw.0 = 1),
        ("a thing's facing", |d| d.map.things[1].facing.0 ^= 1),
        ("the marker's turn", |d| d.map.things[0].turn_ticks = 481),
        ("the top speed", |d| d.movement.max_speed = 4.5),
        ("gravity", |d| d.ground.gravity = 9.8),
    ];
    let mut seen = vec![hash];
    for (what, edit) in edits {
        let mut data = RoomData::room();
        edit(&mut data);
        let edited = room::build_with(config(1, 1), data)
            .unwrap()
            .sim_data_hash();
        assert!(!seen.contains(&edited), "{what}: {edited:#018x}");
        seen.push(edited);
    }
}

#[test]
fn the_dot_map_has_none_and_the_room_has_four_constants() {
    let dots = build(config(1, 1)).unwrap();
    assert!(!dots.has_sim_data());
    assert_eq!(dots.sim_data_hash(), 0);
    let room = room_sim(1, 1);
    assert!(room.has_sim_data());
    let names: Vec<&str> = room.world().constant_names().collect();
    assert_eq!(
        names,
        [
            "eng.collision.grid.Grid",
            "kit_fps.GroundConfig",
            "kit_fps.MoveConfig",
            "testbed.RoomMap"
        ]
    );
    assert_ne!(room.protocol_version(), dots.protocol_version());
}

/// Known answers, so a change to the room shows here before its hash
/// streams exist. They are in `docs/rungs/2/patches/room-scenarios.patch`'s
/// streams (`sim_protocol=` and `sim_data=` in each header): a deliberate
/// change to the room's data, schemas or systems regenerates those streams
/// with the patch, in a scratch clone.
#[test]
fn the_rooms_sim_data_hash_and_protocol_are_pinned() {
    let sim = room_sim(1, 1);
    assert_eq!(sim.sim_data_hash(), 0x3ac5_0d17_1e07_51f1);
    assert_eq!(sim.protocol_version(), 0xc225_0f5f_231f_f6d3);
}

/// The `[sim]` budgets (4,096 state bytes, 16 entities) hold with one
/// player on every walk the scenarios take, and the constants are left out.
#[test]
fn the_room_keeps_the_sim_budgets() {
    for yaw in (0..16).map(|k| k * 0x1000) {
        let mut sim = room_sim(1, 1);
        assert_eq!(sim.world().entity_count(), 4);
        for _ in 0..600 {
            step(&mut sim, 127, yaw);
            assert!(sim.state_len() <= 4096, "{}", sim.state_len());
            assert!(sim.world().entity_count() <= 16);
        }
        assert!(sim.state_len() < 700, "{}", sim.state_len());
        assert_eq!(sim.tick(), Tick(600));
    }
}
