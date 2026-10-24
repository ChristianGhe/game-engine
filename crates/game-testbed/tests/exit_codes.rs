//! A self-test for every probe exit code (rung 1a, done-when 4), run
//! against the real binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use eng_input::{read_recording, write_recording};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("exit-codes")
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

fn code(out: &Output) -> i32 {
    out.status.code().expect("exited normally")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn write(dir: &Path, name: &str, body: &str) -> String {
    std::fs::write(dir.join(name), body).unwrap();
    name.to_string()
}

const WALK: &str = "version = 1\n[[segment]]\nticks = 30\nforward = 127\n";

#[test]
fn exit_0_ok_with_every_output() {
    let dir = scratch("ok");
    let input = write(&dir, "walk.toml", WALK);
    let out = probe(
        &dir,
        &[
            "run",
            "--headless",
            "--ticks",
            "30",
            "--seed",
            "4",
            "--input",
            &input,
            "--report",
            "report.json",
            "--events",
            "events.jsonl",
            "--hashes",
            "hashes.txt",
            "--record",
            "walk.rec",
        ],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(
        text(&out).lines().count() <= 20,
        "console summary stays short"
    );
    let report = std::fs::read_to_string(dir.join("report.json")).unwrap();
    assert!(report.contains("\"exit\": \"ok\"") && report.contains("\"ticks_run\": 30"));
    let hashes = std::fs::read_to_string(dir.join("hashes.txt")).unwrap();
    assert_eq!(hashes.lines().filter(|l| !l.starts_with('#')).count(), 31);
    assert!(
        dir.join("logs").read_dir().unwrap().next().is_some(),
        "agent log written"
    );

    let out = probe(
        &dir,
        &["replay", "verify", "walk.rec", "--report", "verify.json"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    let out = probe(&dir, &["replay", "dump", "walk.rec", "--out", "walk.json"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let out = probe(
        &dir,
        &[
            "dump-world",
            "--at-tick",
            "5",
            "--seed",
            "4",
            "--out",
            "world.json",
        ],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    let world = std::fs::read_to_string(dir.join("world.json")).unwrap();
    assert!(world.contains("kit_fps.MoveState") && world.contains("\"tick\": 5"));
}

#[test]
fn exit_10_assertion_failed() {
    let dir = scratch("assert");
    let asserts = write(
        &dir,
        "a.toml",
        "version = 1\n[[assert]]\nid = \"never\"\nat_tick = 3\n\
         pointer = \"/resources/eng.sim.SimTick/0\"\nop = \"==\"\nvalue = 99\n",
    );
    let out = probe(
        &dir,
        &["run", "--headless", "--ticks", "5", "--assert", &asserts],
    );
    assert_eq!(code(&out), 10, "{}", text(&out));
    assert!(text(&out).contains("[E-ASSERT] never failed at tick 3"));
}

#[test]
fn exit_11_desync() {
    let dir = scratch("desync");
    let input = write(&dir, "walk.toml", WALK);
    let out = probe(
        &dir,
        &[
            "run",
            "--headless",
            "--ticks",
            "30",
            "--input",
            &input,
            "--record",
            "walk.rec",
        ],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    // Tamper with one recorded command: the replay must diverge after it.
    let mut rec = read_recording(&std::fs::read(dir.join("walk.rec")).unwrap()).unwrap();
    rec.commands[9].forward = -100;
    std::fs::write(dir.join("tampered.rec"), write_recording(&rec)).unwrap();
    let out = probe(&dir, &["replay", "verify", "tampered.rec"]);
    assert_eq!(code(&out), 11, "{}", text(&out));
    assert!(text(&out).contains("[E-DESYNC] first divergence at tick 10"));
}

#[test]
fn exit_12_panic() {
    let dir = scratch("panic");
    let out = probe(
        &dir,
        &["run", "--headless", "--ticks", "5", "--panic-at-tick", "3"],
    );
    assert_eq!(code(&out), 12, "{}", text(&out));
    assert!(text(&out).contains("[E-PANIC]"));
}

#[test]
fn exit_13_budget_exceeded() {
    let dir = scratch("budget");
    let budgets = write(&dir, "b.toml", "version = 1\n[sim]\nmax_state_bytes = 64\n");
    let out = probe(
        &dir,
        &["run", "--headless", "--ticks", "5", "--budgets", &budgets],
    );
    assert_eq!(code(&out), 13, "{}", text(&out));
    assert!(text(&out).contains("[E-BUDGET] max_state_bytes exceeded at tick 0"));
}

#[test]
fn exit_64_usage() {
    let dir = scratch("usage");
    write(&dir, "junk.rec", "not a recording");
    write(
        &dir,
        "bad.toml",
        "version = 1\n[[segment]]\nticks = 1\nforwards = 1\n",
    );
    let cases: [&[&str]; 6] = [
        &[],
        &["fly"],
        &["run", "--headless"],
        &[
            "run",
            "--headless",
            "--ticks",
            "5",
            "--input",
            "missing.toml",
        ],
        &["run", "--headless", "--ticks", "5", "--input", "bad.toml"],
        &["replay", "verify", "junk.rec"],
    ];
    for args in cases {
        let out = probe(&dir, args);
        assert_eq!(code(&out), 64, "{args:?}: {}", text(&out));
        assert!(text(&out).contains("[E-USAGE]"), "{args:?}");
    }
}

#[test]
fn exit_64_recording_from_another_build() {
    let dir = scratch("other-build");
    let out = probe(
        &dir,
        &["run", "--headless", "--ticks", "5", "--record", "r.rec"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    let mut rec = read_recording(&std::fs::read(dir.join("r.rec")).unwrap()).unwrap();
    rec.header.sim_protocol ^= 1;
    std::fs::write(dir.join("other.rec"), write_recording(&rec)).unwrap();
    let out = probe(&dir, &["replay", "verify", "other.rec"]);
    assert_eq!(code(&out), 64, "{}", text(&out));
    assert!(text(&out).contains("[E-RECORDING-BUILD]"));

    // A recording without hashes would verify nothing.
    rec.header.sim_protocol ^= 1;
    rec.hashes.clear();
    std::fs::write(dir.join("nohash.rec"), write_recording(&rec)).unwrap();
    let out = probe(&dir, &["replay", "verify", "nohash.rec"]);
    assert_eq!(code(&out), 64, "{}", text(&out));
    assert!(text(&out).contains("no state hash for tick 0"));
}
