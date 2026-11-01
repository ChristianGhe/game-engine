//! `play`'s simulation side (rung 1b, task T11): everything `play` shares
//! with `run`, for eng-app's frame loop to drive.
//!
//! - The same session over Memory and the same tick ([`Session::tick`]), so
//!   `play --input` gives `run`'s hash stream tick for tick (plan, "One
//!   frame": one code path).
//! - The same files: `--hashes` in the committed format, `--record` from
//!   the commands the server applied, and `--report` (version 2, with
//!   `platform`, `dropped_ticks` and the `gpu` object).
//! - The same crash context: the seed, and each tick as it starts, for
//!   `crash.json`.
//!
//! eng-app owns the clock, the window, live input and drawing: it calls
//! [`PlayRun::step`] for each tick that is due, draws [`PlayRun::frame`],
//! and ends with [`PlayRun::finish`] from the platform's `stop`, so the
//! files are saved even where a desktop's quit command ends the process
//! right after `stop` (macOS, rung 1b amendment 39).

use eng_core::{PlayerCommand, Tick};
use eng_gpu::{FrameCounters, GpuReport};
use eng_input::{Bindings, TickHash};
use eng_render::TopDownScene;
use eng_sim::Sim;

use crate::args::{self, Command, RunArgs};
use crate::budgets::BudgetTracker;
use crate::capture::CaptureSummary;
use crate::run::{build_sim, fail, hash, load_inputs, read_text, write_file, write_outputs};
use crate::session::{Session, TickStop, command_for};
use crate::{Exit, GameDef, TopDownFrame, exit, log, report};

/// What a tick left the run to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Go on: more ticks may come.
    Next,
    /// The run is over: its last tick (`--ticks`) ran, or it had to stop.
    /// Leave the loop and call [`PlayRun::finish`].
    Done,
}

/// What the app tells the report at the end of a run.
#[derive(Debug, Clone)]
pub struct PlayStats {
    /// The platform's name, such as `"winit"` or `"headless"`.
    pub platform: &'static str,
    /// The window's GPU and frames; `None` without a window.
    pub gpu: Option<PlayGpu>,
}

/// The report's `gpu` object for a window: the adapter, the window's last
/// size, its surface's format and the frames presented. The per-frame
/// counters come from [`PlayRun::observe_frame`].
#[derive(Debug, Clone)]
pub struct PlayGpu {
    pub report: GpuReport,
    pub width: u32,
    pub height: u32,
    /// `"Bgra8UnormSrgb"`, or `"Bgra8Unorm"` drawn through an sRGB view
    /// (eng-gpu's `WindowSurface::format`).
    pub surface_format: &'static str,
    pub frames: u64,
}

/// One `play` run: the session at its latest tick and the state a tick
/// before it, the script if `--input` gave one, the bindings, and what the
/// files will hold.
pub struct PlayRun<'g> {
    game: &'g GameDef,
    args: RunArgs,
    bindings: Bindings,
    session: Session,
    /// The client's state a tick before its latest, for interpolation.
    previous: Sim,
    script: Option<Box<dyn Iterator<Item = PlayerCommand>>>,
    hashes: Vec<TickHash>,
    recorded: Vec<PlayerCommand>,
    tick_ns: Vec<u64>,
    budgets: BudgetTracker,
    dropped: u64,
    /// A tick that left with an exit that writes nothing (a desync, a
    /// command for the wrong tick), as `run` leaves.
    exit: Option<Exit>,
    done: bool,
}

impl<'g> PlayRun<'g> {
    /// Parses a `play` command line (`args[0]` is `"play"`), loads its files
    /// and starts the session at tick 0. A usage error is printed with
    /// `[E-USAGE]` and returned as [`Exit::Usage`].
    pub fn start(game: &'g GameDef, args: &[String]) -> Result<Self, Exit> {
        match args::parse(args) {
            Ok(Command::Run(a)) if a.play.is_some() => Self::new(game, *a),
            Ok(_) => Err(exit::usage(game.name, "not a play command line")),
            Err(message) => Err(exit::usage(game.name, &message)),
        }
    }

    pub(crate) fn new(game: &'g GameDef, a: RunArgs) -> Result<Self, Exit> {
        let flags = a.play.clone().unwrap_or_default();
        let inputs = load_inputs(&a).map_err(|message| exit::usage(game.name, &message))?;
        let bindings = match &flags.bindings {
            Some(path) => read_text(path).and_then(|text| {
                Bindings::parse(&text, &path.display().to_string()).map_err(|e| e.to_string())
            }),
            None => Ok(Bindings::default()),
        }
        .map_err(|message| exit::usage(game.name, &message))?;
        let (session, too_large) = Session::start(game, a.players, a.seed);
        let previous = build_sim(game, a.players, a.seed);
        let mut budgets = inputs.budgets;
        if let Some(message) = too_large {
            budgets.exceeded.get_or_insert(message);
        }
        let sim = session.sim();
        budgets.observe(0, sim.state_len() as u64, sim.world().entity_count() as u64);
        let hashes = vec![TickHash {
            tick: Tick::ZERO,
            hash: hash(sim),
        }];
        // The iterator keeps the script, so each run frees it.
        let script = inputs.script.map(|script| {
            Box::new(script.into_commands()) as Box<dyn Iterator<Item = PlayerCommand>>
        });
        let done = budgets.exceeded.is_some() || a.ticks == 0;
        Ok(Self {
            game,
            args: a,
            bindings,
            session,
            previous,
            script,
            hashes,
            recorded: Vec::new(),
            tick_ns: Vec::new(),
            budgets,
            dropped: 0,
            exit: None,
            done,
        })
    }

    pub fn game(&self) -> &'g GameDef {
        self.game
    }

    /// The bindings live input reads: `--bindings`, or the defaults (WASD,
    /// the arrows, Esc for `quit`).
    pub fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// Whether `--input` gave a script: its commands then replace the
    /// keyboard's, and only `quit` is read from the keyboard.
    pub fn scripted(&self) -> bool {
        self.script.is_some()
    }

    /// The tick the next [`PlayRun::step`] runs.
    pub fn next_tick(&self) -> Tick {
        self.session.sim().tick().next()
    }

    /// Whether the run is over: see [`Step::Done`].
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Runs the next tick with player 0's `live` command, which must be for
    /// [`PlayRun::next_tick`]; with `--input` the script's command replaces
    /// it.
    ///
    /// # Panics
    /// At `--panic-at-tick N` (test-only), as `run` does.
    pub fn step(&mut self, live: PlayerCommand) -> Step {
        if self.done {
            return Step::Done;
        }
        let tick = self.next_tick();
        exit::note_tick(tick);
        if self.args.panic_at_tick == Some(tick.0) {
            panic!("deliberate panic at {tick} (--panic-at-tick)");
        }
        let command = match self.script.as_mut() {
            Some(script) => script.next().unwrap_or(PlayerCommand::idle(tick)),
            None => live,
        };
        let command = match command_for(tick, command) {
            Ok(command) => command,
            Err(message) => return self.leave(exit::usage(self.game.name, &message)),
        };
        self.previous
            .restore(&self.session.sim().snapshot())
            .unwrap_or_else(|e| panic!("[E-SESSION] keeping the previous state at {tick}: {e}"));
        let recorded = self.args.record.is_some().then_some(&mut self.recorded);
        match self.session.tick(command, recorded) {
            Ok(step_ns) => self.tick_ns.push(step_ns),
            Err(TickStop::Limit(message)) => {
                self.budgets.exceeded.get_or_insert(message);
                self.done = true;
                return Step::Done;
            }
            Err(TickStop::Desync(message)) => return self.leave(fail(Exit::Desync, &message)),
        }
        let sim = self.session.sim();
        self.hashes.push(TickHash {
            tick,
            hash: hash(sim),
        });
        self.budgets.observe(
            tick.0,
            sim.state_len() as u64,
            sim.world().entity_count() as u64,
        );
        if tick.0 >= self.args.ticks {
            self.done = true;
            return Step::Done;
        }
        Step::Next
    }

    fn leave(&mut self, exit: Exit) -> Step {
        self.exit = Some(exit);
        self.done = true;
        Step::Done
    }

    /// The game's top-down view, if it has one.
    pub fn top_down(&self) -> Option<fn(&TopDownFrame<'_>) -> TopDownScene> {
        self.game.top_down
    }

    /// What a frame `alpha` of the way from the previous tick to the latest
    /// one shows, for an image of `width` by `height` pixels.
    pub fn frame(&self, alpha: f64, width: u32, height: u32) -> TopDownFrame<'_> {
        TopDownFrame {
            previous: &self.previous,
            current: self.session.sim(),
            alpha,
            width,
            height,
        }
    }

    /// Keeps a presented frame's counters for the report's `per_frame`
    /// peaks.
    pub fn observe_frame(&mut self, counters: &FrameCounters) {
        let tick = self.session.sim().tick().0;
        self.budgets.observe_frame(tick, counters);
    }

    /// Counts `ticks` of real time the frame loop dropped (it runs at most a
    /// few ticks per wake) and logs `[W-TICK-DROP]`. The simulation never
    /// skips a tick: the run just falls behind the clock.
    pub fn dropped(&mut self, ticks: u64) {
        self.dropped += ticks;
        log::line(
            "warn",
            &format!(
                "[W-TICK-DROP] {ticks} tick(s) dropped before {}: the frame came too late to \
                 catch up",
                self.next_tick()
            ),
        );
    }

    /// Ticks dropped so far.
    pub fn dropped_ticks(&self) -> u64 {
        self.dropped
    }

    /// Ends a run that could not open its window or GPU: prints `message`
    /// (with its tag, such as `[E-NO-ADAPTER]`) and exits 64, writing
    /// nothing, as `shot` does (plan decision 4).
    pub fn refuse(self, message: &str) -> Exit {
        fail(Exit::Usage, message)
    }

    /// Ends the run: writes `--hashes`, `--record` and `--report`, prints the
    /// summary and returns the exit. A tick that left with an exit (a
    /// desync) writes nothing, as in `run`.
    pub fn finish(self, stats: PlayStats) -> Exit {
        let Self {
            game,
            args,
            session,
            hashes,
            recorded,
            tick_ns,
            budgets,
            dropped,
            exit,
            ..
        } = self;
        if let Some(exit) = exit {
            return exit;
        }
        let sim = session.sim();
        log::line("info", &format!("play ended at {}", sim.tick()));
        let stopped = budgets.exceeded.is_some();
        let exit = if stopped {
            Exit::BudgetExceeded
        } else {
            Exit::Ok
        };
        if let Err(message) = write_outputs(game, &args, sim, &hashes, recorded, None, stopped) {
            return exit::usage(game.name, &message);
        }
        let frames = stats.gpu.map(|gpu| CaptureSummary {
            report: gpu.report,
            width: gpu.width,
            height: gpu.height,
            frames: gpu.frames,
            out: None,
            surface_format: Some(gpu.surface_format),
        });
        let summary = report::RunSummary {
            game,
            args: &args,
            sim,
            hashes: &hashes,
            tick_ns,
            events: 0,
            trackers: &[],
            budgets: &budgets,
            frames: frames.as_ref(),
            platform: stats.platform,
            dropped_ticks: dropped,
            exit,
        };
        if let Some(path) = &args.report
            && let Err(message) = write_file(path, summary.to_json().to_pretty().as_bytes())
        {
            return exit::usage(game.name, &message);
        }
        for line in summary.console_lines() {
            println!("{line}");
            log::line("info", &line);
        }
        exit
    }
}

#[cfg(test)]
mod tests {
    use eng_sim::{ScheduleError, Sim, SimBuilder, SimConfig};

    use super::PlayRun;
    use crate::GameDef;

    fn build(config: SimConfig) -> Result<Sim, ScheduleError> {
        SimBuilder::new(config).build()
    }

    #[test]
    fn dropped_ticks_are_counted_and_logged() {
        let game = GameDef {
            name: "empty",
            version: "0",
            build_sha: crate::UNKNOWN,
            tick_rate: 60,
            build,
            top_down: None,
            view_yaw: 0,
        };
        let mut run = PlayRun::start(&game, &["play".to_string()]).unwrap();
        run.dropped(3);
        run.dropped(2);
        assert_eq!(run.dropped_ticks(), 5);
        let logged: Vec<String> = crate::log::tail()
            .iter()
            .filter_map(|record| match record.get("msg") {
                Some(eng_schema_json::JsonValue::Str(message)) => Some(message.clone()),
                _ => None,
            })
            .collect();
        for count in [3, 2] {
            let line = format!("[W-TICK-DROP] {count} tick(s) dropped before tick 1");
            assert!(
                logged.iter().any(|m| m.starts_with(&line)),
                "{line:?} in {logged:#?}"
            );
        }
    }
}
