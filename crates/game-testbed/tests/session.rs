//! `run` and `dump-world` go through `eng-session` (rung 1b, T6): a server
//! and a client in-process over Memory, every message encoded to bytes. Run
//! against the real binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use eng_core::{PlayerCommand, Tick};
use eng_input::{InputScript, read_recording};
use eng_schema_json::JsonValue;
use eng_sim::SimConfig;
use serde_json::Value;

/// Walk east, then turn north, so the dumped world has moved.
const WALK: &str = "version = 1\n[[segment]]\nticks = 8\nforward = 127\n\n\
                    [[segment]]\nticks = 8\nforward = 64\nyaw_deg = 90.0\n";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("session")
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

/// The testbed's state grows by 85 bytes a player, so 10 players' state
/// fits a datagram at tick 0 but not at tick 1, once each slot's command
/// is in it, and 13 players' does not fit at tick 0.
#[test]
fn run_stops_where_a_snapshot_outgrows_a_datagram() {
    for (players, tick) in [("10", 1), ("13", 0)] {
        let dir = scratch(&format!("players-{players}"));
        let out = probe(
            &dir,
            &[
                "run",
                "--headless",
                "--ticks",
                "5",
                "--players",
                players,
                "--report",
                "r.json",
                "--hashes",
                "h.txt",
            ],
        );
        assert_eq!(out.status.code(), Some(13), "{}", text(&out));
        let tag = format!("[E-SNAPSHOT-SIZE] the snapshot of tick {tick} is ");
        assert!(text(&out).contains(&tag), "{}", text(&out));
        assert!(text(&out).lines().count() <= 20, "console stays short");
        let report = read_json(&dir.join("r.json"));
        assert_eq!(report["exit"], "budget-exceeded");
        assert_eq!(report["ticks_run"], 0, "the client's newest state");
        let exceeded = report["budgets"]["exceeded"].as_str().unwrap();
        assert!(exceeded.starts_with(&tag), "{exceeded}");
        let hashes = std::fs::read_to_string(dir.join("h.txt")).unwrap();
        let ticks: Vec<&str> = hashes
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.split(' ').next().unwrap())
            .collect();
        assert_eq!(ticks, ["0"]);
    }
}

#[test]
fn a_two_player_session_records_what_the_server_applied() {
    let dir = scratch("two-players");
    std::fs::write(
        dir.join("walk.toml"),
        "version = 1\n[[segment]]\nticks = 20\nforward = 90\nyaw_deg = 45.0\n",
    )
    .unwrap();
    let out = probe(
        &dir,
        &[
            "run",
            "--headless",
            "--ticks",
            "30",
            "--players",
            "2",
            "--seed",
            "3",
            "--input",
            "walk.toml",
            "--record",
            "r.rec",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let recording = read_recording(&std::fs::read(dir.join("r.rec")).unwrap()).unwrap();
    assert_eq!(recording.header.players, 2);
    assert_eq!(recording.ticks(), 30);
    let walk = PlayerCommand {
        forward: 90,
        view_yaw: 8192,
        ..PlayerCommand::idle(Tick(1))
    };
    for t in 1..=30 {
        let tick = Tick(t);
        let player0 = if t <= 20 {
            walk.repeated_at(tick)
        } else {
            PlayerCommand {
                view_yaw: 8192,
                ..PlayerCommand::idle(tick)
            }
        };
        assert_eq!(
            recording.commands_at(tick),
            [player0, PlayerCommand::idle(tick)],
            "slot 1 has no client, so it idles"
        );
    }
    let out = probe(&dir, &["replay", "verify", "r.rec"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
}

#[test]
fn dump_world_matches_stepping_the_sim_directly() {
    let dir = scratch("dump");
    std::fs::write(dir.join("walk.toml"), WALK).unwrap();
    let out = probe(
        &dir,
        &[
            "dump-world",
            "--at-tick",
            "12",
            "--seed",
            "5",
            "--input",
            "walk.toml",
            "--out",
            "w.json",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let dumped = read_json(&dir.join("w.json"));
    // The same ticks without the session.
    let script = InputScript::parse(WALK, "walk.toml").unwrap();
    let session = SimConfig {
        tick_rate: game_testbed_sim::TICK_RATE,
        players: 1,
        seed: 5,
    };
    let mut sim = game_testbed_sim::build(session).unwrap();
    for command in script.commands(12) {
        sim.step(&[command]).unwrap();
    }
    let direct = JsonValue::from_walk(&|v| sim.world().visit_dump(v)).to_compact();
    assert_eq!(dumped["tick"], 12);
    assert_eq!(
        dumped["sim_protocol"],
        format!("{:#018x}", sim.protocol_version())
    );
    assert_eq!(
        dumped["world"],
        serde_json::from_str::<Value>(&direct).unwrap()
    );
}

#[test]
fn dump_world_stops_where_a_snapshot_outgrows_a_datagram() {
    let dir = scratch("dump-limit");
    let out = probe(
        &dir,
        &[
            "dump-world",
            "--at-tick",
            "5",
            "--players",
            "10",
            "--out",
            "w.json",
        ],
    );
    assert_eq!(out.status.code(), Some(13), "{}", text(&out));
    let console = text(&out);
    let tag = "[E-SNAPSHOT-SIZE] the snapshot of tick 1 is ";
    assert!(console.contains(tag), "{console}");
    assert!(console.contains("dump-world: 0 of 5 ticks"), "{console}");
    assert!(!dir.join("w.json").exists(), "no world at tick 5 to dump");
}
