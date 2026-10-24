use std::fmt::{self, Write};

use eng_schema_core::{Schema, Visitor};

/// A JSON value. Objects keep insertion order, which for schema walks is
/// declaration order, so output is stable.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    U64(u64),
    I64(i64),
    F32(f32),
    F64(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// Builds JSON from a schema walk.
    pub fn from_walk(walk: &dyn Fn(&mut dyn Visitor)) -> Self {
        let mut builder = Builder::default();
        walk(&mut builder);
        builder.root.unwrap_or(JsonValue::Null)
    }

    pub fn of<T: Schema>(value: &T) -> Self {
        Self::from_walk(&|v| value.visit(v))
    }

    pub fn object<K: Into<String>>(pairs: impl IntoIterator<Item = (K, JsonValue)>) -> Self {
        Self::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    pub fn str(s: impl Into<String>) -> Self {
        Self::Str(s.into())
    }

    /// A `u64` as a `0x`-prefixed 16-digit hex string, for hashes.
    pub fn hex(x: u64) -> Self {
        Self::Str(format!("{x:#018x}"))
    }

    /// Looks up an RFC 6901 JSON pointer such as `/entities/0/components`.
    pub fn pointer(&self, pointer: &str) -> Option<&JsonValue> {
        if pointer.is_empty() {
            return Some(self);
        }
        let rest = pointer.strip_prefix('/')?;
        rest.split('/').try_fold(self, |node, raw| {
            let token = raw.replace("~1", "/").replace("~0", "~");
            match node {
                JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| *k == token).map(|(_, v)| v),
                JsonValue::Array(items) => token.parse::<usize>().ok().and_then(|i| items.get(i)),
                _ => None,
            }
        })
    }

    /// The value under `key`, if this is an object that has it.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Removes and returns `key` from an object.
    pub fn remove(&mut self, key: &str) -> Option<JsonValue> {
        match self {
            JsonValue::Object(pairs) => {
                let at = pairs.iter().position(|(k, _)| k == key)?;
                Some(pairs.remove(at).1)
            }
            _ => None,
        }
    }

    /// Sets `key` in an object, replacing any value in place or appending.
    /// Does nothing on other values.
    pub fn insert(&mut self, key: &str, value: JsonValue) {
        if let JsonValue::Object(pairs) = self {
            match pairs.iter_mut().find(|(k, _)| k == key) {
                Some((_, slot)) => *slot = value,
                None => pairs.push((key.to_string(), value)),
            }
        }
    }

    /// The value as a number, if it is one.
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            JsonValue::U64(x) => Some(x as f64),
            JsonValue::I64(x) => Some(x as f64),
            JsonValue::F32(x) => Some(f64::from(x)),
            JsonValue::F64(x) => Some(x),
            _ => None,
        }
    }

    /// Indented with two spaces, one key per line.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, Some(0));
        out
    }

    /// One line, no spaces: for JSON lines.
    pub fn to_compact(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, None);
        out
    }

    fn write(&self, out: &mut String, indent: Option<usize>) {
        let newline = |out: &mut String, level: usize| {
            out.push('\n');
            out.extend(std::iter::repeat_n("  ", level));
        };
        match self {
            JsonValue::Null => out.push_str("null"),
            JsonValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            JsonValue::U64(x) => write!(out, "{x}").expect("write to String"),
            JsonValue::I64(x) => write!(out, "{x}").expect("write to String"),
            // Debug gives the shortest round-trip form, with exponents when
            // needed; both are valid JSON numbers.
            JsonValue::F32(x) if x.is_finite() => write!(out, "{x:?}").expect("write to String"),
            JsonValue::F64(x) if x.is_finite() => write!(out, "{x:?}").expect("write to String"),
            JsonValue::F32(_) | JsonValue::F64(_) => out.push_str("null"),
            JsonValue::Str(s) => write_str(out, s),
            JsonValue::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    if let Some(level) = indent {
                        newline(out, level + 1);
                    }
                    item.write(out, indent.map(|l| l + 1));
                }
                if let (Some(level), false) = (indent, items.is_empty()) {
                    newline(out, level);
                }
                out.push(']');
            }
            JsonValue::Object(pairs) => {
                out.push('{');
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    if let Some(level) = indent {
                        newline(out, level + 1);
                    }
                    write_str(out, key);
                    out.push(':');
                    if indent.is_some() {
                        out.push(' ');
                    }
                    value.write(out, indent.map(|l| l + 1));
                }
                if let (Some(level), false) = (indent, pairs.is_empty()) {
                    newline(out, level);
                }
                out.push('}');
            }
        }
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_compact())
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                write!(out, "\\u{:04x}", u32::from(c)).expect("write to String");
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

enum Frame {
    Object(Vec<(String, JsonValue)>, Option<&'static str>),
    Array(Vec<JsonValue>),
}

#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    root: Option<JsonValue>,
}

impl Builder {
    fn done(&mut self, value: JsonValue) {
        match self.stack.last_mut() {
            Some(Frame::Object(pairs, key)) => {
                let key = key.take().unwrap_or("?");
                pairs.push((key.to_string(), value));
            }
            Some(Frame::Array(items)) => items.push(value),
            None => self.root = Some(value),
        }
    }
}

impl Visitor for Builder {
    fn bool(&mut self, v: bool) {
        self.done(JsonValue::Bool(v));
    }
    fn u8(&mut self, v: u8) {
        self.done(JsonValue::U64(v.into()));
    }
    fn u16(&mut self, v: u16) {
        self.done(JsonValue::U64(v.into()));
    }
    fn u32(&mut self, v: u32) {
        self.done(JsonValue::U64(v.into()));
    }
    fn u64(&mut self, v: u64) {
        self.done(JsonValue::U64(v));
    }
    fn i8(&mut self, v: i8) {
        self.done(JsonValue::I64(v.into()));
    }
    fn i16(&mut self, v: i16) {
        self.done(JsonValue::I64(v.into()));
    }
    fn i32(&mut self, v: i32) {
        self.done(JsonValue::I64(v.into()));
    }
    fn i64(&mut self, v: i64) {
        self.done(JsonValue::I64(v));
    }
    fn f32(&mut self, v: f32) {
        self.done(JsonValue::F32(v));
    }
    fn f64(&mut self, v: f64) {
        self.done(JsonValue::F64(v));
    }
    fn str(&mut self, v: &str) {
        self.done(JsonValue::Str(v.to_string()));
    }
    fn begin_struct(&mut self, _name: &'static str) {
        self.stack.push(Frame::Object(Vec::new(), None));
    }
    fn field(&mut self, name: &'static str) {
        if let Some(Frame::Object(_, key)) = self.stack.last_mut() {
            *key = Some(name);
        }
    }
    fn end_struct(&mut self) {
        if let Some(Frame::Object(pairs, _)) = self.stack.pop() {
            self.done(JsonValue::Object(pairs));
        }
    }
    fn begin_seq(&mut self, len: u32) {
        self.stack
            .push(Frame::Array(Vec::with_capacity(len.min(1024) as usize)));
    }
    fn end_seq(&mut self) {
        if let Some(Frame::Array(items)) = self.stack.pop() {
            self.done(JsonValue::Array(items));
        }
    }
    fn none(&mut self) {
        self.done(JsonValue::Null);
    }
    fn some(&mut self) {}
    fn variant(&mut self, _enum_name: &'static str, _index: u32, name: &'static str) {
        self.done(JsonValue::Str(name.to_string()));
    }
}
