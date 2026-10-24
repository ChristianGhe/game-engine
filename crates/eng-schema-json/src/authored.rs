use std::fmt;

use serde::de::DeserializeOwned;

use crate::JsonValue;

/// An authored file format: TOML with a `version` field.
///
/// Implementors derive `serde::Deserialize` with `#[serde(deny_unknown_fields)]`.
/// Keys starting with `x_` are extensions: removed before parsing, at any depth.
/// Migrations edit the file as a [`JsonValue`], so no TOML type leaks out.
pub trait Authored: DeserializeOwned {
    /// What the file is, for messages, e.g. `"input script"`.
    const KIND: &'static str;
    /// The current format version.
    const VERSION: u32;

    /// Rewrites a `from`-version document into version `from + 1`.
    fn migrate(from: u32, doc: &mut JsonValue) -> Result<(), String> {
        let _ = doc;
        Err(format!("no migration from version {from}"))
    }
}

/// A file that could not be loaded. Displays as `[E-LOAD] origin: message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub origin: String,
    pub message: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[E-LOAD] {}: {}", self.origin, self.message)
    }
}

impl std::error::Error for LoadError {}

/// Parses authored TOML strictly. `origin` names the source in errors.
pub fn load_toml<T: Authored>(text: &str, origin: &str) -> Result<T, LoadError> {
    let fail = |message: String| LoadError {
        origin: origin.to_string(),
        message,
    };
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| fail(e.to_string().trim_end().to_string()))?;
    let mut doc = from_toml(toml::Value::Table(table));
    let version = match doc.get("version") {
        Some(JsonValue::U64(v)) => {
            u32::try_from(*v).map_err(|_| fail(format!("version {v} is not a valid version")))?
        }
        Some(_) => return Err(fail("`version` must be a non-negative integer".into())),
        None => {
            return Err(fail(format!(
                "missing `version = {}` for this {}",
                T::VERSION,
                T::KIND
            )));
        }
    };
    if version > T::VERSION {
        return Err(fail(format!(
            "{} version {version} is newer than this build supports ({})",
            T::KIND,
            T::VERSION
        )));
    }
    for from in version..T::VERSION {
        T::migrate(from, &mut doc).map_err(|e| fail(format!("migrating: {e}")))?;
        doc.insert("version", JsonValue::U64(u64::from(from + 1)));
    }
    strip_extensions(&mut doc);
    to_toml(doc)
        .map_err(fail)?
        .try_into()
        .map_err(|e: toml::de::Error| fail(e.to_string().trim_end().to_string()))
}

/// Removes `x_` keys from every table, at any depth.
fn strip_extensions(doc: &mut JsonValue) {
    match doc {
        JsonValue::Object(pairs) => {
            pairs.retain(|(key, _)| !key.starts_with("x_"));
            pairs.iter_mut().for_each(|(_, v)| strip_extensions(v));
        }
        JsonValue::Array(items) => items.iter_mut().for_each(strip_extensions),
        _ => {}
    }
}

fn from_toml(value: toml::Value) -> JsonValue {
    match value {
        toml::Value::String(s) => JsonValue::Str(s),
        toml::Value::Integer(i) => u64::try_from(i).map_or(JsonValue::I64(i), JsonValue::U64),
        toml::Value::Float(f) => JsonValue::F64(f),
        toml::Value::Boolean(b) => JsonValue::Bool(b),
        // Authored formats do not use dates; keep the text if one appears.
        toml::Value::Datetime(d) => JsonValue::Str(d.to_string()),
        toml::Value::Array(items) => JsonValue::Array(items.into_iter().map(from_toml).collect()),
        toml::Value::Table(t) => {
            JsonValue::Object(t.into_iter().map(|(k, v)| (k, from_toml(v))).collect())
        }
    }
}

fn to_toml(value: JsonValue) -> Result<toml::Value, String> {
    Ok(match value {
        JsonValue::Null => return Err("null has no TOML form".into()),
        JsonValue::Bool(b) => toml::Value::Boolean(b),
        JsonValue::U64(u) => toml::Value::Integer(
            i64::try_from(u).map_err(|_| format!("{u} is too large for TOML"))?,
        ),
        JsonValue::I64(i) => toml::Value::Integer(i),
        JsonValue::F32(f) => toml::Value::Float(f64::from(f)),
        JsonValue::F64(f) => toml::Value::Float(f),
        JsonValue::Str(s) => toml::Value::String(s),
        JsonValue::Array(items) => {
            toml::Value::Array(items.into_iter().map(to_toml).collect::<Result<_, _>>()?)
        }
        JsonValue::Object(pairs) => toml::Value::Table(
            pairs
                .into_iter()
                .map(|(k, v)| Ok((k, to_toml(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}
