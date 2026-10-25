//! The testbed's side of `--version --json`: its exact systems and schemas,
//! and agreement with the probe's other outputs (`dump-world`, reports and
//! recordings). This file changes with the testbed's sim, as its hash
//! streams do. The stamp's format itself is checked in `version.rs`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("version-testbed")
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

fn stamp(dir: &Path) -> Value {
    let out = probe(dir, &["--version", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    serde_json::from_slice(&out.stdout).expect("--version --json prints JSON")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Every named schema of the testbed's world. It changes only with the
/// testbed's sim, and then the protocol and hash streams change too; say
/// why when it does.
const SCHEMAS: [(&str, u64, &str); 17] = [
    ("eng.collision.BoxHull", 1, "nested"),
    ("eng.collision.BoxRoom", 1, "resource"),
    ("eng.core.PlayerCommand", 1, "nested"),
    ("eng.core.PlayerSlot", 1, "component"),
    ("eng.core.Tick", 1, "nested"),
    ("eng.ecs.Entity", 1, "nested"),
    ("eng.math.Pcg32", 1, "nested"),
    ("eng.math.Vec3", 1, "nested"),
    ("eng.math.WorldPos", 1, "nested"),
    ("eng.sim.GameplayRng", 1, "resource"),
    ("eng.sim.SimInfo", 1, "resource"),
    ("eng.sim.SimTick", 1, "resource"),
    ("eng.sim.TickInputs", 1, "resource"),
    ("kit_fps.MoveConfig", 1, "resource"),
    ("kit_fps.MoveState", 1, "component"),
    ("testbed.Dot", 1, "component"),
    ("testbed.WallBump", 1, "event"),
];

#[test]
fn testbed_systems_and_schemas() {
    let v = stamp(&scratch("schemas"));
    assert_eq!(v["sim"]["systems"], json!(["Update testbed.move_dots"]));
    let expected: Vec<Value> = SCHEMAS
        .iter()
        .map(|(name, version, kind)| json!({"name": name, "version": version, "kind": kind}))
        .collect();
    assert_eq!(v["sim"]["schemas"], Value::Array(expected));
}

#[test]
fn schemas_match_the_world_dump() {
    let dir = scratch("dump");
    let v = stamp(&dir);
    let out = probe(&dir, &["dump-world", "--at-tick", "0", "--out", "w.json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let dump = read_json(&dir.join("w.json"));
    assert_eq!(dump["sim_protocol"], v["sim"]["protocol"]);
    let world = &dump["world"];
    let names = |obj: &Value| -> Vec<String> { obj.as_object().unwrap().keys().cloned().collect() };
    let mut components: Vec<String> = world["entities"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| names(&e["components"]))
        .collect();
    components.sort();
    components.dedup();
    let of_kind = |kind: &str| -> Vec<String> {
        let schemas = v["sim"]["schemas"].as_array().unwrap();
        schemas
            .iter()
            .filter(|s| s["kind"] == kind)
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect()
    };
    // The dump lists every resource and every event channel, empty or not,
    // but only the components some entity has at tick 0.
    assert_eq!(of_kind("resource"), names(&world["resources"]));
    assert_eq!(of_kind("event"), names(&world["events"]));
    let registered = of_kind("component");
    assert!(
        !components.is_empty() && components.iter().all(|c| registered.contains(c)),
        "components in the dump {components:?}, in the stamp {registered:?}"
    );
}

#[test]
fn reports_and_recordings_agree_with_the_stamp() {
    let dir = scratch("report");
    let v = stamp(&dir);
    let out = probe(
        &dir,
        &[
            "run",
            "--headless",
            "--ticks",
            "2",
            "--report",
            "r.json",
            "--record",
            "r.rec",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["build"], v["build"]);
    assert_eq!(report["sim_protocol"], v["sim"]["protocol"]);
    assert_eq!(report["report_version"], v["formats"]["report"]);

    // A recording's container header is `ENGF`, `RECD`, then the version
    // (u32 LE). Its build header matches the stamp too.
    let bytes = std::fs::read(dir.join("r.rec")).unwrap();
    assert_eq!(&bytes[..8], b"ENGFRECD");
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!(v["formats"]["recording"], version);
    let header = eng_input::read_recording(&bytes).unwrap().header;
    assert_eq!(v["game"], header.game.as_str());
    assert_eq!(v["build"]["engine_sha"], header.engine_sha.as_str());
    assert_eq!(v["build"]["game_sha"], header.game_sha.as_str());
    assert_eq!(v["build"]["rustc"], header.rustc.as_str());
    assert_eq!(v["sim"]["tick_rate"], header.tick_rate);
    assert_eq!(
        v["sim"]["protocol"],
        format!("{:#018x}", header.sim_protocol).as_str()
    );
}
