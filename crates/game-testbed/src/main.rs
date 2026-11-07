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
    // +Y, up the top-down view: W moves the dot up the window, D right.
    view_yaw: 0x4000,
};

/// The testbed's maps (rung 2 plan, decision 1): `dots`, the dot room
/// `GAME.build` builds, with no sim data, and `room`, the first-person hall
/// (`game_testbed_sim::room`). `--map` defaults to `dots`, so every 1b
/// command line means what it did.
static MAPS: eng_cli::GameMaps = eng_cli::GameMaps {
    first: "dots",
    more: &[eng_cli::GameMap {
        name: "room",
        build: game_testbed_sim::room::build,
    }],
};

fn main() -> ExitCode {
    eng_app::main_with_maps(&GAME, &MAPS)
}

#[cfg(test)]
mod tests {
    use super::{GAME, MAPS};

    /// `replay verify` builds every map for a recording's player count and
    /// seed, so each map must build, and keep the maps' rules, for any
    /// count `--players` takes (1 to 255) and any seed.
    #[test]
    fn every_map_builds_and_keeps_the_rules_for_any_players_and_seed() {
        let names: Vec<&str> = MAPS.all(&GAME).iter().map(|m| m.name).collect();
        assert_eq!(names, ["dots", "room"]);
        for players in [1, 2, 3, 4, 5, 10, 16, 255] {
            for seed in [0, 1, 2, 7, 0x5eed, u64::MAX] {
                assert_eq!(MAPS.check(&GAME, players, seed), Ok(()), "{players} {seed}");
            }
        }
    }
}
