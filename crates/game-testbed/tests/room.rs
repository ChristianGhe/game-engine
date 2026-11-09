//! The room map through the real binary (rung 2 plan, decision 1 and
//! criterion 5; ADR-0023): `--map room` names its sim data in recordings,
//! reports, hash-stream headers and dumps; `replay verify` picks it by the
//! recording's sim-data hash; and its committed input scripts end where its
//! scenarios' assertions (`docs/rungs/2/patches/room-scenarios.patch`, at
//! G5) say. `shot` and `video` draw it in first person (T12; without an
//! adapter those tests skip, unless `ENG_REQUIRE_GPU=1`), and `play`'s
//! first-person flags are checked before a window opens (T15; its window
//! and frame loop are `tests/play_room.rs`'s). The sim's side is
//! `game-testbed-sim/tests/{room,sim_data}.rs`, the view's
//! `game-testbed-client`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use eng_input::read_recording;
use serde_json::Value;

fn testbed(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("room")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn probe(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args(args)
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"))
        .output()
        .expect("run game-testbed")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// The room's sim-data hash, as `game_testbed_sim::room` builds it: the
/// same for every seed and player count.
fn room_sim_data_hash() -> u64 {
    let config = eng_sim::SimConfig {
        tick_rate: game_testbed_sim::TICK_RATE,
        players: 1,
        seed: 0,
    };
    game_testbed_sim::room::build(config)
        .unwrap()
        .sim_data_hash()
}

/// A room script's run, as `cargo xtask replay` runs a scenario with
/// `map = "room"` (budgets included), plus `extra`.
fn room_run(dir: &Path, script: &str, ticks: &str, extra: &[&str]) -> Output {
    let input = testbed(&format!("scenarios/{script}.input.toml"));
    let budgets = testbed("../../budgets.toml");
    let mut args = vec![
        "run",
        "--headless",
        "--map",
        "room",
        "--ticks",
        ticks,
        "--seed",
        "1",
        "--input",
        input.to_str().unwrap(),
        "--budgets",
        budgets.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    probe(dir, &args)
}

/// A `key=value` word of a hash stream's second header line.
fn header_word(stream: &str, key: &str) -> String {
    let prefix = format!("{key}=");
    stream
        .lines()
        .nth(1)
        .and_then(|l| l.split(' ').find_map(|w| w.strip_prefix(prefix.as_str())))
        .unwrap_or_else(|| panic!("no {key} in {stream:.200}"))
        .to_string()
}

#[test]
fn the_room_names_its_sim_data_in_recordings_reports_and_stream_headers() {
    let dir = scratch("names");
    let files = [
        "--hashes", "h.txt", "--report", "r.json", "--record", "r.rec",
    ];
    let out = room_run(&dir, "room-walk", "120", &files);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let hash = room_sim_data_hash();
    assert_ne!(hash, 0);
    let hex = format!("{hash:#018x}");

    let stream = std::fs::read_to_string(dir.join("h.txt")).unwrap();
    assert!(
        stream.starts_with("# eng hash stream v1\n"),
        "{stream:.100}"
    );
    assert_eq!(header_word(&stream, "map"), "room");
    assert_eq!(header_word(&stream, "sim_data"), hex);
    let protocol = header_word(&stream, "sim_protocol");
    let dots = std::fs::read_to_string(testbed("hashes/dot-square.seed1.txt")).unwrap();
    assert_ne!(protocol, header_word(&dots, "sim_protocol"));

    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["map"], "room");
    assert_eq!(report["sim_data_hash"], hex.as_str());
    assert_eq!(report["sim_protocol"], protocol.as_str());
    let budgets = &report["budgets"];
    assert_eq!(budgets["peak_entities"], 4);
    assert!(budgets["peak_state_bytes"].as_u64().unwrap() <= 4096);
    assert_eq!(budgets["exceeded"], Value::Null);

    let recording = read_recording(&std::fs::read(dir.join("r.rec")).unwrap()).unwrap();
    assert_eq!(recording.header.sim_data_hash, hash);
    assert_eq!(format!("{:#018x}", recording.header.sim_protocol), protocol);

    // replay verify builds every map and picks the room by its hash.
    let out = probe(&dir, &["replay", "verify", "r.rec", "--report", "v.json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let verified = read_json(&dir.join("v.json"));
    assert_eq!(verified["map"], "room");
    assert_eq!(verified["sim_data_hash"], hex.as_str());
}

/// `play`'s first-person flags (T15) are checked before a window opens,
/// so these need no display: a bad value, a look file that does not load,
/// or a first-person flag on the dot map, which plays as in rung 1b, exits
/// 64 with `[E-USAGE]`, writing nothing and panicking nowhere.
#[test]
fn plays_first_person_flags_are_checked_before_a_window_opens() {
    let dir = scratch("play-flags");
    std::fs::write(dir.join("bad-look.toml"), "version = 1\nfogg = false\n").unwrap();
    let cases: [(&[&str], &str); 9] = [
        (
            &["--map", "room", "--fov", "0"],
            "--fov needs a vertical field of view",
        ),
        (
            &["--map", "room", "--fov", "1e2"],
            "--fov needs a plain decimal",
        ),
        (
            &["--map", "room", "--sensitivity", "0.0660001"],
            "--sensitivity needs a plain decimal with at most six places",
        ),
        (
            &["--map", "room", "--pitch-limit", "91"],
            "--pitch-limit needs 0 to 90",
        ),
        (&["--map", "room", "--look", "missing.toml"], "missing.toml"),
        (
            &["--map", "room", "--look", "bad-look.toml"],
            "[E-LOAD] bad-look.toml",
        ),
        (
            &["--windowed"],
            "--windowed is for a first-person map; game-testbed's map \"dots\" is top-down",
        ),
        (
            &["--map", "dots", "--fov", "90"],
            "--fov is for a first-person map",
        ),
        (
            &["--invert-pitch"],
            "--invert-pitch is for a first-person map",
        ),
    ];
    for (extra, expected) in cases {
        let mut args = vec![
            "play", "--ticks", "5", "--hashes", "h.txt", "--report", "r.json", "--record", "r.rec",
        ];
        args.extend_from_slice(extra);
        let out = probe(&dir, &args);
        assert_eq!(out.status.code(), Some(64), "{args:?}: {}", text(&out));
        assert!(text(&out).contains("[E-USAGE]"), "{args:?}: {}", text(&out));
        assert!(text(&out).contains(expected), "{args:?}: {}", text(&out));
        for file in ["r.json", "h.txt", "r.rec"] {
            assert!(!dir.join(file).exists(), "{args:?} wrote {file}");
        }
    }
    let crashes: Vec<_> = std::fs::read_dir(dir.join("logs"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("crash-"))
        .collect();
    assert!(crashes.is_empty(), "{crashes:?}");
}

#[test]
fn dump_world_on_the_room_adds_its_map_and_sim_data() {
    let dir = scratch("dump");
    let out = probe(
        &dir,
        &[
            "dump-world",
            "--map",
            "room",
            "--at-tick",
            "3",
            "--out",
            "w.json",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let dump = read_json(&dir.join("w.json"));
    let keys: Vec<&str> = dump
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    // serde_json's map sorts the keys.
    assert_eq!(
        keys,
        [
            "constants",
            "game",
            "map",
            "sim_data_hash",
            "sim_protocol",
            "tick",
            "world"
        ]
    );
    assert_eq!(dump["map"], "room");
    let hex = format!("{:#018x}", room_sim_data_hash());
    assert_eq!(dump["sim_data_hash"], hex.as_str());
    let walls = &dump["constants"]["eng.collision.grid.Grid"]["walls"];
    assert_eq!(walls.as_array().map(Vec::len), Some(8));
    let resources = dump["world"]["resources"].as_object().unwrap();
    assert!(!resources.contains_key("eng.collision.grid.Grid"));
}

/// Where each committed room script gets to, as its scenario's assertions
/// check it: up the step and onto the platform, then off it to a stop
/// under the skylight; stopped by the ledge on the floor; stopped in the
/// alcove's corner on both walls.
#[test]
fn the_rooms_scripts_end_where_their_scenarios_say() {
    type Case<'a> = (&'a str, &'a str, &'a [(&'a str, f64)]);
    let cases: [Case<'_>; 5] = [
        ("room-walk", "160", &[("pos/z", 1.075)]),
        ("room-walk", "170", &[("pos/z", 1.275)]),
        (
            "room-walk",
            "600",
            &[("pos/z", 0.875), ("vel/x", 0.0), ("vel/y", 0.0)],
        ),
        (
            "room-ledge",
            "300",
            &[("pos/y", 1.25), ("pos/z", 0.875), ("vel/y", 0.0)],
        ),
        (
            "room-corner",
            "300",
            &[
                ("pos/x", 13.75),
                ("pos/y", 1.25),
                ("pos/z", 0.875),
                ("vel/x", 0.0),
                ("vel/y", 0.0),
            ],
        ),
    ];
    for (script, tick, expected) in cases {
        let dir = scratch(&format!("{script}-{tick}"));
        let input = testbed(&format!("scenarios/{script}.input.toml"));
        let out = probe(
            &dir,
            &[
                "dump-world",
                "--map",
                "room",
                "--at-tick",
                tick,
                "--seed",
                "1",
                "--input",
                input.to_str().unwrap(),
                "--out",
                "w.json",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{script}: {}", text(&out));
        let dump = read_json(&dir.join("w.json"));
        let state = &dump["world"]["entities"][0]["components"]["kit_fps.MoveState"];
        for &(field, value) in expected {
            let found = state.pointer(&format!("/{field}")).and_then(Value::as_f64);
            assert_eq!(found, Some(value), "{script} at {tick}: {field}");
        }
        if (script, tick) == ("room-walk", "600") {
            let (x, y) = (state["pos"]["x"].as_f64(), state["pos"]["y"].as_f64());
            let (x, y) = (x.unwrap(), y.unwrap());
            assert!(
                (3.0..=5.0).contains(&x) && (6.0..=8.0).contains(&y),
                "{x} {y}"
            );
        }
    }
}

/// Runs a command that renders. `None` if there is no adapter and CI does
/// not require one.
fn render(dir: &Path, args: &[&str]) -> Option<Output> {
    let out = probe(dir, args);
    let no_adapter = out.status.code() == Some(64) && text(&out).contains("[E-NO-ADAPTER]");
    if no_adapter && std::env::var("ENG_REQUIRE_GPU").as_deref() != Ok("1") {
        eprintln!("skipped: {}", text(&out));
        return None;
    }
    Some(out)
}

/// Every frame of a PNG or APNG, as RGBA8, with its size.
fn frames(path: &Path) -> Vec<(u32, u32, Vec<u8>)> {
    let bytes = std::fs::read(path).unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(&bytes[..]))
        .read_info()
        .unwrap();
    let count = reader.info().animation_control.map_or(1, |a| a.num_frames);
    (0..count)
        .map(|_| {
            let mut rgba8 = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut rgba8).unwrap();
            assert_eq!(
                (info.color_type, info.bit_depth),
                (png::ColorType::Rgba, png::BitDepth::Eight)
            );
            (info.width, info.height, rgba8)
        })
        .collect()
}

/// The room's palette, `game-testbed-client/palettes/room.gpl`, as sRGB
/// codes.
fn palette() -> std::collections::BTreeSet<[u8; 3]> {
    let text =
        std::fs::read_to_string(testbed("../game-testbed-client/palettes/room.gpl")).unwrap();
    text.lines()
        .filter_map(|line| {
            let codes: Vec<u8> = line
                .split_whitespace()
                .take(3)
                .map_while(|w| w.parse().ok())
                .collect();
            <[u8; 3]>::try_from(codes).ok()
        })
        .collect()
}

/// `shot --map room` draws the room in first person from player 0's eye,
/// every look feature on: each pixel a colour of the room's palette, and
/// the frame's counters those of the first-person graph (for `budgets.toml`'s
/// coming `[gpu.first_person]`, T16 and T21): 7 draws (one chunk, the
/// sprites, light, sky, fog, the tone map and the palette), 7 pipelines, 15
/// bind groups, and the 624-byte uniform plus 16 bytes for the chunk and 32
/// for each of the 3 sprites. `video` draws it too. What the pixels show is
/// `game-testbed-client`'s `tests/room_gpu.rs`.
#[test]
fn shot_and_video_draw_the_room_in_first_person() {
    let dir = scratch("first-person");
    let shot = [
        "shot",
        "--map",
        "room",
        "--at-tick",
        "60",
        "--out",
        "room.png",
        "--report",
        "r.json",
    ];
    let Some(out) = render(&dir, &shot) else {
        return;
    };
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["map"], "room");
    let per_frame = &report["gpu"]["per_frame"];
    let counters = [
        "draws",
        "pipeline_switches",
        "bind_group_switches",
        "upload_bytes",
    ]
    .map(|key| per_frame[key].as_u64().unwrap());
    assert_eq!(counters, [7, 7, 15, 624 + 16 + 3 * 32]);
    let pictures = frames(&dir.join("room.png"));
    let (width, height, rgba8) = &pictures[0];
    assert_eq!((*width, *height), (320, 240));
    let palette = palette();
    assert_eq!(palette.len(), 256);
    let colours: std::collections::BTreeSet<[u8; 3]> = rgba8
        .chunks(4)
        .map(|p| {
            assert_eq!(p[3], 255);
            [p[0], p[1], p[2]]
        })
        .collect();
    assert!(
        colours.is_subset(&palette),
        "{:?}",
        colours.difference(&palette)
    );
    assert!(colours.len() >= 40, "{} colours", colours.len());

    let video = [
        "video",
        "--map",
        "room",
        "--ticks",
        "120",
        "--every",
        "60",
        "--out",
        "room.apng",
    ];
    let out = render(&dir, &video).expect("an adapter, as for the shot");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let pictures = frames(&dir.join("room.apng"));
    assert_eq!(pictures.len(), 3);
    // The second frame is the shot's tick.
    assert_eq!(pictures[1].2, *rgba8);
    // The marker turns between them.
    assert_ne!(pictures[0].2, pictures[1].2);
}
