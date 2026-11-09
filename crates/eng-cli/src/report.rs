//! The JSON report and the short console summary.

use eng_input::{Bindings, InputScript, TickHash};
use eng_math::Fnv1a64;
use eng_schema_json::{Authored, JsonValue};
use eng_sim::Sim;

use crate::args::RunArgs;
use crate::assertions::{AssertionFile, Status, Tracker};
use crate::budgets::{BudgetFile, BudgetTracker};
use crate::capture::CaptureSummary;
use crate::look_file::LookFile;
use crate::play::PlayWindow;
use crate::{Exit, GameDef, version};

/// The report's format. Version 2 carries the version stamp's `build`
/// object; version 1 (rung 1a) had a smaller one. Keys are added within a
/// version, as the `gpu` object and the `[gpu]` budgets were in rung 1b,
/// and `map`, `sim_data_hash` and `files_read` in rung 2 (ADR-0023), and
/// a first-person `play`'s `window` (T15);
/// readers look keys up by name (`docs/ARCHITECTURE.md`, "The report").
pub(crate) const REPORT_VERSION: u64 = 2;

/// FNV-1a-64 over every `(tick, hash)` pair, for comparing whole streams.
pub(crate) fn stream_digest(hashes: &[TickHash]) -> u64 {
    let mut h = Fnv1a64::new();
    for th in hashes {
        h.write(&th.tick.0.to_le_bytes());
        h.write(&th.hash.to_le_bytes());
    }
    h.finish()
}

/// The format version of each file a run read, keyed as `--version
/// --json`'s `formats` names them, plus `bindings` (`play --bindings`) and
/// `look` (`play --look`).
/// Each reader accepts only its own version (no file format has a
/// migration yet), so a file that loaded was read at that version.
pub(crate) fn files_read(a: &RunArgs) -> JsonValue {
    let bindings = a.play.as_ref().and_then(|p| p.bindings.as_ref());
    let look = a.play.as_ref().and_then(|p| p.first_person.look.as_ref());
    let files = [
        (a.input.is_some(), "input_script", InputScript::VERSION),
        (a.assert.is_some(), "assertions", AssertionFile::VERSION),
        (a.budgets.is_some(), "budgets", BudgetFile::VERSION),
        (bindings.is_some(), "bindings", Bindings::FILE_VERSION),
        (look.is_some(), "look", LookFile::VERSION),
    ];
    JsonValue::object(
        files
            .into_iter()
            .filter(|(read, _, _)| *read)
            .map(|(_, name, version)| (name, JsonValue::U64(version.into()))),
    )
}

pub(crate) struct RunSummary<'a> {
    pub game: &'a GameDef,
    /// The map's name (`--map`, or the game's first).
    pub map: &'static str,
    pub args: &'a RunArgs,
    pub sim: &'a Sim,
    pub hashes: &'a [TickHash],
    pub tick_ns: Vec<u64>,
    pub events: u64,
    pub trackers: &'a [Tracker],
    pub budgets: &'a BudgetTracker,
    /// `shot`, `video` and `play`: the adapter and the frames.
    pub frames: Option<&'a CaptureSummary>,
    /// `"headless"`, or the platform `play` ran on.
    pub platform: &'static str,
    /// Ticks of real time `play`'s frame loop dropped; 0 headless.
    pub dropped_ticks: u64,
    /// A first-person `play`'s window; `None` for every other run, whose
    /// report has no `window` key.
    pub window: Option<&'a PlayWindow>,
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
        let mut budgets = vec![
            ("max_state_bytes", limit(max_bytes)),
            (
                "peak_state_bytes",
                JsonValue::U64(self.budgets.peak_state_bytes),
            ),
            ("max_entities", limit(max_entities)),
            ("peak_entities", JsonValue::U64(self.budgets.peak_entities)),
        ];
        budgets.extend(
            self.budgets
                .gpu_limits()
                .map(|(name, max)| (name, limit(max))),
        );
        budgets.push((
            "exceeded",
            self.budgets
                .exceeded
                .as_ref()
                .map_or(JsonValue::Null, |m| JsonValue::str(m.clone())),
        ));
        let mut fields = vec![
            ("report_version", JsonValue::U64(REPORT_VERSION)),
            ("command", JsonValue::str(self.args.command())),
            ("game", JsonValue::str(self.game.name)),
            ("build", version::build_json(self.game)),
            ("platform", JsonValue::str(self.platform)),
            ("map", JsonValue::str(self.map)),
            ("seed", JsonValue::U64(self.args.seed)),
            ("players", JsonValue::U64(self.args.players.into())),
            ("tick_rate", JsonValue::U64(self.game.tick_rate.into())),
            ("ticks_requested", self.ticks_requested()),
            ("ticks_run", JsonValue::U64(self.sim.tick().0)),
            ("dropped_ticks", JsonValue::U64(self.dropped_ticks)),
            ("sim_protocol", JsonValue::hex(self.sim.protocol_version())),
            ("sim_data_hash", JsonValue::hex(self.sim.sim_data_hash())),
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
            ("budgets", JsonValue::object(budgets)),
            ("files_read", files_read(self.args)),
            (
                "gpu",
                self.frames.map_or(JsonValue::Null, |f| self.gpu_json(f)),
            ),
        ];
        if let Some(window) = self.window {
            fields.push(("window", window.to_json()));
        }
        fields.extend([
            ("exit_code", JsonValue::U64(self.exit.code().into())),
            ("exit", JsonValue::str(self.exit.label())),
        ]);
        JsonValue::object(fields)
    }

    /// The `gpu` object: the adapter (eng-gpu's `GpuReport`), the image
    /// size, the frames rendered and, per frame, the largest of each counter
    /// over them.
    fn gpu_json(&self, frames: &CaptureSummary) -> JsonValue {
        let r = &frames.report;
        let c = r.capabilities;
        let per_frame = self.budgets.peak_frame.map_or(JsonValue::Null, |p| {
            JsonValue::object([
                ("draws", JsonValue::U64(p.draws.into())),
                (
                    "pipeline_switches",
                    JsonValue::U64(p.pipeline_switches.into()),
                ),
                (
                    "bind_group_switches",
                    JsonValue::U64(p.bind_group_switches.into()),
                ),
                ("upload_bytes", JsonValue::U64(p.upload_bytes)),
            ])
        });
        let mut gpu = vec![
            ("adapter", JsonValue::str(r.adapter.clone())),
            ("backend", JsonValue::str(r.backend.as_str())),
            ("device_type", JsonValue::str(r.device_type.as_str())),
            ("vendor", JsonValue::U64(r.vendor.into())),
            ("device", JsonValue::U64(r.device.into())),
            ("driver", JsonValue::str(r.driver.clone())),
            ("driver_info", JsonValue::str(r.driver_info.clone())),
            ("tier", JsonValue::U64(r.tier.into())),
            (
                "capabilities",
                JsonValue::object([
                    ("timestamp_queries", JsonValue::Bool(c.timestamp_queries)),
                    ("binding_arrays", JsonValue::Bool(c.binding_arrays)),
                    (
                        "multi_draw_indirect_count",
                        JsonValue::Bool(c.multi_draw_indirect_count),
                    ),
                    ("ray_query", JsonValue::Bool(c.ray_query)),
                    ("mesh_shaders", JsonValue::Bool(c.mesh_shaders)),
                ]),
            ),
            ("width", JsonValue::U64(frames.width.into())),
            ("height", JsonValue::U64(frames.height.into())),
            ("frames", JsonValue::U64(frames.frames)),
            ("per_frame", per_frame),
        ];
        if let Some(format) = frames.surface_format {
            gpu.push(("surface_format", JsonValue::str(format)));
        }
        JsonValue::object(gpu)
    }

    /// `--ticks`, or `null` for a `play` that runs until `quit`.
    fn ticks_requested(&self) -> JsonValue {
        match &self.args.play {
            Some(play) if play.open_ended => JsonValue::Null,
            _ => JsonValue::U64(self.args.ticks),
        }
    }

    /// `shot`'s, `video`'s and `play`'s two lines: the frames and the
    /// adapter, then the per-frame counters.
    fn frame_lines(&self, frames: &CaptureSummary) -> [String; 2] {
        let r = &frames.report;
        let written = match &frames.out {
            None => "presented in the window".to_string(),
            Some(out) if self.sim.tick().0 == self.args.ticks => {
                format!("written to {}", out.display())
            }
            Some(_) => "not written: the run stopped early".to_string(),
        };
        let counters = match self.budgets.peak_frame {
            Some(p) => format!(
                "per frame: draws {}, pipeline switches {}, bind-group switches {}, uploaded bytes {}",
                p.draws, p.pipeline_switches, p.bind_group_switches, p.upload_bytes
            ),
            None => "no frame rendered".to_string(),
        };
        [
            format!(
                "{} frame(s) of {}x{} on {} ({}, {}), {written}",
                frames.frames,
                frames.width,
                frames.height,
                r.adapter,
                r.backend.as_str(),
                r.device_type.as_str()
            ),
            counters,
        ]
    }

    /// At most about 20 lines.
    pub fn console_lines(&self) -> Vec<String> {
        let last = self
            .hashes
            .last()
            .copied()
            .expect("tick 0 is always hashed");
        let of = match self.ticks_requested() {
            JsonValue::U64(ticks) => format!("of {ticks} "),
            _ => String::new(),
        };
        let mut lines = vec![
            format!(
                "{} {}: {} {of}ticks, seed {}, {} player(s), {}",
                self.game.name,
                self.args.command(),
                self.sim.tick().0,
                self.args.seed,
                self.args.players,
                self.platform
            ),
            format!(
                "final {} hash {:#018x}, stream {:#018x}, protocol {:#018x}",
                last.tick,
                last.hash,
                stream_digest(self.hashes),
                self.sim.protocol_version()
            ),
        ];
        // A map without sim data (the dot's) keeps 1b's lines.
        if self.sim.has_sim_data() {
            lines.push(format!(
                "map {}, sim data {:#018x}",
                self.map,
                self.sim.sim_data_hash()
            ));
        }
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
        if let Some(frames) = self.frames {
            lines.extend(self.frame_lines(frames));
        }
        if let Some(window) = self.window {
            lines.push(window.console_line());
        }
        if self.args.play.is_some() {
            lines.push(format!("dropped ticks: {}", self.dropped_ticks));
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
