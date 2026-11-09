//! Unit tests for maps (ADR-0023): the rules, `--map`, and a map with sim
//! data through `run`, `dump-world` and `replay verify`. The testbed has no
//! such map until the room (rung 2, T7), so a small test game has one: its
//! first map, `plain`, has no sim data, and `walls` has 100 wall segments,
//! 4.8 kB, four times what a snapshot may hold.
//!
//! The commands run through `dispatch`, without the panic hook and the log
//! file, and write into a temporary directory.

use std::path::{Path, PathBuf};

use eng_core::PlayerCommand;
use eng_input::{read_recording, write_recording};
use eng_schema_core::Schema;
use eng_sim::{ScheduleError, Sim, SimBuilder, SimConfig, Stage, System, SystemFn, TickInputs};

use super::{GameMap, GameMaps, select};
use crate::view::MapView;
use crate::{Exit, GameDef, PlayRun, PlayStats, Step, UNKNOWN, args, dispatch, replay};

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Value", version = 1)]
struct Value(f32);

/// A wall segment: two endpoints and a z range.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Wall", version = 1)]
struct Wall {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    z0: f64,
    z1: f64,
}

#[derive(Debug, Clone, PartialEq, Schema)]
#[schema(name = "test.Walls", version = 1)]
struct Walls {
    walls: Vec<Wall>,
}

fn walls(n: usize) -> Walls {
    let walls = (0..n)
        .map(|i| Wall {
            x0: i as f64,
            y0: 0.0,
            x1: i as f64 + 1.0,
            y1: 0.0,
            z0: 0.0,
            z1: 4.0,
        })
        .collect();
    Walls { walls }
}

const GROW: SystemFn = |w| {
    let forward = f32::from(w.resource::<TickInputs>().commands[0].forward);
    w.resource_mut::<Value>().0 += forward;
};

/// Reads the sim data each tick, as a map's collision would.
const GROW_BY_WALLS: SystemFn = |w| {
    let count = w.constant::<Walls>().walls.len() as f32;
    let forward = f32::from(w.resource::<TickInputs>().commands[0].forward);
    w.resource_mut::<Value>().0 += forward * count;
};

fn sim(config: SimConfig, sim_data: Option<Walls>, system: SystemFn) -> Result<Sim, ScheduleError> {
    let mut b = SimBuilder::new(config);
    b.world_mut().insert_resource(Value(0.0));
    if let Some(data) = sim_data {
        b.world_mut().insert_constant(data);
    }
    b.add_system(System::new("test.grow", Stage::Update, system));
    b.build()
}

fn plain(config: SimConfig) -> Result<Sim, ScheduleError> {
    sim(config, None, GROW)
}

fn walled(config: SimConfig) -> Result<Sim, ScheduleError> {
    sim(config, Some(walls(100)), GROW_BY_WALLS)
}

fn walled_too(config: SimConfig) -> Result<Sim, ScheduleError> {
    sim(config, Some(walls(100)), GROW_BY_WALLS)
}

fn short(config: SimConfig) -> Result<Sim, ScheduleError> {
    sim(config, Some(walls(3)), GROW_BY_WALLS)
}

static GAME: GameDef = GameDef {
    name: "maps-test",
    version: "0",
    build_sha: UNKNOWN,
    tick_rate: 60,
    build: plain,
    top_down: None,
    view_yaw: 0,
};

static MAPS: GameMaps = GameMaps {
    first: "plain",
    more: &[
        GameMap {
            name: "walls",
            build: walled,
            view: MapView::None,
        },
        GameMap {
            name: "short",
            build: short,
            view: MapView::None,
        },
    ],
};

fn sim_data_hash(build: fn(SimConfig) -> Result<Sim, ScheduleError>) -> u64 {
    let config = SimConfig {
        tick_rate: 60,
        players: 1,
        seed: 0,
    };
    build(config).unwrap().sim_data_hash()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("eng-cli-maps-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs a command line, with `{dir}` in it replaced by `dir`.
fn probe(dir: &Path, line: &str) -> Exit {
    let args: Vec<String> = line
        .split_whitespace()
        .map(|word| word.replace("{dir}", &dir.display().to_string()))
        .collect();
    let command = args::parse(&args).unwrap_or_else(|e| panic!("{line}: {e}"));
    dispatch(&GAME, &MAPS, command, None)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_rules_hold_for_the_test_game_and_a_game_with_one_map() {
    assert_eq!(MAPS.check(&GAME, 1, 0), Ok(()));
    assert_eq!(MAPS.check(&GAME, 3, 9), Ok(()));
    assert_eq!(GameMaps::FIRST_ONLY.check(&GAME, 1, 0), Ok(()));
    let names: Vec<&str> = MAPS.all(&GAME).iter().map(|m| m.name).collect();
    assert_eq!(names, ["plain", "walls", "short"]);
    assert_eq!(MAPS.first(&GAME).name, "plain");
    assert!(MAPS.find(&GAME, "short").is_some());
    assert!(MAPS.find(&GAME, "nope").is_none());
}

#[test]
fn only_the_first_map_may_lack_sim_data() {
    let maps = GameMaps {
        first: "plain",
        more: &[GameMap {
            name: "bare",
            build: plain,
            view: MapView::None,
        }],
    };
    let err = maps.check(&GAME, 1, 0).unwrap_err();
    assert!(
        err.starts_with("[E-MAP] maps-test's map bare has no sim data: only the first map (plain)"),
        "{err}"
    );
    // The first map may have sim data too.
    let maps = GameMaps {
        first: "walls",
        more: &[],
    };
    let game = GameDef {
        build: walled,
        ..GAME
    };
    assert_eq!(maps.check(&game, 1, 0), Ok(()));
}

#[test]
#[should_panic(expected = "[E-MAP] maps-test's map bare has no sim data")]
fn running_a_later_map_without_sim_data_panics() {
    static BARE: GameMaps = GameMaps {
        first: "plain",
        more: &[GameMap {
            name: "bare",
            build: plain,
            view: MapView::None,
        }],
    };
    let map = select(&GAME, &BARE, Some("bare")).unwrap();
    map.build(&GAME, 1, 0);
}

#[test]
fn no_two_maps_share_a_sim_data_hash_or_a_name() {
    let twins = GameMaps {
        first: "plain",
        more: &[
            GameMap {
                name: "walls",
                build: walled,
                view: MapView::None,
            },
            GameMap {
                name: "walls-too",
                build: walled_too,
                view: MapView::None,
            },
        ],
    };
    let err = twins.check(&GAME, 1, 0).unwrap_err();
    let hash = format!("{:#018x}", sim_data_hash(walled));
    assert_eq!(
        err,
        format!(
            "[E-MAP] maps-test's maps walls and walls-too have the same sim-data hash {hash}: \
             a recording's hash must name one map"
        )
    );
    let same_name = GameMaps {
        first: "walls",
        more: &[GameMap {
            name: "walls",
            build: walled,
            view: MapView::None,
        }],
    };
    let err = same_name.check(&GAME, 1, 0).unwrap_err();
    assert_eq!(err, "[E-MAP] maps-test has two maps named \"walls\"");
    for bad in ["", "Room", "big room", "map=2"] {
        let maps = GameMaps {
            first: "plain",
            more: &[],
        };
        let maps = GameMaps { first: bad, ..maps };
        let err = maps.check(&GAME, 1, 0).unwrap_err();
        assert!(
            err.contains("must be lowercase letters, digits"),
            "{bad:?}: {err}"
        );
    }
}

#[test]
fn map_picks_by_name_and_refuses_an_unknown_one() {
    assert_eq!(select(&GAME, &MAPS, None).unwrap().name(), "plain");
    assert_eq!(select(&GAME, &MAPS, Some("walls")).unwrap().name(), "walls");
    let err = select(&GAME, &MAPS, Some("nope")).unwrap_err();
    assert_eq!(
        err,
        "[E-MAP] unknown map \"nope\"; maps-test has plain, walls, short"
    );
    let dir = scratch("unknown");
    for line in [
        "run --headless --ticks 3 --map nope",
        "dump-world --at-tick 1 --out {dir}/w.json --map nope",
    ] {
        assert_eq!(probe(&dir, line), Exit::Usage, "{line}");
    }
    assert!(!dir.join("w.json").exists());
}

#[test]
fn a_map_with_sim_data_names_it_in_the_stream_the_recording_and_the_report() {
    let dir = scratch("walls");
    let exit = probe(
        &dir,
        "run --headless --ticks 30 --map walls --hashes {dir}/h.txt --record {dir}/r.rec \
         --report {dir}/r.json",
    );
    // 4.8 kB of walls, yet the snapshots fit a datagram: exit 0, not 13.
    assert_eq!(exit, Exit::Ok);
    let hash = sim_data_hash(walled);
    assert_ne!(hash, 0);
    let stream = read(&dir.join("h.txt"));
    let header: Vec<&str> = stream.lines().take(2).collect();
    assert_eq!(header[0], "# eng hash stream v1");
    let protocol = walled(SimConfig {
        tick_rate: 60,
        players: 1,
        seed: 0,
    })
    .unwrap()
    .protocol_version();
    assert_eq!(
        header[1],
        format!(
            "# game=maps-test seed=0 players=1 tick_rate=60 hash_every=1 \
             sim_protocol={protocol:#018x} map=walls sim_data={hash:#018x}"
        )
    );
    let recording = read_recording(&std::fs::read(dir.join("r.rec")).unwrap()).unwrap();
    assert_eq!(recording.header.sim_data_hash, hash);
    assert_eq!(recording.header.sim_protocol, protocol);
    let report = read(&dir.join("r.json"));
    for pair in [
        "\"map\": \"walls\"".to_string(),
        format!("\"sim_protocol\": \"{protocol:#018x}\""),
        format!("\"sim_data_hash\": \"{hash:#018x}\""),
        "\"files_read\": {}".to_string(),
    ] {
        assert!(report.contains(&pair), "{pair} in {report}");
    }
}

#[test]
fn the_first_map_without_sim_data_keeps_1b_s_header() {
    let dir = scratch("plain");
    let script = dir.join("walk.toml");
    std::fs::write(
        &script,
        "version = 1\n[[segment]]\nticks = 5\nforward = 127\n",
    )
    .unwrap();
    let exit = probe(
        &dir,
        "run --headless --ticks 5 --input {dir}/walk.toml --hashes {dir}/h.txt \
         --record {dir}/r.rec --report {dir}/r.json",
    );
    assert_eq!(exit, Exit::Ok);
    let stream = read(&dir.join("h.txt"));
    let second = stream.lines().nth(1).unwrap();
    assert!(second.starts_with("# game=maps-test seed=0 "), "{second}");
    assert!(
        !second.contains(" map=") && !second.contains("sim_data"),
        "{second}"
    );
    let recording = read_recording(&std::fs::read(dir.join("r.rec")).unwrap()).unwrap();
    assert_eq!(recording.header.sim_data_hash, 0);
    let report = read(&dir.join("r.json"));
    for pair in [
        "\"map\": \"plain\"",
        "\"sim_data_hash\": \"0x0000000000000000\"",
        "\"input_script\": 1",
    ] {
        assert!(report.contains(pair), "{pair} in {report}");
    }
}

#[test]
fn replay_verify_picks_the_map_by_its_sim_data_hash() {
    let dir = scratch("replay");
    for map in ["plain", "walls", "short"] {
        let line =
            format!("run --headless --ticks 20 --seed 4 --map {map} --record {{dir}}/{map}.rec");
        assert_eq!(probe(&dir, &line), Exit::Ok, "{line}");
        let line = format!("replay verify {{dir}}/{map}.rec --report {{dir}}/{map}.json");
        assert_eq!(probe(&dir, &line), Exit::Ok, "{line}");
        let report = read(&dir.join(format!("{map}.json")));
        let pair = format!("\"map\": \"{map}\"");
        assert!(report.contains(&pair), "{pair} in {report}");
        assert!(report.contains("\"recording\": 1"), "{report}");
    }

    // A hash no map has: another build's recording.
    let path = dir.join("walls.rec");
    let mut recording = read_recording(&std::fs::read(&path).unwrap()).unwrap();
    recording.header.sim_data_hash ^= 1;
    std::fs::write(dir.join("other.rec"), write_recording(&recording)).unwrap();
    assert_eq!(probe(&dir, "replay verify {dir}/other.rec"), Exit::Usage);
    let err = replay::pick_map(&GAME, &MAPS, &recording.header)
        .err()
        .expect("no map has the hash");
    let walls = sim_data_hash(walled);
    assert_eq!(
        err,
        format!(
            "[E-RECORDING-BUILD] recordings are valid only for the build that made them: \
             sim_data_hash recorded {:#018x}, and no map of this build has it (plain \
             0x0000000000000000, walls {walls:#018x}, short {:#018x})",
            walls ^ 1,
            sim_data_hash(short)
        )
    );

    // Hash 0 names the first map, whose protocol is not the walls'.
    recording.header.sim_data_hash = 0;
    let (map, _) = replay::pick_map(&GAME, &MAPS, &recording.header).unwrap();
    assert_eq!(map.name(), "plain");
    std::fs::write(dir.join("zero.rec"), write_recording(&recording)).unwrap();
    assert_eq!(probe(&dir, "replay verify {dir}/zero.rec"), Exit::Usage);
}

#[test]
fn dump_world_shows_the_sim_data_only_for_a_map_that_has_it() {
    let dir = scratch("dump");
    let plain_line = "dump-world --at-tick 2 --out {dir}/plain.json";
    assert_eq!(probe(&dir, plain_line), Exit::Ok);
    let short_line = "dump-world --at-tick 2 --map short --out {dir}/short.json";
    assert_eq!(probe(&dir, short_line), Exit::Ok);
    let plain = read(&dir.join("plain.json"));
    for key in [
        "\"map\"",
        "\"sim_data_hash\"",
        "\"constants\"",
        "test.Walls",
    ] {
        assert!(!plain.contains(key), "{key} in {plain}");
    }
    let dumped = read(&dir.join("short.json"));
    let hash = format!("\"sim_data_hash\": \"{:#018x}\"", sim_data_hash(short));
    for key in [
        "\"map\": \"short\"",
        &hash,
        "\"constants\": {",
        "\"test.Walls\": {",
    ] {
        assert!(dumped.contains(key), "{key} in {dumped}");
    }
    // The world itself is the per-tick state's dump, without the walls.
    let world = dumped.split("\"world\":").nth(1).unwrap();
    let world = world.split("\"map\":").next().unwrap();
    assert!(!world.contains("test.Walls"), "{world}");
}

#[test]
fn play_runs_the_map_it_is_given() {
    let dir = scratch("play");
    let line = |map: &str| -> Vec<String> {
        let hashes = dir.join(format!("{map}.txt")).display().to_string();
        ["play", "--ticks", "3", "--map", map, "--hashes", &hashes]
            .map(String::from)
            .to_vec()
    };
    let mut run = PlayRun::start_with_maps(&GAME, &MAPS, &line("walls")).unwrap();
    assert_eq!(run.map(), "walls");
    while run.step(PlayerCommand::idle(run.next_tick())) == Step::Next {}
    let stats = PlayStats {
        platform: "headless",
        gpu: None,
    };
    assert_eq!(run.finish(stats), Exit::Ok);
    let stream = read(&dir.join("walls.txt"));
    let hash = format!(" map=walls sim_data={:#018x}", sim_data_hash(walled));
    assert!(stream.lines().nth(1).unwrap().ends_with(&hash), "{stream}");

    assert!(matches!(
        PlayRun::start_with_maps(&GAME, &MAPS, &line("nope")),
        Err(Exit::Usage)
    ));
    // Without maps, the game's build is its only map.
    let run = PlayRun::start(&GAME, &["play".to_string()]).unwrap();
    assert_eq!(run.map(), crate::DEFAULT_MAP);
}

/// A later map names its view (rung 2, T12): `shot`, `video` and `play`
/// refuse one with none before a GPU or window opens; `play` shows a
/// first-person one (T15) and refuses its flags on any other map
/// (`view.rs` and `play/first_person.rs` have the messages).
#[test]
fn a_map_is_refused_where_its_view_cannot_be_drawn() {
    let dir = scratch("no-view");
    for line in [
        "shot --at-tick 1 --out {dir}/s.png",
        "video --ticks 2 --out {dir}/v.apng",
    ] {
        let line = format!("{line} --map walls");
        assert_eq!(probe(&dir, &line), Exit::Usage, "{line}");
    }
    assert!(!dir.join("s.png").exists() && !dir.join("v.apng").exists());
    let walls = select(&GAME, &MAPS, Some("walls")).unwrap();
    assert!(walls.no_view(&GAME).is_some() && walls.no_window_view(&GAME).is_some());
    let plain = select(&GAME, &MAPS, None).unwrap();
    assert_eq!(
        (plain.no_view(&GAME), plain.no_window_view(&GAME)),
        (None, None)
    );
    static SEEN: GameMaps = GameMaps {
        first: "plain",
        more: &[GameMap {
            name: "walls",
            build: walled,
            view: crate::view::tests::FIRST_PERSON,
        }],
    };
    let seen = select(&GAME, &SEEN, Some("walls")).unwrap();
    assert_eq!(seen.no_view(&GAME), None);
    let line = ["play", "--ticks", "1", "--map", "walls"].map(String::from);
    let run = PlayRun::start_with_maps(&GAME, &SEEN, &line).unwrap();
    assert!(matches!(run.view(), MapView::FirstPerson(_)));
    assert_eq!(run.view_refusal(), None);
    // Its flags, and their refusal on other maps: `play/first_person.rs`.
    let fp = run.first_person().expect("a first-person map's settings");
    assert!(!fp.windowed && fp.mouse == eng_input::LookSettings::DEFAULT);
}
