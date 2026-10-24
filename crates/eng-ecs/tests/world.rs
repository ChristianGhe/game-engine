use eng_ecs::{Entity, Schema, World};
use eng_schema_core::{Encoder, find_non_finite};
use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Pos", version = 1)]
struct Pos {
    x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Tag", version = 1)]
struct Tag(u8);

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Counter", version = 1)]
struct Counter(u64);

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Ping", version = 1)]
struct Ping(u32);

fn world() -> World {
    let mut w = World::new();
    w.register_component::<Pos>();
    w.register_component::<Tag>();
    w.insert_resource(Counter(0));
    w.register_event::<Ping>();
    w
}

fn state_bytes(w: &World) -> Vec<u8> {
    let mut out = Vec::new();
    w.visit_state(&mut Encoder::new(&mut out));
    out
}

#[test]
fn entity_lifecycle_and_stale_ids() {
    let mut w = world();
    let a = w.spawn();
    let b = w.spawn();
    w.insert(a, Pos { x: 1.0 });
    w.insert(b, Pos { x: 2.0 });
    w.insert(b, Tag(7));
    assert_eq!(w.get::<Pos>(b), Some(&Pos { x: 2.0 }));
    assert!(w.despawn(a));
    assert!(!w.despawn(a));
    assert_eq!(w.get::<Pos>(a), None);
    let c = w.spawn();
    assert_eq!(c.index, a.index, "freed slot is reused");
    assert_ne!(c, a, "with a new generation");
    assert_eq!(w.get::<Pos>(c), None);
    assert_eq!(w.entity_count(), 2);
}

#[test]
fn dense_order_follows_swap_remove() {
    let mut w = world();
    let es: Vec<Entity> = (0..4).map(|_| w.spawn()).collect();
    for (i, e) in es.iter().enumerate() {
        w.insert(*e, Tag(i as u8));
    }
    w.remove::<Tag>(es[0]);
    let order: Vec<u8> = w.storage::<Tag>().iter().map(|(_, t)| t.0).collect();
    assert_eq!(order, [3, 1, 2]);
}

#[test]
fn storage_pair_join() {
    let mut w = world();
    for i in 0..3 {
        let e = w.spawn();
        w.insert(e, Pos { x: i as f32 });
        if i != 1 {
            w.insert(e, Tag(10));
        }
    }
    let (pos, tags) = w.storage_pair_mut::<Pos, Tag>();
    for (e, p) in pos.iter_mut() {
        if let Some(t) = tags.get_mut(e) {
            p.x += f32::from(t.0);
            t.0 += 1;
        }
    }
    let xs: Vec<f32> = w.storage::<Pos>().iter().map(|(_, p)| p.x).collect();
    assert_eq!(xs, [10.0, 1.0, 12.0]);
}

#[test]
fn resources_and_events() {
    let mut w = world();
    w.resource_mut::<Counter>().0 += 5;
    assert_eq!(w.resource::<Counter>().0, 5);
    w.send_event(Ping(1));
    w.send_event(Ping(2));
    assert_eq!(w.events::<Ping>(), [Ping(1), Ping(2)]);
    assert_eq!(w.event_count(), 2);
    w.clear_events();
    assert!(w.events::<Ping>().is_empty());
}

#[test]
fn deferred_changes_apply_at_sync_point_in_order() {
    let mut w = world();
    let a = w.spawn();
    let b = w.defer().spawn();
    w.defer().insert(b, Tag(1)).insert(a, Tag(2)).despawn(a);
    assert_eq!(w.get::<Tag>(b), None, "not visible before the sync point");
    w.apply_deferred();
    assert_eq!(w.get::<Tag>(b), Some(&Tag(1)));
    assert!(!w.is_alive(a));
    assert!(!w.has_deferred());
}

#[test]
#[should_panic(expected = "not registered")]
fn unregistered_component_panics() {
    #[derive(Schema)]
    #[schema(name = "test.Unknown", version = 1)]
    struct Unknown;
    let w = world();
    let _ = w.storage::<Unknown>();
}

#[test]
fn decode_rejects_other_registrations() {
    let mut w = world();
    let e = w.spawn();
    w.insert(e, Pos { x: 3.0 });
    let bytes = state_bytes(&w);
    let mut other = World::new();
    other.register_component::<Pos>();
    other.insert_resource(Counter(0));
    other.register_event::<Ping>();
    assert!(other.decode_state(&bytes).is_err());
    let mut same = world();
    same.decode_state(&bytes).unwrap();
    assert_eq!(state_bytes(&same), bytes);
}

#[test]
fn dump_paths_point_at_entities() {
    let mut w = world();
    let e = w.spawn();
    w.insert(e, Pos { x: f32::NAN });
    let found = find_non_finite(&|v| w.visit_dump(v)).unwrap();
    assert_eq!(found.path, "/entities/0/components/test.Pos/x");
}

#[derive(Debug, Clone)]
enum Op {
    Spawn,
    Despawn(usize),
    InsertPos(usize, f32),
    InsertTag(usize, u8),
    RemovePos(usize),
    Bump,
    Ping(u32),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::Spawn),
        (0usize..8).prop_map(Op::Despawn),
        (0usize..8, -1e3f32..1e3).prop_map(|(i, x)| Op::InsertPos(i, x)),
        (0usize..8, any::<u8>()).prop_map(|(i, t)| Op::InsertTag(i, t)),
        (0usize..8).prop_map(Op::RemovePos),
        Just(Op::Bump),
        any::<u32>().prop_map(Op::Ping),
    ]
}

fn apply(w: &mut World, ops: &[Op]) {
    for op in ops {
        let live: Vec<Entity> = w.entities().collect();
        let pick = |i: usize| live.get(i % live.len().max(1)).copied();
        match *op {
            Op::Spawn => {
                w.spawn();
            }
            Op::Despawn(i) => {
                if let Some(e) = pick(i) {
                    w.despawn(e);
                }
            }
            Op::InsertPos(i, x) => {
                if let Some(e) = pick(i) {
                    w.insert(e, Pos { x });
                }
            }
            Op::InsertTag(i, t) => {
                if let Some(e) = pick(i) {
                    w.insert(e, Tag(t));
                }
            }
            Op::RemovePos(i) => {
                if let Some(e) = pick(i) {
                    w.remove::<Pos>(e);
                }
            }
            Op::Bump => w.resource_mut::<Counter>().0 += 1,
            Op::Ping(p) => w.send_event(Ping(p)),
        }
    }
}

proptest! {
    /// A restored world behaves exactly like the original from then on.
    #[test]
    fn snapshot_restore_continues_identically(
        before in prop::collection::vec(op(), 0..40),
        after in prop::collection::vec(op(), 0..40),
    ) {
        let mut original = world();
        apply(&mut original, &before);
        let mut restored = world();
        restored.decode_state(&state_bytes(&original)).unwrap();
        prop_assert_eq!(state_bytes(&restored), state_bytes(&original));
        apply(&mut original, &after);
        apply(&mut restored, &after);
        prop_assert_eq!(state_bytes(&restored), state_bytes(&original));
    }

    #[test]
    fn decode_state_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        let mut w = world();
        let _ = w.decode_state(&bytes);
    }
}

#[test]
fn failed_decode_leaves_the_world_unchanged() {
    let mut w = world();
    let e = w.spawn();
    w.insert(e, Pos { x: 1.0 });
    let good = state_bytes(&w);
    let mut other = world();
    for _ in 0..3 {
        let x = other.spawn();
        other.insert(x, Tag(9));
    }
    let before = state_bytes(&other);
    for cut in 1..good.len() {
        assert!(other.decode_state(&good[..cut]).is_err());
        assert_eq!(state_bytes(&other), before, "truncated at {cut}");
    }
}

#[test]
fn entries_for_dead_entities_are_refused_before_allocating() {
    let mut w = world();
    let e = w.spawn();
    w.insert(e, Tag(1));
    let mut bytes = state_bytes(&w);
    // Point the Tag entry at entity index 2^27: it must be refused, not stored.
    let tag_entry = bytes
        .windows(9)
        .rposition(|win| win == [0, 0, 0, 0, 0, 0, 0, 0, 1])
        .expect("entry for entity 0v0 with value 1");
    bytes[tag_entry..tag_entry + 4].copy_from_slice(&(1u32 << 27).to_le_bytes());
    let mut fresh = world();
    let err = fresh.decode_state(&bytes).unwrap_err().to_string();
    assert!(err.contains("dead entity 134217728v0"), "{err}");
}
