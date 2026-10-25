//! `--version` and `--version --json` (rung 1b, spec 9.2 "Probe" and
//! 6.13), run against the real binary. Both forms are checked key by key
//! and value by value. The JSON's keys, their order and their types are
//! pinned here, so every change to them is deliberate: it updates this test
//! and `docs/ARCHITECTURE.md`, "The version stamp". A new key needs nothing
//! more; removing or renaming a key, or changing its type or meaning, also
//! bumps `stamp_version`.
//!
//! The expected values come from the environment this test was compiled
//! with (cargo builds the binary in the same run, with the same
//! environment), the toolchain's own `rustc -V`, and the committed hash
//! streams. `version_testbed.rs` holds what changes with the testbed's sim:
//! its exact systems and schemas, and agreement with `dump-world`, reports
//! and recordings.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Value, json};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("version")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn probe_with(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args(args)
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"))
        .envs(env.iter().copied())
        .output()
        .expect("run game-testbed")
}

fn probe(dir: &Path, args: &[&str]) -> Output {
    probe_with(dir, args, &[])
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Runs a `--version` form that must succeed: exit 0, nothing on stderr,
/// and stdout ending in one newline.
fn version_output(dir: &Path, args: &[&str]) -> String {
    let out = probe(dir, args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", text(&out));
    assert!(
        out.stderr.is_empty(),
        "{args:?} wrote to stderr: {}",
        text(&out)
    );
    let stdout = String::from_utf8(out.stdout).expect("UTF-8");
    assert!(
        stdout.ends_with('\n') && !stdout.ends_with("\n\n"),
        "{stdout:?}"
    );
    stdout
}

/// The stamp, and its object keys in document order (serde_json's `Value`
/// sorts them).
fn stamp_and_order(dir: &Path) -> (Value, Tree) {
    let stdout = version_output(dir, &["--version", "--json"]);
    let value = serde_json::from_str(&stdout).expect("--version --json prints JSON");
    (value, serde_json::from_str(&stdout).unwrap())
}

fn stamp(dir: &Path) -> Value {
    stamp_and_order(dir).0
}

/// Visitor methods for values that are neither objects nor arrays.
macro_rules! leaves {
    ($($method:ident($ty:ty)),*) => {
        $(fn $method<E>(self, _: $ty) -> Result<Tree, E> {
            Ok(Tree::Leaf)
        })*
    };
}

/// A JSON document's shape, with object keys in document order.
enum Tree {
    Object(Vec<(String, Tree)>),
    Array(Vec<Tree>),
    Leaf,
}

impl Tree {
    /// The keys of the object at `path` (object keys and array indices).
    fn keys(&self, path: &[&str]) -> Vec<&str> {
        let mut node = self;
        for step in path {
            node = match node {
                Tree::Object(entries) => entries
                    .iter()
                    .find(|(k, _)| k == step)
                    .map(|(_, v)| v)
                    .unwrap_or_else(|| panic!("no {step:?} in {path:?}")),
                Tree::Array(items) => &items[step.parse::<usize>().unwrap()],
                Tree::Leaf => panic!("{path:?} goes through a leaf"),
            };
        }
        match node {
            Tree::Object(entries) => entries.iter().map(|(k, _)| k.as_str()).collect(),
            _ => panic!("{path:?} is not an object"),
        }
    }
}

impl<'de> Deserialize<'de> for Tree {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Tree;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Tree, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, Tree>()? {
                    entries.push(entry);
                }
                Ok(Tree::Object(entries))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Tree, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<Tree>()? {
                    items.push(item);
                }
                Ok(Tree::Array(items))
            }
            leaves!(
                visit_bool(bool),
                visit_u64(u64),
                visit_i64(i64),
                visit_f64(f64),
                visit_str(&str)
            );
            fn visit_unit<E>(self) -> Result<Tree, E> {
                Ok(Tree::Leaf)
            }
        }
        d.deserialize_any(V)
    }
}

/// A source-tree variable as the build saw it; empty counts as unset.
fn built_with(value: Option<&'static str>) -> Option<&'static str> {
    value.filter(|v| !v.is_empty())
}

/// `rustc -V` of the toolchain running this test, if it can be asked:
/// `$RUSTC` if that was set; else the `rustc` beside the `cargo` that built
/// it (rustup's layout, as on CI); else the `rustc` on `PATH`.
fn toolchain_rustc() -> Option<String> {
    let beside_cargo =
        Path::new(env!("CARGO")).with_file_name(format!("rustc{}", std::env::consts::EXE_SUFFIX));
    let rustc = match built_with(option_env!("RUSTC")) {
        Some(rustc) => PathBuf::from(rustc),
        None if beside_cargo.exists() => beside_cargo,
        None => PathBuf::from("rustc"),
    };
    let out = Command::new(rustc).arg("-V").output().ok()?;
    let version = String::from_utf8(out.stdout).ok()?;
    out.status.success().then(|| version.trim().to_string())
}

/// Checks that `target` is a triple for the machine running this test.
/// Covers the targets CI and the owner use: x86_64, aarch64 and riscv64gc
/// on Linux, macOS and Windows, with `gnu`, `musl` or `msvc` (including
/// variants such as `gnueabihf`), and 32-bit x86 (`i686`, `i586`).
fn check_target(target: &str) {
    let arch_matches = match std::env::consts::ARCH {
        "x86" => target.starts_with("i686") || target.starts_with("i586"),
        arch => target.starts_with(arch),
    };
    assert!(
        arch_matches,
        "{target} is not for {}",
        std::env::consts::ARCH
    );
    let os = match std::env::consts::OS {
        "macos" => "-apple-darwin",
        "linux" => "-linux",
        "windows" => "-windows",
        other => other,
    };
    assert!(target.contains(os), "{target} is not for {os}");
    let env = if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else {
        ""
    };
    let last = target.rsplit('-').next().unwrap_or(target);
    assert!(last.starts_with(env), "{target} is not for {env}");
    let parts = target.split('-').count();
    assert!((3..=4).contains(&parts), "{target} is not a triple");
}

/// The `build` object this binary must print. `target` is checked by
/// [`check_target`] and passed in.
fn expected_build(target: &str) -> Value {
    let sha = built_with(option_env!("ENG_ENGINE_SHA")).unwrap_or("unknown");
    let dirty = match built_with(option_env!("ENG_DIRTY")) {
        None => Value::Null,
        Some("0") => Value::Bool(false),
        Some("1") => Value::Bool(true),
        Some(other) => panic!("the build should have refused ENG_DIRTY={other}"),
    };
    let rustc = toolchain_rustc()
        .unwrap_or_else(|| panic!("cannot run the toolchain's rustc beside {}", env!("CARGO")));
    json!({
        // The game and `eng-cli` both take the workspace version.
        "game_version": env!("CARGO_PKG_VERSION"),
        "engine_version": env!("CARGO_PKG_VERSION"),
        "engine_sha": sha,
        // The testbed's commit is the engine's.
        "game_sha": sha,
        "dirty": dirty,
        "source_date_epoch": built_with(option_env!("SOURCE_DATE_EPOCH"))
            .map(|s| s.parse::<u64>().expect("whole seconds")),
        "ci_run": built_with(option_env!("ENG_CI_RUN")).unwrap_or("unknown"),
        "target": target,
        "rustc": rustc,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "debug_assertions": cfg!(debug_assertions),
    })
}

/// The `--version` line that the stamp `v` implies, as documented.
fn expected_line(v: &Value) -> String {
    let b = &v["build"];
    let s = |key: &str| b[key].as_str().unwrap_or_else(|| panic!("{key}: {b:#}"));
    let short = |sha: &str| sha.chars().take(12).collect::<String>();
    let mut engine = format!("engine {} {}", s("engine_version"), short(s("engine_sha")));
    if s("game_sha") != s("engine_sha") {
        engine.push_str(&format!(", game {}", short(s("game_sha"))));
    }
    engine.push_str(match b["dirty"].as_bool() {
        Some(true) => ", dirty",
        Some(false) => "",
        None => ", dirty unknown",
    });
    format!(
        "{} {} ({engine}) {} {}, rustc {}, sim protocol {}",
        v["game"].as_str().unwrap(),
        s("game_version"),
        s("target"),
        s("profile"),
        // As documented: `unknown` when the compiler could not be asked.
        s("rustc").split(' ').nth(1).unwrap_or("unknown"),
        v["sim"]["protocol"].as_str().unwrap()
    )
}

/// The header of a committed hash stream: its version line and protocol.
fn committed_hash_header() -> (String, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("hashes/dot-square.seed1.txt");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines = text.lines();
    let first = lines.next().unwrap().to_string();
    let protocol = lines
        .next()
        .and_then(|l| l.split(' ').find_map(|w| w.strip_prefix("sim_protocol=")))
        .unwrap_or_else(|| panic!("no sim_protocol in {}", path.display()));
    (first, protocol.to_string())
}

#[test]
fn version_json_keys_and_values() {
    let dir = scratch("json");
    let (v, order) = stamp_and_order(&dir);

    assert_eq!(
        order.keys(&[]),
        ["stamp_version", "game", "build", "sim", "formats"]
    );
    assert_eq!(v["stamp_version"], 1);
    assert_eq!(v["game"], "game-testbed");

    // The build: spec 6.13's `version.json` fields, every value exact.
    assert_eq!(
        order.keys(&["build"]),
        [
            "game_version",
            "engine_version",
            "engine_sha",
            "game_sha",
            "dirty",
            "source_date_epoch",
            "ci_run",
            "target",
            "rustc",
            "profile",
            "debug_assertions",
        ]
    );
    let target = v["build"]["target"].as_str().expect("target is a string");
    check_target(target);
    assert_eq!(v["build"], expected_build(target));

    // The simulation.
    assert_eq!(
        order.keys(&["sim"]),
        ["tick_rate", "protocol", "systems", "schemas"]
    );
    let (hash_version_line, protocol) = committed_hash_header();
    assert_eq!(v["sim"]["tick_rate"], game_testbed_sim::TICK_RATE);
    assert_eq!(v["sim"]["protocol"], protocol.as_str());
    // The exact systems and schemas are the testbed's (`version_testbed.rs`);
    // here, their form.
    let systems = v["sim"]["systems"].as_array().expect("systems array");
    assert!(!systems.is_empty());
    for system in systems {
        let (stage, name) = system.as_str().unwrap().split_once(' ').unwrap();
        assert!(!stage.is_empty() && !name.is_empty(), "{system}");
    }
    let schemas = v["sim"]["schemas"].as_array().expect("schemas array");
    assert!(!schemas.is_empty());
    let mut previous: Option<(&str, u64)> = None;
    for (i, s) in schemas.iter().enumerate() {
        let i = i.to_string();
        assert_eq!(
            order.keys(&["sim", "schemas", &i]),
            ["name", "version", "kind"]
        );
        let entry = (s["name"].as_str().unwrap(), s["version"].as_u64().unwrap());
        assert!(
            previous.is_none_or(|p| p < entry),
            "schemas are sorted by name, then version: {entry:?}"
        );
        let kind = s["kind"].as_str().unwrap();
        assert!(
            ["component", "resource", "event", "nested"].contains(&kind),
            "{s}"
        );
        previous = Some(entry);
    }

    // The probe's file formats, and the versions the files themselves carry.
    assert_eq!(
        order.keys(&["formats"]),
        [
            "report",
            "crash",
            "recording",
            "hash_stream",
            "input_script",
            "assertions",
            "budgets",
        ]
    );
    let formats = &v["formats"];
    assert_eq!(
        formats,
        &json!({
            "report": 2, "crash": 1, "recording": 1, "hash_stream": 1,
            "input_script": 1, "assertions": 1, "budgets": 1,
        })
    );
    assert_eq!(
        hash_version_line,
        format!("# eng hash stream v{}", formats["hash_stream"])
    );
}

#[test]
fn version_line_is_the_stamp_on_one_line() {
    let dir = scratch("line");
    let v = stamp(&dir);
    let stdout = version_output(&dir, &["--version"]);
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    assert_eq!(stdout.trim_end(), expected_line(&v));
}

#[test]
fn version_ignores_the_environment_at_run_time() {
    let quiet = scratch("env-quiet");
    let noisy = scratch("env-noisy");
    let env = [
        ("ENG_ENGINE_SHA", "set-at-run-time"),
        ("ENG_DIRTY", "1"),
        ("SOURCE_DATE_EPOCH", "1"),
        ("ENG_CI_RUN", "set-at-run-time"),
        ("RUST_BACKTRACE", "full"),
    ];
    for args in [&["--version"][..], &["--version", "--json"]] {
        // The quiet run has none of them, whatever this test inherited.
        let a = Command::new(env!("CARGO_BIN_EXE_game-testbed"))
            .args(args)
            .current_dir(&quiet)
            .env("ENG_AGENT_LOG_DIR", quiet.join("logs"))
            .env_remove("ENG_ENGINE_SHA")
            .env_remove("ENG_DIRTY")
            .env_remove("SOURCE_DATE_EPOCH")
            .env_remove("ENG_CI_RUN")
            .env_remove("RUST_BACKTRACE")
            .output()
            .expect("run game-testbed");
        let b = probe_with(&noisy, args, &env);
        assert_eq!(a.status.code(), Some(0), "{}", text(&a));
        assert_eq!(b.status.code(), Some(0), "{}", text(&b));
        assert_eq!(
            String::from_utf8_lossy(&a.stdout),
            String::from_utf8_lossy(&b.stdout),
            "{args:?} depends on the run-time environment or directory"
        );
    }
}

#[test]
fn version_usage_errors_exit_64() {
    let dir = scratch("usage");
    for (args, message) in [
        (&["--version", "--yaml"][..], "unknown flag \"--yaml\""),
        (&["--version", "--json", "--json"], "--json given twice"),
        (&["--version", "run"], "unknown flag \"run\""),
        (&["--json"], "unknown command \"--json\""),
        (&["version"], "unknown command \"version\""),
    ] {
        let out = probe(&dir, args);
        assert_eq!(out.status.code(), Some(64), "{args:?}: {}", text(&out));
        assert!(
            text(&out).contains(&format!("[E-USAGE] {message}")),
            "{args:?}: {}",
            text(&out)
        );
        assert!(out.stdout.is_empty(), "{args:?} printed a stamp anyway");
    }
}
