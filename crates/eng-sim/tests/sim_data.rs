//! Sim data (ADR-0023): a sim with constant resources hashes and
//! snapshots its per-tick state without them, `sim_data_hash` covers their
//! content once, and the protocol covers their names and schemas.

use eng_core::{PlayerCommand, Tick};
use eng_ecs::{Schema, World};
use eng_math::Fnv1a64;
use eng_sim::*;

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Value", version = 1)]
struct Value(f32);

/// How fast `Value` grows: sim data in one sim, a literal in the other.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Gain", version = 1)]
struct Gain(f32);

/// A wall segment: two endpoints and a z range, 48 bytes encoded.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Segment", version = 1)]
struct Segment {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    z0: f64,
    z1: f64,
}

#[derive(Debug, Clone, PartialEq, Schema)]
#[schema(name = "test.Walls", version = 1)]
struct Walls {
    segments: Vec<Segment>,
}

const CONFIG: SimConfig = SimConfig {
    tick_rate: 60,
    players: 1,
    seed: 7,
};

/// The most a snapshot may be: one datagram (`eng-net-transport`).
const MAX_DATAGRAM: usize = 1200;

fn walls(n: usize) -> Walls {
    let segments = (0..n)
        .map(|i| Segment {
            x0: i as f64,
            y0: 0.0,
            x1: i as f64 + 1.0,
            y1: 0.25,
            z0: 0.0,
            z1: 4.0,
        })
        .collect();
    Walls { segments }
}

/// Grows `Value` by the constant `Gain` times the forward input.
fn grow_by_constant(w: &mut World) {
    let gain = w.constant::<Gain>().0;
    // Read as a map's collision would read its walls.
    assert!(!w.constant::<Walls>().segments.is_empty());
    let forward = f32::from(w.resource::<TickInputs>().commands[0].forward);
    w.resource_mut::<Value>().0 += gain * forward;
}

/// The same step with the gain as a literal: no sim data.
fn grow_by_literal(w: &mut World) {
    let forward = f32::from(w.resource::<TickInputs>().commands[0].forward);
    w.resource_mut::<Value>().0 += 0.5 * forward;
}

fn plain_sim() -> Sim {
    let mut b = SimBuilder::new(CONFIG);
    b.world_mut().insert_resource(Value(0.0));
    b.add_system(System::new("test.grow", Stage::Update, grow_by_literal));
    b.build().unwrap()
}

fn data_sim(gain: f32, wall_count: usize) -> Sim {
    let mut b = SimBuilder::new(CONFIG);
    b.world_mut().insert_resource(Value(0.0));
    b.world_mut().insert_constant(Gain(gain));
    b.world_mut().insert_constant(walls(wall_count));
    b.add_system(System::new("test.grow", Stage::Update, grow_by_constant));
    b.build().unwrap()
}

fn cmd(tick: u64) -> PlayerCommand {
    PlayerCommand {
        forward: (tick % 5) as i8,
        ..PlayerCommand::idle(Tick(tick))
    }
}

#[test]
fn the_state_hash_and_snapshot_leave_the_sim_data_out() {
    let mut plain = plain_sim();
    // 100 walls: 4.8 kB of sim data, four datagrams' worth.
    let mut data = data_sim(0.5, 100);
    for tick in 0..=30 {
        if tick > 0 {
            plain.step(&[cmd(tick)]).unwrap();
            data.step(&[cmd(tick)]).unwrap();
        }
        assert_eq!(data.state_hash(), plain.state_hash(), "tick {tick}");
        assert_eq!(data.state_bytes(), plain.state_bytes(), "tick {tick}");
        assert_eq!(data.state_len(), plain.state_len(), "tick {tick}");
        // The snapshot is the protocol, then the same state bytes.
        let (snapshot, without) = (data.snapshot(), plain.snapshot());
        assert_eq!(snapshot[..8], data.protocol_version().to_le_bytes());
        assert_eq!(snapshot[8..], without[8..], "tick {tick}");
        assert!(snapshot.len() < MAX_DATAGRAM, "{} bytes", snapshot.len());
    }
    assert_eq!(data.world().resource::<Value>().0, 30.0);
}

#[test]
fn the_sim_data_hash_is_fnv_over_the_constants_in_name_order() {
    assert_eq!(plain_sim().sim_data_hash(), 0, "no sim data, hash 0");
    assert!(!plain_sim().has_sim_data());
    let sim = data_sim(0.5, 3);
    assert!(sim.has_sim_data());
    // `test.Gain` sorts before `test.Walls`.
    let mut h = Fnv1a64::new();
    h.write(&eng_schema_core::to_bytes(&Gain(0.5)));
    h.write(&eng_schema_core::to_bytes(&walls(3)));
    assert_eq!(sim.sim_data_hash(), h.finish());
}

#[test]
fn the_sim_data_hash_follows_the_content_and_the_protocol_the_schemas() {
    let base = data_sim(0.5, 3);
    let other_gain = data_sim(0.25, 3);
    let other_walls = data_sim(0.5, 4);
    assert_eq!(data_sim(0.5, 3).sim_data_hash(), base.sim_data_hash());
    assert_ne!(other_gain.sim_data_hash(), base.sim_data_hash());
    assert_ne!(other_walls.sim_data_hash(), base.sim_data_hash());
    // The content is not protocol: the same schemas and systems with other
    // data share a protocol (rung 5's handshake carries the data's hash).
    assert_eq!(other_gain.protocol_version(), base.protocol_version());
    assert_eq!(other_walls.protocol_version(), base.protocol_version());
    // The schemas are: the constants change the protocol.
    assert_ne!(plain_sim().protocol_version(), base.protocol_version());
}

#[test]
fn a_constant_changes_the_protocol_even_with_the_same_registry() {
    // `Gain` as a resource and as a constant: the registry holds the same
    // schemas, and only the protocol's `constants` section tells them apart.
    let build = |constant: bool| {
        let mut b = SimBuilder::new(CONFIG);
        b.world_mut().insert_resource(Value(0.0));
        if constant {
            b.world_mut().insert_constant(Gain(1.0));
        } else {
            b.world_mut().insert_resource(Gain(1.0));
        }
        b.build().unwrap()
    };
    let (resource, constant) = (build(false), build(true));
    assert_eq!(
        resource.world().registry().iter().count(),
        constant.world().registry().iter().count()
    );
    assert_ne!(resource.protocol_version(), constant.protocol_version());
}

#[test]
fn a_snapshot_restores_into_a_sim_with_its_own_sim_data() {
    let mut source = data_sim(0.5, 3);
    for tick in 1..=10 {
        source.step(&[cmd(tick)]).unwrap();
    }
    let mut target = data_sim(0.5, 3);
    target.restore(&source.snapshot()).unwrap();
    assert_eq!(target.state_hash(), source.state_hash());
    assert_eq!(target.world().constant::<Walls>(), &walls(3));
    assert_eq!(target.sim_data_hash(), source.sim_data_hash());
    // Stepping on gives the same states.
    for tick in 11..=20 {
        source.step(&[cmd(tick)]).unwrap();
        target.step(&[cmd(tick)]).unwrap();
        assert_eq!(target.state_hash(), source.state_hash());
    }
}

#[test]
#[should_panic(expected = "[E-SIM-DATA] constant test.Gain inserted after the sim was built")]
fn a_system_cannot_insert_sim_data() {
    fn sneak(w: &mut World) {
        w.insert_constant(Gain(2.0));
    }
    let mut b = SimBuilder::new(CONFIG);
    b.world_mut().insert_constant(Gain(1.0));
    b.add_system(System::new("test.sneak", Stage::Update, sneak));
    let mut sim = b.build().unwrap();
    sim.step(&[cmd(1)]).unwrap();
}

#[test]
#[should_panic(
    expected = "[E-NAN] in the sim data (constant resources): non-finite float at /test.Gain/0"
)]
fn non_finite_sim_data_fails_the_build() {
    let mut b = SimBuilder::new(CONFIG);
    b.world_mut().insert_constant(Gain(f32::INFINITY));
    let _ = b.build();
}
