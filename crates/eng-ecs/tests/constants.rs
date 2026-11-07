//! Constant resources (sim data, ADR-0023): registered like resources,
//! read through a shared reference, sealed once the sim is built, and
//! outside the per-tick state, its dump and its decoding.

use eng_ecs::{Schema, World};
use eng_schema_core::{Encoder, find_non_finite};

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Pos", version = 1)]
struct Pos {
    x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Counter", version = 1)]
struct Counter(u64);

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Ping", version = 1)]
struct Ping(u32);

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

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Gravity", version = 1)]
struct Gravity(f32);

/// The same stable name as `Counter`, on another type.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Counter", version = 1)]
struct FakeCounter(u32);

fn walls(n: usize) -> Walls {
    let segments = (0..n)
        .map(|i| {
            let x = i as f64;
            Segment {
                x0: x,
                y0: 0.0,
                x1: x + 1.0,
                y1: 0.5,
                z0: 0.0,
                z1: 4.0,
            }
        })
        .collect();
    Walls { segments }
}

/// A world with one entity, a resource, an event and no constants.
fn world() -> World {
    let mut w = World::new();
    w.register_component::<Pos>();
    w.insert_resource(Counter(3));
    w.register_event::<Ping>();
    let e = w.spawn();
    w.insert(e, Pos { x: 1.5 });
    w.send_event(Ping(9));
    w
}

fn state_bytes(w: &World) -> Vec<u8> {
    let mut out = Vec::new();
    w.visit_state(&mut Encoder::new(&mut out));
    out
}

fn constant_bytes(w: &World) -> Vec<u8> {
    let mut out = Vec::new();
    w.visit_constants(&mut Encoder::new(&mut out));
    out
}

#[test]
fn constants_are_read_by_name_and_replaced_until_sealed() {
    let mut w = world();
    assert_eq!(w.constant_count(), 0);
    w.insert_constant(Gravity(9.81));
    w.insert_constant(walls(2));
    w.insert_constant(Gravity(1.62));
    assert_eq!(*w.constant::<Gravity>(), Gravity(1.62));
    assert_eq!(w.constant::<Walls>(), &walls(2));
    assert_eq!(w.constant_count(), 2);
    assert_eq!(
        w.constant_names().collect::<Vec<_>>(),
        ["test.Gravity", "test.Walls"]
    );
    // In the registry, so the protocol version covers their schemas (the
    // description of `test.Walls` holds `test.Segment`'s).
    assert!(w.registry().get("test.Walls").is_some());
    assert!(!w.constants_sealed());
    w.seal_constants();
    assert!(w.constants_sealed());
    assert_eq!(*w.constant::<Gravity>(), Gravity(1.62));
}

#[test]
fn constants_are_outside_the_state_and_the_dump() {
    let plain = world();
    let mut with = world();
    // 100 walls: 4.8 kB of sim data, four times a datagram.
    with.insert_constant(walls(100));
    with.insert_constant(Gravity(f32::NAN));
    assert_eq!(state_bytes(&with), state_bytes(&plain));
    // The dump has no constants: the NaN above is not in it, so neither
    // the per-system `[E-NAN]` walk nor `state_hash`'s sees it.
    assert_eq!(find_non_finite(&|v| with.visit_dump(v)), None);
    // Relative to the walk; eng-sim's build check and `dump-world` put it
    // under `/constants` (eng-sim's `SIM_DATA_KEY`).
    let found = find_non_finite(&|v| with.visit_constants(v)).expect("the NaN");
    assert_eq!(found.path, "/test.Gravity/0");
}

#[test]
fn decoding_a_state_keeps_the_constants() {
    let mut source = world();
    source.resource_mut::<Counter>().0 = 41;
    let bytes = state_bytes(&source);
    let mut target = world();
    target.insert_constant(walls(3));
    target.seal_constants();
    target.decode_state(&bytes).unwrap();
    assert_eq!(target.resource::<Counter>().0, 41);
    assert_eq!(target.constant::<Walls>(), &walls(3));
    assert_eq!(state_bytes(&target), bytes);
}

#[test]
fn the_canonical_encoding_is_the_values_in_name_order() {
    let mut a = World::new();
    a.insert_constant(walls(1));
    a.insert_constant(Gravity(2.0));
    let mut b = World::new();
    b.insert_constant(Gravity(2.0));
    b.insert_constant(walls(1));
    let mut expected = eng_schema_core::to_bytes(&Gravity(2.0));
    expected.extend(eng_schema_core::to_bytes(&walls(1)));
    assert_eq!(constant_bytes(&a), expected);
    assert_eq!(
        constant_bytes(&b),
        expected,
        "insertion order does not count"
    );
    assert_eq!(constant_bytes(&World::new()), Vec::<u8>::new());
}

#[test]
#[should_panic(expected = "[E-SIM-DATA] constant test.Gravity inserted after the sim was built")]
fn a_sealed_world_refuses_a_new_constant() {
    let mut w = world();
    w.seal_constants();
    w.insert_constant(Gravity(1.0));
}

#[test]
#[should_panic(expected = "[E-SIM-DATA]")]
fn a_sealed_world_refuses_a_new_value_for_a_constant() {
    let mut w = world();
    w.insert_constant(Gravity(1.0));
    w.seal_constants();
    w.insert_constant(Gravity(2.0));
}

#[test]
#[should_panic(expected = "is already registered as a Resource, not a Constant")]
fn a_resource_cannot_also_be_a_constant() {
    let mut w = world();
    w.insert_constant(Counter(1));
}

#[test]
#[should_panic(expected = "is already registered as a Constant, not a Resource")]
fn a_constant_cannot_also_be_a_resource() {
    let mut w = World::new();
    w.insert_constant(Gravity(1.0));
    w.insert_resource(Gravity(1.0));
}

#[test]
#[should_panic(expected = "[E-SCHEMA]")]
fn a_constant_cannot_take_a_resource_s_stable_name() {
    let mut w = world();
    w.insert_constant(FakeCounter(1));
}

#[test]
#[should_panic(expected = "is not registered as a Constant")]
fn reading_a_missing_constant_panics() {
    let w = world();
    let _ = w.constant::<Gravity>();
}
