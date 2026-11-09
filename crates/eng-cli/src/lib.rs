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
//!             [--view top-down|first-person] [--pose X,Y,Z,YAW,PITCH] [--look look.toml]
//! <game> video --ticks N --out proof.apng [--every K] [--size 320x240] [--seed S] [--players P]
//!              [--input script.toml] [--budgets budgets.toml] [--report report.json]
//!              [--view top-down|first-person] [--pose X,Y,Z,YAW,PITCH] [--look look.toml]
//! <game> play [--ticks N] [--input script.toml] [--bindings bindings.toml] [--seed S]
//!             [--hashes hashes.txt] [--record out.rec] [--report report.json]
//!             [--windowed] [--fov DEG] [--look look.toml]
//!             [--sensitivity DEG_PER_COUNT] [--pitch-limit DEG] [--invert-pitch]
//! <game> --version [--json]
//! ```
//!
//! `run`, `dump-world`, `shot`, `video` and `play` also take `--map NAME`:
//! a game names its maps in a [`GameMaps`] beside its [`GameDef`], the first
//! being [`GameDef::build`] and the default (rung 2 plan, decision 1;
//! ADR-0023). An unknown name exits 64 with `[E-MAP]`. A map with sim data
//! (constant resources) puts its name and sim-data hash in the hash
//! stream's header and in `dump-world`'s file; every run's report names its
//! `map`, `sim_protocol` and `sim_data_hash`, and `files_read` gives the
//! format version of each file it read. Recordings carry the sim-data hash
//! in their header, and `replay verify` replays on the map it names. Each
//! map names its view ([`MapView`]): the first map's is
//! [`GameDef::top_down`], and another's may be first person
//! ([`FirstPersonView`], rung 2's T12) or none. `shot`, `video` and `play`
//! refuse a map with none before a GPU or window opens (exit 64,
//! `[E-MAP]`). A map has exactly the one view it names: rung 2's `--view`
//! (T16) may name that view and refuses any other the same way.
//!
//! `run`, `dump-world`, `shot` and `video` go through `eng-session` like
//! every other mode: a server and player 0's client in-process over Memory,
//! every message encoded to bytes. `replay verify` steps the sim directly
//! from the recorded commands, the ones the server applied.
//!
//! `shot` and `video` run like `run` and render the map's view (the top-down
//! [`GameDef::top_down`], or a [`FirstPersonView`]) offscreen through
//! eng-render, from a tick's state at alpha 1, so the pictures are exact.
//! A first-person view loads its world once, before the first frame, and
//! the game's scene picks the camera. `shot` writes the frame at its tick
//! as a PNG; `video` writes tick 0 and every K-th tick after it (the
//! last multiple of K not past N) as an animated PNG (the proof video,
//! spec 7.1), each frame shown for K ticks. The files hold 8-bit sRGB RGBA
//! and nothing else: the same pixels give the same bytes. Their reports
//! carry a `gpu` object (the adapter, its capabilities and the per-frame
//! counters), and the budgets file's `[gpu]` table, when it has one, holds
//! every frame (exit 13, `[E-BUDGET]`), or its `[gpu.first_person]` table
//! each first-person frame. `--view` names the view the map must have
//! (refused before a GPU opens if it lacks it), and for a first-person view
//! `--pose` sets a camera independent of movement ([`Pose`], plan addition
//! A3) and `--look` a look file; the report's `view` names the view, the
//! pose, the look and the last frame's camera. Without an adapter they exit 64
//! with `[E-NO-ADAPTER]`, writing nothing (plan decision 4); a frame that
//! fails once the GPU is open is a bug and panics (exit 12). Golden tests
//! compare a shot's decoded pixels, not its file's bytes, and read the
//! report's `gpu` object to know which adapter drew it (the testbed's
//! `tests/shots.rs`).
//!
//! `play` is the game: a window, live input and Esc to quit. Its frame loop
//! is eng-app's ([`main_with_play`]); eng-cli parses its flags and gives it
//! a [`PlayRun`], the same session, tick, files and report as `run`, so
//! `play --input` gives `run`'s hash stream tick for tick. Its report adds
//! the platform, the ticks the frame loop dropped and the window's `gpu`
//! object. On a first-person map (rung 2, T15) `play` takes the
//! first-person flags (`--windowed`, `--fov`, `--look`, `--sensitivity`,
//! `--pitch-limit`, `--invert-pitch`: first-person maps only), checked
//! before the window opens and refused on any other map
//! ([`FirstPersonPlay`]), and its report adds a `window` object:
//! fullscreen, the cursor grab, the refresh rate and, on macOS, the
//! GCMouse shim's callback rate ([`PlayWindow`]).
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
mod look_file;
mod maps;
mod play;
mod replay;
mod report;
mod run;
mod session;
mod shot_view;
mod ticker;
mod version;
mod view;

use std::process::ExitCode;

use eng_render::TopDownScene;
use eng_sim::{ScheduleError, Sim, SimConfig};

pub use exit::Exit;
pub use look_file::LookSwitches;
pub use maps::{BuildFn, DEFAULT_MAP, GameMap, GameMaps};
pub use play::{
    FirstPersonPlay, FrameLens, MouseRate, PlayGpu, PlayRun, PlayStats, PlayWindow, Step,
};
pub use shot_view::{Pose, degrees_to_bangle};
pub use version::{ENGINE_SHA, ENGINE_VERSION, PROFILE, RUSTC_VERSION, TARGET, UNKNOWN};
pub use view::{FirstPersonFrame, FirstPersonView, FirstPersonWorld, MapView, WorldTexture};

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
    /// The view yaw `play` holds for live input, as a `PlayerCommand`
    /// carries it (65,536 to a turn; 0 looks along +X, 0x4000 along +Y).
    /// Movement goes along it: the top-down view draws +Y up the image, so
    /// at 0x4000 forward (W, the up arrow) moves up the view and right (D,
    /// the right arrow) to its right. A script's commands carry their own.
    pub view_yaw: u16,
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

/// What runs `play`: eng-app's frame loop, given the run eng-cli built.
pub type Play<'a> = &'a dyn Fn(PlayRun<'_>) -> Exit;

/// Runs the probe with the process arguments, for a game with one map
/// ([`GameMaps::FIRST_ONLY`]). `play` exits 64 here: a game gets its
/// window through eng-app ([`main_with_play`]).
pub fn main(game: &GameDef) -> ExitCode {
    main_with_maps(game, &GameMaps::FIRST_ONLY, None)
}

/// Runs the probe with explicit arguments (without the program name).
pub fn run_args(game: &GameDef, args: &[String]) -> Exit {
    run_args_with_maps(game, &GameMaps::FIRST_ONLY, args, None)
}

/// [`main`] with `play`: eng-cli parses the command line, installs the
/// panic hook and builds the [`PlayRun`], and `play` runs it in a window.
pub fn main_with_play(game: &GameDef, play: Play<'_>) -> ExitCode {
    main_with_maps(game, &GameMaps::FIRST_ONLY, Some(play))
}

/// [`run_args`] with `play`, as [`main_with_play`].
pub fn run_args_with_play(game: &GameDef, args: &[String], play: Play<'_>) -> Exit {
    run_args_with_maps(game, &GameMaps::FIRST_ONLY, args, Some(play))
}

/// [`main`] for a game with `maps` (ADR-0023): `--map` picks one, and
/// `replay verify` the one a recording's sim-data hash names. With
/// `play`, as [`main_with_play`]; without, `play` exits 64.
pub fn main_with_maps(game: &GameDef, maps: &GameMaps, play: Option<Play<'_>>) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_args_with_maps(game, maps, &args, play).into()
}

/// [`main_with_maps`] with explicit arguments (without the program name).
pub fn run_args_with_maps(
    game: &GameDef,
    maps: &GameMaps,
    args: &[String],
    play: Option<Play<'_>>,
) -> Exit {
    exit::install_panic_hook(game);
    exit::begin(args);
    let command = match args::parse(args) {
        Ok(command) => command,
        Err(message) => return exit::usage(game.name, &message),
    };
    log::open(game.name, command.name());
    log::line("info", &format!("start: {} {}", game.name, args.join(" ")));
    dispatch(game, maps, command, play)
}

/// Runs a parsed command. Unit tests call it without the panic hook and
/// the log file.
fn dispatch(
    game: &GameDef,
    maps: &GameMaps,
    command: args::Command,
    play: Option<Play<'_>>,
) -> Exit {
    match command {
        args::Command::Version { json } => version::print(game, json),
        args::Command::Help => {
            println!("{}", args::USAGE.replace("<game>", game.name));
            Exit::Ok
        }
        args::Command::Run(run_args) => {
            let map = match maps::select(game, maps, run_args.map.as_deref()) {
                Ok(map) => map,
                Err(message) => return exit::usage(game.name, &message),
            };
            match (run_args.play.is_some(), play) {
                (false, _) => run::run(game, map, &run_args),
                (true, Some(play)) => match PlayRun::new(game, map, *run_args) {
                    Ok(run) => play(run),
                    Err(exit) => exit,
                },
                (true, None) => exit::usage(
                    game.name,
                    &format!(
                        "play needs a window, and {} links only the probe; a game gets one \
                         through eng-app",
                        game.name
                    ),
                ),
            }
        }
        args::Command::ReplayVerify { recording, report } => {
            replay::verify(game, maps, &recording, report.as_deref())
        }
        args::Command::ReplayDump { recording, out } => {
            replay::dump(game, &recording, out.as_deref())
        }
    }
}
