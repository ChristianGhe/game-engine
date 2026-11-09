//! `play --map room`, the first-person window (rung 2, T15; criteria 1,
//! 9 and 10), in two halves:
//!
//! - **The headless twin**: eng-app's first-person frame loop on the
//!   headless platform with scripted events, on every target without a
//!   display or a GPU. A scripted run gives `run --headless`'s hash stream;
//!   raw mouse motion turns the live view by mouse look's rules and
//!   reaches the simulation only in each tick's command; the live view
//!   starts at the spawn's yaw, not the top-down view's; focus loss drops
//!   unsampled motion and frees the cursor; the flags reach the commands;
//!   the report names the window's requests and the mouse's rate.
//! - **The window** (a child process, as `tests/play.rs` runs the dots):
//!   `play --map room --windowed` with the room's walk gives `run
//!   --headless`'s stream (the committed one lands at G5 with
//!   `room-scenarios.patch`, so the two runs are compared with each other),
//!   and its report names the window. Without a display or an adapter it
//!   skips, unless `ENG_REQUIRE_DISPLAY=1` and `ENG_REQUIRE_GPU=1`. A
//!   scripted run never grabs the cursor, and `--windowed` keeps it off
//!   fullscreen, so the test leaves the desktop it runs on alone.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant as WallClock};

use eng_app::{play_on, play_on_with_callbacks};
use eng_cli::{Exit, GameDef, GameMap, GameMaps, MapView, PlayRun};
use eng_core::PlayerCommand;
use eng_platform_api::{Instant, Key, PlatformEvent, Request};
use eng_platform_headless::HeadlessPlatform;
use serde_json::Value;

/// The testbed's definition and maps (`src/main.rs`), with no top-down
/// view: the headless platform draws nothing, and the room's loop never
/// reads it.
static GAME: GameDef = GameDef {
    name: "game-testbed",
    version: "0.0.0",
    build_sha: eng_cli::ENGINE_SHA,
    tick_rate: game_testbed_sim::TICK_RATE,
    build: game_testbed_sim::build,
    top_down: None,
    view_yaw: 0x4000,
};

static MAPS: GameMaps = GameMaps {
    first: "dots",
    more: &[GameMap {
        name: "room",
        build: game_testbed_sim::room::build,
        view: MapView::FirstPerson(game_testbed_client::ROOM),
    }],
};

/// A tick at 60 Hz in nanoseconds, as the frame loop's clock counts it.
const TICK: u64 = 16_666_666;

fn testbed(path: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(path)
        .display()
        .to_string()
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("play-room")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn args(dir: &Path, extra: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = ["play", "--map", "room", "--seed", "1"]
        .map(String::from)
        .to_vec();
    args.extend(extra.iter().map(|s| s.to_string()));
    for (flag, file) in [
        ("--hashes", "h.txt"),
        ("--report", "r.json"),
        ("--record", "r.rec"),
    ] {
        args.push(flag.into());
        args.push(dir.join(file).display().to_string());
    }
    args
}

fn script(events: &[(u64, PlatformEvent)]) -> HeadlessPlatform {
    HeadlessPlatform::new()
        .script(
            events
                .iter()
                .map(|&(nanos, event)| (Instant::from_nanos(nanos), event)),
        )
        .time_limit(Instant::from_millis(60_000))
}

/// Runs `play --map room` with `extra` on the headless platform with
/// `events`; the exit, the directory and the requests the app made.
fn twin(
    name: &str,
    extra: &[&str],
    events: &[(u64, PlatformEvent)],
) -> (Exit, PathBuf, Vec<Request>) {
    let dir = scratch(name);
    let run = PlayRun::start_with_maps(&GAME, &MAPS, &args(&dir, extra))
        .unwrap_or_else(|exit| panic!("{exit:?}"));
    let mut platform = script(events);
    let exit = play_on(&mut platform, run);
    let requests = platform.requests().iter().map(|&(_, r)| r).collect();
    (exit, dir, requests)
}

fn report(dir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join("r.json")).unwrap()).unwrap()
}

fn commands(dir: &Path) -> Vec<PlayerCommand> {
    eng_input::read_recording(&std::fs::read(dir.join("r.rec")).unwrap())
        .unwrap()
        .commands
}

/// The commands' `(yaw, pitch)` for ticks 1, 2, ...
fn angles(dir: &Path) -> Vec<(u16, u16)> {
    commands(dir)
        .iter()
        .map(|c| (c.view_yaw, c.view_pitch))
        .collect()
}

/// `-units` as a binary angle.
const fn neg(units: u16) -> u16 {
    0u16.wrapping_sub(units)
}

fn motion(dx: f64, dy: f64) -> PlatformEvent {
    PlatformEvent::MouseMotion { dx, dy }
}

/// The real binary's `run --headless` of the room's walk at seed 1, its
/// stream in `dir/run.txt`.
fn headless_walk(dir: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args([
            "run",
            "--headless",
            "--map",
            "room",
            "--ticks",
            "600",
            "--seed",
            "1",
        ])
        .args(["--input", &testbed("scenarios/room-walk.input.toml")])
        .args(["--hashes", "run.txt"])
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"))
        .output()
        .expect("run game-testbed");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    std::fs::read_to_string(dir.join("run.txt")).unwrap()
}

#[test]
fn a_scripted_twin_gives_run_headless_stream() {
    let walk = testbed("scenarios/room-walk.input.toml");
    // Motion and keys change nothing with --input.
    let events = [
        (1, PlatformEvent::FocusGained),
        (TICK * 10, motion(500.0, -300.0)),
        (TICK * 20, PlatformEvent::KeyDown(Key::KeyD)),
    ];
    let (exit, dir, requests) = twin("scripted", &["--ticks", "600", "--input", &walk], &events);
    assert_eq!(exit, Exit::Ok);
    let ours = std::fs::read_to_string(dir.join("h.txt")).unwrap();
    assert_eq!(ours, headless_walk(&dir), "the twin's stream is run's");
    // A scripted run never grabs; the headless platform has no window to
    // make fullscreen.
    assert_eq!(requests, []);
    let report = report(&dir);
    assert_eq!(report["map"], "room");
    assert_eq!(report["ticks_run"], 600);
    assert_eq!(report["gpu"], Value::Null);
    let window = &report["window"];
    assert_eq!(window["view"], "first-person");
    assert_eq!(window["grab"], "none");
    assert_eq!(window["fullscreen_asked"], false);
    assert_eq!(window["grab_asked"], false);
    assert_eq!(window["refresh_rate_mhz"], Value::Null);
    assert_eq!(window["frame_period_ns"], TICK);
    assert_eq!(window["frame"], Value::Null);
    assert_eq!(window["gcmouse"], Value::Null);
}

/// Expected angles worked by hand (eng-input's `mouse_look.rs`): N counts
/// at 0.066° turn `round(N × 22528 / 1875)` units, so 100 counts 1201.49,
/// 1201 units, and 50 counts 600.75, 601.
#[test]
fn mouse_motion_turns_the_live_view_and_reaches_the_sim_in_commands() {
    let events = [
        (1, PlatformEvent::FocusGained),
        // Before tick 3: right, which lowers yaw.
        (TICK * 2 + 1, motion(60.0, 0.0)),
        (TICK * 2 + 2, motion(40.0, 0.0)),
        // Before tick 6: away from the user, which raises pitch.
        (TICK * 5 + 1, motion(0.0, -50.0)),
        // Before tick 8, then dropped by the focus loss before its sample.
        (TICK * 7 + 1, motion(1000.0, 1000.0)),
        (TICK * 7 + 2, PlatformEvent::FocusLost),
        // W held from tick 9: forward along the live yaw.
        (TICK * 8 + 1, PlatformEvent::FocusGained),
        (TICK * 8 + 2, PlatformEvent::KeyDown(Key::KeyW)),
    ];
    let (exit, dir, requests) = twin("motion", &["--ticks", "10"], &events);
    assert_eq!(exit, Exit::Ok);
    let yaw = neg(1201);
    let mut expected = vec![(0, 0); 2];
    expected.extend([(yaw, 0); 3]);
    expected.extend([(yaw, 601); 5]);
    // The spawn's yaw, 0, not GameDef::view_yaw's 0x4000.
    assert_eq!(angles(&dir), expected);
    let forward: Vec<i8> = commands(&dir).iter().map(|c| c.forward).collect();
    assert_eq!(forward, [0, 0, 0, 0, 0, 0, 0, 0, 127, 127]);
    // Grabbed on each focus, freed on its loss (headless grants nothing).
    assert_eq!(
        requests,
        [
            Request::GrabCursor(true),
            Request::GrabCursor(false),
            Request::GrabCursor(true),
        ]
    );
    let window = &report(&dir)["window"];
    assert_eq!(window["focus_lost"], 1);
    assert_eq!(window["grab_asked"], true);
    assert_eq!(window["grab"], "none");
}

/// 100 counts at 0.022° turn 400.5 units, so 400; 1000 counts down,
/// inverted, look up 22°, past a 10° limit of 1820 units (10 × 65536 /
/// 360 = 1820.4, rounded down).
#[test]
fn the_look_flags_reach_the_commands() {
    let events = [
        (1, PlatformEvent::FocusGained),
        (TICK + 1, motion(100.0, 1000.0)),
    ];
    let flags = [
        "--ticks",
        "2",
        "--sensitivity",
        "0.022",
        "--invert-pitch",
        "--pitch-limit",
        "10",
    ];
    let (exit, dir, _) = twin("flags", &flags, &events);
    assert_eq!(exit, Exit::Ok);
    assert_eq!(angles(&dir), [(0, 0), (neg(400), 1820)]);
}

#[test]
fn escape_quits_and_the_recording_replays() {
    let events = [
        (1, PlatformEvent::FocusGained),
        (TICK * 3 + 1, motion(-250.0, 80.0)),
        (TICK * 5 + 1, PlatformEvent::KeyDown(Key::KeyW)),
        (TICK * 30 + 1, PlatformEvent::KeyDown(Key::Escape)),
    ];
    let (exit, dir, _) = twin("escape", &[], &events);
    assert_eq!(exit, Exit::Ok);
    let report = report(&dir);
    assert_eq!(report["ticks_run"], 30);
    assert_eq!(report["ticks_requested"], Value::Null);
    let out = Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args(["replay", "verify"])
        .arg(dir.join("r.rec"))
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
}

/// The callback count is read once a frame, and only its growth with
/// focus counts: here 10 a read, at a frame a tick on the headless clock,
/// so 600 a second.
#[test]
fn the_callback_rate_is_read_each_frame_while_focused() {
    let dir = scratch("callbacks");
    let reads = Cell::new(0u64);
    let count = || {
        reads.set(reads.get() + 10);
        reads.get()
    };
    let run = PlayRun::start_with_maps(&GAME, &MAPS, &args(&dir, &["--ticks", "180"])).unwrap();
    let mut platform = script(&[
        (1, PlatformEvent::FocusGained),
        (TICK * 150 + 1, PlatformEvent::FocusLost),
    ]);
    assert_eq!(
        play_on_with_callbacks(&mut platform, run, Some(&count as &dyn Fn() -> u64)),
        Exit::Ok
    );
    let gcmouse = &report(&dir)["window"]["gcmouse"];
    assert_eq!(gcmouse["peak_per_second"], 600, "{gcmouse:#}");
    // Focused from just after 0 to tick 150: 2.5 s of frames, 10 apiece.
    let callbacks = gcmouse["callbacks"].as_u64().unwrap();
    assert!((1_490..=1_500).contains(&callbacks), "{gcmouse:#}");
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// `true` if the run could not open a window or a GPU and may skip, as
/// `tests/play.rs` decides.
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

/// How long the window's child may run: its 600 ticks are 10 s.
const LIMIT: Duration = Duration::from_secs(120);

/// Waits up to [`LIMIT`] for `child`, killing it after.
fn finish(mut child: Child) -> Output {
    let start = WallClock::now();
    let mut done = false;
    while start.elapsed() < LIMIT {
        if child.try_wait().unwrap().is_some() {
            done = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !done {
        let _ = child.kill();
    }
    let out = child.wait_with_output().unwrap();
    assert!(done, "killed after {LIMIT:?}: {}", text(&out));
    out
}

/// Criterion 1's command line (`room-window`), compared with `run
/// --headless` until the committed stream lands at G5.
#[test]
fn the_windowed_room_gives_run_headless_stream() {
    let dir = scratch("window");
    let child = Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args([
            "play",
            "--map",
            "room",
            "--windowed",
            "--ticks",
            "600",
            "--seed",
            "1",
        ])
        .args(["--input", &testbed("scenarios/room-walk.input.toml")])
        .args(["--hashes", "h.txt", "--report", "r.json"])
        .current_dir(&dir)
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run game-testbed");
    let out = finish(child);
    if skipped(&out) {
        return;
    }
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(text(&out).lines().count() <= 20, "console stays short");
    let ours = std::fs::read_to_string(dir.join("h.txt")).unwrap();
    assert_eq!(ours, headless_walk(&dir), "the window's stream is run's");

    let report = report(&dir);
    assert_eq!(report["command"], "play");
    assert_eq!(report["platform"], "winit");
    assert_eq!(report["map"], "room");
    assert_eq!(report["ticks_run"], 600);
    let window = &report["window"];
    assert_eq!(window["view"], "first-person");
    assert_eq!(window["fullscreen_asked"], false, "{window:#}");
    assert_eq!(window["grab_asked"], false, "a scripted run never grabs");
    assert_eq!(window["grab"], "none");
    let period = window["frame_period_ns"].as_u64().unwrap();
    match window["refresh_rate_mhz"].as_u64() {
        Some(mhz) => assert_eq!(period, 1_000_000_000_000 / mhz, "{window:#}"),
        None => assert_eq!(period, TICK, "{window:#}"),
    }
    let gpu = &report["gpu"];
    if gpu["frames"] == 0 && !eng_platform_winit::display_required() {
        // A locked or sleeping Mac screen shows the window occluded, and
        // nothing is drawn (as tests/play.rs allows); CI must present.
        eprintln!("no frame presented: the window was never visible");
        return;
    }
    let frames = gpu["frames"].as_u64().unwrap();
    assert!(frames > 0, "{gpu:#}");
    // At most a frame per refresh (a tick without one), and a few more for
    // the slack and the run's ends: never one per event.
    let seconds = 10.5;
    let most = seconds * 1e9 / period as f64 * 16.0 / 15.0 + 5.0;
    assert!((frames as f64) < most, "{frames} frames, period {period}");
    // T11's room: one chunk and three things, through the look's graph,
    // then the present.
    assert!(gpu["per_frame"]["draws"].as_u64().unwrap() >= 7, "{gpu:#}");
    let frame = &window["frame"];
    assert_eq!(
        frame["upscale"], 4,
        "Look::ON draws each texel 4x4: {window:#}"
    );
    assert_eq!(
        frame["near_m"].as_f64().map(|n| n as f32),
        Some(0.1),
        "{window:#}"
    );
    if cfg!(target_os = "macos") {
        assert!(window["gcmouse"].is_object(), "{window:#}");
    }
}
