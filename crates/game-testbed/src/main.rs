//! The testbed game binary: the probe command set over the testbed sim.

use std::process::ExitCode;

static GAME: eng_cli::GameDef = eng_cli::GameDef {
    name: "game-testbed",
    version: env!("CARGO_PKG_VERSION"),
    // The testbed lives in the engine repo, so its commit is the engine's.
    build_sha: eng_cli::ENGINE_SHA,
    tick_rate: game_testbed_sim::TICK_RATE,
    build: game_testbed_sim::build,
};

fn main() -> ExitCode {
    eng_cli::main(&GAME)
}
