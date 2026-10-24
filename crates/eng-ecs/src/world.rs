use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::any::{TypeId, type_name};

use eng_core::GenAllocator;
use eng_schema_core::{Schema, TypeRegistry};

use crate::deferred::Command;
use crate::storage::{ErasedStorage, ErasedValue, EventQueue, ResourceSlot, Storage};
use crate::{Deferred, Entity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Component,
    Resource,
    Event,
}

/// Entities, components, resources and event queues.
///
/// Registration mistakes (an unregistered type, a type registered as two
/// kinds, two types sharing a stable name) are programming errors and panic.
pub struct World {
    pub(crate) ids: GenAllocator,
    pub(crate) components: BTreeMap<&'static str, Box<dyn ErasedStorage>>,
    pub(crate) resources: BTreeMap<&'static str, Box<dyn ErasedValue>>,
    pub(crate) events: BTreeMap<&'static str, Box<dyn ErasedValue>>,
    kinds: BTreeMap<TypeId, (Kind, &'static str)>,
    registry: TypeRegistry,
    pub(crate) deferred: Vec<Command>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            ids: GenAllocator::new(),
            components: BTreeMap::new(),
            resources: BTreeMap::new(),
            events: BTreeMap::new(),
            kinds: BTreeMap::new(),
            registry: TypeRegistry::new(),
            deferred: Vec::new(),
        }
    }

    fn register<T: Schema + 'static>(&mut self, kind: Kind) -> bool {
        match self.kinds.get(&TypeId::of::<T>()) {
            Some((existing, _)) if *existing == kind => return false,
            Some((existing, _)) => panic!(
                "[E-SCHEMA] {} is already registered as a {existing:?}, not a {kind:?}",
                type_name::<T>()
            ),
            None => {}
        }
        if let Err(err) = self.registry.register::<T>() {
            panic!("[E-SCHEMA] registering {}: {err}", type_name::<T>());
        }
        let taken = self.components.contains_key(T::NAME)
            || self.resources.contains_key(T::NAME)
            || self.events.contains_key(T::NAME);
        assert!(
            !taken,
            "[E-SCHEMA] stable name {:?} is already used by another type",
            T::NAME
        );
        self.kinds.insert(TypeId::of::<T>(), (kind, T::NAME));
        true
    }

    fn name_of<T: 'static>(&self, kind: Kind) -> &'static str {
        match self.kinds.get(&TypeId::of::<T>()) {
            Some((k, name)) if *k == kind => name,
            _ => panic!(
                "[E-SCHEMA] {} is not registered as a {kind:?}",
                type_name::<T>()
            ),
        }
    }

    pub fn register_component<T: Schema + 'static>(&mut self) {
        if self.register::<T>(Kind::Component) {
            self.components
                .insert(T::NAME, Box::new(Storage::<T>::default()));
        }
    }

    /// Registers the resource on first use; later calls replace its value.
    pub fn insert_resource<T: Schema + 'static>(&mut self, value: T) {
        if self.register::<T>(Kind::Resource) {
            self.resources
                .insert(T::NAME, Box::new(ResourceSlot(value)));
        } else {
            *self.resource_mut::<T>() = value;
        }
    }

    pub fn register_event<T: Schema + 'static>(&mut self) {
        if self.register::<T>(Kind::Event) {
            self.events
                .insert(T::NAME, Box::new(EventQueue::<T>(Vec::new())));
        }
    }

    pub fn registry(&self) -> &TypeRegistry {
        &self.registry
    }

    /// Registered component names, in name order.
    pub fn component_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.components.keys().copied()
    }

    pub fn resource_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.resources.keys().copied()
    }

    pub fn event_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.events.keys().copied()
    }

    // --- Entities -------------------------------------------------------

    pub fn spawn(&mut self) -> Entity {
        self.ids.alloc().into()
    }

    /// Removes the entity and all its components. Returns false if it was
    /// not alive.
    pub fn despawn(&mut self, e: Entity) -> bool {
        if !self.ids.free(e.into()) {
            return false;
        }
        for storage in self.components.values_mut() {
            storage.remove_entity(e);
        }
        true
    }

    pub fn is_alive(&self, e: Entity) -> bool {
        self.ids.is_alive(e.into())
    }

    /// Live entities in index order.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.ids.iter_alive().map(Entity::from)
    }

    pub fn entity_count(&self) -> usize {
        self.ids.alive_count()
    }

    // --- Components -----------------------------------------------------

    pub fn storage<T: Schema + 'static>(&self) -> &Storage<T> {
        let name = self.name_of::<T>(Kind::Component);
        self.components[name]
            .as_any()
            .downcast_ref()
            .expect("storage type matches its registration")
    }

    pub fn storage_mut<T: Schema + 'static>(&mut self) -> &mut Storage<T> {
        let name = self.name_of::<T>(Kind::Component);
        self.components
            .get_mut(name)
            .expect("registered")
            .as_any_mut()
            .downcast_mut()
            .expect("storage type matches its registration")
    }

    /// Two different component storages at once, for joins.
    ///
    /// # Panics
    /// If `A` and `B` are the same type.
    pub fn storage_pair_mut<A: Schema + 'static, B: Schema + 'static>(
        &mut self,
    ) -> (&mut Storage<A>, &mut Storage<B>) {
        let (name_a, name_b) = (
            self.name_of::<A>(Kind::Component),
            self.name_of::<B>(Kind::Component),
        );
        assert_ne!(name_a, name_b, "storage_pair_mut needs two different types");
        let (mut a, mut b) = (None, None);
        for (name, storage) in self.components.iter_mut() {
            if *name == name_a {
                a = storage.as_any_mut().downcast_mut::<Storage<A>>();
            } else if *name == name_b {
                b = storage.as_any_mut().downcast_mut::<Storage<B>>();
            }
        }
        (a.expect("registered"), b.expect("registered"))
    }

    /// Adds or replaces `e`'s component, returning the old value.
    ///
    /// # Panics
    /// If `e` is not alive.
    pub fn insert<T: Schema + 'static>(&mut self, e: Entity, value: T) -> Option<T> {
        assert!(self.is_alive(e), "insert on dead entity {e}");
        self.storage_mut::<T>().insert(e, value)
    }

    pub fn remove<T: Schema + 'static>(&mut self, e: Entity) -> Option<T> {
        self.storage_mut::<T>().remove(e)
    }

    pub fn get<T: Schema + 'static>(&self, e: Entity) -> Option<&T> {
        self.storage::<T>().get(e)
    }

    pub fn get_mut<T: Schema + 'static>(&mut self, e: Entity) -> Option<&mut T> {
        self.storage_mut::<T>().get_mut(e)
    }

    // --- Resources and events -------------------------------------------

    pub fn resource<T: Schema + 'static>(&self) -> &T {
        let name = self.name_of::<T>(Kind::Resource);
        let slot: &ResourceSlot<T> = self.resources[name]
            .as_any()
            .downcast_ref()
            .expect("resource type matches its registration");
        &slot.0
    }

    pub fn resource_mut<T: Schema + 'static>(&mut self) -> &mut T {
        let name = self.name_of::<T>(Kind::Resource);
        let slot: &mut ResourceSlot<T> = self
            .resources
            .get_mut(name)
            .expect("registered")
            .as_any_mut()
            .downcast_mut()
            .expect("resource type matches its registration");
        &mut slot.0
    }

    pub fn send_event<T: Schema + 'static>(&mut self, event: T) {
        self.event_queue_mut::<T>().0.push(event);
    }

    /// This tick's events of type `T`, in send order.
    pub fn events<T: Schema + 'static>(&self) -> &[T] {
        let name = self.name_of::<T>(Kind::Event);
        let queue: &EventQueue<T> = self.events[name]
            .as_any()
            .downcast_ref()
            .expect("event type matches its registration");
        &queue.0
    }

    fn event_queue_mut<T: Schema + 'static>(&mut self) -> &mut EventQueue<T> {
        let name = self.name_of::<T>(Kind::Event);
        self.events
            .get_mut(name)
            .expect("registered")
            .as_any_mut()
            .downcast_mut()
            .expect("event type matches its registration")
    }

    /// Empties every event queue; the tick driver calls this before each tick.
    pub fn clear_events(&mut self) {
        for queue in self.events.values_mut() {
            queue.clear_events();
        }
    }

    /// Events sent this tick, over all queues.
    pub fn event_count(&self) -> usize {
        self.events.values().map(|queue| queue.event_len()).sum()
    }

    // --- Deferred structural changes ------------------------------------

    /// Structural changes that become visible at the next sync point.
    pub fn defer(&mut self) -> Deferred<'_> {
        Deferred::new(self)
    }

    /// Applies deferred changes in the order they were made. `eng-sim` calls
    /// this at the end of every stage.
    pub fn apply_deferred(&mut self) {
        let commands = core::mem::take(&mut self.deferred);
        for command in commands {
            match command {
                Command::Despawn(e) => {
                    self.despawn(e);
                }
                Command::Run(f) => f(self),
            }
        }
    }

    pub fn has_deferred(&self) -> bool {
        !self.deferred.is_empty()
    }
}
