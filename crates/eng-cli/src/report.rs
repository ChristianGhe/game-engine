//! The JSON report and the short console summary.

use eng_input::TickHash;
use eng_math::Fnv1a64;
use eng_schema_json::JsonValue;
use eng_sim::Sim;

use crate::args::RunArgs;
use crate::assertions::{Status, Tracker};
use crate::budgets::BudgetTracker;
use crate::{ENGINE_SHA, Exit, GameDef, RUSTC_VERSION};

pub(crate) const REPORT_VERSION: u64 = 1;

pub(crate) fn build_info(game: &GameDef) -> JsonValue {
    JsonValue::object([
        ("game_version", JsonValue::str(game.version)),
        ("engine_sha", JsonValue::str(ENGINE_SHA)),
        ("game_sha", JsonValue::str(game.build_sha)),
        ("rustc", JsonValue::str(RUSTC_VERSION)),
        (
            "target",
            JsonValue::str(format!(
                "{}-{}",
                std::env::consts::ARCH,
                std::env::consts::OS
            )),
        ),
        (
            "profile",
            JsonValue::str(if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }),
        ),
    ])
}

/// FNV-1a-64 over every `(tick, hash)` pair, for comparing whole streams.
pub(crate) fn stream_digest(hashes: &[TickHash]) -> u64 {
    let mut h = Fnv1a64::new();
    for th in hashes {
        h.write(&th.tick.0.to_le_bytes());
        h.write(&th.hash.to_le_bytes());
    }
    h.finish()
}

pub(crate) struct RunSummary<'a> {
    pub game: &'a GameDef,
    pub args: &'a RunArgs,
    pub sim: &'a Sim,
    pub hashes: &'a [TickHash],
    pub tick_ns: Vec<u64>,
    pub events: u64,
    pub trackers: &'a [Tracker],
    pub budgets: &'a BudgetTracker,
    pub exit: Exit,
}

impl RunSummary<'_> {
    fn percentiles(&self) -> [(&'static str, u64); 4] {
        let mut sorted = self.tick_ns.clone();
        sorted.sort_unstable();
        let at = |q: usize| {
            if sorted.is_empty() {
                0
            } else {
                sorted[(sorted.len() - 1) * q / 100]
            }
        };
        [
            ("p50", at(50)),
            ("p90", at(90)),
            ("p99", at(99)),
            ("max", at(100)),
        ]
    }

    pub fn to_json(&self) -> JsonValue {
        let last = self
            .hashes
            .last()
            .copied()
            .expect("tick 0 is always hashed");
        let systems = self
            .sim
            .system_order()
            .map(|(stage, name)| JsonValue::str(format!("{} {name}", stage.name())))
            .collect();
        let assertions = self
            .trackers
            .iter()
            .map(|t| {
                let (tick, message) = match &t.status {
                    Status::Passed { tick } => (Some(*tick), String::new()),
                    Status::Failed { tick, message } => (Some(*tick), message.clone()),
                    Status::Pending => (None, "pending".into()),
                };
                JsonValue::object([
                    ("id", JsonValue::str(t.id())),
                    ("ok", JsonValue::Bool(t.ok())),
                    ("tick", tick.map_or(JsonValue::Null, JsonValue::U64)),
                    ("message", JsonValue::str(message)),
                ])
            })
            .collect();
        let (max_bytes, max_entities) = self.budgets.limits();
        let limit = |l: Option<u64>| l.map_or(JsonValue::Null, JsonValue::U64);
        JsonValue::object([
            ("report_version", JsonValue::U64(REPORT_VERSION)),
            (
                "command",
                JsonValue::str(if self.args.dump.is_some() {
                    "dump-world"
                } else {
                    "run"
                }),
            ),
            ("game", JsonValue::str(self.game.name)),
            ("build", build_info(self.game)),
            ("platform", JsonValue::str("headless")),
            ("seed", JsonValue::U64(self.args.seed)),
            ("players", JsonValue::U64(self.args.players.into())),
            ("tick_rate", JsonValue::U64(self.game.tick_rate.into())),
            ("ticks_requested", JsonValue::U64(self.args.ticks)),
            ("ticks_run", JsonValue::U64(self.sim.tick().0)),
            ("sim_protocol", JsonValue::hex(self.sim.protocol_version())),
            ("systems", JsonValue::Array(systems)),
            ("hash_every", JsonValue::U64(self.args.hash_every)),
            ("hashes_recorded", JsonValue::U64(self.hashes.len() as u64)),
            ("final_tick", JsonValue::U64(last.tick.0)),
            ("final_hash", JsonValue::hex(last.hash)),
            (
                "hash_stream_digest",
                JsonValue::hex(stream_digest(self.hashes)),
            ),
            ("state_bytes", JsonValue::U64(self.sim.state_len() as u64)),
            (
                "entities",
                JsonValue::U64(self.sim.world().entity_count() as u64),
            ),
            ("events_written", JsonValue::U64(self.events)),
            (
                "tick_time_ns",
                JsonValue::object(self.percentiles().map(|(k, v)| (k, JsonValue::U64(v)))),
            ),
            ("assertions", JsonValue::Array(assertions)),
            (
                "budgets",
                JsonValue::object([
                    ("max_state_bytes", limit(max_bytes)),
                    (
                        "peak_state_bytes",
                        JsonValue::U64(self.budgets.peak_state_bytes),
                    ),
                    ("max_entities", limit(max_entities)),
                    ("peak_entities", JsonValue::U64(self.budgets.peak_entities)),
                    (
                        "exceeded",
                        self.budgets
                            .exceeded
                            .as_ref()
                            .map_or(JsonValue::Null, |m| JsonValue::str(m.clone())),
                    ),
                ]),
            ),
            ("exit_code", JsonValue::U64(self.exit.code().into())),
            ("exit", JsonValue::str(self.exit.label())),
        ])
    }

    /// At most about 20 lines.
    pub fn console_lines(&self) -> Vec<String> {
        let last = self
            .hashes
            .last()
            .copied()
            .expect("tick 0 is always hashed");
        let mut lines = vec![
            format!(
                "{} {}: {} of {} ticks, seed {}, {} player(s), headless",
                self.game.name,
                if self.args.dump.is_some() {
                    "dump-world"
                } else {
                    "run"
                },
                self.sim.tick().0,
                self.args.ticks,
                self.args.seed,
                self.args.players
            ),
            format!(
                "final {} hash {:#018x}, stream {:#018x}, protocol {:#018x}",
                last.tick,
                last.hash,
                stream_digest(self.hashes),
                self.sim.protocol_version()
            ),
        ];
        let failed: Vec<&Tracker> = self.trackers.iter().filter(|t| !t.ok()).collect();
        if !self.trackers.is_empty() {
            lines.push(format!(
                "assertions: {}/{} ok",
                self.trackers.len() - failed.len(),
                self.trackers.len()
            ));
        }
        lines.extend(failed.iter().take(8).map(|t| t.summary()));
        if failed.len() > 8 {
            lines.push(format!(
                "... {} more failed assertions in the report",
                failed.len() - 8
            ));
        }
        if let Some(message) = &self.budgets.exceeded {
            lines.push(message.clone());
        }
        let [p50, _, p99, _] = self.percentiles();
        lines.push(format!(
            "state {} bytes, {} entities; tick time p50 {} ns, p99 {} ns",
            self.sim.state_len(),
            self.sim.world().entity_count(),
            p50.1,
            p99.1
        ));
        lines.push(format!("exit {} ({})", self.exit.code(), self.exit.label()));
        lines
    }
}
