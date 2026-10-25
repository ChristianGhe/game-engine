//! The version stamp (spec 6.13): `<game> --version` prints it on one line,
//! `<game> --version --json` prints all of it. Reports carry its `build`
//! object as `build`, and `crash.json` as `version`. The format is
//! documented in `docs/ARCHITECTURE.md`, "The version stamp".
//!
//! Where the values come from:
//! - the game: its name, version and commit ([`GameDef`]);
//! - this compilation, through `build.rs`: target triple, profile, rustc;
//! - the source tree, through environment variables read at build time:
//!   `ENG_ENGINE_SHA`, `ENG_DIRTY` (`0` or `1`), `SOURCE_DATE_EPOCH` (the
//!   commit time in Unix seconds) and `ENG_CI_RUN`. CI's workflow sets the
//!   first today; `cargo xtask` is to set all four (patch X1). Without
//!   them the strings read `unknown`, and `dirty` and `source_date_epoch`
//!   read `null`. An empty variable counts as unset, and a malformed
//!   `ENG_DIRTY` or `SOURCE_DATE_EPOCH` fails the build;
//! - the simulation, built with 1 player and seed 0 (the protocol version
//!   depends on neither): tick rate, protocol version, system order and
//!   schema versions;
//! - the probe's own file formats.

use std::collections::BTreeMap;

use eng_input::{InputScript, RECORDING_VERSION};
use eng_schema_core::TypeDesc;
use eng_schema_json::{Authored, JsonValue};
use eng_sim::Sim;

use crate::assertions::AssertionFile;
use crate::budgets::BudgetFile;
use crate::{Exit, GameDef, exit, report, run};

/// The stamp's format. Bump it when a field is removed or renamed, or
/// changes type or meaning; adding a field does not. A change to the
/// `build` object also bumps the report and `crash.json` versions.
pub(crate) const STAMP_VERSION: u64 = 1;

/// What a string field says when the build could not learn its value.
pub const UNKNOWN: &str = "unknown";

const fn known(value: Option<&'static str>) -> &'static str {
    match value {
        Some(v) if !v.is_empty() => v,
        _ => UNKNOWN,
    }
}

/// The engine's version: the `eng-cli` crate's.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The engine's source commit, set by `cargo xtask` builds.
pub const ENGINE_SHA: &str = known(option_env!("ENG_ENGINE_SHA"));

/// `rustc -V` of the compiler that built this, from `build.rs`.
pub const RUSTC_VERSION: &str = env!("ENG_BUILD_RUSTC");

/// The target triple, from `build.rs`.
pub const TARGET: &str = env!("ENG_BUILD_TARGET");

/// `debug` or `release`: the Cargo profile the build's profile inherits
/// from, from `build.rs`.
pub const PROFILE: &str = env!("ENG_BUILD_PROFILE");

/// The CI run that built this, set by `cargo xtask` builds.
const CI_RUN: &str = known(option_env!("ENG_CI_RUN"));

/// Whether the source tree had uncommitted changes, set by `cargo xtask`.
/// Empty counts as unset, as for every variable here.
const DIRTY: Option<bool> = match option_env!("ENG_DIRTY") {
    None => None,
    Some(v) if v.is_empty() => None,
    Some(v) => match v.as_bytes() {
        b"0" => Some(false),
        b"1" => Some(true),
        _ => panic!("[E-VERSION] ENG_DIRTY must be 0 or 1"),
    },
};

/// The commit time in Unix seconds (`SOURCE_DATE_EPOCH`), set by
/// `cargo xtask`.
const SOURCE_DATE_EPOCH: Option<u64> = match option_env!("SOURCE_DATE_EPOCH") {
    None => None,
    Some(v) if v.is_empty() => None,
    Some(v) => Some(parse_seconds(v)),
};

const fn parse_seconds(text: &str) -> u64 {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        panic!("[E-VERSION] SOURCE_DATE_EPOCH must be whole Unix seconds");
    }
    let mut value: u64 = 0;
    let mut i = 0;
    while i < bytes.len() {
        let digit = bytes[i];
        if !digit.is_ascii_digit() {
            panic!("[E-VERSION] SOURCE_DATE_EPOCH must be whole Unix seconds");
        }
        value = match value.checked_mul(10) {
            Some(v) => match v.checked_add((digit - b'0') as u64) {
                Some(v) => v,
                None => panic!("[E-VERSION] SOURCE_DATE_EPOCH is too large"),
            },
            None => panic!("[E-VERSION] SOURCE_DATE_EPOCH is too large"),
        };
        i += 1;
    }
    value
}

/// The game's commit, or [`UNKNOWN`] if it gave none. Recordings and
/// `replay verify` use this too, so they agree with the stamp.
pub(crate) fn game_sha(game: &GameDef) -> &'static str {
    known(Some(game.build_sha))
}

/// The build half of the stamp: the fields of spec 6.13's `version.json`.
/// Both forms of `--version` render from it, so they cannot disagree, and
/// tests can render values that one build's environment cannot show.
#[derive(Debug, Clone, Copy)]
struct Build<'a> {
    game_version: &'a str,
    engine_version: &'a str,
    engine_sha: &'a str,
    game_sha: &'a str,
    dirty: Option<bool>,
    source_date_epoch: Option<u64>,
    ci_run: &'a str,
    target: &'a str,
    rustc: &'a str,
    profile: &'a str,
    debug_assertions: bool,
}

impl Build<'static> {
    /// This binary's build.
    fn of(game: &GameDef) -> Self {
        Build {
            game_version: game.version,
            engine_version: ENGINE_VERSION,
            engine_sha: ENGINE_SHA,
            game_sha: game_sha(game),
            dirty: DIRTY,
            source_date_epoch: SOURCE_DATE_EPOCH,
            ci_run: CI_RUN,
            target: TARGET,
            rustc: RUSTC_VERSION,
            profile: PROFILE,
            debug_assertions: cfg!(debug_assertions),
        }
    }
}

impl Build<'_> {
    fn to_json(self) -> JsonValue {
        JsonValue::object([
            ("game_version", JsonValue::str(self.game_version)),
            ("engine_version", JsonValue::str(self.engine_version)),
            ("engine_sha", JsonValue::str(self.engine_sha)),
            ("game_sha", JsonValue::str(self.game_sha)),
            ("dirty", self.dirty.map_or(JsonValue::Null, JsonValue::Bool)),
            (
                "source_date_epoch",
                self.source_date_epoch
                    .map_or(JsonValue::Null, JsonValue::U64),
            ),
            ("ci_run", JsonValue::str(self.ci_run)),
            ("target", JsonValue::str(self.target)),
            ("rustc", JsonValue::str(self.rustc)),
            ("profile", JsonValue::str(self.profile)),
            ("debug_assertions", JsonValue::Bool(self.debug_assertions)),
        ])
    }

    /// The `--version` line for `game` at `protocol`.
    fn line(self, game: &str, protocol: u64) -> String {
        let mut engine = format!(
            "engine {} {}",
            self.engine_version,
            short_sha(self.engine_sha)
        );
        if self.game_sha != self.engine_sha {
            engine.push_str(&format!(", game {}", short_sha(self.game_sha)));
        }
        engine.push_str(match self.dirty {
            Some(true) => ", dirty",
            Some(false) => "",
            None => ", dirty unknown",
        });
        // `rustc 1.99.1 (hash date)` -> `1.99.1`.
        let rustc = self.rustc.split_whitespace().nth(1).unwrap_or(UNKNOWN);
        format!(
            "{game} {} ({engine}) {} {}, rustc {rustc}, sim protocol {protocol:#018x}",
            self.game_version, self.target, self.profile
        )
    }
}

/// The build half of the stamp. Reports carry it as `build` and
/// `crash.json` as `version`. It needs no simulation, so the panic hook can
/// hold a copy.
pub(crate) fn build_json(game: &GameDef) -> JsonValue {
    Build::of(game).to_json()
}

/// `--version --json`.
pub(crate) fn stamp_json(game: &GameDef, sim: &Sim) -> JsonValue {
    let systems = sim
        .system_order()
        .map(|(stage, name)| JsonValue::str(format!("{} {name}", stage.name())))
        .collect();
    JsonValue::object([
        ("stamp_version", JsonValue::U64(STAMP_VERSION)),
        ("game", JsonValue::str(game.name)),
        ("build", build_json(game)),
        (
            "sim",
            JsonValue::object([
                ("tick_rate", JsonValue::U64(game.tick_rate.into())),
                ("protocol", JsonValue::hex(sim.protocol_version())),
                ("systems", JsonValue::Array(systems)),
                ("schemas", schemas(sim)),
            ]),
        ),
        ("formats", formats()),
    ])
}

/// `--version`: one line for people. The JSON has every field.
pub(crate) fn stamp_line(game: &GameDef, sim: &Sim) -> String {
    Build::of(game).line(game.name, sim.protocol_version())
}

/// Prints the stamp and returns [`Exit::Ok`].
pub(crate) fn print(game: &GameDef, json: bool) -> Exit {
    let sim = run::build_sim(game, 1, 0);
    if json {
        println!("{}", stamp_json(game, &sim).to_pretty());
    } else {
        println!("{}", stamp_line(game, &sim));
    }
    Exit::Ok
}

/// The first 12 characters of a commit: enough to be unique in practice,
/// and the same length in every stamp.
fn short_sha(sha: &str) -> &str {
    match sha.char_indices().nth(12) {
        Some((end, _)) => &sha[..end],
        None => sha,
    }
}

/// Every named schema the world encodes, in (name, version) order: the
/// registered components, resources and events, and the named types inside
/// them (`nested`). A name with two versions shows up twice.
fn schemas(sim: &Sim) -> JsonValue {
    let world = sim.world();
    let registered: Vec<(&'static str, &'static str, &TypeDesc)> = world
        .component_names()
        .map(|n| (n, "component"))
        .chain(world.resource_names().map(|n| (n, "resource")))
        .chain(world.event_names().map(|n| (n, "event")))
        // `World` puts every component, resource and event in its registry.
        .filter_map(|(name, kind)| Some((name, kind, world.registry().get(name)?)))
        .collect();
    let mut found: BTreeMap<(&'static str, u32), &'static str> = BTreeMap::new();
    for (name, kind, desc) in &registered {
        found.insert((name, version_of(desc)), kind);
    }
    for (_, _, desc) in &registered {
        nested(desc, &mut found);
    }
    JsonValue::Array(
        found
            .into_iter()
            .map(|((name, version), kind)| {
                JsonValue::object([
                    ("name", JsonValue::str(name)),
                    ("version", JsonValue::U64(version.into())),
                    ("kind", JsonValue::str(kind)),
                ])
            })
            .collect(),
    )
}

fn version_of(desc: &TypeDesc) -> u32 {
    match desc {
        TypeDesc::Struct { version, .. } | TypeDesc::Enum { version, .. } => *version,
        _ => 0,
    }
}

/// Adds the named types inside `desc`, at any depth.
fn nested(desc: &TypeDesc, found: &mut BTreeMap<(&'static str, u32), &'static str>) {
    match desc {
        TypeDesc::Seq(inner) | TypeDesc::Option(inner) => {
            named(inner, found);
        }
        TypeDesc::Struct { fields, .. } => {
            for (_, field) in fields {
                named(field, found);
            }
        }
        _ => {}
    }
}

fn named(desc: &TypeDesc, found: &mut BTreeMap<(&'static str, u32), &'static str>) {
    if let TypeDesc::Struct { name, version, .. } | TypeDesc::Enum { name, version, .. } = desc {
        found.entry((name, *version)).or_insert("nested");
    }
    nested(desc, found);
}

/// The versions of the files the probe reads and writes.
fn formats() -> JsonValue {
    JsonValue::object([
        ("report", JsonValue::U64(report::REPORT_VERSION)),
        ("crash", JsonValue::U64(exit::CRASH_VERSION)),
        ("recording", JsonValue::U64(RECORDING_VERSION.into())),
        ("hash_stream", JsonValue::U64(run::HASH_STREAM_VERSION)),
        (
            "input_script",
            JsonValue::U64(<InputScript as Authored>::VERSION.into()),
        ),
        (
            "assertions",
            JsonValue::U64(<AssertionFile as Authored>::VERSION.into()),
        ),
        (
            "budgets",
            JsonValue::U64(<BudgetFile as Authored>::VERSION.into()),
        ),
    ])
}

#[cfg(test)]
mod tests;
