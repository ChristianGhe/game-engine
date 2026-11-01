//! `play`'s headless twin (rung 1b, acceptance `esc-quits` and
//! `native-window`): eng-app's frame loop, the one that runs in the window,
//! on the headless platform with scripted keys, so every target checks it
//! without a display or a GPU. The windowed runs are `tests/play.rs`.
//!
//! - `play --input` gives the committed hash stream tick for tick, keys or
//!   not.
//! - Escape (`quit`) and a close request end the run with exit 0, its files
//!   written.
//! - Live keys become commands, sampled once per tick; losing focus
//!   releases them.
//! - Ticks a late frame cannot catch up are dropped, counted in the report
//!   and logged as `[W-TICK-DROP]`.
//! - W moves the dot up the top-down view and D to its right, and the
//!   recording of a live run passes the real binary's `replay verify`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use eng_app::play_on;
use eng_cli::{Exit, GameDef, PlayRun};
use eng_platform_api::{App, Control, Instant, Key, Platform, PlatformError, PlatformEvent, Wake};
use eng_platform_headless::HeadlessPlatform;
use eng_sim::{Sim, SimConfig};
use game_testbed_sim::Dot;
use kit_fps::MoveState;
use serde_json::Value;

/// The testbed's own definition (`src/main.rs`) without the top-down view,
/// which the headless platform never draws. Its commit is the engine's, as
/// there, so the binary's `replay verify` accepts the twin's recordings.
static GAME: GameDef = GameDef {
    name: "game-testbed",
    version: "0.0.0",
    build_sha: eng_cli::ENGINE_SHA,
    tick_rate: game_testbed_sim::TICK_RATE,
    build: game_testbed_sim::build,
    top_down: None,
    view_yaw: 0x4000,
};

/// A tick at 60 Hz in nanoseconds, as the frame loop's clock counts it.
const TICK: u64 = 16_666_666;

fn testbed(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("twin")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn args(dir: &Path, extra: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = ["play", "--seed", "1"].map(String::from).to_vec();
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

/// Runs `play` with `extra` flags on the headless platform with `script`.
fn play(name: &str, extra: &[&str], script: &[(u64, PlatformEvent)]) -> (Exit, PathBuf) {
    let dir = scratch(name);
    let run = PlayRun::start(&GAME, &args(&dir, extra)).unwrap_or_else(|exit| panic!("{exit:?}"));
    let mut platform = HeadlessPlatform::new()
        .script(
            script
                .iter()
                .map(|&(nanos, event)| (Instant::from_nanos(nanos), event)),
        )
        .time_limit(Instant::from_millis(60_000));
    (play_on(&mut platform, run), dir)
}

fn report(dir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join("r.json")).unwrap()).unwrap()
}

/// Player 0's applied `forward` for ticks 1, 2, ...
fn forwards(dir: &Path) -> Vec<i8> {
    let recording = eng_input::read_recording(&std::fs::read(dir.join("r.rec")).unwrap()).unwrap();
    recording.commands.iter().map(|c| c.forward).collect()
}

fn dot_square() -> String {
    testbed("scenarios/dot-square.input.toml")
        .display()
        .to_string()
}

#[test]
fn scripted_play_gives_the_committed_hash_stream() {
    let input = dot_square();
    // Keys that would move the dot change nothing with --input.
    let keys = [
        (TICK * 10, PlatformEvent::KeyDown(Key::KeyD)),
        (TICK * 100, PlatformEvent::KeyUp(Key::KeyD)),
    ];
    let (exit, dir) = play("scripted", &["--ticks", "240", "--input", &input], &keys);
    assert_eq!(exit, Exit::Ok);
    let ours = std::fs::read_to_string(dir.join("h.txt")).unwrap();
    let committed = std::fs::read_to_string(testbed("hashes/dot-square.seed1.txt")).unwrap();
    assert_eq!(ours, committed, "the twin's stream is the headless one");
    let report = report(&dir);
    assert_eq!(report["command"], "play");
    assert_eq!(report["platform"], "headless");
    assert_eq!(report["ticks_requested"], 240);
    assert_eq!(report["ticks_run"], 240);
    assert_eq!(report["dropped_ticks"], 0);
    assert_eq!(report["exit"], "ok");
    assert_eq!(report["gpu"], Value::Null);
}

#[test]
fn escape_quits_with_exit_0_and_the_files_written() {
    // Escape goes down half a second in: tick 30 has run by then, and tick
    // 31's sample sees `quit`, so the run ends without stepping it.
    let script = [(500_000_000, PlatformEvent::KeyDown(Key::Escape))];
    let (exit, dir) = play("escape", &[], &script);
    assert_eq!(exit, Exit::Ok);
    let report = report(&dir);
    assert_eq!(report["ticks_run"], 30);
    assert_eq!(
        report["ticks_requested"],
        Value::Null,
        "no --ticks: until quit"
    );
    assert_eq!(report["exit"], "ok");
    let hashes = std::fs::read_to_string(dir.join("h.txt")).unwrap();
    assert_eq!(hashes.lines().filter(|l| !l.starts_with('#')).count(), 31);
}

#[test]
fn a_tap_of_escape_between_two_ticks_still_quits() {
    let script = [
        (TICK * 10 + 1, PlatformEvent::KeyDown(Key::Escape)),
        (TICK * 10 + 2, PlatformEvent::KeyUp(Key::Escape)),
    ];
    let (exit, dir) = play("escape-tap", &[], &script);
    assert_eq!(exit, Exit::Ok);
    assert_eq!(report(&dir)["ticks_run"], 10);
}

#[test]
fn escape_quits_a_scripted_run_too() {
    let input = dot_square();
    let script = [(TICK * 50 + 3, PlatformEvent::KeyDown(Key::Escape))];
    let (exit, dir) = play(
        "escape-scripted",
        &["--ticks", "240", "--input", &input],
        &script,
    );
    assert_eq!(exit, Exit::Ok);
    assert_eq!(report(&dir)["ticks_run"], 50);
}

#[test]
fn a_close_request_quits() {
    let script = [(TICK * 20 + 3, PlatformEvent::CloseRequested)];
    let (exit, dir) = play("close", &[], &script);
    assert_eq!(exit, Exit::Ok);
    assert_eq!(report(&dir)["ticks_run"], 20);
}

#[test]
fn ticks_closes_the_run_after_its_last_tick() {
    let (exit, dir) = play("ticks", &["--ticks", "12"], &[]);
    assert_eq!(exit, Exit::Ok);
    assert_eq!(report(&dir)["ticks_run"], 12);
    assert_eq!(forwards(&dir), vec![0; 12]);
}

#[test]
fn live_keys_become_one_command_per_tick() {
    // W from just after tick 3 to just after tick 6: ticks 4 to 6 see it
    // held when sampled, and tick 7 sees it already up.
    let script = [
        (TICK * 3 + 1, PlatformEvent::KeyDown(Key::KeyW)),
        (TICK * 6 + 1, PlatformEvent::KeyUp(Key::KeyW)),
        // A tap shorter than a tick counts for exactly one tick (tick 10).
        (TICK * 9 + 1, PlatformEvent::KeyDown(Key::ArrowUp)),
        (TICK * 9 + 2, PlatformEvent::KeyUp(Key::ArrowUp)),
        // S held, then focus lost: tick 13 moves back, 14 on do not.
        (TICK * 12 + 1, PlatformEvent::KeyDown(Key::KeyS)),
        (TICK * 13 + 1, PlatformEvent::FocusLost),
    ];
    let (exit, dir) = play("live", &["--ticks", "15"], &script);
    assert_eq!(exit, Exit::Ok);
    let mut expected = vec![0; 15];
    for tick in [4, 5, 6, 10] {
        expected[tick - 1] = 127;
    }
    expected[13 - 1] = -127;
    assert_eq!(forwards(&dir), expected);
}

/// The dot's position (x, y) at ticks 0, 1, 2, ... of a run's recording:
/// a fresh simulation stepped through the commands the server applied.
fn dot_path(dir: &Path) -> Vec<(f64, f64)> {
    let recording = eng_input::read_recording(&std::fs::read(dir.join("r.rec")).unwrap()).unwrap();
    let config = SimConfig {
        tick_rate: GAME.tick_rate,
        players: 1,
        seed: recording.header.seed,
    };
    let mut sim = (GAME.build)(config).unwrap();
    let dot = |sim: &Sim| {
        let world = sim.world();
        let (dot, Dot) = world.storage::<Dot>().iter().next().expect("one dot");
        let pos = world.get::<MoveState>(dot).expect("a dot moves").pos;
        (pos.x, pos.y)
    };
    let mut path = vec![dot(&sim)];
    for command in &recording.commands {
        sim.step(std::slice::from_ref(command)).unwrap();
        path.push(dot(&sim));
    }
    path
}

/// How long a child process may run before it is killed: far longer than
/// any run here takes, and far shorter than a CI job's limit.
const CHILD_LIMIT: Duration = Duration::from_secs(120);

/// The real binary's `replay verify` of `recording`: it steps the
/// simulation directly from the recorded commands and checks every hash.
/// Killed after [`CHILD_LIMIT`], so it cannot hang the test.
fn replay_verify(recording: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args(["replay", "verify"])
        .arg(recording)
        .env("ENG_AGENT_LOG_DIR", recording.with_file_name("logs"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run game-testbed");
    let start = std::time::Instant::now();
    let finished = loop {
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        if start.elapsed() > CHILD_LIMIT {
            let _ = child.kill();
            break false;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        finished,
        "replay verify killed after {CHILD_LIMIT:?}: {text}"
    );
    out
}

/// The top-down view draws +X to the right and +Y up, so `play` holds the
/// game's view yaw for live input: 0x4000 (90 degrees, +Y) for the testbed,
/// with which W (`move_forward`) moves the dot up the view and D
/// (`move_right`) to its right. At yaw 0 W moved it right and D down (rung
/// 1b, T15's first review). The recording of the live run then passes the
/// real binary's `replay verify`: live play is the same simulation.
#[test]
fn w_moves_the_dot_up_the_view_and_d_to_its_right() {
    // W for ticks 1 to 15, D for ticks 41 to 55; the dot stops in between.
    let script = [
        (1, PlatformEvent::KeyDown(Key::KeyW)),
        (TICK * 15 + 1, PlatformEvent::KeyUp(Key::KeyW)),
        (TICK * 40 + 1, PlatformEvent::KeyDown(Key::KeyD)),
        (TICK * 55 + 1, PlatformEvent::KeyUp(Key::KeyD)),
    ];
    let (exit, dir) = play("view-yaw", &["--ticks", "80"], &script);
    assert_eq!(exit, Exit::Ok);
    let path = dot_path(&dir);
    let moved = |from: usize, to: usize| (path[to].0 - path[from].0, path[to].1 - path[from].1);
    let (dx, dy) = moved(0, 40);
    assert!(
        dy > 0.3 && dx.abs() < 0.01,
        "W moved the dot by ({dx}, {dy}), not up the view (+Y)"
    );
    let (dx, dy) = moved(40, 80);
    assert!(
        dx > 0.3 && dy.abs() < 0.01,
        "D moved the dot by ({dx}, {dy}), not to the view's right (+X)"
    );

    let out = replay_verify(&dir.join("r.rec"));
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn bindings_replace_the_defaults() {
    let dir = scratch("bindings-file");
    let bindings = dir.join("b.toml");
    std::fs::write(
        &bindings,
        "version = 1\n[actions]\nmove_forward = [\"KeyI\"]\nquit = [\"KeyQ\"]\n",
    )
    .unwrap();
    let bindings = bindings.display().to_string();
    let script = [
        (TICK * 2 + 1, PlatformEvent::KeyDown(Key::KeyW)),
        (TICK * 3 + 1, PlatformEvent::KeyDown(Key::KeyI)),
        // Escape is unbound now; Q quits.
        (TICK * 4 + 1, PlatformEvent::KeyDown(Key::Escape)),
        (TICK * 6 + 1, PlatformEvent::KeyDown(Key::KeyQ)),
    ];
    let (exit, dir) = play("bindings", &["--bindings", &bindings], &script);
    assert_eq!(exit, Exit::Ok);
    assert_eq!(report(&dir)["ticks_run"], 6);
    assert_eq!(forwards(&dir), vec![0, 0, 0, 127, 127, 127]);
}

#[test]
fn bad_bindings_are_a_usage_error_before_anything_runs() {
    let dir = scratch("bad-bindings");
    let bindings = dir.join("b.toml");
    std::fs::write(&bindings, "version = 1\n[actions]\nfly = [\"KeyF\"]\n").unwrap();
    let bindings = bindings.display().to_string();
    let result = PlayRun::start(&GAME, &args(&dir, &["--bindings", &bindings]));
    assert!(matches!(result, Err(Exit::Usage)));
}

/// A platform whose frames come when it says, whatever the app asks: here
/// at 0, then a second late, then a tick after that.
struct Late {
    frames: Vec<u64>,
}

struct LateClock {
    now: Instant,
    exit: bool,
}

impl Control for LateClock {
    fn now(&self) -> Instant {
        self.now
    }
    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Platform for Late {
    fn name(&self) -> &'static str {
        "late"
    }

    fn run(&mut self, app: &mut dyn App) -> Result<(), PlatformError> {
        let mut ctx = LateClock {
            now: Instant::ZERO,
            exit: false,
        };
        app.start(&mut ctx);
        for &nanos in &self.frames {
            ctx.now = Instant::from_nanos(nanos);
            let _: Wake = app.frame(&mut ctx);
            if ctx.exit {
                break;
            }
        }
        app.stop(&ctx);
        Ok(())
    }
}

#[test]
fn ticks_a_late_frame_cannot_catch_up_are_dropped_and_reported() {
    let dir = scratch("late");
    let run = PlayRun::start(&GAME, &args(&dir, &["--ticks", "100"])).unwrap();
    let mut late = Late {
        frames: vec![0, 60 * TICK + 5, 61 * TICK],
    };
    let exit = play_on(&mut late, run);
    assert_eq!(exit, Exit::Ok);
    let report = report(&dir);
    // A second late: 60 ticks due, 5 run and 55 dropped; then one more.
    assert_eq!(report["ticks_run"], 6);
    assert_eq!(report["dropped_ticks"], 55);
    assert_eq!(report["platform"], "late");
}

#[test]
fn a_frame_reads_the_previous_tick_and_the_newest() {
    let dir = scratch("frame");
    let mut run = PlayRun::start(&GAME, &args(&dir, &["--ticks", "5"])).unwrap();
    let tick0 = run.frame(0.0, 8, 8);
    assert_eq!((tick0.previous.tick().0, tick0.current.tick().0), (0, 0));
    for t in 1..=3 {
        let command = eng_core::PlayerCommand {
            forward: 127,
            ..eng_core::PlayerCommand::idle(eng_core::Tick(t))
        };
        assert_eq!(run.step(command), eng_cli::Step::Next);
    }
    let frame = run.frame(0.25, 320, 240);
    assert_eq!((frame.previous.tick().0, frame.current.tick().0), (2, 3));
    assert_ne!(frame.previous.state_hash(), frame.current.state_hash());
    assert_eq!((frame.alpha, frame.width, frame.height), (0.25, 320, 240));
}

/// A platform that cannot run here, as winit's without a display.
struct Unavailable;

impl Platform for Unavailable {
    fn name(&self) -> &'static str {
        "unavailable"
    }

    fn run(&mut self, app: &mut dyn App) -> Result<(), PlatformError> {
        let _ = app;
        Err(PlatformError::Unavailable("no display here".into()))
    }
}

#[test]
fn a_platform_that_cannot_run_exits_64_and_writes_nothing() {
    let dir = scratch("unavailable");
    let run = PlayRun::start(&GAME, &args(&dir, &["--ticks", "5"])).unwrap();
    assert_eq!(play_on(&mut Unavailable, run), Exit::Usage);
    for file in ["h.txt", "r.json", "r.rec"] {
        assert!(!dir.join(file).exists(), "{file} written");
    }
}
