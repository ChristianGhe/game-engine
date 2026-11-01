//! The native job's contract with the acceptance list and the workflow, and
//! how a pending step reaches the job's report.

use std::path::Path;
use std::time::Instant;

use serde_json::{Map, Value, json};

use super::{RUNNERS, steps_for};
use crate::util::{Done, Steps};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the repo root")
}

#[test]
fn each_runner_runs_its_steps_in_order() {
    let common = ["build", "tests", "replay", "crash-json", "version"];
    let linux_x64 = steps_for("x86_64-unknown-linux-gnu");
    let mut expected: Vec<&str> = common.to_vec();
    expected.extend(["shot-exact", "window", "gpu-budgets", "esc", "proof-video"]);
    assert_eq!(linux_x64, expected);
    for host in [
        "aarch64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
    ] {
        let mut expected: Vec<&str> = common.to_vec();
        expected.extend(["shot-flip", "window", "gpu-budgets"]);
        assert_eq!(steps_for(host), expected, "{host}");
    }
    let mut expected: Vec<&str> = common.to_vec();
    expected.extend(["shot-flip", "window", "gpu-budgets", "check-intel-mac"]);
    assert_eq!(steps_for("aarch64-apple-darwin"), expected);
}

/// Every native check in the current rung's acceptance list names a runner's
/// host and a step that the native job runs there, so each can pass.
#[test]
fn every_native_check_in_acceptance_finds_its_step() {
    let rung = std::fs::read_to_string(root().join("docs/rungs/current.txt")).unwrap();
    let path = root()
        .join("docs/rungs")
        .join(rung.trim())
        .join("acceptance.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let hosts: Vec<&str> = RUNNERS.iter().map(|r| r.1).collect();
    let mut checked = 0;
    for feature in doc["features"].as_array().unwrap() {
        // An owner feature may leave `checks` out (X4).
        for check in feature["checks"].as_array().into_iter().flatten() {
            if check["job"] != "native" {
                continue;
            }
            let target = check["target"].as_str().unwrap_or_default();
            let step = check["step"].as_str().unwrap();
            assert!(
                hosts.contains(&target),
                "{}: no runner reports {target:?}",
                feature["id"]
            );
            assert!(
                steps_for(target).contains(&step),
                "{}: the native job on {target} has no step {step}",
                feature["id"]
            );
            checked += 1;
        }
    }
    if rung.trim() == "1b" {
        assert_eq!(checked, 53, "rung 1b's native checks");
    }
}

/// The workflow's matrix is these runners, and it uploads each one's hash
/// streams under the name `compare-hashes` looks for.
#[test]
fn the_workflow_runs_the_native_job_on_every_runner() {
    let workflow = std::fs::read_to_string(root().join(".github/workflows/ci.yml"))
        .unwrap()
        .replace("\r\n", "\n");
    let names: Vec<&str> = RUNNERS.iter().map(|r| r.0).collect();
    let matrix = format!("runner: [{}]", names.join(", "));
    assert!(workflow.contains(&matrix), "ci.yml has no `{matrix}`");
    for artifact in [
        "hashes-native-${{ matrix.runner }}",
        "report-native-${{ matrix.runner }}",
        "golden-candidates-${{ matrix.runner }}",
        "crash-native-${{ matrix.runner }}",
    ] {
        assert!(workflow.contains(artifact), "ci.yml uploads no {artifact}");
    }
    assert!(workflow.contains("cargo xtask ci --job native"));
    // `pr-duration` finds the PR tier's jobs by their ids.
    for job in crate::ci_durations::PR_JOBS {
        assert!(
            workflow.contains(&format!("\n  {job}:\n")),
            "ci.yml has no job {job}"
        );
    }
}

/// On Linux x64 the offscreen renders name the pinned lavapipe: Ubuntu's own
/// lavapipe is there too, for the windows, which the pinned build cannot
/// present to. The name is the one `tests/shots.rs` pins.
#[test]
fn offscreen_renders_on_linux_x64_name_the_pinned_adapter() {
    let harness = std::fs::read_to_string(root().join("crates/game-testbed/tests/shots.rs"))
        .expect("the shot harness");
    let pinned = super::offscreen_adapter("x86_64-unknown-linux-gnu").expect("named");
    assert!(
        harness.contains(&format!("const PINNED_ADAPTER: &str = \"{pinned}\";")),
        "shots.rs pins another adapter than {pinned:?}"
    );
    for (_, host) in RUNNERS
        .iter()
        .filter(|(_, host)| *host != "x86_64-unknown-linux-gnu")
    {
        assert_eq!(super::offscreen_adapter(host), None, "{host}");
    }
}

#[test]
fn a_pending_step_fails_no_job_and_passes_no_check() {
    let mut steps = Steps::default();
    steps.run("build", || Ok(String::new()));
    let passed = steps.run_judged("shot-flip", || Ok(Done::Pending("no golden yet".into())));
    assert!(!passed, "pending is not a pass");
    assert!(steps.ok(), "a pending step fails no job");
    let report =
        crate::ci_report::report("native", "aarch64-apple-darwin", &steps, 1.0, Map::new());
    let step = &report["steps"][1];
    assert_eq!(step["name"], "shot-flip");
    assert_eq!(step["ok"], false, "acceptance counts only ok steps");
    assert_eq!(step["pending"], true);
    assert_eq!(report["steps"][0]["pending"], false);
    assert_eq!(report["ok"], true);
    steps.run("window", || Err("no frame presented".into()));
    assert!(!steps.ok(), "a failed step still fails the job");
}

/// The job's exit, as CI sees it: a pending step leaves it green and is
/// written as pending; a failed step turns it red.
#[test]
fn a_pending_step_leaves_the_job_green_and_a_failed_one_red() {
    let root = std::env::temp_dir().join(format!("xtask-native-finish-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let finish =
        |steps| crate::ci_report::finish(&root, "native", steps, Instant::now(), Map::new());
    let mut steps = Steps::default();
    steps.run("build", || Ok(String::new()));
    steps.run_judged("shot-flip", || Ok(Done::Pending("no golden yet".into())));
    let green = finish(steps);
    let written = std::fs::read_to_string(root.join("target/ci/report.json")).unwrap();
    let mut steps = Steps::default();
    steps.run_judged("shot-flip", || Ok(Done::Pending(String::new())));
    steps.run("window", || Err("no frame presented".into()));
    let red = finish(steps);
    std::fs::remove_dir_all(&root).unwrap();
    let green = green.unwrap();
    assert!(
        green.contains("pending") && green.contains("shot-flip"),
        "{green}"
    );
    let report: Value = serde_json::from_str(&written).unwrap();
    assert_eq!(report["steps"][1]["pending"], true);
    assert_eq!(report["steps"][1]["ok"], false);
    let red = red.unwrap_err();
    assert!(
        red.contains("window") && !red.contains("shot-flip"),
        "{red}"
    );
}

/// The report names the adapter, driver and per-frame counters of the
/// budget shot (spec 8.2), else of the window.
#[test]
fn the_report_names_the_adapter_driver_and_counters() {
    let root = std::env::temp_dir().join(format!("xtask-native-extra-{}", std::process::id()));
    let ci = root.join("target/ci");
    std::fs::create_dir_all(&ci).unwrap();
    assert_eq!(
        super::extra(&root)["gpu"],
        Value::Null,
        "no render, no adapter"
    );
    let gpu = json!({
        "adapter": "Microsoft Basic Render Driver", "backend": "dx12", "device_type": "cpu",
        "driver": "", "driver_info": "", "tier": 0, "capabilities": {},
        "per_frame": {"draws": 3},
    });
    std::fs::write(ci.join("window.json"), json!({ "gpu": gpu }).to_string()).unwrap();
    let extra = super::extra(&root);
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(extra["gpu"]["adapter"], "Microsoft Basic Render Driver");
    assert_eq!(extra["gpu"]["driver_info"], "");
    assert_eq!(extra["gpu"]["per_frame"]["draws"], 3);
    assert!(extra["os_version"].is_string() || extra["os_version"].is_null());
}

#[test]
fn the_report_carries_the_extra_keys() {
    let mut extra = Map::new();
    extra.insert(
        "gpu".into(),
        json!({"adapter": "Microsoft Basic Render Driver"}),
    );
    let report = crate::ci_report::report(
        "native",
        "x86_64-pc-windows-msvc",
        &Steps::default(),
        0.0,
        extra,
    );
    assert_eq!(report["gpu"]["adapter"], "Microsoft Basic Render Driver");
    assert_eq!(report["job"], "native");
}
