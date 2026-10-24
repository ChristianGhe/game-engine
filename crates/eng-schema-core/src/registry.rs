use alloc::collections::BTreeMap;
use core::fmt;

use crate::{Schema, Sink, TypeDesc};

/// Stable names mapped to their descriptions, kept in name order.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    types: BTreeMap<&'static str, TypeDesc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Two different types claim the same stable name.
    Conflict { name: &'static str },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { name } => {
                write!(f, "two different types use the stable name {name:?}")
            }
        }
    }
}

impl core::error::Error for RegistryError {}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `T`. Registering the same type twice is allowed.
    pub fn register<T: Schema>(&mut self) -> Result<(), RegistryError> {
        let desc = T::describe();
        match self.types.get(T::NAME) {
            Some(existing) if *existing != desc => Err(RegistryError::Conflict { name: T::NAME }),
            Some(_) => Ok(()),
            None => {
                self.types.insert(T::NAME, desc);
                Ok(())
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<&TypeDesc> {
        self.types.get(name)
    }

    /// Registered types in stable-name order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &TypeDesc)> {
        self.types.iter().map(|(name, desc)| (*name, desc))
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Writes every description in name order, for fingerprinting.
    pub fn encode(&self, sink: &mut dyn Sink) {
        for desc in self.types.values() {
            desc.encode(sink);
        }
    }
}
