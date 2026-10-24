//! Semantic assertions (spec 6.4): protected oracles about what the
//! simulation must do, checked against the world dump.
//!
//! ```toml
//! version = 1
//!
//! [[assert]]
//! id = "dot-reaches-east-wall"
//! by_tick = 120                  # or at_tick = N, or always = true
//! pointer = "/entities/0/components/kit_fps.MoveState/pos/x"
//! op = ">="                      # ==, !=, <, <=, >, >=
//! value = 7.75                   # a number, a bool or a string
//! ```

use eng_core::Tick;
use eng_schema_json::{Authored, JsonValue, LoadError, load_toml};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssertionFile {
    /// Checked by `load_toml`.
    #[serde(rename = "version")]
    _version: u32,
    #[serde(default, rename = "assert")]
    pub asserts: Vec<AssertionSpec>,
}

impl Authored for AssertionFile {
    const KIND: &'static str = "assertion file";
    const VERSION: u32 = 1;
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssertionSpec {
    pub id: String,
    pub pointer: String,
    pub op: Op,
    pub value: Expected,
    pub at_tick: Option<u64>,
    pub by_tick: Option<u64>,
    pub always: Option<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) enum Op {
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Expected {
    Bool(bool),
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum When {
    At(u64),
    By(u64),
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Status {
    Pending,
    Passed { tick: u64 },
    Failed { tick: u64, message: String },
}

pub(crate) struct Tracker {
    spec: AssertionSpec,
    when: When,
    pub status: Status,
}

impl Tracker {
    pub fn id(&self) -> &str {
        &self.spec.id
    }

    pub fn ok(&self) -> bool {
        matches!(self.status, Status::Passed { .. })
    }

    fn pending(&self) -> bool {
        matches!(self.status, Status::Pending)
    }

    /// Whether this assertion wants the world at `tick`.
    pub fn wants(&self, tick: Tick) -> bool {
        match self.when {
            When::At(t) => self.pending() && tick.0 == t,
            When::By(t) => self.pending() && tick.0 <= t,
            When::Always => self.pending(),
        }
    }

    pub fn observe(&mut self, tick: Tick, dump: &JsonValue) {
        let tick = tick.0;
        let pointer = &self.spec.pointer;
        self.status = match (self.when, evaluate(&self.spec, dump)) {
            (_, Err(message)) => Status::Failed { tick, message },
            // by_tick: a value that does not exist yet is not true yet.
            (When::By(t), Ok(None)) if tick < t => Status::Pending,
            (_, Ok(None)) => Status::Failed {
                tick,
                message: format!("pointer {pointer} not found in the world dump"),
            },
            (When::At(_) | When::By(_), Ok(Some((true, _)))) => Status::Passed { tick },
            (When::At(_), Ok(Some((false, actual)))) => Status::Failed {
                tick,
                message: format!("{pointer} is {actual}"),
            },
            (When::By(t), Ok(Some((false, actual)))) if tick == t => Status::Failed {
                tick,
                message: format!("still false by tick {t}: {pointer} is {actual}"),
            },
            (When::Always, Ok(Some((false, actual)))) => Status::Failed {
                tick,
                message: format!("broken: {pointer} is {actual}"),
            },
            _ => Status::Pending,
        };
    }

    /// Resolves what is still pending when the run ends at `last`.
    pub fn finish(&mut self, last: Tick) {
        if !self.pending() {
            return;
        }
        self.status = match self.when {
            When::Always => Status::Passed { tick: last.0 },
            When::At(t) | When::By(t) => Status::Failed {
                tick: last.0,
                message: format!("the run ended at tick {} before tick {t}", last.0),
            },
        };
    }

    pub fn summary(&self) -> String {
        match &self.status {
            Status::Pending => format!("{}: pending", self.spec.id),
            Status::Passed { tick } => format!("{}: ok at tick {tick}", self.spec.id),
            Status::Failed { tick, message } => {
                format!(
                    "[E-ASSERT] {} failed at tick {tick}: {message}",
                    self.spec.id
                )
            }
        }
    }
}

pub(crate) fn load(text: &str, origin: &str) -> Result<Vec<Tracker>, LoadError> {
    let file: AssertionFile = load_toml(text, origin)?;
    let fail = |id: &str, message: &str| LoadError {
        origin: origin.to_string(),
        message: format!("assertion {id:?}: {message}"),
    };
    let mut trackers = Vec::new();
    for spec in file.asserts {
        let when = match (spec.at_tick, spec.by_tick, spec.always) {
            (Some(t), None, None) => When::At(t),
            (None, Some(t), None) => When::By(t),
            (None, None, Some(true)) => When::Always,
            _ => {
                return Err(fail(
                    &spec.id,
                    "needs exactly one of at_tick, by_tick or always = true",
                ));
            }
        };
        if !spec.pointer.starts_with('/') {
            return Err(fail(&spec.id, "pointer must start with '/'"));
        }
        if trackers.iter().any(|t: &Tracker| t.id() == spec.id) {
            return Err(fail(&spec.id, "duplicate id"));
        }
        trackers.push(Tracker {
            spec,
            when,
            status: Status::Pending,
        });
    }
    Ok(trackers)
}

/// `Some((holds, actual value as text))`, `None` if the pointer does not
/// resolve, or why the comparison is impossible.
fn evaluate(spec: &AssertionSpec, dump: &JsonValue) -> Result<Option<(bool, String)>, String> {
    let Some(actual) = dump.pointer(&spec.pointer) else {
        return Ok(None);
    };
    let shown = actual.to_compact();
    let ordering = match (&spec.value, actual) {
        (Expected::Number(want), _) => {
            let got = actual
                .as_f64()
                .ok_or_else(|| format!("{} is {shown}, not a number", spec.pointer))?;
            got.partial_cmp(want)
        }
        (Expected::Bool(want), JsonValue::Bool(got)) => Some(got.cmp(want)),
        (Expected::Text(want), JsonValue::Str(got)) => Some(got.as_str().cmp(want.as_str())),
        _ => return Err(format!("{} is {shown}, a different type", spec.pointer)),
    };
    let Some(ordering) = ordering else {
        return Ok(Some((false, shown)));
    };
    let holds = match spec.op {
        Op::Eq => ordering.is_eq(),
        Op::Ne => ordering.is_ne(),
        Op::Lt => ordering.is_lt(),
        Op::Le => ordering.is_le(),
        Op::Gt => ordering.is_gt(),
        Op::Ge => ordering.is_ge(),
    };
    Ok(Some((holds, shown)))
}
