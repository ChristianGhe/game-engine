//! Criterion 21's report keys (rung 2, T21): a `shot`'s and a `video`'s
//! `gpu` object holds the last frame's GPU time per pass (`pass_times`,
//! `null` without timestamp queries) and the targets' pool size
//! (`transient_textures`), beside `per_frame`, which stays as 1b pinned it.
//!
//! Tests that render need an adapter. Without one they skip, unless
//! `ENG_REQUIRE_GPU=1` (CI's native jobs) makes them fail.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("gpu-report")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Runs a command that renders and returns its report: `None` without an
/// adapter, unless CI requires one.
fn render(dir: &Path, args: &[&str]) -> Option<Value> {
    let out = Command::new(env!("CARGO_BIN_EXE_game-testbed"))
        .args(args)
        .args(["--report", "r.json"])
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"))
        .output()
        .expect("run game-testbed");
    let no_adapter = out.status.code() == Some(64) && text(&out).contains("[E-NO-ADAPTER]");
    if no_adapter && std::env::var("ENG_REQUIRE_GPU").as_deref() != Ok("1") {
        eprintln!("skipped: {}", text(&out));
        return None;
    }
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    Some(serde_json::from_slice(&std::fs::read(dir.join("r.json")).unwrap()).unwrap())
}

/// The passes that ran, in order, when the adapter times passes; each
/// time a whole number of nanoseconds or `null` (a pass the GPU left
/// untimed), but not every one `null`, which would be no measurement.
/// `None` when the report says the adapter has no timestamps.
fn passes(gpu: &Value) -> Option<Vec<String>> {
    let timed = gpu["capabilities"]["timestamp_queries"] == true;
    let times = &gpu["pass_times"];
    if !timed {
        assert_eq!(*times, Value::Null, "no timestamps, no times: {gpu:#}");
        return None;
    }
    let times = times.as_array().unwrap_or_else(|| panic!("{gpu:#}"));
    for time in times {
        let ns = &time["nanoseconds"];
        assert!(ns.is_u64() || ns.is_null(), "{time}");
    }
    assert!(
        times.iter().any(|t| t["nanoseconds"].is_u64()),
        "timestamps, but no pass timed: {gpu:#}"
    );
    Some(
        times
            .iter()
            .map(|t| t["pass"].as_str().unwrap().to_string())
            .collect(),
    )
}

#[test]
fn a_room_shot_reports_its_pass_times_and_pool() {
    let dir = scratch("room");
    let Some(report) = render(
        &dir,
        &["shot", "--map", "room", "--at-tick", "60", "--out", "r.png"],
    ) else {
        return;
    };
    let gpu = &report["gpu"];
    // T10's graph pools five transient textures (albedo and `fogged` share
    // one); the room's shot is one chunk and three things (T11, T12).
    assert_eq!(gpu["transient_textures"], 5, "{gpu:#}");
    assert_eq!(
        gpu["per_frame"],
        json!({"draws": 7, "pipeline_switches": 7, "bind_group_switches": 15, "upload_bytes": 736})
    );
    if let Some(passes) = passes(gpu) {
        // Offscreen, so no present; the sprites pass is always timed.
        let expected = [
            "opaque", "sprites", "light", "sky", "fog", "tonemap", "palette",
        ];
        assert_eq!(passes, expected, "{gpu:#}");
    }
}

#[test]
fn a_dot_shot_and_a_video_report_them_too() {
    let dir = scratch("dots");
    let Some(report) = render(&dir, &["shot", "--at-tick", "30", "--out", "r.png"]) else {
        return;
    };
    let gpu = &report["gpu"];
    assert!(gpu["transient_textures"].is_u64(), "{gpu:#}");
    assert!(passes(gpu).is_none_or(|p| !p.is_empty()), "{gpu:#}");
    let Some(report) = render(&dir, &["video", "--ticks", "4", "--out", "v.apng"]) else {
        return;
    };
    let gpu = &report["gpu"];
    assert_eq!(gpu["frames"], 5);
    assert!(gpu["transient_textures"].is_u64(), "{gpu:#}");
    assert!(passes(gpu).is_none_or(|p| !p.is_empty()), "{gpu:#}");
}

/// A video's last frame is at the last multiple of `--every`, not at the
/// last tick when `--every` does not divide `--ticks`; that frame is the
/// one timed.
#[test]
fn a_video_whose_every_does_not_divide_its_ticks_times_its_last_frame() {
    let dir = scratch("every");
    let args = [
        "video", "--map", "room", "--ticks", "10", "--every", "3", "--out", "v.apng",
    ];
    let Some(report) = render(&dir, &args) else {
        return;
    };
    let gpu = &report["gpu"];
    // Ticks 0, 3, 6 and 9.
    assert_eq!(gpu["frames"], 4, "{gpu:#}");
    assert_eq!(gpu["transient_textures"], 5, "{gpu:#}");
    if let Some(passes) = passes(gpu) {
        assert_eq!(
            passes.first().map(String::as_str),
            Some("opaque"),
            "{gpu:#}"
        );
    }
}
