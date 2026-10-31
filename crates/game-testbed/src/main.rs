//! The testbed game binary: the probe command set and `play` over the
//! testbed sim. It enters through eng-app (rung 1b plan, difference 6).

use std::process::ExitCode;

mod top_down;

static GAME: eng_cli::GameDef = eng_cli::GameDef {
    name: "game-testbed",
    version: env!("CARGO_PKG_VERSION"),
    // The testbed lives in the engine repo, so its commit is the engine's.
    build_sha: eng_cli::ENGINE_SHA,
    tick_rate: game_testbed_sim::TICK_RATE,
    build: game_testbed_sim::build,
    top_down: Some(top_down::scene),
};

fn main() -> ExitCode {
    eng_app::main(&GAME)
}
