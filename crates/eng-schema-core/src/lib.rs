//! Stable type names and versions, the canonical binary format, and the
//! schema walker every other tool builds on.
//!
//! - [`Schema`] is implemented by every component, resource, event and
//!   recording type, usually through `#[derive(Schema)]`.
//! - [`Visitor`] walks a value in declaration order. The canonical encoder,
//!   the state hasher, the JSON dumper and the NaN check are all visitors, so
//!   they cannot disagree about what a value contains.
//! - The canonical encoding is little-endian and fixed-width: structs are
//!   their fields in order, sequences and strings carry a `u32` length,
//!   options a `u8` tag, enums a `u32` variant index.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
// Lets the derive's `::eng_schema_core` paths resolve inside this crate.
extern crate self as eng_schema_core;

pub mod container;
mod decode;
mod desc;
mod encode;
mod finite;
mod impls;
mod registry;
mod visit;

pub use decode::{DecodeError, MAX_SEQ_LEN, Reader, from_bytes};
pub use desc::TypeDesc;
pub use encode::{Encoder, Sink, encode, to_bytes};
pub use eng_derive::Schema;
pub use finite::{NonFinite, find_non_finite};
pub use registry::{RegistryError, TypeRegistry};
pub use visit::Visitor;

/// A plain-data type with a stable name, a schema version and a canonical
/// encoding.
pub trait Schema: Sized {
    /// Stable dotted name such as `"eng.core.PlayerCommand"`. Never derived
    /// from Rust paths, so refactors don't change it.
    const NAME: &'static str;
    /// Bumped whenever the encoding of this type changes.
    const VERSION: u32;

    /// Describes the type's shape; used by the registry and the simulation
    /// protocol fingerprint.
    fn describe() -> TypeDesc;

    /// Walks the value in declaration order.
    fn visit(&self, v: &mut dyn Visitor);

    /// Reads a value written by the canonical encoder.
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError>;
}

#[doc(hidden)]
pub mod __private {
    pub use alloc::vec;
}
