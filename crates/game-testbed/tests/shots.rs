//! Golden images of the testbed's shots (rung 1b, task T12; plan decision
//! 3, spec 8.3), rendered by the real binary's `shot`.
//!
//! Each shot in [`SHOTS`] is rendered at 320x240, and its decoded RGBA8
//! pixels, not its PNG bytes, are compared with `golden/<name>.png`
//! (`shots/compare.rs`):
//! - exactly on the pinned configuration (plan criterion 4): Linux x64 and
//!   Mesa 25.2.7's lavapipe (gfx-rs/ci-build build26) with LLVM 18.1.3 and
//!   256-bit vectors, which the harness fixes by passing
//!   `LP_NATIVE_VECTOR_WIDTH=256` to every shot (and no other `LP_*` or
//!   `GALLIVM_*` variable);
//! - with FLIP everywhere else (criterion 5), against the limits in
//!   `golden/thresholds.toml`: the mean and the 99th and 99.9th
//!   percentiles of the per-pixel errors.
//!
//! The goldens are the owner's (`golden/` is protected), so the harness
//! never writes there. While there are none, each shot is saved as a
//! candidate in `target/golden-candidates/` and reported as "no golden yet"
//! (`[W-NO-GOLDEN]`), which does not fail (G6); without `thresholds.toml` a
//! FLIP comparison is measured but cannot fail either
//! (`[W-NO-THRESHOLDS]`). Neither is a pass for acceptance: `shots.json`
//! says which it was. Once one golden exists, every shot needs its own, and
//! a golden that no shot names fails, as `replay` fails on an unreferenced
//! assertion. A shot that fails is saved there too, every FLIP comparison
//! leaves its error map in `golden-candidates/flip/`, and each run, after
//! emptying the directory, writes `golden-candidates/shots.json`: the
//! configuration, the machine and each shot's result.
//!
//! The environment (an empty value is unset):
//! - `ENG_REQUIRE_GPU=1` (CI's native jobs): no adapter fails instead of
//!   skipping, as in eng-gpu's tests.
//! - `ENG_GPU_ADAPTER`: the probe picks its adapter by name.
//! - `ENG_SHOT_MODE=exact` (CI's `shot-exact` step): any configuration but
//!   the pinned one fails, so a runner that drifted cannot fall back to
//!   FLIP. Unset, the configuration decides.
//! - `ENG_GOLDEN_DIR`: another directory of goldens and thresholds, such as
//!   another machine's candidates or a copy with a pixel edited (the
//!   selftest of criterion 12), instead of `crates/game-testbed/golden`. It
//!   must exist, outside `golden-candidates/` (which each run empties), and
//!   hold a golden for every shot.
//!
//! Each shot is killed after [`LIMIT`] and fails, so a GPU readback that
//! never returns cannot hang a CI job.

#[path = "shots/compare.rs"]
mod compare;
#[path = "shots/parts.rs"]
mod parts;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use compare::{Mode, Status, check};
use serde_json::{Value, json};

/// One golden shot: `shot --at-tick <tick> --seed 1 --players <players>`.
struct Shot {
    name: &'static str,
    tick: u32,
    players: u32,
    /// An input script in `scenarios/`.
    input: Option<&'static str>,
}

/// T9's suggestion (its open question 1), and its ten dots at tick 0, one
/// in each slot colour. On 2026-10-31 all four were byte-identical on Metal
/// (Apple M2 Pro) and on the pinned lavapipe. Pixels whose centre lies
/// within rounding of a dot's edge could still differ elsewhere: the
/// nearest centres are 0.032, 0.006, 0.005 and 0.0009 px from an edge.
const SHOTS: [Shot; 4] = [
    Shot {
        name: "dot-square-0",
        tick: 0,
        players: 1,
        input: Some("dot-square.input.toml"),
    },
    Shot {
        name: "dot-square-120",
        tick: 120,
        players: 1,
        input: Some("dot-square.input.toml"),
    },
    Shot {
        name: "dot-square-240",
        tick: 240,
        players: 1,
        input: Some("dot-square.input.toml"),
    },
    Shot {
        name: "ten-dots-0",
        tick: 0,
        players: 10,
        input: None,
    },
];

/// The pinned configuration's adapter and driver, as a shot's report names
/// them (T9's `gpu` object): Mesa 25.2.7's lavapipe from gfx-rs/ci-build
/// build26, which links Ubuntu 24.04's LLVM 18.1.3, with 256-bit vectors.
const PINNED_ADAPTER: &str = "llvmpipe (LLVM 18.1.3, 256 bits)";
const PINNED_DRIVER_INFO: &str = "Mesa 25.2.7 (LLVM 18.1.3)";

/// Where a shot was rendered: the test's platform and the adapter its
/// report names.
#[derive(Clone, Debug, PartialEq)]
struct Config {
    os: String,
    arch: String,
    adapter: String,
    backend: String,
    device_type: String,
    driver: String,
    driver_info: String,
}

impl Config {
    fn from_report(gpu: &Value) -> Config {
        let field = |key: &str| match gpu[key].as_str() {
            Some(value) => value.to_owned(),
            None => panic!("the report's gpu.{key} is missing: {gpu}"),
        };
        Config {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            adapter: field("adapter"),
            backend: field("backend"),
            device_type: field("device_type"),
            driver: field("driver"),
            driver_info: field("driver_info"),
        }
    }

    fn pinned(&self) -> bool {
        self.os == "linux"
            && self.arch == "x86_64"
            && self.backend == "vulkan"
            && self.device_type == "cpu"
            && self.driver == "llvmpipe"
            && self.adapter == PINNED_ADAPTER
            && self.driver_info == PINNED_DRIVER_INFO
    }

    fn json(&self) -> Value {
        json!({
            "os": self.os, "arch": self.arch, "adapter": self.adapter,
            "backend": self.backend, "device_type": self.device_type,
            "driver": self.driver, "driver_info": self.driver_info,
            "pinned": self.pinned(),
        })
    }
}

/// How shots rendered on `config` are compared; `requested` is
/// `ENG_SHOT_MODE`.
fn mode(config: &Config, requested: Option<&str>) -> Result<Mode, String> {
    match requested {
        None | Some("") if config.pinned() => Ok(Mode::Exact),
        None | Some("") => Ok(Mode::Flip),
        Some("exact") if config.pinned() => Ok(Mode::Exact),
        Some("exact") => Err(format!(
            "[E-GOLDEN] ENG_SHOT_MODE=exact needs the pinned configuration (linux \
             x86_64, vulkan, `{PINNED_ADAPTER}`, `{PINNED_DRIVER_INFO}`), not {} {} {} \
             `{}` `{}`",
            config.os, config.arch, config.backend, config.adapter, config.driver_info
        )),
        Some(other) => Err(format!(
            "[E-GOLDEN] ENG_SHOT_MODE is `exact` or unset, not `{other}`"
        )),
    }
}

/// The command that renders `shot` into `dir`, with the llvmpipe
/// environment fixed: `LP_NATIVE_VECTOR_WIDTH=256` and no other `LP_*` or
/// `GALLIVM_*` variable.
fn shot_command(shot: &Shot, dir: &Path) -> Command {
    let (tick, players) = (shot.tick.to_string(), shot.players.to_string());
    let mut command = Command::new(env!("CARGO_BIN_EXE_game-testbed"));
    command.args(["shot", "--at-tick", &tick, "--seed", "1"]);
    command.args(["--players", &players, "--size", "320x240", "--out"]);
    command.arg(dir.join(format!("{}.png", shot.name)));
    command
        .arg("--report")
        .arg(dir.join(format!("{}.json", shot.name)));
    if let Some(input) = shot.input {
        let scenarios = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios");
        command.arg("--input").arg(scenarios.join(input));
    }
    command.env("ENG_AGENT_LOG_DIR", dir.join("logs"));
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("LP_") || name.starts_with("GALLIVM_") {
            command.env_remove(&key);
        }
    }
    command.env("LP_NATIVE_VECTOR_WIDTH", "256");
    command
}

/// How long a shot may take: far longer than any takes, and far shorter
/// than a CI job's limit.
const LIMIT: Duration = Duration::from_secs(120);

/// `command`'s output, as `Command::output` gives it, but the shot is killed
/// after [`LIMIT`] and fails. The probe prints a few lines, so its pipes
/// cannot fill while it runs.
fn output_within(command: &mut Command, name: &str) -> Output {
    let mut child = command
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
    let output = child.wait_with_output().unwrap();
    assert!(
        finished,
        "{name}: killed after {LIMIT:?}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// A shot's PNG file's bytes and where it was rendered; `None` when it was
/// skipped for want of an adapter.
type Rendered = Option<(Vec<u8>, Config)>;

/// Renders `shot` with the real binary into `dir`, with `env` added to its
/// environment. Without an adapter, `None`, or an error if one is
/// `required`.
fn render(
    shot: &Shot,
    dir: &Path,
    required: bool,
    env: &[(&str, &str)],
) -> Result<Rendered, String> {
    let mut command = shot_command(shot, dir);
    command.envs(env.iter().copied());
    let output = output_within(&mut command, shot.name);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.code() == Some(64) && text.contains("[E-NO-ADAPTER]") {
        if required {
            return Err(format!("ENG_REQUIRE_GPU=1: {text}"));
        }
        eprintln!("skipped: {text}");
        return Ok(None);
    }
    assert_eq!(output.status.code(), Some(0), "{}: {text}", shot.name);
    let report = std::fs::read(dir.join(format!("{}.json", shot.name))).unwrap();
    let report: Value = serde_json::from_slice(&report).unwrap();
    let png = std::fs::read(dir.join(format!("{}.png", shot.name))).unwrap();
    Ok(Some((png, Config::from_report(&report["gpu"]))))
}

/// The golden directory: `ENG_GOLDEN_DIR` (`requested`) if set and not
/// empty, else `golden/`, and whether it was asked for. One asked for must
/// exist outside `candidates`, which each run empties first.
fn golden_dir(requested: Option<OsString>, candidates: &Path) -> Result<(PathBuf, bool), String> {
    let Some(dir) = requested.filter(|dir| !dir.is_empty()).map(PathBuf::from) else {
        let default = Path::new(env!("CARGO_MANIFEST_DIR")).join("golden");
        return Ok((default, false));
    };
    let shown = dir.display();
    let Ok(canonical) = dir.canonicalize() else {
        return Err(format!("[E-GOLDEN] ENG_GOLDEN_DIR {shown} does not exist"));
    };
    if !canonical.is_dir() {
        return Err(format!(
            "[E-GOLDEN] ENG_GOLDEN_DIR {shown} is not a directory"
        ));
    }
    if candidates
        .canonicalize()
        .is_ok_and(|c| canonical.starts_with(c))
    {
        return Err(format!(
            "[E-GOLDEN] ENG_GOLDEN_DIR {shown} lies in the candidates directory, which each \
             run empties; copy it elsewhere"
        ));
    }
    Ok((dir, true))
}

/// The OS version and CPU, as far as they can be read, for `shots.json`
/// (spec 8.3 records OS and driver versions; Metal names no driver).
fn machine() -> Value {
    let run = |program: &str, args: &[&str]| {
        let output = Command::new(program).args(args).output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (output.status.success() && !text.is_empty()).then_some(text)
    };
    let field = |path: &str, key: &str| {
        let text = std::fs::read_to_string(path).ok()?;
        let line = text.lines().find(|line| line.starts_with(key))?;
        let value = line.split_once(['=', ':'])?.1;
        Some(value.trim().trim_matches('"').to_owned())
    };
    let (os_version, cpu) = match std::env::consts::OS {
        "macos" => (
            run("sw_vers", &["-productVersion"]).map(|v| format!("macOS {v}")),
            run("sysctl", &["-n", "machdep.cpu.brand_string"]),
        ),
        "linux" => (
            field("/etc/os-release", "PRETTY_NAME="),
            field("/proc/cpuinfo", "model name"),
        ),
        "windows" => (
            run("cmd", &["/c", "ver"]),
            std::env::var("PROCESSOR_IDENTIFIER").ok(),
        ),
        _ => (None, None),
    };
    json!({"os_version": os_version, "cpu": cpu})
}

/// Checks each rendered shot (name, PNG) against `refs`, writes
/// `shots.json` into `candidates`, and returns the failures. A missing
/// golden fails when the directory was asked for (`explicit`) or holds any
/// shot's golden: only a directory with none yet is "no golden yet".
fn judge(
    shots: &[(&str, Vec<u8>)],
    config: &Config,
    mode: Mode,
    (refs, explicit): (&Path, bool),
    candidates: &Path,
) -> Vec<String> {
    let has_golden = |name: &str| refs.join(format!("{name}.png")).exists();
    let complete = explicit || shots.iter().any(|(name, _)| has_golden(name));
    let mut results = Vec::new();
    let mut failures = Vec::new();
    for (name, png) in shots {
        let mut verdict = check(name, png, mode, refs, candidates);
        if verdict.status == Status::NoGolden && complete {
            verdict.status = Status::Fail;
            let why = if explicit {
                "ENG_GOLDEN_DIR is set"
            } else {
                "other shots have theirs"
            };
            let at = refs.join(format!("{name}.png"));
            verdict.message = format!("{name}: [E-GOLDEN] {} is missing; {why}", at.display());
        }
        eprintln!("{}", verdict.message);
        if verdict.status == Status::Fail {
            failures.push(verdict.message.clone());
        }
        results.push(verdict.json(name));
    }
    // A golden that no shot names (a shot renamed or dropped) would never be
    // checked; only the owner can remove it.
    let mut files: Vec<PathBuf> = match std::fs::read_dir(refs) {
        Ok(entries) => entries.map(|entry| entry.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    for file in files {
        let named = |name: &str| shots.iter().any(|(shot, _)| *shot == name);
        let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned());
        let png = file.extension().is_some_and(|e| e == "png");
        if let Some(stem) = stem.filter(|stem| png && !named(stem)) {
            let message = format!("{stem}: [E-GOLDEN] {} is no shot's golden", file.display());
            eprintln!("{message}");
            failures.push(message);
        }
    }
    let summary = json!({
        "configuration": config.json(),
        "machine": machine(),
        "mode": if mode == Mode::Exact { "exact" } else { "flip" },
        "golden_dir": refs.display().to_string(),
        "shots": results,
    });
    let text = serde_json::to_string_pretty(&summary).unwrap() + "\n";
    std::fs::write(candidates.join("shots.json"), text).unwrap();
    failures
}

#[test]
fn shots_match_their_goldens() {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    // `CARGO_TARGET_TMPDIR` is `target/tmp`.
    let candidates = tmp.parent().unwrap().join("golden-candidates");
    let requested = std::env::var_os("ENG_GOLDEN_DIR");
    let refs = golden_dir(requested, &candidates).unwrap_or_else(|e| panic!("{e}"));
    let _ = std::fs::remove_dir_all(&candidates);
    std::fs::create_dir_all(&candidates).unwrap();
    let dir = tmp.join("shots").join("render");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let required = std::env::var("ENG_REQUIRE_GPU").as_deref() == Ok("1");
    let mut shots = Vec::new();
    let mut configs = Vec::new();
    for shot in &SHOTS {
        let rendered = render(shot, &dir, required, &[]);
        let Some((png, config)) = rendered.unwrap_or_else(|e| panic!("{e}")) else {
            return;
        };
        shots.push((shot.name, png));
        configs.push(config);
    }
    let config = configs[0].clone();
    assert!(configs.iter().all(|c| *c == config), "one adapter");
    let requested = std::env::var("ENG_SHOT_MODE").ok();
    let mode = mode(&config, requested.as_deref()).unwrap_or_else(|e| panic!("{e}"));
    let refs = (refs.0.as_path(), refs.1);
    let failures = judge(&shots, &config, mode, refs, &candidates);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
