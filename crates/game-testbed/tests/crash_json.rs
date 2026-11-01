//! `crash.json` from the panic hook (rung 1b, spec 9.2 done-when 4 and
//! 6.13), run against the real binary: a panic writes every field and
//! exits 12, in `run` and in the window of `play`. `play` skips without a
//! display unless `ENG_REQUIRE_DISPLAY=1`, and without a GPU adapter unless
//! `ENG_REQUIRE_GPU=1`; CI's native jobs set both, so it cannot skip there.
//! Every run is killed after [`LIMIT`] and its test fails, so a window or a
//! GPU that never answers cannot hang a CI job.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("crash-json")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// How long a run may take: far longer than any run here takes, and far
/// shorter than a CI job's limit.
const LIMIT: Duration = Duration::from_secs(120);

/// Runs the probe to its end, as `Command::output` does, but kills it after
/// [`LIMIT`] and fails with what it printed. The probe prints a few lines,
/// so its pipes cannot fill while it runs.
fn probe(dir: &Path, log_dir: &Path, args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args(args)
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run game-testbed");
    let start = Instant::now();
    let finished = loop {
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        if start.elapsed() > LIMIT {
            let _ = child.kill();
            break false;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let out = child.wait_with_output().unwrap();
    assert!(finished, "{args:?} killed after {LIMIT:?}: {}", text(&out));
    out
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

const PANIC_PLAY: [&str; 7] = [
    "play",
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
    check_crash(&dir, &out, &PANIC_RUN, "run.rs:", "eng_cli::run::run");
}

/// The same from `play`: the panic comes in a tick of the frame loop of the
/// window (eng-app), on the main thread, where winit runs.
#[test]
fn play_panic_writes_crash_json_with_every_field() {
    let dir = scratch("play");
    let logs = dir.join("logs");
    let out = probe(&dir, &logs, &PANIC_PLAY);
    let required = |var: &str| std::env::var(var).as_deref() == Ok("1");
    let cannot = |tag: &str, var: &str| text(&out).contains(tag) && !required(var);
    if out.status.code() == Some(64)
        && (cannot("[E-NO-DISPLAY]", "ENG_REQUIRE_DISPLAY")
            || cannot("[E-NO-ADAPTER]", "ENG_REQUIRE_GPU"))
    {
        eprintln!("skipped: {}", text(&out));
        return;
    }
    check_crash(&dir, &out, &PANIC_PLAY, "play.rs:", "eng_app::");
}

/// Every field of the one crash file that a panic at tick 3 of seed 7 left:
/// `location` is in the location of the panic, `frame` in a backtrace line.
fn check_crash(dir: &Path, out: &Output, args: &[&str], location: &str, frame: &str) {
    let logs = dir.join("logs");
    let out_text = text(out);
    let command = args[0];
    assert_eq!(out.status.code(), Some(12), "{out_text}");
    assert!(out_text.contains("[E-PANIC]"), "{out_text}");
    assert!(out_text.lines().count() <= 20, "console stays short");

    let files = crash_files(&logs);
    assert_eq!(files.len(), 1, "one crash file in {}", logs.display());
    let path = &files[0];
    assert!(
        out_text.contains(&format!("crash: {}", path.display())),
        "the console names the crash file: {out_text}"
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
    let version = probe(&version_dir, &version_dir, &["--version", "--json"]);
    assert_eq!(version.status.code(), Some(0), "{}", text(&version));
    let stamp: Value = serde_json::from_slice(&version.stdout).unwrap();
    // Every key and value, so a change to the stamp's `build` shows here.
    assert_eq!(crash["version"], stamp["build"]);
    assert_eq!(crash["crash_version"], stamp["formats"]["crash"]);

    // What was running.
    assert_eq!(crash["args"], serde_json::json!(args));
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
    assert!(str_at(&crash, "/panic/location").contains(location));

    // Backtrace: one string per line, through the probe's own frames.
    let backtrace = crash["backtrace"].as_array().expect("backtrace array");
    assert!(backtrace.iter().all(Value::is_string), "{backtrace:#?}");
    assert!(
        backtrace
            .iter()
            .any(|l| l.as_str().unwrap().contains(frame)),
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
    assert!(
        first.starts_with(&format!("start: game-testbed {command}")),
        "{first}"
    );
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
