//! A game's maps (rung 2 plan, decision 1; ADR-0023).
//!
//! A map is a build function under a name. A game names its maps in a
//! [`GameMaps`] beside its [`GameDef`]: the first is [`GameDef::build`], the
//! others are [`GameMap`]s. `--map NAME` picks one for `run`, `dump-world`,
//! `shot`, `video` and `play` (the first by default; an unknown name exits
//! 64 with `[E-MAP]`), and `replay verify` picks the one whose sim-data
//! hash is the recording's. `--version` describes the first map only.
//!
//! The rules, which make a recording's `sim_data_hash` name its map:
//! - only the first map may lack sim data (constant resources);
//! - no two maps share a sim-data hash;
//! - names are unique, and lowercase letters, digits, `-` and `_`, so that
//!   they fit a hash stream's header as `map=<name>`.
//!
//! A map that breaks a rule is a bug in the game: it panics with `[E-MAP]`
//! (exit 12) where the probe meets it, and [`GameMaps::check`] lets a
//! game's tests find it first.
//!
//! Each map also names its view ([`MapView`], rung 2's T12): the first map
//! is drawn by [`GameDef::top_down`]; another names its own, which `shot`
//! and `video` draw, or none, which they refuse.

use eng_sim::{ScheduleError, Sim, SimConfig};

use crate::view::MapView;
use crate::{GameDef, run};

/// What builds a map's simulation for a session: [`GameDef::build`]'s type.
pub type BuildFn = fn(SimConfig) -> Result<Sim, ScheduleError>;

/// One of a game's maps after its first: its name for `--map`, its build
/// function and its view. Its sim must have sim data.
#[derive(Debug, Clone, Copy)]
pub struct GameMap {
    pub name: &'static str,
    pub build: BuildFn,
    /// What `shot` and `video` draw of it. The first map's is
    /// [`MapView::TopDown`], [`GameDef::top_down`].
    pub view: MapView,
}

/// A game's maps, beside its [`GameDef`] (whose fields stay as they are).
///
/// ```text
/// static MAPS: GameMaps = GameMaps {
///     first: "dots",
///     more: &[GameMap { name: "room", build: room::build, view: MapView::FirstPerson(ROOM) }],
/// };
/// fn main() -> ExitCode { eng_app::main_with_maps(&GAME, &MAPS) }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct GameMaps {
    /// The first map's name. [`GameDef::build`] builds it: it is `--map`'s
    /// default, the map `--version` describes, and the only one that may
    /// lack sim data (a 1b recording, sim-data hash 0, replays on it).
    pub first: &'static str,
    /// The other maps, in the order `--map`'s message lists them.
    pub more: &'static [GameMap],
}

/// The first map's name for a game that names no maps: the entry points
/// without a [`GameMaps`] (`main`, `run_args`, `PlayRun::start`) use
/// [`GameMaps::FIRST_ONLY`].
pub const DEFAULT_MAP: &str = "default";

impl GameMaps {
    /// [`GameDef::build`] alone, as [`DEFAULT_MAP`].
    pub const FIRST_ONLY: GameMaps = GameMaps {
        first: DEFAULT_MAP,
        more: &[],
    };

    /// The first map: [`GameDef::build`] under [`GameMaps::first`], drawn
    /// by [`GameDef::top_down`].
    pub fn first(&self, game: &GameDef) -> GameMap {
        GameMap {
            name: self.first,
            build: game.build,
            view: MapView::TopDown,
        }
    }

    /// Every map, the first first.
    pub fn all(&self, game: &GameDef) -> Vec<GameMap> {
        let mut maps = vec![self.first(game)];
        maps.extend_from_slice(self.more);
        maps
    }

    /// The map named `name`, if the game has one.
    pub fn find(&self, game: &GameDef, name: &str) -> Option<GameMap> {
        self.all(game).into_iter().find(|map| map.name == name)
    }

    /// Checks every rule above for sessions of `players` and `seed`,
    /// building each map once. The error is the `[E-MAP]` message.
    pub fn check(&self, game: &GameDef, players: u8, seed: u64) -> Result<(), String> {
        self.build_all(game, players, seed).map(|_| ())
    }

    /// The names' rules, which need no sim.
    fn check_names(&self, game: &GameDef) -> Result<(), String> {
        let names: Vec<&str> = self.all(game).iter().map(|map| map.name).collect();
        for (i, name) in names.iter().enumerate() {
            let valid = !name.is_empty()
                && name.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'
                });
            if !valid {
                return Err(format!(
                    "[E-MAP] {}'s map name {name:?} must be lowercase letters, digits, '-' \
                     and '_'",
                    game.name
                ));
            }
            if names[..i].contains(name) {
                return Err(format!("[E-MAP] {} has two maps named {name:?}", game.name));
            }
        }
        Ok(())
    }

    /// Builds every map at tick 0, first first, and checks the rules.
    pub(crate) fn build_all(
        &self,
        game: &GameDef,
        players: u8,
        seed: u64,
    ) -> Result<Vec<(MapChoice, Sim)>, String> {
        self.check_names(game)?;
        let mut built: Vec<(MapChoice, Sim)> = Vec::new();
        for map in self.all(game) {
            let choice = MapChoice {
                map,
                first: self.first,
            };
            let sim = run::build_sim(game, map.build, players, seed);
            choice.check(game, &sim)?;
            if let Some((other, _)) = built
                .iter()
                .find(|(_, s)| s.sim_data_hash() == sim.sim_data_hash())
            {
                return Err(format!(
                    "[E-MAP] {}'s maps {} and {} have the same sim-data hash {:#018x}: a \
                     recording's hash must name one map",
                    game.name,
                    other.map.name,
                    map.name,
                    sim.sim_data_hash()
                ));
            }
            built.push((choice, sim));
        }
        Ok(built)
    }

    fn names(&self, game: &GameDef) -> String {
        let names: Vec<&str> = self.all(game).iter().map(|map| map.name).collect();
        names.join(", ")
    }
}

/// The map a command runs, and the name of the game's first map.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MapChoice {
    pub map: GameMap,
    /// [`GameMaps::first`].
    pub first: &'static str,
}

impl MapChoice {
    pub fn name(&self) -> &'static str {
        self.map.name
    }

    /// Builds the map's sim at tick 0 and tells the panic hook its seed.
    ///
    /// # Panics
    /// With `[E-MAP]` if a map other than the first has no sim data.
    pub fn build(&self, game: &GameDef, players: u8, seed: u64) -> Sim {
        let sim = run::build_sim(game, self.map.build, players, seed);
        self.check(game, &sim).unwrap_or_else(|e| panic!("{e}"));
        sim
    }

    /// The map's view.
    pub fn view(&self) -> MapView {
        self.map.view
    }

    /// Why `shot` or `video` cannot draw this map, if they cannot: it has
    /// no view ([`MapView::None`]). A usage error, exit 64, refused before
    /// a GPU opens.
    pub fn no_view(&self, game: &GameDef) -> Option<String> {
        self.map.view.refusal(game, self.map.name)
    }

    /// Why `play`'s window cannot show this map yet, if it cannot: it
    /// draws [`GameDef::top_down`] only, until the first-person window
    /// (rung 2, T15). A usage error, exit 64, refused before a window
    /// opens.
    pub fn no_window_view(&self, game: &GameDef) -> Option<String> {
        self.map.view.window_refusal(game, self.map.name)
    }

    fn check(&self, game: &GameDef, sim: &Sim) -> Result<(), String> {
        if self.map.name == self.first || sim.has_sim_data() {
            return Ok(());
        }
        Err(format!(
            "[E-MAP] {}'s map {} has no sim data: only the first map ({}) may lack it, so \
             that a recording's sim-data hash names its map",
            game.name, self.map.name, self.first
        ))
    }
}

/// The map `--map` names, or the first without one. An unknown name is a
/// usage error, `[E-MAP]`, listing the game's maps.
///
/// # Panics
/// With `[E-MAP]` if the game's map names break the rules.
pub(crate) fn select(
    game: &GameDef,
    maps: &GameMaps,
    name: Option<&str>,
) -> Result<MapChoice, String> {
    maps.check_names(game).unwrap_or_else(|e| panic!("{e}"));
    let name = name.unwrap_or(maps.first);
    match maps.find(game, name) {
        Some(map) => Ok(MapChoice {
            map,
            first: maps.first,
        }),
        None => Err(format!(
            "[E-MAP] unknown map {name:?}; {} has {}",
            game.name,
            maps.names(game)
        )),
    }
}

#[cfg(test)]
mod tests;
