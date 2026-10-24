use alloc::boxed::Box;

use eng_schema_core::Schema;

use crate::{Entity, World};

pub(crate) enum Command {
    Despawn(Entity),
    Run(Box<dyn FnOnce(&mut World)>),
}

/// Records structural changes that apply, in order, at the next sync point.
///
/// Ids are reserved at once, so a system can refer to an entity it spawned
/// in the same stage; its components appear at the sync point.
pub struct Deferred<'w> {
    world: &'w mut World,
}

impl<'w> Deferred<'w> {
    pub(crate) fn new(world: &'w mut World) -> Self {
        Self { world }
    }

    /// Reserves a new entity now; it has no components until the sync point.
    pub fn spawn(&mut self) -> Entity {
        self.world.spawn()
    }

    /// Adds or replaces a component at the sync point, if `e` is still alive.
    pub fn insert<T: Schema + 'static>(&mut self, e: Entity, value: T) -> &mut Self {
        self.world.deferred.push(Command::Run(Box::new(move |w| {
            if w.is_alive(e) {
                w.insert(e, value);
            }
        })));
        self
    }

    pub fn remove<T: Schema + 'static>(&mut self, e: Entity) -> &mut Self {
        self.world.deferred.push(Command::Run(Box::new(move |w| {
            w.remove::<T>(e);
        })));
        self
    }

    pub fn despawn(&mut self, e: Entity) -> &mut Self {
        self.world.deferred.push(Command::Despawn(e));
        self
    }
}
