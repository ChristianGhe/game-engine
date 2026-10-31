//! `shot` and `video` (rung 1b, task T9: "`shot` writes the PNG and the
//! `gpu` report; `video` writes the APNG"), run against the real binary.
//! They render the testbed's extraction offscreen at alpha 1 (plan, "One
//! frame").
//!
//! Tests that render need an adapter. Without one they skip, unless
//! `ENG_REQUIRE_GPU=1` (CI's native jobs) makes them fail, as eng-gpu's do.
//! The tests that need none (no adapter, `[gpu]` in a budgets file) run in
//! CI's test job too.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

/// Amendment 30's `[gpu]` table (rung 1b), with today's tables.
const BUDGETS: &str = "version = 1\n[sim]\nmax_state_bytes = 4096\nmax_entities = 16\n\
    [gpu]\nmax_draws_per_frame = 12\nmax_pipeline_switches_per_frame = 12\n\
    max_bind_group_switches_per_frame = 8\nmax_upload_bytes_per_frame = 2048\n\
    [ci]\npr_tier_minutes = 15\nnative_job_minutes = 25\n";

/// The testbed's colours as the shot stores them: its linear colours,
/// sRGB-encoded (`src/top_down.rs`). Each is the centre of an 8-bit level,
/// so every adapter should store it exactly; one level of slack is allowed.
const DOT_0: [u8; 3] = [255, 153, 39];
const FLOOR: [u8; 3] = [53, 56, 60];
const WALL: [u8; 3] = [149, 149, 155];

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("shot_video")
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
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"));
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("run game-testbed")
}

fn probe(dir: &Path, args: &[&str]) -> Output {
    probe_env(dir, args, &[])
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

/// Runs a command that renders. `None` if there is no adapter and CI does
/// not require one.
fn render(dir: &Path, args: &[&str]) -> Option<Output> {
    let out = probe(dir, args);
    let no_adapter = out.status.code() == Some(64) && text(&out).contains("[E-NO-ADAPTER]");
    if no_adapter && std::env::var("ENG_REQUIRE_GPU").as_deref() != Ok("1") {
        eprintln!("skipped: {}", text(&out));
        return None;
    }
    Some(out)
}

fn assert_exit(out: &Output, code: i32) {
    assert_eq!(out.status.code(), Some(code), "{}", text(out));
}

/// A decoded PNG: width, height and RGBA8 rows, top first.
struct Picture {
    width: u32,
    height: u32,
    rgba8: Vec<u8>,
}

impl Picture {
    fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        let at = ((y * self.width + x) * 4) as usize;
        [self.rgba8[at], self.rgba8[at + 1], self.rgba8[at + 2]]
    }
}

fn reader(bytes: &[u8]) -> png::Reader<std::io::Cursor<&[u8]>> {
    png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap()
}

/// Every frame of a PNG or APNG, and the APNG's frame delay.
fn frames(path: &Path) -> (Vec<Picture>, Option<(u16, u16)>) {
    let bytes = std::fs::read(path).unwrap();
    let mut reader = reader(&bytes);
    let count = reader.info().animation_control.map_or(1, |a| a.num_frames);
    let mut pictures = Vec::new();
    let mut delay = None;
    for _ in 0..count {
        let mut rgba8 = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut rgba8).unwrap();
        assert_eq!(
            (info.color_type, info.bit_depth),
            (png::ColorType::Rgba, png::BitDepth::Eight)
        );
        delay = reader
            .info()
            .frame_control
            .map(|f| (f.delay_num, f.delay_den));
        pictures.push(Picture {
            width: info.width,
            height: info.height,
            rgba8,
        });
    }
    (pictures, delay)
}

fn close(got: [u8; 3], want: [u8; 3], what: &str) {
    let near = got.iter().zip(want).all(|(&g, w)| g.abs_diff(w) <= 1);
    assert!(near, "{what}: {got:?}, expected about {want:?}");
}

/// Where the dot is at `tick`, from `dump-world`.
fn dot_at(dir: &Path, tick: u64, seed: &str, input: Option<&str>) -> (f64, f64) {
    let world = dir.join(format!("world-{tick}.json"));
    let tick = tick.to_string();
    let mut args = vec![
        "dump-world",
        "--at-tick",
        &tick,
        "--seed",
        seed,
        "--out",
        world.to_str().unwrap(),
    ];
    if let Some(input) = input {
        args.extend(["--input", input]);
    }
    let out = probe(dir, &args);
    assert_exit(&out, 0);
    let entity = &read_json(&world)["world"]["entities"][0];
    let pos = &entity["components"]["kit_fps.MoveState"]["pos"];
    (pos["x"].as_f64().unwrap(), pos["y"].as_f64().unwrap())
}

#[test]
fn without_an_adapter_shot_and_video_exit_64() {
    let dir = scratch("no-adapter");
    let env = [("ENG_GPU_ADAPTER", "no adapter is called this")];
    for args in [
        [
            "shot",
            "--at-tick",
            "3",
            "--out",
            "out.png",
            "--report",
            "r.json",
        ],
        [
            "video", "--ticks", "3", "--out", "out.png", "--report", "r.json",
        ],
    ] {
        let out = probe_env(&dir, &args, &env);
        assert_exit(&out, 64);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.starts_with("[E-NO-ADAPTER] no GPU adapter's name contains"),
            "{stderr}"
        );
        assert!(!dir.join("out.png").exists() && !dir.join("r.json").exists());
    }
}

#[test]
fn run_reads_a_gpu_budgets_table() {
    let dir = scratch("gpu-table");
    std::fs::write(dir.join("budgets.toml"), BUDGETS).unwrap();
    let args = [
        "run",
        "--headless",
        "--ticks",
        "5",
        "--budgets",
        "budgets.toml",
        "--report",
        "r.json",
    ];
    let out = probe(&dir, &args);
    assert_exit(&out, 0);
    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["gpu"], Value::Null, "run renders nothing");
    assert_eq!(report["budgets"]["max_draws_per_frame"], 12);
    assert_eq!(report["budgets"]["max_upload_bytes_per_frame"], 2048);

    let unknown = BUDGETS.replace("max_draws_per_frame", "max_triangles_per_frame");
    std::fs::write(dir.join("budgets.toml"), unknown).unwrap();
    let out = probe(&dir, &args);
    assert_exit(&out, 64);
    assert!(
        text(&out).contains("max_triangles_per_frame"),
        "{}",
        text(&out)
    );
}

#[test]
fn shot_writes_the_png_and_the_gpu_report() {
    let dir = scratch("shot");
    let Some(out) = render(
        &dir,
        &[
            "shot",
            "--at-tick",
            "60",
            "--seed",
            "1",
            "--out",
            "shots/s.png",
            "--report",
            "r.json",
        ],
    ) else {
        return;
    };
    assert_exit(&out, 0);
    let console = String::from_utf8_lossy(&out.stdout);
    assert!(console.lines().count() <= 20, "{console}");
    assert!(console.contains("shots/s.png"), "{console}");

    let (pictures, delay) = frames(&dir.join("shots/s.png"));
    assert_eq!((pictures.len(), delay), (1, None), "a still image");
    let shot = &pictures[0];
    assert_eq!((shot.width, shot.height), (320, 240));
    assert!(shot.rgba8.chunks(4).all(|p| p[3] == 255), "opaque");
    // 40 px/m from the room's corner, rows top first.
    let (x, y) = dot_at(&dir, 60, "1", None);
    let (px, py) = ((x * 40.0) as u32, ((6.0 - y) * 40.0) as u32);
    close(shot.rgb(px, py), DOT_0, "the dot's centre");
    // Across the room from the dot, and inside the 4-pixel walls.
    let (fx, fy) = (
        if px < 160 { 280 } else { 40 },
        if py < 120 { 200 } else { 40 },
    );
    close(shot.rgb(fx, fy), FLOOR, "the floor");
    for (wx, wy) in [(1, 120), (318, 120), (160, 1), (160, 238)] {
        close(shot.rgb(wx, wy), WALL, "a wall");
    }

    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["command"], "shot");
    assert_eq!(report["exit"], "ok");
    assert_eq!(report["ticks_run"], 60);
    let gpu = &report["gpu"];
    assert!(!gpu["adapter"].as_str().unwrap().is_empty(), "{gpu}");
    assert!(
        ["vulkan", "metal", "dx12"].contains(&gpu["backend"].as_str().unwrap()),
        "{gpu}"
    );
    assert!(gpu["device_type"].is_string() && gpu["driver"].is_string());
    assert!(gpu["driver_info"].is_string() && gpu["vendor"].is_u64());
    assert!(gpu["device"].is_u64());
    assert_eq!(gpu["tier"], 0);
    let capabilities = gpu["capabilities"].as_object().unwrap();
    // serde_json's map sorts its keys.
    let names: Vec<&str> = capabilities.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "binding_arrays",
            "mesh_shaders",
            "multi_draw_indirect_count",
            "ray_query",
            "timestamp_queries",
        ]
    );
    assert!(capabilities.values().all(Value::is_boolean));
    assert_eq!(
        (gpu["width"].clone(), gpu["height"].clone()),
        (json!(320), json!(240))
    );
    assert_eq!(gpu["frames"], 1);
    // The room, its floor and one dot: 3 shapes of 32 bytes (T8).
    assert_eq!(
        gpu["per_frame"],
        json!({"draws": 2, "pipeline_switches": 2, "bind_group_switches": 1, "upload_bytes": 96})
    );
    assert_eq!(report["budgets"]["max_draws_per_frame"], Value::Null);
}

#[test]
fn shot_size_sets_the_image() {
    let dir = scratch("size");
    let Some(out) = render(
        &dir,
        &[
            "shot",
            "--at-tick",
            "0",
            "--size",
            "64x48",
            "--out",
            "s.png",
        ],
    ) else {
        return;
    };
    assert_exit(&out, 0);
    let (pictures, _) = frames(&dir.join("s.png"));
    assert_eq!((pictures[0].width, pictures[0].height), (64, 48));
    // 8 px/m fits the room exactly again; its corner pixel is wall.
    close(pictures[0].rgb(0, 0), WALL, "a wall");
}

#[test]
fn video_frames_are_the_shots_at_their_ticks() {
    let dir = scratch("video");
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/dot-square.input.toml");
    let input = input.to_str().unwrap();
    let Some(out) = render(
        &dir,
        &[
            "video",
            "--ticks",
            "60",
            "--every",
            "30",
            "--seed",
            "2",
            "--input",
            input,
            "--out",
            "proof.apng",
            "--report",
            "r.json",
        ],
    ) else {
        return;
    };
    assert_exit(&out, 0);
    let (video, delay) = frames(&dir.join("proof.apng"));
    assert_eq!(video.len(), 3, "ticks 0, 30 and 60");
    assert_eq!(delay, Some((1, 2)), "30 ticks at 60 Hz");
    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["command"], "video");
    assert_eq!(report["gpu"]["frames"], 3);
    assert_eq!(report["gpu"]["per_frame"]["upload_bytes"], 96);

    for (frame, tick) in video.iter().zip(["0", "30", "60"]) {
        let shot = format!("shot-{tick}.png");
        let args = [
            "shot",
            "--at-tick",
            tick,
            "--seed",
            "2",
            "--input",
            input,
            "--out",
            &shot,
        ];
        assert_exit(&probe(&dir, &args), 0);
        let (pictures, _) = frames(&dir.join(&shot));
        assert!(pictures[0].rgba8 == frame.rgba8, "tick {tick}");
    }
    assert!(video[0].rgba8 != video[2].rgba8, "the dot moved");
}

#[test]
fn the_same_run_gives_the_same_files() {
    let dir = scratch("same");
    let shot = ["shot", "--at-tick", "20", "--seed", "3", "--out"];
    let video = ["video", "--ticks", "4", "--seed", "3", "--out"];
    for (args, a, b) in [(shot, "a.png", "b.png"), (video, "a.apng", "b.apng")] {
        for name in [a, b] {
            let Some(out) = render(&dir, &[&args[..], &[name]].concat()) else {
                return;
            };
            assert_exit(&out, 0);
        }
        let (a, b) = (
            std::fs::read(dir.join(a)).unwrap(),
            std::fs::read(dir.join(b)).unwrap(),
        );
        assert!(a == b, "{args:?}");
    }
}

#[test]
fn gpu_budgets_hold_every_frame() {
    let dir = scratch("gpu-budgets");
    std::fs::write(dir.join("budgets.toml"), BUDGETS).unwrap();
    let Some(out) = render(
        &dir,
        &[
            "video",
            "--ticks",
            "2",
            "--budgets",
            "budgets.toml",
            "--out",
            "v.apng",
            "--report",
            "r.json",
        ],
    ) else {
        return;
    };
    assert_exit(&out, 0);
    let budgets = &read_json(&dir.join("r.json"))["budgets"];
    assert_eq!(budgets["max_pipeline_switches_per_frame"], 12);
    assert_eq!(budgets["max_bind_group_switches_per_frame"], 8);
    assert_eq!(budgets["exceeded"], Value::Null);

    // The shot's frame uploads 96 bytes.
    let tight = BUDGETS.replace("= 2048", "= 95");
    std::fs::write(dir.join("budgets.toml"), tight).unwrap();
    let out = probe(
        &dir,
        &[
            "shot",
            "--at-tick",
            "4",
            "--budgets",
            "budgets.toml",
            "--out",
            "s.png",
            "--report",
            "r.json",
        ],
    );
    assert_exit(&out, 13);
    let message = "[E-BUDGET] max_upload_bytes_per_frame exceeded at tick 4: 96 > 95";
    assert!(text(&out).contains(message), "{}", text(&out));
    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["budgets"]["exceeded"], message);
    assert_eq!(report["exit"], "budget-exceeded");
    assert!(dir.join("s.png").exists(), "the shot is still written");
}

#[test]
fn a_video_stopped_early_writes_no_file() {
    let dir = scratch("stopped");
    // One dot is one entity, so the run stops at tick 0.
    let sim = BUDGETS.replace("max_entities = 16", "max_entities = 0");
    std::fs::write(dir.join("budgets.toml"), sim).unwrap();
    let Some(out) = render(
        &dir,
        &[
            "video",
            "--ticks",
            "10",
            "--budgets",
            "budgets.toml",
            "--out",
            "v.apng",
            "--report",
            "r.json",
        ],
    ) else {
        return;
    };
    assert_exit(&out, 13);
    assert!(text(&out).contains("not written: the run stopped early"));
    assert!(!dir.join("v.apng").exists(), "no partial video");
    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["ticks_run"], 0);
    assert_eq!(report["gpu"]["frames"], 1, "tick 0 was rendered");
}

/// dot-square's script, from the repo.
fn dot_square() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/dot-square.input.toml");
    path.to_str().unwrap().to_string()
}

/// The frame shows its own tick's state, not the one before or after: at
/// tick 30 of dot-square the dot moves 0.034 m a tick, 5.4 pixels at 160
/// px/m, and the disc's centroid must be where `dump-world` puts the dot.
#[test]
fn a_shot_shows_its_own_tick() {
    let dir = scratch("own-tick");
    let input = dot_square();
    let Some(out) = render(
        &dir,
        &[
            "shot",
            "--at-tick",
            "30",
            "--seed",
            "2",
            "--input",
            &input,
            "--size",
            "1280x960",
            "--out",
            "s.png",
        ],
    ) else {
        return;
    };
    assert_exit(&out, 0);
    let (pictures, _) = frames(&dir.join("s.png"));
    let shot = &pictures[0];
    let (mut sum_x, mut sum_y, mut count) = (0.0, 0.0, 0.0);
    for y in 0..shot.height {
        for x in 0..shot.width {
            let rgb = shot.rgb(x, y);
            if rgb.iter().zip(DOT_0).all(|(&g, w)| g.abs_diff(w) <= 1) {
                sum_x += f64::from(x) + 0.5;
                sum_y += f64::from(y) + 0.5;
                count += 1.0;
            }
        }
    }
    assert!(count > 4000.0, "a 40-pixel disc: {count} pixels");
    let (x, y) = dot_at(&dir, 30, "2", Some(&input));
    let (want_x, want_y) = (x * 160.0, (6.0 - y) * 160.0);
    let (got_x, got_y) = (sum_x / count, sum_y / count);
    assert!(
        (got_x - want_x).abs() < 1.0 && (got_y - want_y).abs() < 1.0,
        "disc at ({got_x}, {got_y}), dot at ({want_x}, {want_y})"
    );
}

/// A `[gpu]` excess stops a video as a `[sim]` one does: no file.
#[test]
fn a_video_over_a_gpu_budget_stops_without_a_file() {
    let dir = scratch("video-over");
    std::fs::write(dir.join("budgets.toml"), BUDGETS.replace("= 2048", "= 95")).unwrap();
    let Some(out) = render(
        &dir,
        &[
            "video",
            "--ticks",
            "30",
            "--budgets",
            "budgets.toml",
            "--out",
            "v.apng",
            "--report",
            "r.json",
        ],
    ) else {
        return;
    };
    assert_exit(&out, 13);
    let message = "[E-BUDGET] max_upload_bytes_per_frame exceeded at tick 0: 96 > 95";
    assert!(text(&out).contains(message), "{}", text(&out));
    assert!(!dir.join("v.apng").exists(), "no partial video");
    let report = read_json(&dir.join("r.json"));
    assert_eq!(report["ticks_run"], 0, "it stops at the frame over budget");
    assert_eq!(report["gpu"]["frames"], 1);
}
