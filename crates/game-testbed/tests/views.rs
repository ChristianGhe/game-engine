//! `shot`'s and `video`'s view flags (rung 2, T16 and plan addition A3),
//! run against the real binary: `--view` refuses a view the map lacks,
//! `--pose` and `--look` a top-down map, and a bad look file, all before a
//! GPU opens (exit 64); `--pose` draws the room from a camera independent
//! of movement and the report's `view` says so; a budgets file's
//! `[gpu.first_person]` table holds the room's frames.
//!
//! Tests that render need an adapter. Without one they skip, unless
//! `ENG_REQUIRE_GPU=1` (CI's native jobs) makes them fail.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// How long a run may take.
const LIMIT: Duration = Duration::from_secs(120);

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("views")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn probe_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_game-testbed"));
    command
        .args(args)
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"))
        .envs(env.iter().copied());
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run game-testbed");
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > LIMIT {
            let _ = child.kill();
            panic!("{args:?} killed after {LIMIT:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
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

/// Runs a command that renders: `None` without an adapter, unless CI
/// requires one.
fn render(dir: &Path, args: &[&str]) -> Option<Output> {
    let out = probe_env(dir, args, &[]);
    let no_adapter = out.status.code() == Some(64) && text(&out).contains("[E-NO-ADAPTER]");
    if no_adapter && std::env::var("ENG_REQUIRE_GPU").as_deref() != Ok("1") {
        eprintln!("skipped: {}", text(&out));
        return None;
    }
    Some(out)
}

/// The committed look file with every switch off.
fn off_look() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("looks/off.look.toml");
    path.to_string_lossy().into_owned()
}

/// Each is refused before the session or a GPU opens: with no adapter to
/// be had, the message is still the refusal, never `[E-NO-ADAPTER]`, and
/// nothing is written.
#[test]
fn views_a_map_lacks_and_first_person_flags_on_the_dots_are_refused_first() {
    let dir = scratch("refused");
    std::fs::write(dir.join("bad.look.toml"), "version = 1\nfogg = false\n").unwrap();
    let look = off_look();
    let room = ["shot", "--map", "room", "--at-tick", "0", "--out", "s.png"];
    let dots = ["shot", "--at-tick", "0", "--out", "s.png"];
    for (base, extra, says) in [
        (
            &room[..],
            &["--view", "top-down"][..],
            "[E-USAGE] [E-MAP] game-testbed's map \"room\" has no top-down view (its view is first-person)",
        ),
        (
            &dots[..],
            &["--view", "first-person"][..],
            "[E-USAGE] [E-MAP] game-testbed's map \"dots\" has no first-person view (its view is top-down)",
        ),
        (
            &dots[..],
            &["--pose", "1,1,1.6,0,0"][..],
            "[E-USAGE] --pose needs a first-person view; game-testbed's map \"dots\" is drawn top-down",
        ),
        (
            &dots[..],
            &["--look", look.as_str()][..],
            "[E-USAGE] --look needs a first-person view; game-testbed's map \"dots\" is drawn top-down",
        ),
        (
            &room[..],
            &["--look", "bad.look.toml"][..],
            "[E-LOAD] bad.look.toml",
        ),
        (
            &room[..],
            &["--look", "missing.look.toml"][..],
            "cannot read missing.look.toml",
        ),
        (
            &room[..],
            &["--pose", "1,1,1.6,0"][..],
            "--pose needs X,Y,Z,YAW,PITCH",
        ),
        (
            &room[..],
            &["--view", "sideways"][..],
            "--view is top-down or first-person",
        ),
    ] {
        let args = [base, extra].concat();
        let none = [("ENG_GPU_ADAPTER", "no adapter is called this")];
        let out = probe_env(&dir, &args, &none);
        assert_eq!(out.status.code(), Some(64), "{args:?}: {}", text(&out));
        assert!(text(&out).contains(says), "{args:?}: {}", text(&out));
        assert!(!text(&out).contains("[E-NO-ADAPTER]"), "{args:?}");
        assert!(!dir.join("s.png").exists(), "{args:?} wrote a shot");
    }
    // `video` takes them as `shot` does.
    let video = ["video", "--map", "room", "--ticks", "2", "--out", "v.apng"];
    let out = probe_env(&dir, &[&video[..], &["--view", "top-down"]].concat(), &[]);
    assert_eq!(out.status.code(), Some(64), "{}", text(&out));
    assert!(
        text(&out).contains("has no top-down view"),
        "{}",
        text(&out)
    );
}

#[test]
fn a_pose_draws_the_room_from_a_camera_independent_of_movement() {
    let dir = scratch("pose");
    let ledge = "9,3.5,1.6,-90,-15";
    let walk = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/room-walk.input.toml");
    let walk = walk.to_string_lossy().into_owned();
    let shot = |name: &str, tick: &str, extra: &[&str]| {
        let base = ["shot", "--map", "room", "--seed", "1", "--at-tick", tick];
        let out = [format!("{name}.png"), format!("{name}.json")];
        let files = ["--out", out[0].as_str(), "--report", out[1].as_str()];
        render(&dir, &[&base[..], &files[..], extra].concat())
    };
    // The same pose at tick 0 and after 300 ticks of the walk.
    let Some(out) = shot("still", "0", &["--pose", ledge]) else {
        return;
    };
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let moved = shot("moved", "300", &["--pose", ledge, "--input", &walk]).unwrap();
    assert_eq!(moved.status.code(), Some(0), "{}", text(&moved));
    let view = read_json(&dir.join("moved.json"))["view"].clone();
    assert_eq!(view["kind"], "first-person");
    assert_eq!(view["camera_from"], "pose");
    let pose = json!({"eye": [9.0, 3.5, 1.6], "yaw_deg": -90.0, "pitch_deg": -15.0});
    assert_eq!(view["pose"], pose);
    assert_eq!(view["camera"]["eye"], pose["eye"]);
    assert_eq!(view["camera"]["yaw_deg"], 270.0);
    let pitch = view["camera"]["pitch_deg"].as_f64().unwrap();
    assert!((pitch + 15.0).abs() < 1e-6, "{view}");
    assert_eq!(view["look"], Value::Null);
    // Player 0 moved, but the camera did not.
    let player = shot("player", "300", &["--input", &walk]).unwrap();
    assert_eq!(player.status.code(), Some(0), "{}", text(&player));
    let report = read_json(&dir.join("player.json"));
    assert_eq!(report["view"]["camera_from"], "game");
    assert_ne!(report["view"]["camera"]["eye"], pose["eye"]);
    let pixels = |name: &str| std::fs::read(dir.join(format!("{name}.png"))).unwrap();
    assert_ne!(pixels("player"), pixels("moved"));
    // The marker turns between tick 0 and 300 and is not in the ledge's
    // view, so the pose's two pictures are the same.
    assert_eq!(pixels("still"), pixels("moved"));

    // A look file is read and named.
    let look = off_look();
    let off = shot("off", "0", &["--pose", ledge, "--look", &look]).unwrap();
    assert_eq!(off.status.code(), Some(0), "{}", text(&off));
    let report = read_json(&dir.join("off.json"));
    assert_eq!(report["files_read"], json!({"look": 1}));
    assert_eq!(report["view"]["look"]["palette"], false);
    assert_ne!(pixels("off"), pixels("still"));
    // `--view` naming the map's own view changes nothing.
    let named = shot("named", "0", &["--pose", ledge, "--view", "first-person"]).unwrap();
    assert_eq!(named.status.code(), Some(0), "{}", text(&named));
    assert_eq!(pixels("named"), pixels("still"));
    // The dots' shot names its view too.
    let dots = [
        "shot",
        "--at-tick",
        "0",
        "--out",
        "d.png",
        "--report",
        "d.json",
    ];
    let out = render(&dir, &dots).unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_eq!(
        read_json(&dir.join("d.json"))["view"],
        json!({"kind": "top-down", "camera_from": "game", "pose": null, "look": null,
            "camera": null})
    );
}

/// `budgets.toml`'s tables today, and a `[gpu.first_person]` table as G5
/// would add it.
const GPU: &str = "version = 1\n[gpu]\nmax_draws_per_frame = 12\n\
    max_pipeline_switches_per_frame = 12\nmax_bind_group_switches_per_frame = 8\n\
    max_upload_bytes_per_frame = 2048\n";
const FIRST_PERSON: &str = "[gpu.first_person]\nmax_draws_per_frame = 7\n\
    max_pipeline_switches_per_frame = 7\nmax_bind_group_switches_per_frame = 15\n\
    max_upload_bytes_per_frame = 736\n";

/// The room's frame from spawn 0: 7 draws (its one chunk, the sprites,
/// light, sky, fog, the tone map and the palette), 7 pipeline switches, 15
/// bind-group switches and 736 bytes (the 624-byte uniform, 16 for the
/// chunk and 32 for each of the 3 sprites), so a table at exactly those
/// holds it and one byte less does not. `[gpu]` alone holds it too, and
/// its 8 bind-group switches are too few.
#[test]
fn a_first_person_budget_table_holds_the_rooms_frames() {
    let dir = scratch("budgets");
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
    write("gpu.toml", GPU);
    write("both.toml", &format!("{GPU}{FIRST_PERSON}"));
    let tight = FIRST_PERSON.replace("= 736", "= 735");
    write("tight.toml", &format!("{GPU}{tight}"));
    let shot = |budgets: &str| {
        let args = ["shot", "--map", "room", "--at-tick", "60", "--out", "r.png"];
        let more = ["--budgets", budgets, "--report", "r.json"];
        render(&dir, &[&args[..], &more[..]].concat())
    };
    let Some(out) = shot("both.toml") else {
        return;
    };
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let report = read_json(&dir.join("r.json"));
    let per_frame = &report["gpu"]["per_frame"];
    assert_eq!(
        *per_frame,
        json!({"draws": 7, "pipeline_switches": 7, "bind_group_switches": 15,
            "upload_bytes": 736})
    );
    assert_eq!(
        report["budgets"]["first_person"],
        json!({"max_draws_per_frame": 7, "max_pipeline_switches_per_frame": 7,
            "max_bind_group_switches_per_frame": 15, "max_upload_bytes_per_frame": 736})
    );
    assert_eq!(report["budgets"]["max_bind_group_switches_per_frame"], 8);

    let out = shot("tight.toml").unwrap();
    assert_eq!(out.status.code(), Some(13), "{}", text(&out));
    let exceeded = "[E-BUDGET] gpu.first_person.max_upload_bytes_per_frame exceeded at tick 60: \
                    736 > 735";
    assert!(text(&out).contains(exceeded), "{}", text(&out));
    let out = shot("gpu.toml").unwrap();
    assert_eq!(out.status.code(), Some(13), "{}", text(&out));
    let exceeded = "[E-BUDGET] max_bind_group_switches_per_frame exceeded at tick 60: 15 > 8";
    assert!(text(&out).contains(exceeded), "{}", text(&out));
    assert_eq!(
        read_json(&dir.join("r.json"))["budgets"]["first_person"],
        Value::Null
    );
    // The dots' frames stay `[gpu]`'s, whatever the table says.
    let dots = [
        "shot",
        "--at-tick",
        "0",
        "--out",
        "d.png",
        "--budgets",
        "both.toml",
    ];
    let out = render(&dir, &dots).unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
}
