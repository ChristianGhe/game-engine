use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;

use eng_schema_core::{DecodeError, Reader, Schema, Visitor};

use crate::Entity;

const NONE: u32 = u32::MAX;

/// Dense storage for one component type (a sparse set).
///
/// Iteration follows dense order: insertion order, with `swap_remove` on
/// removal. That order is part of the simulation state and is encoded.
#[derive(Debug, Clone)]
pub struct Storage<T> {
    sparse: Vec<u32>,
    entities: Vec<Entity>,
    values: Vec<T>,
}

impl<T> Default for Storage<T> {
    fn default() -> Self {
        Self {
            sparse: Vec::new(),
            entities: Vec::new(),
            values: Vec::new(),
        }
    }
}

impl<T> Storage<T> {
    fn position(&self, e: Entity) -> Option<usize> {
        let pos = *self.sparse.get(e.index as usize)?;
        (pos != NONE && self.entities[pos as usize] == e).then_some(pos as usize)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn contains(&self, e: Entity) -> bool {
        self.position(e).is_some()
    }

    pub fn get(&self, e: Entity) -> Option<&T> {
        self.position(e).map(|pos| &self.values[pos])
    }

    pub fn get_mut(&mut self, e: Entity) -> Option<&mut T> {
        self.position(e).map(|pos| &mut self.values[pos])
    }

    /// Entries in dense order.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.entities.iter().copied().zip(self.values.iter())
    }

    /// Entries in dense order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        self.entities.iter().copied().zip(self.values.iter_mut())
    }

    pub(crate) fn insert(&mut self, e: Entity, value: T) -> Option<T> {
        if let Some(pos) = self.position(e) {
            return Some(core::mem::replace(&mut self.values[pos], value));
        }
        let index = e.index as usize;
        if self.sparse.len() <= index {
            self.sparse.resize(index + 1, NONE);
        }
        self.sparse[index] = u32::try_from(self.values.len()).expect("fewer than 2^32 components");
        self.entities.push(e);
        self.values.push(value);
        None
    }

    pub(crate) fn remove(&mut self, e: Entity) -> Option<T> {
        let pos = self.position(e)?;
        self.sparse[e.index as usize] = NONE;
        self.entities.swap_remove(pos);
        let value = self.values.swap_remove(pos);
        if let Some(moved) = self.entities.get(pos) {
            self.sparse[moved.index as usize] = pos as u32;
        }
        Some(value)
    }
}

/// Type-erased access to a component storage, keyed by stable name.
pub(crate) trait ErasedStorage: Any {
    /// Canonical form: a sequence of `{entity, value}` in dense order.
    fn visit(&self, v: &mut dyn Visitor);
    /// Visits `e`'s value if present.
    fn visit_entity(&self, e: Entity, v: &mut dyn Visitor) -> bool;
    fn contains_entity(&self, e: Entity) -> bool;
    fn remove_entity(&mut self, e: Entity);
    /// A new storage of the same type, decoded from the canonical form.
    /// Entries for entities that `alive` rejects are refused before any
    /// allocation, so untrusted bytes cannot force a huge sparse array.
    fn decode_fresh(
        &self,
        r: &mut Reader<'_>,
        alive: &dyn Fn(Entity) -> bool,
    ) -> Result<Box<dyn ErasedStorage>, DecodeError>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Schema + 'static> ErasedStorage for Storage<T> {
    fn visit(&self, v: &mut dyn Visitor) {
        v.begin_seq(u32::try_from(self.len()).expect("fewer than 2^32 components"));
        for (e, value) in self.iter() {
            v.begin_struct("eng.ecs.ComponentEntry");
            v.field("entity");
            e.visit(v);
            v.field("value");
            value.visit(v);
            v.end_struct();
        }
        v.end_seq();
    }

    fn visit_entity(&self, e: Entity, v: &mut dyn Visitor) -> bool {
        match self.get(e) {
            Some(value) => {
                value.visit(v);
                true
            }
            None => false,
        }
    }

    fn contains_entity(&self, e: Entity) -> bool {
        self.contains(e)
    }

    fn remove_entity(&mut self, e: Entity) {
        self.remove(e);
    }

    fn decode_fresh(
        &self,
        r: &mut Reader<'_>,
        alive: &dyn Fn(Entity) -> bool,
    ) -> Result<Box<dyn ErasedStorage>, DecodeError> {
        let mut out = Storage::<T>::default();
        let len = r.read_len()?;
        for _ in 0..len {
            let e = Entity::decode(r)?;
            if !alive(e) {
                return Err(DecodeError::Mismatch {
                    expected: alloc::format!("{} entries for live entities", T::NAME),
                    found: alloc::format!("an entry for dead entity {e}"),
                });
            }
            let value = T::decode(r)?;
            if out.insert(e, value).is_some() {
                return Err(DecodeError::Mismatch {
                    expected: alloc::format!("one {} per entity", T::NAME),
                    found: alloc::format!("a duplicate for {e}"),
                });
            }
        }
        Ok(Box::new(out))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A registered resource value.
pub(crate) struct ResourceSlot<T>(pub(crate) T);

/// A registered event channel, cleared at the start of every tick.
pub(crate) struct EventQueue<T>(pub(crate) Vec<T>);

/// Type-erased access to resources and event queues.
pub(crate) trait ErasedValue: Any {
    fn visit(&self, v: &mut dyn Visitor);
    /// A new value of the same type, decoded from the canonical form.
    fn decode_fresh(&self, r: &mut Reader<'_>) -> Result<Box<dyn ErasedValue>, DecodeError>;
    fn clear_events(&mut self) {}
    fn event_len(&self) -> usize {
        0
    }
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Schema + 'static> ErasedValue for ResourceSlot<T> {
    fn visit(&self, v: &mut dyn Visitor) {
        self.0.visit(v);
    }
    fn decode_fresh(&self, r: &mut Reader<'_>) -> Result<Box<dyn ErasedValue>, DecodeError> {
        Ok(Box::new(ResourceSlot(T::decode(r)?)))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl<T: Schema + 'static> ErasedValue for EventQueue<T> {
    fn visit(&self, v: &mut dyn Visitor) {
        self.0.visit(v);
    }
    fn decode_fresh(&self, r: &mut Reader<'_>) -> Result<Box<dyn ErasedValue>, DecodeError> {
        Ok(Box::new(EventQueue(Vec::<T>::decode(r)?)))
    }
    fn clear_events(&mut self) {
        self.0.clear();
    }
    fn event_len(&self) -> usize {
        self.0.len()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
