//! `play` in a real window (rung 1b, acceptance `native-window` and
//! `esc-quits`), run as a child process: winit needs the main thread, which
//! libtest's threads are not (plan, risk table). The frame loop's headless
//! twin is `tests/twin.rs`.
//!
//! Without a display a windowed test skips, unless `ENG_REQUIRE_DISPLAY=1`
//! (rung 1b amendment 36); without a GPU adapter it skips, unless
//! `ENG_REQUIRE_GPU=1` (plan decision 4). CI's test job has neither, and
//! its native jobs set both.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("play")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn testbed(path: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(path)
        .display()
        .to_string()
}

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_game-testbed"));
    command
        .args(args)
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"));
    command
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// `true` if the run could not open a window or a GPU and may skip.
fn skipped(out: &Output) -> bool {
    let text = text(out);
    let skip = out.status.code() == Some(64)
        && ((text.contains("[E-NO-DISPLAY]") && !eng_platform_winit::display_required())
            || (text.contains("[E-NO-ADAPTER]")
                && std::env::var("ENG_REQUIRE_GPU").as_deref() != Ok("1")));
    if skip {
        eprintln!("skipped: {text}");
    }
    skip
}

fn report(dir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join("r.json")).unwrap()).unwrap()
}

#[test]
fn windowed_play_gives_the_committed_hash_stream() {
    let dir = scratch("dot-square");
    let input = testbed("scenarios/dot-square.input.toml");
    let out = command(
        &dir,
        &[
            "play", "--ticks", "240", "--seed", "1", "--input", &input, "--hashes", "h.txt",
            "--report", "r.json",
        ],
    )
    .output()
    .unwrap();
    if skipped(&out) {
        return;
    }
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(text(&out).lines().count() <= 20, "console stays short");
    let ours = std::fs::read_to_string(dir.join("h.txt")).unwrap();
    let committed = std::fs::read_to_string(testbed("hashes/dot-square.seed1.txt")).unwrap();
    assert_eq!(ours, committed, "the window's stream is the headless one");

    let report = report(&dir);
    assert_eq!(report["command"], "play");
    assert_eq!(report["platform"], "winit");
    assert_eq!(report["ticks_run"], 240);
    assert_eq!(report["exit"], "ok");
    assert!(report["dropped_ticks"].is_u64(), "{report:#}");
    // The window's frames: the top-down view of the room and the dot (3
    // shapes, 96 bytes), tone-mapped, then presented (T8's counts).
    let gpu = &report["gpu"];
    assert!(gpu["width"].as_u64().unwrap() > 0 && gpu["height"].as_u64().unwrap() > 0);
    // T8's open question 1: the format each runner's window gets.
    let format = gpu["surface_format"].as_str().unwrap_or_default();
    assert!(
        ["Bgra8UnormSrgb", "Bgra8Unorm"].contains(&format),
        "{gpu:#}"
    );
    if gpu["frames"] == 0 && !eng_platform_winit::display_required() {
        // macOS reports a window occluded while the screen is locked or
        // asleep, and nothing is drawn then (wgpu skips it too); CI's
        // native jobs (ENG_REQUIRE_DISPLAY=1) must present.
        eprintln!("no frame presented: the window was never visible");
        return;
    }
    assert!(gpu["frames"].as_u64().unwrap() > 0, "{gpu:#}");
    let per_frame = &gpu["per_frame"];
    assert_eq!(per_frame["draws"], 3, "{gpu:#}");
    assert_eq!(per_frame["pipeline_switches"], 3, "{gpu:#}");
    assert_eq!(per_frame["bind_group_switches"], 2, "{gpu:#}");
    assert_eq!(per_frame["upload_bytes"], 96, "{gpu:#}");
}

/// Waits up to `limit` for `child` to exit.
fn wait(child: &mut Child, limit: Duration) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// Runs xdotool, killed after 10 s, so no step can hang the test; its
/// output on success.
fn xdotool(args: &[&str]) -> Option<String> {
    let mut child = Command::new("xdotool")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let status = wait(&mut child, Duration::from_secs(10));
    if status.is_none() {
        let _ = child.kill();
    }
    let out = child.wait_with_output().ok()?;
    status
        .is_some_and(|s| s.success())
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Escape through the OS's own input path (XTEST, rung 1b amendment 37):
/// the window gets focus, then the key, and `play` exits 0 within 5 s with
/// its report written. On Linux with X11 and `xdotool` (CI's ubuntu-24.04
/// `esc` step); with `ENG_REQUIRE_DISPLAY=1` on Linux, a missing display or
/// xdotool fails. The headless twin covers every target.
#[test]
fn escape_from_the_keyboard_quits_within_5_seconds() {
    let linux = cfg!(target_os = "linux");
    let has_x = std::env::var_os("DISPLAY").is_some();
    let has_xdotool = xdotool(&["version"]).is_some();
    if !(linux && has_x && has_xdotool) {
        assert!(
            !(linux && eng_platform_winit::display_required()),
            "[E-NO-DISPLAY] ENG_REQUIRE_DISPLAY=1, but DISPLAY set: {has_x}, xdotool: {has_xdotool}"
        );
        eprintln!("skipped: needs X11 and xdotool");
        return;
    }
    let dir = scratch("escape");
    let mut child = command(&dir, &["play", "--report", "r.json"])
        // xdotool sees X11 windows only, and winit takes Wayland when it can.
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The window shows once the GPU is open; a `play` that exits first (no
    // adapter) never shows it. It is found by the child's PID (winit sets
    // `_NET_WM_PID`), not by its title: the windowed hash-stream test runs
    // at the same time with the same title.
    let pid = child.id().to_string();
    let deadline = Instant::now() + Duration::from_secs(30);
    let window = loop {
        let found = xdotool(&["search", "--onlyvisible", "--pid", &pid]);
        if let Some(id) = found.and_then(|ids| ids.lines().next().map(str::to_string)) {
            break Some(id);
        }
        if child.try_wait().unwrap().is_some() || Instant::now() > deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let Some(window) = window else {
        let _ = child.kill();
        let out = child.wait_with_output().unwrap();
        if skipped(&out) {
            return;
        }
        panic!("no window of process {pid} within 30 s: {}", text(&out));
    };
    let focused = xdotool(&["windowfocus", "--sync", &window]).is_some();
    let sent = Instant::now();
    let pressed = focused && xdotool(&["key", "Escape"]).is_some();
    let status = if pressed {
        wait(&mut child, Duration::from_secs(5))
    } else {
        None
    };
    if status.is_none() {
        let _ = child.kill();
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        focused && pressed,
        "xdotool focused the window: {focused}, pressed Escape: {pressed}: {}",
        text(&out)
    );
    assert_eq!(
        status.and_then(|s| s.code()),
        Some(0),
        "exit within 5 s of Escape ({:?}): {}",
        sent.elapsed(),
        text(&out)
    );
    let report = report(&dir);
    assert_eq!(report["exit"], "ok");
    assert_eq!(report["ticks_requested"], Value::Null);
}

/// Without a display `play` exits 64 before its first tick, writing
/// nothing; with `ENG_REQUIRE_DISPLAY=1` too, since nothing can be shown.
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
#[test]
fn without_a_display_play_exits_64_and_writes_nothing() {
    let dir = scratch("no-display");
    let out = command(
        &dir,
        &[
            "play", "--ticks", "5", "--hashes", "h.txt", "--report", "r.json",
        ],
    )
    .env_remove("DISPLAY")
    .env_remove("WAYLAND_DISPLAY")
    .env_remove("WAYLAND_SOCKET")
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(64), "{}", text(&out));
    assert!(text(&out).contains("[E-NO-DISPLAY]"), "{}", text(&out));
    assert!(!dir.join("h.txt").exists() && !dir.join("r.json").exists());
}

/// `play`'s flags are checked before any window opens.
#[test]
fn bad_play_flags_exit_64_before_a_window_opens() {
    let dir = scratch("usage");
    std::fs::write(
        dir.join("bad.toml"),
        "version = 1\n[actions]\nfly = [\"KeyF\"]\n",
    )
    .unwrap();
    for args in [
        &["play", "--ticks"][..],
        &["play", "--players", "2"],
        &["play", "--input", "missing.toml"],
        &["play", "--bindings", "bad.toml"],
    ] {
        let out = command(&dir, args).output().unwrap();
        assert_eq!(out.status.code(), Some(64), "{args:?}: {}", text(&out));
        assert!(text(&out).contains("[E-USAGE]"), "{args:?}: {}", text(&out));
    }
    let out = command(&dir, &["play", "--bindings", "bad.toml"])
        .output()
        .unwrap();
    assert!(text(&out).contains("[E-BINDINGS]"), "{}", text(&out));
}
