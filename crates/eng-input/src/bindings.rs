use std::collections::BTreeMap;
use std::fmt;

use eng_schema_json::{Authored, PathStep, TomlSpans, load_toml};
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::key::Key;

/// Something a key can do.
///
/// Movement actions become [`PlayerCommand`](eng_core::PlayerCommand)
/// fields. App actions ([`Action::Quit`]) go to the app beside the command
/// and never reach the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    /// `forward = +127` while active.
    MoveForward,
    /// `forward = -127` while active.
    MoveBack,
    /// `side = -127` while active.
    MoveLeft,
    /// `side = +127` while active.
    MoveRight,
    /// Leave the game. Handled by the app.
    Quit,
}

impl Action {
    /// Every action, in declaration order.
    pub const ALL: [Action; 5] = [
        Action::MoveForward,
        Action::MoveBack,
        Action::MoveLeft,
        Action::MoveRight,
        Action::Quit,
    ];

    /// The name a bindings file uses, for example `"move_forward"`.
    pub const fn name(self) -> &'static str {
        match self {
            Action::MoveForward => "move_forward",
            Action::MoveBack => "move_back",
            Action::MoveLeft => "move_left",
            Action::MoveRight => "move_right",
            Action::Quit => "quit",
        }
    }

    /// The action with this name. Exact.
    pub fn from_name(name: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|action| action.name() == name)
    }

    /// Whether the app handles this action instead of the simulation.
    pub const fn is_app(self) -> bool {
        matches!(self, Action::Quit)
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A set of actions, such as those active during one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ActionSet(u32);

impl ActionSet {
    pub const EMPTY: ActionSet = ActionSet(0);

    pub const fn contains(self, action: Action) -> bool {
        self.0 & (1 << action as u32) != 0
    }

    pub const fn with(self, action: Action) -> ActionSet {
        ActionSet(self.0 | (1 << action as u32))
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The actions in the set, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = Action> {
        Action::ALL.into_iter().filter(move |&a| self.contains(a))
    }
}

/// The text of the default bindings: WASD and the arrows move, Escape
/// quits. [`Bindings::default`] loads it with [`Bindings::parse`].
pub const DEFAULT_BINDINGS: &str = include_str!("default.bindings.toml");

/// Which action each key triggers. A key triggers at most one action; an
/// action may have any number of keys.
///
/// A bindings file, version 1, is strict TOML (spec D12):
///
/// ```toml
/// version = 1
///
/// [actions]
/// move_forward = ["KeyW", "ArrowUp"]   # W3C `code` names
/// quit = ["Escape"]
/// ```
///
/// - Unknown top-level keys are rejected and `x_` keys are ignored, as in
///   every authored format.
/// - Unknown action and key names, an action whose value is not a list of
///   names, and an unknown top-level key are rejected with their line. The
///   first such problem in the file is the one reported.
/// - A key may appear only once in the whole file.
/// - An action the file leaves out is unbound: a file replaces the
///   defaults ([`DEFAULT_BINDINGS`]) rather than adding to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings {
    by_key: [Option<Action>; Key::COUNT],
}

/// The serde shape of a bindings file; see [`Bindings`]. Anything with a
/// line to report is accepted here and rejected by [`Bindings::parse`], so
/// serde never reports a problem that comes later in the file first.
#[derive(Debug, Deserialize)]
struct BindingsFile {
    /// Checked and migrated by `load_toml`: always [`Authored::VERSION`].
    version: u32,
    #[serde(default)]
    actions: BTreeMap<String, KeyNames>,
    /// Top-level keys other than these two (`x_` keys are already gone).
    #[serde(flatten)]
    unknown: BTreeMap<String, IgnoredAny>,
}

/// An action's value: a list of key names, or anything else.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum KeyNames {
    List(Vec<String>),
    Other(IgnoredAny),
}

/// A problem at a byte offset in the file (`None`: not found).
struct Problem {
    offset: Option<usize>,
    message: String,
}

impl Problem {
    fn position(&self) -> usize {
        self.offset.unwrap_or(usize::MAX)
    }
}

impl Authored for BindingsFile {
    const KIND: &'static str = "bindings file";
    const VERSION: u32 = 1;
}

/// A bindings file that could not be loaded. Displays as
/// `[E-BINDINGS] origin:line: message`. Only a TOML syntax error (whose
/// message names its own line), a missing or newer `version`, and an
/// `actions` that is not a table have no `line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingsError {
    pub origin: String,
    pub line: Option<u32>,
    pub message: String,
}

impl fmt::Display for BindingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "[E-BINDINGS] {}:{line}: {}", self.origin, self.message),
            None => write!(f, "[E-BINDINGS] {}: {}", self.origin, self.message),
        }
    }
}

impl std::error::Error for BindingsError {}

impl Bindings {
    /// Nothing bound.
    pub const fn empty() -> Self {
        Self {
            by_key: [None; Key::COUNT],
        }
    }

    /// Loads a bindings file. `origin` names it in errors. Past TOML
    /// syntax and the version, every problem names its line, and the first
    /// one in the file is reported.
    pub fn parse(text: &str, origin: &str) -> Result<Self, BindingsError> {
        let file: BindingsFile = load_toml(text, origin).map_err(|e| BindingsError {
            origin: origin.to_string(),
            line: None,
            message: e.message,
        })?;
        debug_assert_eq!(file.version, BindingsFile::VERSION);
        let spans = TomlSpans::parse(text);
        let unknown = file
            .unknown
            .keys()
            .map(|name| Problem {
                offset: spans.offset(&[PathStep::Key(name)]),
                message: format!(
                    "unknown field `{}`; a bindings file has `version` and `[actions]`",
                    name.escape_debug()
                ),
            })
            .min_by_key(Problem::position);
        let problem = match (bind_actions(&file.actions, &spans), unknown) {
            (Ok(bindings), None) => return Ok(bindings),
            (Ok(_), Some(problem)) | (Err(problem), None) => problem,
            (Err(a), Some(b)) => {
                if b.position() < a.position() {
                    b
                } else {
                    a
                }
            }
        };
        Err(BindingsError {
            origin: origin.to_string(),
            line: problem.offset.map(|o| spans.line(o)),
            message: problem.message,
        })
    }

    /// The action `key` triggers, if any.
    pub const fn action(&self, key: Key) -> Option<Action> {
        self.by_key[key.index()]
    }

    /// Binds `key` to `action` and returns what it was bound to before.
    pub const fn bind(&mut self, key: Key, action: Action) -> Option<Action> {
        self.by_key[key.index()].replace(action)
    }

    /// Unbinds `key` and returns what it was bound to.
    pub const fn unbind(&mut self, key: Key) -> Option<Action> {
        self.by_key[key.index()].take()
    }

    /// The keys bound to `action`, in [`Key::ALL`] order.
    pub fn keys(&self, action: Action) -> impl Iterator<Item = Key> + '_ {
        Key::ALL
            .iter()
            .copied()
            .filter(move |&key| self.action(key) == Some(action))
    }
}

impl Default for Bindings {
    /// WASD and the arrows move, Escape quits: [`DEFAULT_BINDINGS`].
    fn default() -> Self {
        Self::parse(DEFAULT_BINDINGS, "eng-input default bindings")
            .unwrap_or_else(|e| panic!("the built-in default bindings must load: {e}"))
    }
}

/// Binds every action's keys, in file order: the problem returned is the
/// first one, and a key given twice is reported where it appears again.
fn bind_actions(
    actions: &BTreeMap<String, KeyNames>,
    spans: &TomlSpans<'_>,
) -> Result<Bindings, Problem> {
    let mut entries: Vec<_> = actions
        .iter()
        .map(|(name, keys)| {
            let path = [PathStep::Key("actions"), PathStep::Key(name)];
            (spans.offset(&path), name, keys)
        })
        .collect();
    entries.sort_by_key(|&(offset, _, _)| offset.unwrap_or(usize::MAX));

    let mut bindings = Bindings::empty();
    let mut bound_on: [Option<u32>; Key::COUNT] = [None; Key::COUNT];
    for (offset, name, keys) in entries {
        let problem = |offset, message| Problem { offset, message };
        let Some(action) = Action::from_name(name) else {
            return Err(problem(offset, unknown_action(name)));
        };
        let KeyNames::List(keys) = keys else {
            let message = format!("`{action}` must be a list of key names, such as `[\"KeyW\"]`");
            return Err(problem(offset, message));
        };
        for (i, key_name) in keys.iter().enumerate() {
            let path = [
                PathStep::Key("actions"),
                PathStep::Key(name),
                PathStep::Index(i),
            ];
            let at = spans.offset(&path).or(offset);
            let Some(key) = Key::from_name(key_name) else {
                return Err(problem(at, unknown_key(key_name)));
            };
            if let Some(previous) = bindings.action(key) {
                let first =
                    bound_on[key.index()].map_or(String::new(), |l| format!(" on line {l}"));
                let message = format!("key `{key}` is already bound to `{previous}`{first}");
                return Err(problem(at, message));
            }
            bindings.bind(key, action);
            bound_on[key.index()] = at.map(|o| spans.line(o));
        }
    }
    Ok(bindings)
}

// Names from the file are escaped, so a message stays on one line.

fn unknown_action(name: &str) -> String {
    let names: Vec<_> = Action::ALL.iter().map(|a| format!("`{a}`")).collect();
    format!(
        "unknown action `{}`; the actions are {}",
        name.escape_debug(),
        names.join(", ")
    )
}

fn unknown_key(name: &str) -> String {
    let hint = suggest_key(name).map_or(String::new(), |key| format!(" (did you mean `{key}`?)"));
    format!(
        "unknown key `{}`{hint}; keys are W3C `code` names such as `KeyW`, `Digit1`, `ArrowUp`, `Space` or `Escape`",
        name.escape_debug()
    )
}

/// A key whose name differs only in case, or lacks its `Key`, `Digit` or
/// `Arrow` prefix: `w`, `1`, `up`, `escape`, or `Esc`.
fn suggest_key(name: &str) -> Option<Key> {
    if name.eq_ignore_ascii_case("esc") {
        return Some(Key::Escape);
    }
    let candidates = ["", "Key", "Digit", "Arrow"].map(|prefix| format!("{prefix}{name}"));
    Key::ALL.iter().copied().find(|key| {
        candidates
            .iter()
            .any(|candidate| key.name().eq_ignore_ascii_case(candidate))
    })
}
