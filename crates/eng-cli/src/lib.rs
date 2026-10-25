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
//! <game> --version [--json]
//! ```
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
mod exit;
mod log;
mod replay;
mod report;
mod run;
mod version;

use std::process::ExitCode;

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
