//! `cargo xtask ci --job native` (rung 1b, patch X2; spec 8.2): on each of
//! the five native runners, build and test the workspace with a GPU and a
//! display required, run the replays, and check the probe's window, shots,
//! crash files, version stamp, GPU counters and proof video on the runner's
//! own adapter.
//!
//! [`STEPS`] is the job. Its names are the steps that the acceptance list's
//! native checks name (`docs/rungs/1b/acceptance.json`), on the hosts it
//! names, and `ci_native_tests.rs` holds the two together. The report
//! (`target/ci/reports/native-<host>.json`) adds the adapter and driver
//! (`gpu`, from the `gpu-budgets` shot) and the OS version. The steps write
//! their files to `target/ci/` (`version.*`, `window.*`, `shot.*`,
//! `proof.*`), the hash streams to `target/ci/hashes/<host>/`, and the shot
//! harness its candidates to `target/golden-candidates/`.
//!
//! The job sets `ENG_REQUIRE_GPU=1` and `ENG_REQUIRE_DISPLAY=1` for every
//! step that renders or opens a window, so none of those tests can skip. On
//! Linux the workflow runs the job under `xvfb-run`. `ENG_CI_STEPS`, a
//! comma-separated list of step names, runs only those, for local
//! debugging; the report then lists them under `only_steps`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value, json};

use crate::util::{self, Done, Steps};
use crate::{ci, ci_verdicts as verdicts, replay};

/// The five native runners (spec 8.2) and the host each reports, as the
/// acceptance checks' targets name them. `windows-11-arm` needs a native
/// aarch64 toolchain to report its own (rung 1b, T5 open question 4).
pub const RUNNERS: [(&str, &str); 5] = [
    ("ubuntu-24.04", "x86_64-unknown-linux-gnu"),
    ("ubuntu-24.04-arm", "aarch64-unknown-linux-gnu"),
    ("windows-2025", "x86_64-pc-windows-msvc"),
    ("windows-11-arm", "aarch64-pc-windows-msvc"),
    ("macos-26", "aarch64-apple-darwin"),
];

/// The pinned configuration's host (plan decision 3): exact shots, and the
/// steps that run only there.
const LINUX_X64: &str = "x86_64-unknown-linux-gnu";
const INTEL_MAC: &str = "x86_64-apple-darwin";

/// Which hosts a step runs on.
#[derive(Clone, Copy)]
enum Hosts {
    All,
    LinuxX64,
    NotLinuxX64,
    MacOs,
}

impl Hosts {
    fn have(self, host: &str) -> bool {
        match self {
            Hosts::All => true,
            Hosts::LinuxX64 => host == LINUX_X64,
            Hosts::NotLinuxX64 => host != LINUX_X64,
            Hosts::MacOs => host.ends_with("-apple-darwin"),
        }
    }
}

type Step = fn(&Path) -> Result<Done, String>;

/// The native job: each step's name, where it runs, and what it does.
const STEPS: [(&str, Hosts, Step); 12] = [
    ("build", Hosts::All, build),
    ("tests", Hosts::All, tests),
    ("replay", Hosts::All, replay),
    ("crash-json", Hosts::All, crash_json),
    ("version", Hosts::All, version),
    ("shot-exact", Hosts::LinuxX64, shot_exact),
    ("shot-flip", Hosts::NotLinuxX64, shot_flip),
    ("window", Hosts::All, window),
    ("gpu-budgets", Hosts::All, gpu_budgets),
    ("esc", Hosts::LinuxX64, esc),
    ("proof-video", Hosts::LinuxX64, proof_video),
    ("check-intel-mac", Hosts::MacOs, check_intel_mac),
];

/// What a step that renders or opens a window must have (amendments 36, 46).
const REQUIRE: [(&str, &str); 2] = [("ENG_REQUIRE_GPU", "1"), ("ENG_REQUIRE_DISPLAY", "1")];

/// The scripted session every windowed and rendered step plays.
const SCRIPT: &str = "crates/game-testbed/scenarios/dot-square.input.toml";
const STREAM: &str = "crates/game-testbed/hashes/dot-square.seed1.txt";

/// The files the steps leave in `target/ci/` (`<stem>.<extension>`),
/// removed when the job starts so a step never reads an earlier run's.
const OUTPUTS: [&str; 4] = ["version", "window", "shot", "proof"];
const EXTENSIONS: [&str; 4] = ["txt", "json", "png", "apng"];

/// The `gpu` keys the report copies from a render's report: the adapter and
/// driver, and the frame's counters (spec 8.2).
const GPU_KEYS: [&str; 8] = [
    "adapter",
    "backend",
    "device_type",
    "driver",
    "driver_info",
    "tier",
    "capabilities",
    "per_frame",
];

/// The adapter the offscreen renders on `host` ask for (`ENG_GPU_ADAPTER`):
/// on Linux x64, the pinned lavapipe that `tests/shots.rs` pins. Ubuntu's
/// own lavapipe is installed beside it there, because the pinned build
/// offers no window surface (only `VK_EXT_headless_surface`), and a window
/// can only open on an adapter that presents to it, so `play` takes
/// Ubuntu's. The shot harness's exact mode still checks the whole
/// configuration.
pub fn offscreen_adapter(host: &str) -> Option<&'static str> {
    (host == LINUX_X64).then_some("llvmpipe (LLVM 18.1.3, 256 bits)")
}

/// The native job's steps on `host`, in order.
pub fn steps_for(host: &str) -> Vec<&'static str> {
    STEPS
        .iter()
        .filter(|(_, hosts, _)| hosts.have(host))
        .map(|(name, _, _)| *name)
        .collect()
}

/// `ENG_CI_STEPS`, if set and not empty.
fn only_steps() -> Option<Vec<String>> {
    let list = std::env::var("ENG_CI_STEPS").ok()?;
    let names: Vec<String> = list
        .split(',')
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    (!names.is_empty()).then_some(names)
}

/// Runs the native job's steps for this host.
pub fn steps(root: &Path, steps: &mut Steps) {
    let host = match replay::host_triple(root) {
        Ok(host) => host,
        Err(e) => {
            steps.run("build", || Err(e));
            return;
        }
    };
    let out = root.join("target/ci");
    for (stem, extension) in OUTPUTS.iter().flat_map(|s| EXTENSIONS.map(|e| (s, e))) {
        let _ = fs::remove_file(out.join(format!("{stem}.{extension}")));
    }
    let _ = fs::create_dir_all(&out);
    let only = only_steps();
    for name in steps_for(&host) {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == name)) {
            continue;
        }
        let step = STEPS
            .iter()
            .find(|(step, _, _)| *step == name)
            .map(|(_, _, run)| *run)
            .expect("steps_for names steps of STEPS");
        steps.run_judged(name, || step(root));
    }
}

/// The report's additions: the adapter and driver the steps rendered on
/// (spec 8.2), the OS version and the runner image.
pub fn extra(root: &Path) -> Map<String, Value> {
    let gpu = ["shot.json", "window.json", "proof.json"]
        .iter()
        .find_map(|file| {
            let report = read_json(&root.join("target/ci").join(file)).ok()?;
            let gpu = report.get("gpu").filter(|gpu| gpu.is_object())?;
            let copy = GPU_KEYS
                .iter()
                .map(|key| ((*key).to_string(), gpu[*key].clone()));
            Some(Value::Object(copy.collect()))
        })
        .unwrap_or(Value::Null);
    let mut extra = Map::new();
    extra.insert("gpu".into(), gpu);
    extra.insert("os_version".into(), json!(os_version()));
    let image = std::env::var("ImageVersion").ok();
    extra.insert("image_version".into(), json!(image));
    if let Some(only) = only_steps() {
        extra.insert("only_steps".into(), json!(only));
    }
    extra
}

fn os_version() -> Option<String> {
    let run = |program: &str, args: &[&str]| {
        let out = Command::new(program).args(args).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (out.status.success() && !text.is_empty()).then_some(text)
    };
    match std::env::consts::OS {
        "macos" => run("sw_vers", &["-productVersion"]).map(|v| format!("macOS {v}")),
        "windows" => run("cmd", &["/c", "ver"]),
        _ => {
            let release = fs::read_to_string("/etc/os-release").ok()?;
            let line = release.lines().find(|l| l.starts_with("PRETTY_NAME="))?;
            Some(line["PRETTY_NAME=".len()..].trim_matches('"').to_string())
        }
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn pass(result: Result<String, String>) -> Result<Done, String> {
    result.map(Done::Pass)
}

/// Builds the testbed and returns its binary.
fn testbed(root: &Path) -> Result<PathBuf, String> {
    ci::cargo_step(root, "native-testbed", &["build", "-p", "game-testbed"])?;
    let exe = format!("game-testbed{}", std::env::consts::EXE_SUFFIX);
    Ok(root.join("target/debug").join(exe))
}

/// [`probe`] for an offscreen render, on [`offscreen_adapter`] if this host
/// names one.
fn offscreen(root: &Path, binary: &Path, args: &[&str]) -> Result<Command, String> {
    let mut cmd = probe(root, binary, args);
    if let Some(adapter) = offscreen_adapter(&replay::host_triple(root)?) {
        cmd.env("ENG_GPU_ADAPTER", adapter);
    }
    Ok(cmd)
}

/// The testbed's binary run from the repo root, its logs (and any
/// `crash.json`) in `target/agent-logs/`, with a GPU and a display required.
fn probe(root: &Path, binary: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(binary);
    cmd.current_dir(root)
        .args(args)
        .envs(REQUIRE)
        .env("ENG_AGENT_LOG_DIR", root.join("target/agent-logs"))
        // winit takes Wayland when it can; xvfb and xdotool are X11.
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET");
    cmd
}

/// A path under the repo root as an argument.
fn arg(root: &Path, path: &str) -> String {
    root.join(path).display().to_string()
}

/// The workspace and every test target build (criterion 1).
fn build(root: &Path) -> Result<Done, String> {
    pass(ci::cargo_step(
        root,
        "native-build",
        &["build", "--workspace", "--all-targets"],
    ))
}

/// Every test, with a GPU and a display required (criteria 1, 3 and 18).
fn tests(root: &Path) -> Result<Done, String> {
    pass(ci::tests_env(root, &REQUIRE))
}

/// The testbed's scenarios against the committed hash streams, which the
/// workflow uploads for `compare-hashes` (criterion 7).
fn replay(root: &Path) -> Result<Done, String> {
    pass(replay::run(root, &replay::Options::default()))
}

/// A panic in `run` and in `play` writes `crash.json` (criterion 6,
/// amendment 46); `play`'s half cannot skip here.
fn crash_json(root: &Path) -> Result<Done, String> {
    let args = ["test", "-p", "game-testbed", "--test", "crash_json"];
    pass(ci::cargo_step_env(
        root,
        "native-crash-json",
        &args,
        &REQUIRE,
    ))
}

/// Both forms of the version stamp, every value set (criterion 8).
fn version(root: &Path) -> Result<Done, String> {
    let binary = testbed(root)?;
    let plain = util::run_quiet(
        root,
        "native-version",
        &mut probe(root, &binary, &["--version"]),
    )?;
    let mut json = probe(root, &binary, &["--version", "--json"]);
    let json = util::run_quiet(root, "native-version-json", &mut json)?;
    let out = root.join("target/ci");
    let _ = fs::write(out.join("version.txt"), &plain);
    let _ = fs::write(out.join("version.json"), &json);
    let github =
        ["GITHUB_SHA", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT"].map(|v| std::env::var(v).ok());
    let expect = match github {
        [Some(sha), Some(id), Some(attempt)] => Some((sha, format!("github-{id}/{attempt}"))),
        _ => None,
    };
    let expect = expect
        .as_ref()
        .map(|(sha, run)| (sha.as_str(), run.as_str()));
    pass(verdicts::version(&plain, &json, expect))
}

/// The shot harness against the committed goldens (criteria 4 and 5,
/// amendment 57): exactly on the pinned configuration, or with FLIP. Its
/// `shots.json` decides: a pass only if every shot passed, pending while a
/// golden is missing (plan decision 3).
fn shots(root: &Path, exact: bool) -> Result<Done, String> {
    let summary = root.join("target/golden-candidates/shots.json");
    let _ = fs::remove_file(&summary);
    let mut cmd = ci::cargo_command(root, &["test", "-p", "game-testbed", "--test", "shots"]);
    cmd.env("ENG_REQUIRE_GPU", "1").env_remove("ENG_GOLDEN_DIR");
    if let Some(adapter) = offscreen_adapter(&replay::host_triple(root)?) {
        cmd.env("ENG_GPU_ADAPTER", adapter);
    }
    if exact {
        cmd.env("ENG_SHOT_MODE", "exact");
    } else {
        cmd.env_remove("ENG_SHOT_MODE");
    }
    let name = if exact {
        "native-shot-exact"
    } else {
        "native-shot-flip"
    };
    util::run_quiet(root, name, &mut cmd)?;
    let text = read_text(&summary)
        .map_err(|e| format!("[E-GOLDEN] the shot harness wrote no shots.json: {e}"))?;
    verdicts::shots(&text, exact)
}

fn shot_exact(root: &Path) -> Result<Done, String> {
    shots(root, true)
}

fn shot_flip(root: &Path) -> Result<Done, String> {
    shots(root, false)
}

/// `play` in a real window gives the committed hash stream and presents
/// frames (criterion 2, amendment 46).
fn window(root: &Path) -> Result<Done, String> {
    let binary = testbed(root)?;
    let (hashes, report) = (
        arg(root, "target/ci/window.txt"),
        arg(root, "target/ci/window.json"),
    );
    let script = arg(root, SCRIPT);
    let args = [
        "play", "--ticks", "240", "--seed", "1", "--input", &script, "--hashes", &hashes,
        "--report", &report,
    ];
    util::run_quiet(root, "native-window", &mut probe(root, &binary, &args))?;
    let stream = read_text(&root.join("target/ci/window.txt"))?;
    let committed = read_text(&root.join(STREAM))?;
    let report = read_json(&root.join("target/ci/window.json"))?;
    pass(verdicts::window(&stream, &committed, &report))
}

/// A shot held to `budgets.toml [gpu]` by the probe (exit 13 over), whose
/// report records the adapter; the window's frames are held too (criterion
/// 9, amendment 42).
fn gpu_budgets(root: &Path) -> Result<Done, String> {
    let binary = testbed(root)?;
    let (png, report) = (
        arg(root, "target/ci/shot.png"),
        arg(root, "target/ci/shot.json"),
    );
    let (script, budgets) = (arg(root, SCRIPT), arg(root, "budgets.toml"));
    let args = [
        "shot",
        "--at-tick",
        "240",
        "--seed",
        "1",
        "--input",
        &script,
        "--budgets",
        &budgets,
        "--report",
        &report,
        "--out",
        &png,
    ];
    util::run_quiet(
        root,
        "native-gpu-budgets",
        &mut offscreen(root, &binary, &args)?,
    )?;
    let shot = read_json(&root.join("target/ci/shot.json"))?;
    let window = read_json(&root.join("target/ci/window.json")).ok();
    let budgets = read_text(&root.join("budgets.toml"))?;
    pass(verdicts::gpu_budgets(&shot, &budgets, window.as_ref()))
}

/// Escape from the keyboard through XTEST quits `play` within 5 s
/// (criterion 3): `play.rs`'s test, which finds the window by the child's
/// PID (`xdotool search --pid`), then runs `windowfocus --sync` and `key
/// Escape` (amendments 37 and 46).
fn esc(root: &Path) -> Result<Done, String> {
    const TEST: &str = "escape_from_the_keyboard_quits_within_5_seconds";
    let args = [
        "test",
        "-p",
        "game-testbed",
        "--test",
        "play",
        "--",
        "--exact",
        TEST,
    ];
    let mut cmd = ci::cargo_command(root, &args);
    cmd.envs(REQUIRE);
    let output = util::run_quiet(root, "native-esc", &mut cmd)?;
    verdicts::ran_test(&output, TEST)?;
    Ok(Done::Pass(format!(
        "{TEST}: Escape through XTEST (xdotool windowfocus --sync, key Escape) quit play \
         within 5 s"
    )))
}

/// The proof video of the scripted session (criterion 15, amendment 42):
/// 241 frames, uploaded with the job's files.
fn proof_video(root: &Path) -> Result<Done, String> {
    let binary = testbed(root)?;
    let (apng, report) = (
        arg(root, "target/ci/proof.apng"),
        arg(root, "target/ci/proof.json"),
    );
    let script = arg(root, SCRIPT);
    let args = [
        "video", "--ticks", "240", "--seed", "1", "--input", &script, "--out", &apng, "--report",
        &report,
    ];
    util::run_quiet(
        root,
        "native-proof-video",
        &mut offscreen(root, &binary, &args)?,
    )?;
    let bytes = fs::read(root.join("target/ci/proof.apng")).map_err(|e| format!("{apng}: {e}"))?;
    let report = read_json(&root.join("target/ci/proof.json"))?;
    pass(verdicts::video(&bytes, &report, 241))
}

/// The Intel Mac is compile-checked only (spec 8.2, criterion 1). The
/// workflow adds the target in the macOS job only.
fn check_intel_mac(root: &Path) -> Result<Done, String> {
    let mut libdir = Command::new("rustc");
    libdir
        .current_dir(root)
        .args(["--print", "target-libdir", "--target", INTEL_MAC]);
    let libdir = util::run_quiet(root, "native-intel-libdir", &mut libdir)?;
    if !Path::new(libdir.trim()).is_dir() {
        return Err(format!(
            "[E-NO-TARGET] {INTEL_MAC}'s standard library is not installed: `rustup target add \
             {INTEL_MAC}` (the macos-26 job does)"
        ));
    }
    let args = [
        "check",
        "--workspace",
        "--all-targets",
        "--target",
        INTEL_MAC,
    ];
    pass(ci::cargo_step(root, "native-check-intel-mac", &args))
}

#[cfg(test)]
#[path = "ci_native_tests.rs"]
mod tests;
