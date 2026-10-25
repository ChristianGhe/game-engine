//! `crash.json` from the panic hook (rung 1b, spec 9.2 done-when 4 and
//! 6.13), run against the real binary: a panic writes every field and
//! exits 12.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("crash-json")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn probe(dir: &Path, log_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args(args)
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", log_dir)
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

const PANIC_RUN: [&str; 8] = [
    "run",
    "--headless",
    "--ticks",
    "5",
    "--seed",
    "7",
    "--panic-at-tick",
    "3",
];

fn crash_files(log_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(log_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with("crash-game-testbed-") && name.ends_with(".json")
        })
        .collect();
    files.sort();
    files
}

fn str_at<'a>(crash: &'a Value, pointer: &str) -> &'a str {
    crash
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{pointer} is not a string in {crash:#}"))
}

#[test]
fn run_panic_writes_crash_json_with_every_field() {
    let dir = scratch("run");
    let logs = dir.join("logs");
    let out = probe(&dir, &logs, &PANIC_RUN);
    assert_eq!(out.status.code(), Some(12), "{}", text(&out));
    assert!(text(&out).contains("[E-PANIC]"), "{}", text(&out));
    assert!(text(&out).lines().count() <= 20, "console stays short");

    let files = crash_files(&logs);
    assert_eq!(files.len(), 1, "one crash file in {}", logs.display());
    let path = &files[0];
    assert!(
        text(&out).contains(&format!("crash: {}", path.display())),
        "the console names the crash file: {}",
        text(&out)
    );
    let crash: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();

    assert_eq!(crash["crash_version"], 1);
    assert_eq!(crash["game"], "game-testbed");

    // Version stamp: the `build` object of `--version --json`.
    assert_eq!(
        str_at(&crash, "/version/game_version"),
        env!("CARGO_PKG_VERSION")
    );
    let version_dir = dir.join("version");
    std::fs::create_dir_all(&version_dir).unwrap();
    let out = probe(&version_dir, &version_dir, &["--version", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let stamp: Value = serde_json::from_slice(&out.stdout).unwrap();
    // Every key and value, so a change to the stamp's `build` shows here.
    assert_eq!(crash["version"], stamp["build"]);
    assert_eq!(crash["crash_version"], stamp["formats"]["crash"]);

    // What was running.
    assert_eq!(crash["args"], serde_json::json!(PANIC_RUN));
    assert_eq!(crash["seed"], 7);
    assert_eq!(crash["tick"], 3);

    // Thread.
    assert_eq!(crash["thread"]["name"], "main");
    assert!(str_at(&crash, "/thread/id").starts_with("ThreadId("));

    // The panic itself.
    assert_eq!(
        crash["panic"]["message"],
        "deliberate panic at tick 3 (--panic-at-tick)"
    );
    assert!(str_at(&crash, "/panic/location").contains("run.rs:"));

    // Backtrace: one string per line, through the probe's own frames.
    let backtrace = crash["backtrace"].as_array().expect("backtrace array");
    assert!(backtrace.iter().all(Value::is_string), "{backtrace:#?}");
    assert!(
        backtrace
            .iter()
            .any(|l| l.as_str().unwrap().contains("eng_cli::run::run")),
        "{backtrace:#?}"
    );

    // Last log lines: the start line first, the panic last, as in the log.
    let tail = crash["last_log_lines"].as_array().expect("log lines array");
    assert!(tail.len() >= 2, "{tail:#?}");
    for record in tail {
        assert!(
            record["ts_ms"].is_u64() && record["level"].is_string(),
            "{record}"
        );
    }
    let first = tail[0]["msg"].as_str().unwrap();
    assert!(first.starts_with("start: game-testbed run"), "{first}");
    let last = tail.last().unwrap();
    assert_eq!(last["level"], "error");
    assert!(last["msg"].as_str().unwrap().starts_with("[E-PANIC]"));

    let log_file = Path::new(str_at(&crash, "/log_file"));
    let log = std::fs::read_to_string(log_file).unwrap();
    let last_logged: Value = serde_json::from_str(log.lines().last().unwrap()).unwrap();
    assert_eq!(&last_logged, last, "the tail matches the log file");
}

#[test]
fn panic_exits_12_when_crash_json_cannot_be_written() {
    let dir = scratch("unwritable");
    // A file where the log directory should be: nothing can be created.
    std::fs::write(dir.join("blocker"), "not a directory").unwrap();
    let logs = dir.join("blocker").join("logs");
    let out = probe(&dir, &logs, &PANIC_RUN);
    assert_eq!(out.status.code(), Some(12), "{}", text(&out));
    assert!(text(&out).contains("[E-PANIC]"), "{}", text(&out));
    assert!(text(&out).contains("[E-CRASH-JSON]"), "{}", text(&out));
    assert!(!text(&out).contains("crash: "), "{}", text(&out));
}
