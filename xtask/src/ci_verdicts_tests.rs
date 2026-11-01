//! The verdicts of `ci_verdicts.rs`, from made-up files shaped like the real
//! ones (shots.json from `tests/shots.rs`, the probe's reports and stamp).

use serde_json::{Value, json};

use super::*;
use crate::util::Done;

/// A `shots.json` with one shot per status.
fn shots_json(pinned: bool, mode: &str, statuses: &[&str]) -> String {
    let shots: Vec<Value> = statuses
        .iter()
        .enumerate()
        .map(|(i, status)| {
            json!({
                "name": format!("shot-{i}"), "status": status,
                "message": format!("shot-{i}: {status}"),
                "differing_pixels": null, "flip": null,
            })
        })
        .collect();
    json!({
        "configuration": {
            "os": "linux", "arch": "x86_64", "adapter": "llvmpipe (LLVM 18.1.3, 256 bits)",
            "backend": "vulkan", "device_type": "cpu", "driver": "llvmpipe",
            "driver_info": "Mesa 25.2.7 (LLVM 18.1.3)", "pinned": pinned,
        },
        "machine": {"os_version": "Ubuntu 24.04.3 LTS", "cpu": "AMD EPYC 7763"},
        "mode": mode,
        "golden_dir": "/home/runner/work/game-engine/crates/game-testbed/golden",
        "shots": shots,
    })
    .to_string()
}

#[test]
fn shots_pass_only_when_every_shot_passes() {
    let exact = shots(&shots_json(true, "exact", &["pass"; 4]), true);
    assert!(matches!(exact, Ok(Done::Pass(_))), "{exact:?}");
    let flip = shots(&shots_json(false, "flip", &["pass"; 4]), false);
    assert!(matches!(flip, Ok(Done::Pass(_))), "{flip:?}");
}

#[test]
fn a_shot_without_its_golden_is_pending_never_a_pass() {
    for status in ["no-golden", "no-thresholds"] {
        let every = shots_json(false, "flip", &[status; 4]);
        let one = shots_json(false, "flip", &["pass", "pass", status, "pass"]);
        for text in [every, one] {
            match shots(&text, false) {
                Ok(Done::Pending(detail)) => assert!(detail.contains(status), "{detail}"),
                other => panic!("{status}: {other:?}"),
            }
        }
    }
    let exact = shots(&shots_json(true, "exact", &["no-golden"; 4]), true);
    assert!(matches!(exact, Ok(Done::Pending(_))), "{exact:?}");
}

#[test]
fn a_failed_shot_fails_the_step() {
    let text = shots_json(false, "flip", &["pass", "fail", "no-golden", "pass"]);
    let error = shots(&text, false).unwrap_err();
    assert!(error.contains("shot-1"), "{error}");
}

#[test]
fn shot_exact_needs_the_pinned_configuration_and_shot_flip_needs_flip() {
    for (pinned, mode, exact) in [
        (false, "flip", true),
        (false, "exact", true),
        (true, "flip", true),
        (true, "exact", false),
    ] {
        let text = shots_json(pinned, mode, &["pass"; 4]);
        let verdict = shots(&text, exact);
        assert!(
            verdict.is_err(),
            "pinned {pinned}, {mode}, exact {exact}: {verdict:?}"
        );
    }
    let error = shots(&shots_json(false, "flip", &["pass"; 4]), true).unwrap_err();
    assert!(
        error.contains("llvmpipe (LLVM 18.1.3, 256 bits)"),
        "names the adapter: {error}"
    );
}

#[test]
fn shots_json_must_parse_name_shots_and_use_the_committed_goldens() {
    assert!(shots("", false).is_err());
    assert!(shots("{}", false).is_err());
    assert!(shots(&shots_json(false, "flip", &[]), false).is_err());
    assert!(shots(&shots_json(false, "flip", &["skipped"]), false).is_err());
    let mut summary: Value =
        serde_json::from_str(&shots_json(false, "flip", &["pass"; 4])).unwrap();
    summary["golden_dir"] = json!("/tmp/copy-of-golden");
    let error = shots(&summary.to_string(), false).unwrap_err();
    assert!(error.contains("/tmp/copy-of-golden"), "{error}");
    summary["golden_dir"] = json!(r"D:\a\game-engine\game-engine\crates\game-testbed\golden");
    let windows = shots(&summary.to_string(), false);
    assert!(matches!(windows, Ok(Done::Pass(_))), "{windows:?}");
}

const SHA: &str = "a6c1550fb90be0ba656428b616bfa97627138979";
const PLAIN: &str = "game-testbed 0.1.0 (engine 0.1.0 a6c1550fb90b) x86_64-unknown-linux-gnu \
                     debug, rustc 1.99.1, sim protocol 0x69fd4996880a6508\n";

fn stamp() -> Value {
    json!({
        "stamp_version": 1,
        "game": "game-testbed",
        "build": {
            "game_version": "0.1.0", "engine_version": "0.1.0",
            "engine_sha": SHA, "game_sha": SHA, "dirty": false,
            "source_date_epoch": 1_793_471_790_u64, "ci_run": "github-42/1",
            "target": "x86_64-unknown-linux-gnu",
            "rustc": "rustc 1.99.1 (58cf8f9c3 2026-10-13)", "profile": "debug",
            "debug_assertions": true,
        },
    })
}

#[test]
fn version_passes_a_full_stamp_printed_both_ways() {
    let detail = version(PLAIN, &stamp().to_string(), None).unwrap();
    assert!(detail.contains("a6c1550fb90b"), "{detail}");
}

#[test]
fn version_fails_on_an_unknown_or_missing_build_value() {
    for key in [
        "engine_sha",
        "game_sha",
        "source_date_epoch",
        "ci_run",
        "rustc",
        "dirty",
    ] {
        for bad in [json!("unknown"), Value::Null] {
            let mut stamp = stamp();
            stamp["build"][key] = bad.clone();
            let error = version(PLAIN, &stamp.to_string(), None).unwrap_err();
            assert!(error.contains(key), "{key} = {bad}: {error}");
        }
    }
    assert!(version(PLAIN, "{}", None).is_err());
    assert!(version(PLAIN, "not json", None).is_err());
}

#[test]
fn version_plain_is_the_same_stamp_on_one_line() {
    let json = stamp().to_string();
    for plain in [
        format!("{PLAIN}and a second line\n"),
        PLAIN.replace("a6c1550fb90b", "000000000000"),
        PLAIN.replace("x86_64-unknown-linux-gnu", "aarch64-apple-darwin"),
        PLAIN.replace("game-testbed 0.1.0", "other-game 0.1.0"),
        PLAIN.replace(" debug,", " release,"),
        String::new(),
    ] {
        assert!(version(&plain, &json, None).is_err(), "{plain:?}");
    }
}

#[test]
fn on_github_the_stamp_names_this_commit_and_this_run() {
    let json = stamp().to_string();
    assert!(version(PLAIN, &json, Some((SHA, "github-42/1"))).is_ok());
    let other = "b".repeat(40);
    assert!(version(PLAIN, &json, Some((&other, "github-42/1"))).is_err());
    assert!(version(PLAIN, &json, Some((SHA, "github-43/1"))).is_err());
}

const STREAM: &str = "# eng hash stream v1\n# game=game-testbed seed=1\n0 0x1\n1 0x2\n";

fn window_report(frames: u64) -> Value {
    json!({
        "command": "play", "platform": "winit", "exit": "ok", "ticks_run": 240,
        "dropped_ticks": 0,
        "gpu": {
            "adapter": "llvmpipe (LLVM 19.1.7, 128 bits)", "width": 800, "height": 600,
            "frames": frames, "surface_format": "Bgra8UnormSrgb",
            "per_frame": {
                "draws": 3, "pipeline_switches": 3, "bind_group_switches": 2,
                "upload_bytes": 96,
            },
        },
    })
}

#[test]
fn window_passes_with_the_committed_stream_and_frames_presented() {
    let detail = window(STREAM, STREAM, &window_report(233)).unwrap();
    assert!(
        detail.contains("233") && detail.contains("Bgra8UnormSrgb"),
        "{detail}"
    );
}

#[test]
fn window_fails_when_its_stream_is_not_the_committed_one() {
    let other = STREAM.replace("1 0x2", "1 0x3");
    let error = window(&other, STREAM, &window_report(233)).unwrap_err();
    assert!(
        error.contains("[E-HASH]") && error.contains("line 4"),
        "{error}"
    );
    let short = window("# eng hash stream v1\n", STREAM, &window_report(233)).unwrap_err();
    assert!(short.contains("[E-HASH]"), "{short}");
    // A checkout that turned the committed file's line ends into CRLF.
    let crlf = STREAM.replace('\n', "\r\n");
    assert!(window(STREAM, &crlf, &window_report(233)).is_err());
}

#[test]
fn window_fails_without_a_presented_frame_or_a_real_window() {
    let error = window(STREAM, STREAM, &window_report(0)).unwrap_err();
    assert!(error.contains("no frame"), "{error}");
    let mut headless = window_report(233);
    headless["platform"] = json!("headless");
    assert!(window(STREAM, STREAM, &headless).is_err());
    let mut failed = window_report(233);
    failed["exit"] = json!("budget-exceeded");
    assert!(window(STREAM, STREAM, &failed).is_err());
    assert!(window(STREAM, STREAM, &Value::Null).is_err());
}

const BUDGETS: &str = "version = 1\n[sim]\nmax_state_bytes = 4096\n[gpu]\n\
    max_draws_per_frame = 12\nmax_pipeline_switches_per_frame = 12\n\
    max_bind_group_switches_per_frame = 8\nmax_upload_bytes_per_frame = 2048\n";

fn shot_report() -> Value {
    json!({
        "command": "shot", "exit": "ok",
        "gpu": {
            "adapter": "llvmpipe (LLVM 18.1.3, 256 bits)", "backend": "vulkan",
            "device_type": "cpu", "vendor": 65541, "device": 0, "driver": "llvmpipe",
            "driver_info": "Mesa 25.2.7 (LLVM 18.1.3)", "tier": 0,
            "capabilities": {"timestamp_queries": true, "binding_arrays": false},
            "width": 320, "height": 240, "frames": 1,
            "per_frame": {
                "draws": 2, "pipeline_switches": 2, "bind_group_switches": 1,
                "upload_bytes": 96,
            },
        },
    })
}

const COUNTERS: [(&str, u64); 4] = [
    ("draws", 12),
    ("pipeline_switches", 12),
    ("bind_group_switches", 8),
    ("upload_bytes", 2048),
];

#[test]
fn gpu_budgets_records_the_adapter_and_driver() {
    let detail = gpu_budgets(&shot_report(), BUDGETS, Some(&window_report(233))).unwrap();
    for wanted in [
        "llvmpipe (LLVM 18.1.3, 256 bits)",
        "Mesa 25.2.7 (LLVM 18.1.3)",
        "vulkan",
    ] {
        assert!(detail.contains(wanted), "{wanted}: {detail}");
    }
}

#[test]
fn gpu_budgets_holds_each_counter_of_the_shot_and_the_window_to_its_limit() {
    for (counter, limit) in COUNTERS {
        let mut at = shot_report();
        at["gpu"]["per_frame"][counter] = json!(limit);
        assert!(
            gpu_budgets(&at, BUDGETS, None).is_ok(),
            "{counter} at its limit"
        );
        let mut over = shot_report();
        over["gpu"]["per_frame"][counter] = json!(limit + 1);
        let error = gpu_budgets(&over, BUDGETS, None).unwrap_err();
        assert!(error.contains(counter), "{counter}: {error}");
        let mut window = window_report(233);
        window["gpu"]["per_frame"][counter] = json!(limit + 1);
        let error = gpu_budgets(&shot_report(), BUDGETS, Some(&window)).unwrap_err();
        assert!(
            error.contains(counter) && error.contains("window"),
            "{error}"
        );
    }
}

#[test]
fn gpu_budgets_needs_the_adapter_capabilities_counters_and_limits() {
    let mut broken = Vec::new();
    for (key, value) in [
        ("adapter", json!("")),
        ("adapter", Value::Null),
        ("driver_info", Value::Null),
        ("capabilities", Value::Null),
        ("per_frame", Value::Null),
    ] {
        let mut report = shot_report();
        report["gpu"][key] = value;
        broken.push(report);
    }
    let mut no_gpu = shot_report();
    no_gpu["gpu"] = Value::Null;
    broken.push(no_gpu);
    let mut failed = shot_report();
    failed["exit"] = json!("budget-exceeded");
    broken.push(failed);
    for report in &broken {
        assert!(gpu_budgets(report, BUDGETS, None).is_err(), "{report}");
    }
    assert!(gpu_budgets(&shot_report(), "version = 1\n", None).is_err());
    let partial = BUDGETS.replace("max_upload_bytes_per_frame = 2048\n", "");
    let error = gpu_budgets(&shot_report(), &partial, None).unwrap_err();
    assert!(error.contains("max_upload_bytes_per_frame"), "{error}");
}

#[test]
fn a_window_that_presented_nothing_is_noted_not_held() {
    let detail = gpu_budgets(&shot_report(), BUDGETS, Some(&window_report(0))).unwrap();
    assert!(detail.contains("window"), "{detail}");
    let detail = gpu_budgets(&shot_report(), BUDGETS, None).unwrap();
    assert!(detail.contains("window"), "{detail}");
}

/// A PNG file's chunks, with the `acTL` chunk of an animation of `frames`
/// frames unless `None`. CRCs are zero: the verdict does not check them.
fn apng(frames: Option<u32>) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        let length = u32::try_from(data.len()).unwrap();
        bytes.extend(length.to_be_bytes());
        bytes.extend(kind);
        bytes.extend(data);
        bytes.extend([0; 4]);
    };
    chunk(b"IHDR", &[0; 13]);
    chunk(b"sRGB", &[0]);
    if let Some(frames) = frames {
        let mut actl = frames.to_be_bytes().to_vec();
        actl.extend(0_u32.to_be_bytes());
        chunk(b"acTL", &actl);
    }
    chunk(b"IEND", &[]);
    bytes
}

fn video_report(frames: u64) -> Value {
    json!({"command": "video", "exit": "ok", "gpu": {"adapter": "llvmpipe", "frames": frames}})
}

#[test]
fn video_passes_an_animation_with_every_frame() {
    let detail = video(&apng(Some(241)), &video_report(241), 241).unwrap();
    assert!(detail.contains("241"), "{detail}");
}

#[test]
fn video_fails_without_an_animation_or_with_frames_missing() {
    let report = video_report(241);
    assert!(video(b"", &report, 241).is_err());
    assert!(video(b"GIF89a", &report, 241).is_err());
    assert!(video(&apng(None), &report, 241).is_err(), "a still PNG");
    assert!(video(&apng(Some(240)), &report, 241).is_err());
    assert!(video(&apng(Some(241)), &video_report(240), 241).is_err());
    let mut failed = video_report(241);
    failed["exit"] = json!("budget-exceeded");
    assert!(video(&apng(Some(241)), &failed, 241).is_err());
    let whole = apng(Some(241));
    for cut in 0..whole.len() {
        let verdict = video(&whole[..cut], &report, 241);
        assert!(verdict.is_err(), "cut at {cut}");
    }
}

const ESC: &str = "escape_from_the_keyboard_quits_within_5_seconds";

#[test]
fn ran_test_needs_the_named_test_to_run_and_pass() {
    let ok = format!(
        "\nrunning 1 test\ntest {ESC} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; \
         0 measured; 3 filtered out; finished in 1.20s\n\n"
    );
    assert!(ran_test(&ok, ESC).is_ok());
    let none = "\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; \
                4 filtered out; finished in 0.00s\n";
    assert!(ran_test(none, ESC).is_err());
    assert!(ran_test(&ok, "another_test").is_err());
    assert!(ran_test(&ok.replace("... ok", "... ignored"), ESC).is_err());
    assert!(ran_test(&format!("{ok}test {ESC}_too ... ok\n"), ESC).is_ok());
}

#[test]
fn compare_hashes_needs_every_pr_tier_and_native_stream() {
    let all: Vec<String> = [
        "hashes-ubuntu-24.04",
        "hashes-ubuntu-24.04-arm",
        "hashes-riscv",
        "hashes-native-ubuntu-24.04",
        "hashes-native-ubuntu-24.04-arm",
        "hashes-native-windows-2025",
        "hashes-native-windows-11-arm",
        "hashes-native-macos-26",
    ]
    .map(String::from)
    .to_vec();
    assert!(missing_streams(&all).is_empty());
    let without: Vec<String> = all
        .iter()
        .filter(|name| *name != "hashes-native-windows-11-arm")
        .cloned()
        .collect();
    assert_eq!(missing_streams(&without), ["hashes-native-windows-11-arm"]);
    assert_eq!(missing_streams(&[]).len(), all.len());
}
