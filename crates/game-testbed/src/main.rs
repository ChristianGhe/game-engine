//! The testbed game binary: the probe command set and `play` over the
//! testbed sim. It enters through eng-app (rung 1b plan, difference 6); its
//! presentation is `game-testbed-client`'s.

use std::process::ExitCode;

static GAME: eng_cli::GameDef = eng_cli::GameDef {
    name: "game-testbed",
    version: env!("CARGO_PKG_VERSION"),
    // The testbed lives in the engine repo, so its commit is the engine's.
    build_sha: eng_cli::ENGINE_SHA,
    tick_rate: game_testbed_sim::TICK_RATE,
    build: game_testbed_sim::build,
    top_down: Some(game_testbed_client::top_down::scene),
    // +Y, up the top-down view: W moves the dot up the window, D right.
    view_yaw: 0x4000,
};

/// The testbed's maps (rung 2 plan, decision 1): `dots`, the dot room
/// `GAME.build` builds, with no sim data and the top-down view, and
/// `room`, the first-person hall (`game_testbed_sim::room`), in first
/// person (`game_testbed_client::ROOM`). `--map` defaults to `dots`, so
/// every 1b command line means what it did.
static MAPS: eng_cli::GameMaps = eng_cli::GameMaps {
    first: "dots",
    more: &[eng_cli::GameMap {
        name: "room",
        build: game_testbed_sim::room::build,
        view: eng_cli::MapView::FirstPerson(game_testbed_client::ROOM),
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

    /// The dots are drawn top-down and the room in first person.
    #[test]
    fn each_map_names_its_view() {
        let views: Vec<_> = MAPS.all(&GAME).iter().map(|m| m.view).collect();
        assert!(matches!(views[0], eng_cli::MapView::TopDown));
        assert!(matches!(views[1], eng_cli::MapView::FirstPerson(_)));
    }
}
