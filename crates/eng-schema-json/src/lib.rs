//! Machine-readable output and authored input (spec D12).
//!
//! - [`JsonValue`]: JSON built from any schema walk, keys in declaration
//!   order, floats in shortest round-trip form, NaN and infinities as `null`.
//! - [`load_toml`]: strict loading of authored TOML: a `version` field with
//!   a migration path, unknown keys rejected except under the `x_` prefix.
#![forbid(unsafe_code)]

mod authored;
mod json;

pub use authored::{Authored, LoadError, load_toml};
pub use json::JsonValue;
