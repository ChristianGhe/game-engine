//! Machine-readable output and authored input (spec D12).
//!
//! - [`JsonValue`]: JSON built from any schema walk, keys in declaration
//!   order, floats in shortest round-trip form, NaN and infinities as `null`.
//! - [`load_toml`]: strict loading of authored TOML: a `version` field with
//!   a migration path, unknown keys rejected except under the `x_` prefix.
//! - [`TomlSpans`]: where a key or array element is in authored TOML, so
//!   a format's own checks can name the line.
#![forbid(unsafe_code)]

mod authored;
mod json;
mod spans;

pub use authored::{Authored, LoadError, load_toml};
pub use json::JsonValue;
pub use spans::{PathStep, TomlSpans};
