//! What the native job's steps and the summary conclude from the files they
//! read (rung 1b, patches X2 and X3). Pure functions, so each verdict is
//! tested without a GPU, a window or GitHub (`ci_verdicts_tests.rs`).

use serde_json::Value;

use crate::ci_native::RUNNERS;
use crate::util::{self, Done};

/// A JSON value as text: a string as it is, anything else as JSON.
fn text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), String::from)
}

/// A shot step's verdict from `target/golden-candidates/shots.json`, which
/// `crates/game-testbed/tests/shots.rs` writes (amendment 57):
/// - a pass only if every shot's status is `pass` against the committed
///   goldens: exactly on the pinned configuration for `shot-exact`
///   (`exact`), with FLIP for `shot-flip`;
/// - pending while any shot has no golden or no thresholds, so acceptance
///   stays false until the owner commits the goldens (plan decision 3);
/// - otherwise a failure naming each shot that failed.
pub fn shots(summary: &str, exact: bool) -> Result<Done, String> {
    let summary: Value =
        serde_json::from_str(summary).map_err(|e| format!("[E-GOLDEN] shots.json: {e}"))?;
    let config = &summary["configuration"];
    let adapter = format!(
        "`{}` `{}`",
        text(&config["adapter"]),
        text(&config["driver_info"])
    );
    let mode = summary["mode"].as_str().unwrap_or_default();
    let wanted = if exact { "exact" } else { "flip" };
    if mode != wanted {
        return Err(format!(
            "[E-GOLDEN] the shots were compared by {mode:?}, not {wanted}, on {adapter}"
        ));
    }
    if exact && config["pinned"] != true {
        return Err(format!(
            "[E-GOLDEN] shot-exact needs the pinned configuration, not {adapter}"
        ));
    }
    let golden = text(&summary["golden_dir"]).replace('\\', "/");
    if !golden.ends_with("crates/game-testbed/golden") {
        return Err(format!(
            "[E-GOLDEN] the shots were compared with {golden:?}, not the committed goldens"
        ));
    }
    let shots = summary["shots"]
        .as_array()
        .filter(|shots| !shots.is_empty())
        .ok_or("[E-GOLDEN] shots.json names no shot")?;
    let (mut failed, mut pending) = (Vec::new(), Vec::new());
    for shot in shots {
        let name = text(&shot["name"]);
        match shot["status"].as_str() {
            Some("pass") => {}
            Some(status @ ("no-golden" | "no-thresholds")) => {
                pending.push(format!("{name} {status}"));
            }
            Some("fail") => failed.push(text(&shot["message"])),
            other => failed.push(format!("{name}: [E-GOLDEN] unknown status {other:?}")),
        }
    }
    if !failed.is_empty() {
        return Err(failed.join("\n"));
    }
    let count = shots.len();
    Ok(if pending.is_empty() {
        Done::Pass(format!("{count} shots pass ({mode}) on {adapter}"))
    } else {
        Done::Pending(format!(
            "pending, not a pass, until the goldens are committed (G6): {}; {count} shots \
             ({mode}) on {adapter}; candidates in target/golden-candidates/",
            pending.join(", ")
        ))
    })
}

/// The `version` step's verdict (criterion 8, amendment 15): `--version
/// --json` (`json`) has every `build` value, none of them `unknown` or null,
/// and `--version` (`plain`) prints the same stamp on one line: the game and
/// its version, the engine commit, the target and the profile. On GitHub,
/// `expect` is the commit and `ci_run` the stamp must name.
pub fn version(plain: &str, json: &str, expect: Option<(&str, &str)>) -> Result<String, String> {
    let stamp: Value =
        serde_json::from_str(json).map_err(|e| format!("[E-VERSION] --version --json: {e}"))?;
    let build = stamp["build"]
        .as_object()
        .filter(|build| !build.is_empty())
        .ok_or("[E-VERSION] --version --json has no build values")?;
    let unknown: Vec<&str> = build
        .iter()
        .filter(|(_, value)| value.is_null() || value.as_str() == Some("unknown"))
        .map(|(key, _)| key.as_str())
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "[E-VERSION] build values unknown or null: {}",
            unknown.join(", ")
        ));
    }
    let field = |key: &str| build.get(key).map(text).unwrap_or_default();
    let line = plain.strip_suffix('\n').unwrap_or(plain);
    let line = line.strip_suffix('\r').unwrap_or(line);
    let sha = field("engine_sha");
    let short = sha.get(..12).unwrap_or(&sha);
    let head = format!("{} {} ", text(&stamp["game"]), field("game_version"));
    let tail = format!("{} {},", field("target"), field("profile"));
    let same = line.starts_with(&head) && line.contains(short) && line.contains(&tail);
    if line.is_empty() || line.contains('\n') || !same {
        return Err(format!(
            "[E-VERSION] --version is not the stamp on one line (`{head}`, `{short}`, \
             `{tail}`): {plain:?}"
        ));
    }
    if let Some((commit, run)) = expect
        && (sha != commit || field("ci_run") != run)
    {
        return Err(format!(
            "[E-VERSION] the stamp names {sha} and {}, not this run's {commit} and {run}",
            field("ci_run")
        ));
    }
    Ok(format!("{line}; every build value set"))
}

/// The `window` step's verdict (criterion 2, amendment 46): `play` exited ok
/// in a real window (`winit`), its hash stream (`stream`) is the committed
/// one byte for byte, and it presented frames. A window that is never
/// visible, as on a Mac with the screen locked, presents nothing and fails.
pub fn window(stream: &str, committed: &str, report: &Value) -> Result<String, String> {
    if stream != committed {
        let line = stream
            .split_inclusive('\n')
            .zip(committed.split_inclusive('\n'))
            .take_while(|(ours, theirs)| ours == theirs)
            .count()
            + 1;
        return Err(format!(
            "[E-HASH] the window's hash stream differs from the committed one at line {line}"
        ));
    }
    if report["exit"] != "ok" {
        return Err(format!("play exited {}", report["exit"]));
    }
    if report["platform"] != "winit" {
        return Err(format!(
            "play ran on {}, not in a window (winit)",
            report["platform"]
        ));
    }
    let gpu = &report["gpu"];
    let frames = gpu["frames"].as_u64().unwrap_or(0);
    if frames == 0 {
        return Err(
            "no frame presented: the window was never visible (on macOS, a locked or sleeping \
             screen hides every window)"
                .into(),
        );
    }
    Ok(format!(
        "{} ticks, the committed hash stream; {frames} frames of {}x{} presented on {} \
         (surface {}), {} dropped ticks",
        report["ticks_run"],
        gpu["width"],
        gpu["height"],
        text(&gpu["adapter"]),
        text(&gpu["surface_format"]),
        report["dropped_ticks"]
    ))
}

/// The four per-frame counters and their `budgets.toml [gpu]` limits.
const COUNTERS: [(&str, &str); 4] = [
    ("draws", "max_draws_per_frame"),
    ("pipeline_switches", "max_pipeline_switches_per_frame"),
    ("bind_group_switches", "max_bind_group_switches_per_frame"),
    ("upload_bytes", "max_upload_bytes_per_frame"),
];

fn gpu_limits(budgets: &str) -> Result<Vec<(&'static str, u64)>, String> {
    let table: toml::Table = budgets
        .parse()
        .map_err(|e: toml::de::Error| format!("budgets.toml: {e}"))?;
    let gpu = table
        .get("gpu")
        .and_then(toml::Value::as_table)
        .ok_or("[E-BUDGET] budgets.toml has no [gpu]")?;
    COUNTERS
        .iter()
        .map(|&(counter, key)| {
            let limit = gpu
                .get(key)
                .and_then(toml::Value::as_integer)
                .and_then(|limit| u64::try_from(limit).ok())
                .ok_or_else(|| format!("[E-BUDGET] budgets.toml [gpu] has no {key}"))?;
            Ok((counter, limit))
        })
        .collect()
}

/// Holds a report's `per_frame` peaks to `limits`.
fn hold(per_frame: &Value, limits: &[(&str, u64)], what: &str) -> Result<(), String> {
    if !per_frame.is_object() {
        return Err(format!(
            "[E-BUDGET] the {what}'s report has no per-frame counters"
        ));
    }
    let over: Vec<String> = limits
        .iter()
        .filter_map(|&(counter, limit)| match per_frame[counter].as_u64() {
            Some(used) if used <= limit => None,
            Some(used) => Some(format!("{counter} {used} > {limit}")),
            None => Some(format!("{counter} missing")),
        })
        .collect();
    if over.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "[E-BUDGET] the {what}'s frame exceeds budgets.toml [gpu]: {}",
            over.join(", ")
        ))
    }
}

/// The `gpu-budgets` step's verdict (criterion 9, amendment 42): the shot's
/// report records the adapter, its driver and capabilities, and the frame's
/// counters are within `budgets.toml [gpu]`; so are the window's peaks when
/// the `window` step presented frames (a window frame adds the present
/// pass).
pub fn gpu_budgets(shot: &Value, budgets: &str, window: Option<&Value>) -> Result<String, String> {
    if shot["exit"] != "ok" {
        return Err(format!("the shot exited {}", shot["exit"]));
    }
    let gpu = &shot["gpu"];
    let adapter = gpu["adapter"]
        .as_str()
        .filter(|adapter| !adapter.is_empty())
        .ok_or("[E-GPU-REPORT] the shot's report names no adapter")?;
    for key in ["backend", "device_type", "driver", "driver_info"] {
        if !gpu[key].is_string() {
            return Err(format!("[E-GPU-REPORT] the shot's report has no gpu.{key}"));
        }
    }
    if !gpu["capabilities"].is_object() {
        return Err("[E-GPU-REPORT] the shot's report has no gpu.capabilities".into());
    }
    let limits = gpu_limits(budgets)?;
    hold(&gpu["per_frame"], &limits, "shot")?;
    let windowed = match window {
        Some(report) if report["gpu"]["frames"].as_u64().unwrap_or(0) > 0 => {
            hold(&report["gpu"]["per_frame"], &limits, "window")?;
            format!("the window's frames too ({})", report["gpu"]["per_frame"])
        }
        Some(_) => "the window presented no frame, so only the shot is held".into(),
        None => "no window report, so only the shot is held".into(),
    };
    Ok(format!(
        "{adapter} ({}, {}), driver `{}` `{}`; the shot's frame {} within budgets.toml [gpu]; \
         {windowed}",
        text(&gpu["backend"]),
        text(&gpu["device_type"]),
        text(&gpu["driver"]),
        text(&gpu["driver_info"]),
        gpu["per_frame"]
    ))
}

/// The `proof-video` step's verdict (criterion 15, amendment 42): an
/// animated PNG whose `acTL` chunk says `frames` frames, whole up to its
/// `IEND`, and a report that rendered as many.
pub fn video(apng: &[u8], report: &Value, frames: u64) -> Result<String, String> {
    if report["exit"] != "ok" {
        return Err(format!("video exited {}", report["exit"]));
    }
    let mut rest = apng
        .strip_prefix(b"\x89PNG\r\n\x1a\n".as_slice())
        .ok_or("[E-VIDEO] proof.apng is not a PNG file")?;
    let (mut animation, mut ended) = (None, false);
    while let (Some(head), false) = (rest.get(..8), ended) {
        let length = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
        let length = usize::try_from(length).map_err(|e| e.to_string())?;
        let data = rest
            .get(8..8 + length)
            .filter(|_| rest.len() >= 12 + length)
            .ok_or("[E-VIDEO] proof.apng is cut short")?;
        match &head[4..8] {
            b"acTL" if length >= 8 => {
                animation = Some(u32::from_be_bytes([data[0], data[1], data[2], data[3]]));
            }
            b"IEND" => ended = true,
            _ => {}
        }
        rest = &rest[12 + length..];
    }
    if !ended {
        return Err("[E-VIDEO] proof.apng is cut short: no IEND chunk".into());
    }
    let count = animation.ok_or("[E-VIDEO] proof.apng is a still PNG: no acTL chunk")?;
    if u64::from(count) != frames {
        return Err(format!(
            "[E-VIDEO] proof.apng has {count} frames, not {frames}"
        ));
    }
    if report["gpu"]["frames"].as_u64() != Some(frames) {
        return Err(format!(
            "[E-VIDEO] the report rendered {} frames, not {frames}",
            report["gpu"]["frames"]
        ));
    }
    Ok(format!(
        "proof.apng: {frames} frames, {} bytes, on {}",
        apng.len(),
        text(&report["gpu"]["adapter"])
    ))
}

/// Whether libtest's output shows the test named `test` run and passed: a
/// filter that matches nothing runs no test and still exits 0.
pub fn ran_test(output: &str, test: &str) -> Result<(), String> {
    let line = format!("test {test} ... ok");
    if output.lines().any(|l| l.trim_end() == line) {
        Ok(())
    } else {
        Err(format!(
            "[E-TEST] the test {test} did not run and pass:\n{}",
            util::tail(output, 5)
        ))
    }
}

/// The hash-stream artifacts `compare-hashes` needs (criterion 7): the test
/// job's two Linux runners, RISC-V, and each native runner.
pub fn expected_streams() -> Vec<String> {
    let mut names: Vec<String> = [
        "hashes-ubuntu-24.04",
        "hashes-ubuntu-24.04-arm",
        "hashes-riscv",
    ]
    .map(String::from)
    .to_vec();
    names.extend(
        RUNNERS
            .iter()
            .map(|(runner, _)| format!("hashes-native-{runner}")),
    );
    names
}

/// The hash-stream artifacts that `compare-hashes` needs but did not get.
pub fn missing_streams(found: &[String]) -> Vec<String> {
    expected_streams()
        .into_iter()
        .filter(|name| !found.contains(name))
        .collect()
}

#[cfg(test)]
#[path = "ci_verdicts_tests.rs"]
mod tests;
