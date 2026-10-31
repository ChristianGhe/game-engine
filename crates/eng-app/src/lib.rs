//! The app (spec 6.1, 6.6): the frame loop, and the composition of the
//! platform, the in-process session, live input and the renderer. It runs
//! `play`, and a game binary enters through it:
//!
//! ```text
//! fn main() -> ExitCode { eng_app::main(&GAME) }
//! ```
//!
//! [`main`] is eng-cli's probe (every command, the panic hook and
//! `crash.json`, which stays in eng-cli: plan difference 9) plus `play`:
//!
//! ```text
//! <game> play [--ticks N] [--input script.toml] [--bindings bindings.toml] [--seed S]
//!             [--hashes hashes.txt] [--record out.rec] [--report report.json]
//! ```
//!
//! With no flags `play` is the game: a resizable window titled with the
//! game's name, the keyboard (WASD and the arrows move, Esc quits) and the
//! game's top-down view. `--ticks N` closes it after tick N. With `--input`
//! the commands come from the script and only `quit` is read from the
//! keyboard, so `play --input` gives `run`'s hash stream tick for tick
//! (acceptance `native-window`).
//!
//! # One frame (plan, "One frame")
//!
//! The platform (`eng-platform-winit`, or the headless one in tests) calls
//! the frame loop back:
//!
//! - **Ticks come from the timer** ([`TickClock`]): a fixed-timestep
//!   accumulator on the platform's clock, woken with `Wake::At` the next
//!   tick. A wake runs at most [`MAX_TICKS_PER_WAKE`] ticks; the rest are
//!   dropped, counted in the report's `dropped_ticks` and logged as
//!   `[W-TICK-DROP]`. The simulation never skips a tick, it falls behind.
//! - **Input** is sampled once per tick from the keys held, with a latch so a
//!   tap shorter than a tick counts (`eng-input`'s `LiveInput`); losing
//!   focus releases every key. `quit` and a close request end the run, exit
//!   0, and never reach the simulation (plan decision 8).
//! - **Each tick** goes through eng-cli's [`PlayRun`]: the same session over
//!   Memory, tick, hash stream, recording and report as `run`.
//! - **Drawing**: the game's extraction builds a top-down scene from the
//!   previous and newest states at the clock's alpha (interpolation), and
//!   eng-render draws it through the three passes into the window's image,
//!   presented after the submit (Fifo, plan decision 7). Positions are
//!   camera-relative, subtracted in f64. Nothing is acquired while the
//!   window is minimised or occluded; ticks go on.
//! - **Stop** writes the files and the report, so they are saved even where
//!   a desktop's quit command ends the process right after it (macOS).
//!
//! Without a display `play` exits 64 with `[E-NO-DISPLAY]`; without a GPU
//! adapter, 64 with `[E-NO-ADAPTER]` (plan decision 4); both before the
//! first tick, writing nothing. [`play_on`] runs the same loop on any
//! platform: tests run it headless, with scripted keys (the headless twin).
#![forbid(unsafe_code)]

mod clock;
mod frame;
mod view;

use std::process::ExitCode;

use eng_cli::{Exit, GameDef, PlayRun};
use eng_platform_api::{Platform, PlatformError};
use eng_platform_winit::WinitPlatform;

pub use clock::{Due, MAX_TICKS_PER_WAKE, TickClock};

use crate::frame::Frames;

/// Runs the probe with the process arguments, `play` included.
pub fn main(game: &GameDef) -> ExitCode {
    eng_cli::main_with_play(game, &play_in_window)
}

/// Runs the probe with explicit arguments (without the program name).
pub fn run_args(game: &GameDef, args: &[String]) -> Exit {
    eng_cli::run_args_with_play(game, args, &play_in_window)
}

/// `play` in a window: the desktop platform, titled with the game's name.
fn play_in_window(run: PlayRun<'_>) -> Exit {
    let game = run.game();
    if game.top_down.is_none() {
        return run.refuse(&format!(
            "[E-USAGE] {} has no top-down view to show (GameDef::top_down)",
            game.name
        ));
    }
    match WinitPlatform::new(game.name) {
        Ok(mut platform) => play_on(&mut platform, run),
        Err(e) => run.refuse(&format!("[E-NO-DISPLAY] {e}")),
    }
}

/// Runs `play`'s frame loop on `platform` until the run ends: its last tick,
/// `quit`, a close request, or the platform's own end. Returns the run's
/// exit.
///
/// # Panics
/// If the platform fails while running (`[E-PLATFORM]`).
pub fn play_on(platform: &mut dyn Platform, run: PlayRun<'_>) -> Exit {
    let name = platform.name();
    let mut frames = Frames::new(run, name);
    let result = platform.run(&mut frames);
    match (result, frames.exit) {
        (Ok(()), Some(exit)) => exit,
        (Err(PlatformError::Unavailable(why)), None) => frames
            .into_unfinished()
            .expect("a run that never started still has its run")
            .refuse(&format!(
                "[E-NO-DISPLAY] the {name} platform cannot run here: {why}"
            )),
        (Ok(()), None) => panic!("[E-PLATFORM] the {name} platform returned without stop"),
        (Err(e), _) => panic!("[E-PLATFORM] the {name} platform failed: {e}"),
    }
}
