//! The world's canonical form (hashed, snapshotted) and its dump for agents.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::ToString;

use eng_core::GenAllocator;
use eng_schema_core::{DecodeError, Reader, Schema, Visitor};

use crate::World;

impl World {
    /// Walks the canonical form: the entity allocator, then resources,
    /// components (dense order) and this tick's events, each section in
    /// stable-name order. The state hash is FNV-1a-64 over its encoding.
    pub fn visit_state(&self, v: &mut dyn Visitor) {
        v.begin_struct("eng.ecs.World");
        v.field("entities");
        self.ids.visit(v);
        v.field("resources");
        visit_named(v, &self.resources, |value, v| value.visit(v));
        v.field("components");
        visit_named(v, &self.components, |storage, v| storage.visit(v));
        v.field("events");
        visit_named(v, &self.events, |queue, v| queue.visit(v));
        v.end_struct();
    }

    /// Walks an entity-centric view for people and agents. Its JSON form has
    /// the pointers assertions use, e.g.
    /// `/entities/0/components/kit_fps.MoveState/pos/x`.
    pub fn visit_dump(&self, v: &mut dyn Visitor) {
        v.begin_struct("eng.ecs.WorldDump");
        v.field("resources");
        v.begin_struct("eng.ecs.Resources");
        for (name, value) in &self.resources {
            v.field(name);
            value.visit(v);
        }
        v.end_struct();
        v.field("entities");
        v.begin_seq(u32::try_from(self.entity_count()).expect("fewer than 2^32 entities"));
        for e in self.entities() {
            v.begin_struct("eng.ecs.EntityDump");
            v.field("index");
            v.u32(e.index);
            v.field("generation");
            v.u32(e.generation);
            v.field("components");
            v.begin_struct("eng.ecs.Components");
            for (name, storage) in &self.components {
                if storage.contains_entity(e) {
                    v.field(name);
                    storage.visit_entity(e, v);
                }
            }
            v.end_struct();
            v.end_struct();
        }
        v.end_seq();
        v.field("events");
        self.visit_events(v);
        v.end_struct();
    }

    /// Walks this tick's events as `{ <event name>: [events...] }`, in
    /// stable-name order.
    pub fn visit_events(&self, v: &mut dyn Visitor) {
        v.begin_struct("eng.ecs.Events");
        for (name, queue) in &self.events {
            v.field(name);
            queue.visit(v);
        }
        v.end_struct();
    }

    /// Replaces the world's contents with a canonical encoding produced by a
    /// world with the same registrations.
    ///
    /// All-or-nothing: every section decodes into new storage first, so on
    /// error the world is unchanged.
    pub fn decode_state(&mut self, bytes: &[u8]) -> Result<(), DecodeError> {
        assert!(
            !self.has_deferred(),
            "decode_state with deferred changes pending"
        );
        let mut r = Reader::new(bytes);
        let ids = GenAllocator::decode(&mut r)?;
        if !ids.is_consistent() {
            return Err(mismatch("a consistent entity allocator", "a corrupt one"));
        }
        let resources = decode_named(&mut r, &self.resources, |value, r| value.decode_fresh(r))?;
        let alive = |e: crate::Entity| ids.is_alive(e.into());
        let components = decode_named(&mut r, &self.components, |storage, r| {
            storage.decode_fresh(r, &alive)
        })?;
        let events = decode_named(&mut r, &self.events, |queue, r| queue.decode_fresh(r))?;
        r.finish()?;
        self.ids = ids;
        self.resources = resources;
        self.components = components;
        self.events = events;
        Ok(())
    }
}

fn visit_named<T: ?Sized>(
    v: &mut dyn Visitor,
    map: &BTreeMap<&'static str, Box<T>>,
    mut visit: impl FnMut(&T, &mut dyn Visitor),
) {
    v.begin_seq(u32::try_from(map.len()).expect("fewer than 2^32 types"));
    for (name, value) in map {
        v.begin_struct("eng.ecs.Named");
        v.field("name");
        v.str(name);
        v.field("value");
        visit(value, v);
        v.end_struct();
    }
    v.end_seq();
}

/// Decodes a named section into a new map with the same names, in order.
fn decode_named<T: ?Sized>(
    r: &mut Reader<'_>,
    map: &BTreeMap<&'static str, Box<T>>,
    mut decode: impl FnMut(&T, &mut Reader<'_>) -> Result<Box<T>, DecodeError>,
) -> Result<BTreeMap<&'static str, Box<T>>, DecodeError> {
    let len = r.read_len()? as usize;
    if len != map.len() {
        return Err(mismatch(
            &format!("{} registered types", map.len()),
            &format!("{len}"),
        ));
    }
    let mut out = BTreeMap::new();
    for (name, value) in map {
        let found = r.read_str()?;
        if found != *name {
            return Err(mismatch(name, &found));
        }
        out.insert(*name, decode(value, r)?);
    }
    Ok(out)
}

fn mismatch(expected: &str, found: &str) -> DecodeError {
    DecodeError::Mismatch {
        expected: expected.to_string(),
        found: found.to_string(),
    }
}
