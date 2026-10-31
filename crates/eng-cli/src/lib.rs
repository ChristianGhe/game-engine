//! The probe command set (spec 7.1), linked by every game binary:
//!
//! ```text
//! <game> run --headless --ticks N [--seed S] [--players P] [--input script.toml]
//!            [--assert assertions.toml] [--budgets budgets.toml]
//!            [--report report.json] [--events events.jsonl]
//!            [--hashes hashes.txt] [--hash-every N] [--record out.rec]
//! <game> replay verify <recording> [--report report.json]
//! <game> replay dump <recording> [--out recording.json]
//! <game> dump-world --at-tick N --out world.json [--seed S] [--players P] [--input script.toml]
//! <game> shot --at-tick N --out shot.png [--size 320x240] [--seed S] [--players P]
//!             [--input script.toml] [--budgets budgets.toml] [--report report.json]
//! <game> video --ticks N --out proof.apng [--every K] [--size 320x240] [--seed S] [--players P]
//!              [--input script.toml] [--budgets budgets.toml] [--report report.json]
//! <game> --version [--json]
//! ```
//!
//! `run`, `dump-world`, `shot` and `video` go through `eng-session` like
//! every other mode: a server and player 0's client in-process over Memory,
//! every message encoded to bytes. `replay verify` steps the sim directly
//! from the recorded commands, the ones the server applied.
//!
//! `shot` and `video` run like `run` and render the game's top-down view
//! ([`GameDef::top_down`]) offscreen through eng-render, from a tick's state
//! at alpha 1, so the pictures are exact. `shot` writes the frame at its
//! tick as a PNG; `video` writes tick 0 and every K-th tick after it (the
//! last multiple of K not past N) as an
//! animated PNG (the proof video, spec 7.1), each frame shown for K ticks.
//! The files hold 8-bit sRGB RGBA and nothing else: the same pixels give
//! the same bytes. Their reports carry a `gpu` object (the adapter, its
//! capabilities and the per-frame counters), and `budgets.toml [gpu]` holds
//! every frame (exit 13, `[E-BUDGET]`). Without an adapter they exit 64
//! with `[E-NO-ADAPTER]`, writing nothing (plan decision 4); a frame that
//! fails once the GPU is open is a bug and panics (exit 12).
//!
//! `--version` prints the version stamp (spec 6.13) on one line and
//! `--version --json` prints all of it; see the `version` module and
//! `docs/ARCHITECTURE.md`, "The version stamp".
//!
//! The console gets a summary of at most about 20 lines; the full log goes
//! to `target/agent-logs/` (or `$ENG_AGENT_LOG_DIR`) as JSON lines with
//! greppable tags such as `[E-DESYNC]`. A panic also writes
//! `crash-<game>-<pid>.json` there (version, thread, backtrace, last log
//! lines, seed, tick) and exits 12. Exit codes are listed on [`Exit`].
#![forbid(unsafe_code)]

mod args;
mod assertions;
mod budgets;
mod capture;
mod exit;
mod image_file;
mod log;
mod replay;
mod report;
mod run;
mod session;
mod ticker;
mod version;

use std::process::ExitCode;

use eng_render::TopDownScene;
use eng_sim::{ScheduleError, Sim, SimConfig};

pub use exit::Exit;
pub use version::{ENGINE_SHA, ENGINE_VERSION, PROFILE, RUSTC_VERSION, TARGET, UNKNOWN};

/// What a game binary tells the probe about itself.
pub struct GameDef {
    pub name: &'static str,
    pub version: &'static str,
    /// The game's source commit, or [`UNKNOWN`] outside `cargo xtask` builds.
    pub build_sha: &'static str,
    pub tick_rate: u32,
    pub build: fn(SimConfig) -> Result<Sim, ScheduleError>,
    /// The game's extraction: what its top-down view shows in a frame
    /// (`shot`, `video`, and `play`'s window). `None` for a game without
    /// one, where `shot` and `video` exit 64.
    pub top_down: Option<fn(&TopDownFrame<'_>) -> TopDownScene>,
}

/// What an extraction builds one frame from (plan, "One frame"). It reads
/// the states and never writes: nothing flows back into the simulation.
#[derive(Clone, Copy)]
pub struct TopDownFrame<'a> {
    /// The state a tick before `current`, or `current` itself.
    pub previous: &'a Sim,
    /// The newest state.
    pub current: &'a Sim,
    /// How far the frame is from `previous` (0) to `current` (1). `shot`
    /// and `video` draw a tick's state exactly: both states are the same
    /// and `alpha` is 1.
    pub alpha: f64,
    /// The image's size in pixels.
    pub width: u32,
    pub height: u32,
}

/// Runs the probe with the process arguments.
pub fn main(game: &GameDef) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_args(game, &args).into()
}

/// Runs the probe with explicit arguments (without the program name).
pub fn run_args(game: &GameDef, args: &[String]) -> Exit {
    exit::install_panic_hook(game);
    exit::begin(args);
    let command = match args::parse(args) {
        Ok(command) => command,
        Err(message) => return exit::usage(game.name, &message),
    };
    log::open(game.name, command.name());
    log::line("info", &format!("start: {} {}", game.name, args.join(" ")));
    match command {
        args::Command::Version { json } => version::print(game, json),
        args::Command::Help => {
            println!("{}", args::USAGE.replace("<game>", game.name));
            Exit::Ok
        }
        args::Command::Run(run_args) => run::run(game, &run_args),
        args::Command::ReplayVerify { recording, report } => {
            replay::verify(game, &recording, report.as_deref())
        }
        args::Command::ReplayDump { recording, out } => {
            replay::dump(game, &recording, out.as_deref())
        }
    }
}
