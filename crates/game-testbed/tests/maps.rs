//! The testbed's maps through the real binary (rung 2 plan, decision 1;
//! ADR-0023): `dots`, the first map, is `--map`'s default and has no sim
//! data, so every 1b command line, stream, dump and recording keeps its
//! bytes; an unknown map exits 64 with `[E-MAP]`; reports name the map, its
//! protocol and sim-data hash, and the files read; `replay verify` picks a
//! map by the recording's sim-data hash. `room`, the map with sim data, is
//! `tests/room.rs`'s.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use eng_input::{read_recording, write_recording};
use serde_json::{Value, json};

fn testbed(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("maps")
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

/// dot-square at seed 1, as `cargo xtask replay` runs it, plus `extra`.
fn dot_square(dir: &Path, extra: &[&str]) -> Output {
    let input = testbed("scenarios/dot-square.input.toml");
    let assert = testbed("scenarios/dot-square.assert.toml");
    let budgets = testbed("../../budgets.toml");
    let mut args = vec![
        "run",
        "--headless",
        "--ticks",
        "240",
        "--seed",
        "1",
        "--input",
        input.to_str().unwrap(),
        "--assert",
        assert.to_str().unwrap(),
        "--budgets",
        budgets.to_str().unwrap(),
        "--hashes",
        "h.txt",
        "--report",
        "r.json",
    ];
    args.extend_from_slice(extra);
    probe(dir, &args)
}

#[test]
fn dots_is_the_default_map_and_keeps_its_committed_stream() {
    let committed = std::fs::read(testbed("hashes/dot-square.seed1.txt")).unwrap();
    for (name, extra) in [("default", &[][..]), ("named", &["--map", "dots"][..])] {
        let dir = scratch(name);
        let out = dot_square(&dir, extra);
        assert_eq!(out.status.code(), Some(0), "{name}: {}", text(&out));
        let stream = std::fs::read(dir.join("h.txt")).unwrap();
        assert!(stream == committed, "{name}: the dot stream changed");

        let report = read_json(&dir.join("r.json"));
        assert_eq!(report["report_version"], 2);
        assert_eq!(report["map"], "dots", "{name}");
        assert_eq!(report["sim_data_hash"], "0x0000000000000000", "{name}");
        let header = String::from_utf8(stream).unwrap();
        let protocol = header
            .lines()
            .nth(1)
            .and_then(|l| l.split(' ').find_map(|w| w.strip_prefix("sim_protocol=")))
            .unwrap()
            .to_string();
        assert_eq!(report["sim_protocol"], protocol.as_str());
        assert_eq!(
            report["files_read"],
            json!({"input_script": 1, "assertions": 1, "budgets": 1})
        );
    }
}

#[test]
fn a_run_without_files_reads_none() {
    let dir = scratch("no-files");
    let out = probe(
        &dir,
        &["run", "--headless", "--ticks", "3", "--report", "r.json"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_eq!(read_json(&dir.join("r.json"))["files_read"], json!({}));
}

#[test]
fn an_unknown_map_exits_64_before_anything_runs() {
    let dir = scratch("unknown");
    let cases: [&[&str]; 5] = [
        &["run", "--headless", "--ticks", "5", "--report", "r.json"],
        &["dump-world", "--at-tick", "5", "--out", "w.json"],
        &["shot", "--at-tick", "5", "--out", "s.png"],
        &["video", "--ticks", "5", "--out", "v.apng"],
        &["play", "--ticks", "5", "--report", "r.json"],
    ];
    for args in cases {
        let mut args = args.to_vec();
        args.extend(["--map", "nowhere"]);
        let out = probe(&dir, &args);
        assert_eq!(out.status.code(), Some(64), "{args:?}: {}", text(&out));
        let expected = "[E-USAGE] [E-MAP] unknown map \"nowhere\"; game-testbed has dots, room\n";
        assert!(text(&out).contains(expected), "{args:?}: {}", text(&out));
        for file in ["r.json", "w.json", "s.png", "v.apng"] {
            assert!(!dir.join(file).exists(), "{args:?} wrote {file}");
        }
    }
}

#[test]
fn a_dot_recording_has_no_sim_data_and_replays_on_dots() {
    let dir = scratch("replay");
    let out = probe(
        &dir,
        &["run", "--headless", "--ticks", "30", "--record", "r.rec"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let mut recording = read_recording(&std::fs::read(dir.join("r.rec")).unwrap()).unwrap();
    assert_eq!(recording.header.sim_data_hash, 0);

    let out = probe(&dir, &["replay", "verify", "r.rec", "--report", "v.json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let report = read_json(&dir.join("v.json"));
    assert_eq!(report["map"], "dots");
    assert_eq!(report["sim_data_hash"], "0x0000000000000000");
    assert_eq!(
        report["sim_protocol"],
        format!("{:#018x}", recording.header.sim_protocol).as_str()
    );
    assert_eq!(report["files_read"], json!({"recording": 1}));

    // Sim data no map of this build has: another build's recording.
    recording.header.sim_data_hash = 0x0123_4567_89ab_cdef;
    std::fs::write(dir.join("other.rec"), write_recording(&recording)).unwrap();
    let out = probe(
        &dir,
        &["replay", "verify", "other.rec", "--report", "o.json"],
    );
    assert_eq!(out.status.code(), Some(64), "{}", text(&out));
    // Every map is listed with its hash, the room's as its sim builds it
    // (and pinned in game-testbed-sim's `tests/sim_data.rs`).
    let room = game_testbed_sim::room::build(eng_sim::SimConfig {
        tick_rate: game_testbed_sim::TICK_RATE,
        players: 1,
        seed: 0,
    })
    .unwrap()
    .sim_data_hash();
    let expected = format!(
        "[E-RECORDING-BUILD] recordings are valid only for the build that made them: \
         sim_data_hash recorded 0x0123456789abcdef, and no map of this build has it (dots \
         0x0000000000000000, room {room:#018x})\n"
    );
    assert!(text(&out).contains(&expected), "{}", text(&out));
    assert!(!dir.join("o.json").exists());
}

#[test]
fn dump_world_on_dots_keeps_its_keys_and_bytes() {
    let dir = scratch("dump");
    for (file, extra) in [("a.json", &[][..]), ("b.json", &["--map", "dots"][..])] {
        let mut args = vec![
            "dump-world",
            "--at-tick",
            "12",
            "--seed",
            "3",
            "--out",
            file,
        ];
        args.extend_from_slice(extra);
        let out = probe(&dir, &args);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    }
    let a = std::fs::read(dir.join("a.json")).unwrap();
    assert!(a == std::fs::read(dir.join("b.json")).unwrap());
    let dump = read_json(&dir.join("a.json"));
    let keys: Vec<&str> = dump
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    // serde_json's map sorts the keys; the bytes above keep their order.
    assert_eq!(keys, ["game", "sim_protocol", "tick", "world"]);
}
