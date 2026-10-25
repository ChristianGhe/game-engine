//! `cargo xtask quick` and `cargo xtask ci --job <job>` (spec 8.2).
//!
//! Each job prints one line per step and writes `target/ci/report.json`
//! (plus a copy under `target/ci/reports/`), which `acceptance` reads.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde_json::json;

use crate::util::{self, Steps};
use crate::{arch, docs, replay, selftest, size};

pub const RISCV: &str = "riscv64gc-unknown-linux-gnu";
pub const WASM: &str = "wasm32-unknown-unknown";

fn cargo_step(root: &Path, name: &str, args: &[&str]) -> Result<String, String> {
    let mut args = args.to_vec();
    if util::on_ci() && !matches!(args.first(), Some(&("fmt" | "deny"))) {
        // After the subcommand, never after a `--`.
        let at = if args.first() == Some(&"nextest") {
            2
        } else {
            1
        };
        args.insert(at.min(args.len()), "--locked");
    }
    let mut cmd = util::cargo(root);
    cmd.args(&args);
    util::run_quiet(root, name, &mut cmd).map(|_| String::new())
}

fn clippy(root: &Path) -> Result<String, String> {
    let mut args = vec!["clippy", "--workspace", "--all-targets"];
    if util::on_ci() {
        args.extend(["--", "-D", "warnings"]);
    }
    cargo_step(root, "clippy", &args)
}

fn has_tool(root: &Path, args: &[&str]) -> bool {
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]).current_dir(root);
    cmd.output().is_ok_and(|o| o.status.success())
}

fn tests(root: &Path) -> Result<String, String> {
    let nextest = has_tool(root, &["cargo", "nextest", "--version"]);
    let mut args = if nextest {
        vec!["nextest", "run", "--workspace", "--no-fail-fast"]
    } else {
        vec!["test", "--workspace", "--no-fail-fast"]
    };
    if nextest && util::on_ci() {
        args.extend(["--profile", "ci"]);
    }
    cargo_step(root, "tests", &args)?;
    if nextest {
        cargo_step(root, "doctests", &["test", "--workspace", "--doc"])?;
    }
    Ok(if nextest {
        String::new()
    } else {
        "cargo-nextest not installed; ran cargo test".into()
    })
}

/// The `test` job's named harness steps, in order: the step's name, then
/// the package and test target it runs (`crates/<package>/tests/<target>.rs`).
/// `tests` runs them as well; a step of their own gives each its own entry
/// in the job's report, where acceptance checks find steps by name, and its
/// own log. They build on the dev profile: `crash_json` looks for the
/// `eng_cli::run::run` frame, which a release build without debug info can
/// inline away. Like every cargo command here, they get the stamp's four
/// values from `util::cargo` (`stamp.rs`), so `version` checks a stamp with
/// all of them set.
const HARNESSES: [(&str, &str, &str); 5] = [
    ("known-answers", "eng-math", "known_answers"),
    ("exit-codes", "game-testbed", "exit_codes"),
    ("crash-json", "game-testbed", "crash_json"),
    ("version", "game-testbed", "version"),
    ("live-input", "eng-input", "live_input"),
];

/// One harness from `HARNESSES`, logged under the step's name.
fn harness(root: &Path, step: &str, package: &str, target: &str) -> Result<String, String> {
    cargo_step(root, step, &["test", "-p", package, "--test", target])
}

fn core_crates(root: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(root.join("architecture.toml")).map_err(|e| e.to_string())?;
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let crates = table
        .get("crates")
        .and_then(toml::Value::as_table)
        .ok_or("no [crates]")?;
    Ok(crates
        .iter()
        .filter(|(_, v)| v.get("core").and_then(toml::Value::as_bool) == Some(true))
        .map(|(k, _)| k.clone())
        .collect())
}

fn package_args(crates: &[String]) -> Vec<&str> {
    crates.iter().flat_map(|c| ["-p", c.as_str()]).collect()
}

fn tool_step(
    root: &Path,
    name: &str,
    program: &str,
    args: &[&str],
    install: &str,
) -> Result<String, String> {
    if !has_tool(root, &[program, "--version"])
        && !has_tool(
            root,
            &["cargo", program.trim_start_matches("cargo-"), "--version"],
        )
    {
        return Err(format!("{program} is not installed ({install})"));
    }
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]).current_dir(root);
    util::run_quiet(root, name, &mut cmd).map(|_| String::new())
}

pub fn quick() -> Result<String, String> {
    let started = Instant::now();
    let root = util::root()?;
    let mut steps = Steps::default();
    steps.run("fmt", || {
        cargo_step(&root, "fmt", &["fmt", "--all", "--check"])
    });
    steps.run("clippy", || clippy(&root));
    steps.run("arch-check", || arch::check(&root));
    steps.run("size-check", || size::check(&root));
    steps.run("docs-check", || docs::check(&root));
    steps.run("tests", || tests(&root));
    steps.run("replay", || replay::run(&root, &replay::Options::default()));
    finish(&root, "quick", steps, started)
}

pub fn job(name: &str) -> Result<String, String> {
    let root = util::root()?;
    let started = Instant::now();
    let mut steps = Steps::default();
    match name {
        "lint" => lint(&root, &mut steps),
        "test" => test(&root, &mut steps),
        "riscv" => riscv(&root, &mut steps)?,
        "wasm" => {
            let crates = core_crates(&root)?;
            let mut args = vec!["check", "--target", WASM];
            args.extend(package_args(&crates));
            steps.run("wasm-check", || cargo_step(&root, "wasm-check", &args));
        }
        "selftest" => selftest::steps(&root, &mut steps),
        "summary" => summary(&root, &mut steps),
        "all" => {
            lint(&root, &mut steps);
            test(&root, &mut steps);
            steps.run("wasm-check", || {
                let crates = core_crates(&root)?;
                let mut args = vec!["check", "--target", WASM];
                args.extend(package_args(&crates));
                cargo_step(&root, "wasm-check", &args)
            });
            selftest::steps(&root, &mut steps);
        }
        other => {
            return Err(format!(
                "unknown job {other:?}: lint test riscv wasm selftest summary all"
            ));
        }
    }
    finish(&root, name, steps, started)
}

fn lint(root: &Path, steps: &mut Steps) {
    steps.run("fmt", || {
        cargo_step(root, "fmt", &["fmt", "--all", "--check"])
    });
    steps.run("clippy", || clippy(root));
    steps.run("arch-check", || arch::check(root));
    steps.run("size-check", || size::check(root));
    steps.run("docs-check", || docs::check(root));
    steps.run("deny", || {
        tool_step(
            root,
            "deny",
            "cargo-deny",
            &["cargo", "deny", "check"],
            "cargo install --locked cargo-deny",
        )
    });
    steps.run("typos", || {
        tool_step(
            root,
            "typos",
            "typos",
            &["typos"],
            "cargo install --locked typos-cli",
        )
    });
}

fn test(root: &Path, steps: &mut Steps) {
    steps.run("tests", || tests(root));
    for (step, package, target) in HARNESSES {
        steps.run(step, || harness(root, step, package, target));
    }
    steps.run("replay", || replay::run(root, &replay::Options::default()));
}

/// The core set's tests and the testbed replays on RISC-V under QEMU. The
/// workflow sets the target's linker and `CARGO_TARGET_..._RUNNER`.
fn riscv(root: &Path, steps: &mut Steps) -> Result<(), String> {
    let runner = format!(
        "CARGO_TARGET_{}_RUNNER",
        RISCV.to_uppercase().replace('-', "_")
    );
    if std::env::var_os(&runner).is_none() {
        return Err(format!(
            "set {runner} (for example `qemu-riscv64 -L /usr/riscv64-linux-gnu`) and the linker; see .github/workflows/ci.yml"
        ));
    }
    let crates = core_crates(root)?;
    let mut args = vec!["test", "--target", RISCV];
    args.extend(package_args(&crates));
    steps.run("core-tests", || cargo_step(root, "riscv-core-tests", &args));
    steps.run("known-answers", || {
        cargo_step(
            root,
            "riscv-known-answers",
            &[
                "test",
                "--target",
                RISCV,
                "-p",
                "eng-math",
                "--test",
                "known_answers",
            ],
        )
    });
    steps.run("replay", || {
        replay::run(
            root,
            &replay::Options {
                target: Some(RISCV.into()),
                ..replay::Options::default()
            },
        )
    });
    Ok(())
}

/// The last PR-tier job: cross-architecture equality of the downloaded hash
/// streams, and the measured duration of the whole run.
fn summary(root: &Path, steps: &mut Steps) {
    let incoming = root.join("target/ci/in");
    steps.run("compare-hashes", || {
        let mut dirs: Vec<String> = fs::read_dir(&incoming)
            .map_err(|e| format!("{}: {e}", incoming.display()))?
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("hashes-"))
            .map(|e| stream_dir(&e.path()).to_string_lossy().into_owned())
            .collect();
        dirs.sort();
        if dirs.len() < 3 {
            return Err(format!(
                "[E-CROSS-ARCH] expected x64, arm64 and riscv hash streams, found {dirs:?}"
            ));
        }
        let refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
        replay::compare_dirs(root, &refs)
    });
    steps.run("pr-duration", || pr_duration(root));
}

/// An artifact holds `<triple>/*.txt`; use the directory with the streams.
fn stream_dir(artifact: &Path) -> std::path::PathBuf {
    util::walk(artifact)
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "txt"))
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| artifact.to_path_buf())
}

fn pr_duration(root: &Path) -> Result<String, String> {
    let (Ok(repo), Ok(run)) = (
        std::env::var("GITHUB_REPOSITORY"),
        std::env::var("GITHUB_RUN_ID"),
    ) else {
        return Err("not on GitHub Actions; the PR-tier duration is measured there".into());
    };
    let mut cmd = Command::new("gh");
    cmd.args([
        "api",
        &format!("repos/{repo}/actions/runs/{run}"),
        "--jq",
        ".run_started_at",
    ]);
    let started = util::run_quiet(root, "gh-run", &mut cmd)?;
    let mut date = Command::new("date");
    date.args(["-u", "-d", started.trim(), "+%s"]);
    let started_s: u64 = util::run_quiet(root, "date", &mut date)?
        .trim()
        .parse()
        .map_err(|e| format!("{e}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let minutes = now.saturating_sub(started_s) as f64 / 60.0;
    let budget = fs::read_to_string(root.join("budgets.toml"))
        .ok()
        .and_then(|t| t.parse::<toml::Table>().ok())
        .and_then(|t| t.get("ci")?.get("pr_tier_minutes")?.as_integer())
        .unwrap_or(15);
    let verdict = if minutes <= budget as f64 {
        "within"
    } else {
        "OVER"
    };
    Ok(format!(
        "PR tier took {minutes:.1} min ({verdict} the {budget} min budget)"
    ))
}

fn finish(root: &Path, job: &str, steps: Steps, started: Instant) -> Result<String, String> {
    let target = replay::host_triple(root).unwrap_or_else(|_| "unknown".into());
    let report = json!({
        "report_version": 1,
        "job": job,
        "target": target,
        "runner": std::env::var("RUNNER_NAME").unwrap_or_else(|_| "local".into()),
        "os": std::env::var("ImageOS").unwrap_or_else(|_| std::env::consts::OS.into()),
        "git_sha": std::env::var("GITHUB_SHA").unwrap_or_default(),
        "ok": steps.ok(),
        "seconds": started.elapsed().as_secs_f64(),
        "steps": steps.results.iter().map(|r| json!({
            "name": r.name, "ok": r.ok, "seconds": r.seconds,
            "detail": util::tail(&r.detail, 5),
        })).collect::<Vec<_>>(),
    });
    let dir = root.join("target/ci");
    let text = serde_json::to_string_pretty(&report).expect("report serializes");
    let _ = fs::create_dir_all(dir.join("reports"));
    let _ = fs::write(dir.join("report.json"), &text);
    let _ = fs::write(
        dir.join("reports").join(format!("{job}-{target}.json")),
        &text,
    );
    let failed: Vec<&str> = steps
        .results
        .iter()
        .filter(|r| !r.ok)
        .map(|r| r.name.as_str())
        .collect();
    if failed.is_empty() {
        Ok(format!(
            "{job}: all {} steps ok in {:.0}s",
            steps.results.len(),
            started.elapsed().as_secs_f64()
        ))
    } else {
        Err(format!("{job}: failed steps: {}", failed.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::HARNESSES;

    /// Every named step runs a test file that exists, under a name of its
    /// own, so renaming a harness fails here and not only on CI.
    #[test]
    fn every_harness_step_names_an_existing_test_file() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits in the repo root");
        for (step, package, target) in HARNESSES {
            let file = root
                .join("crates")
                .join(package)
                .join("tests")
                .join(format!("{target}.rs"));
            assert!(file.is_file(), "step {step}: no {}", file.display());
        }
        // With the job's other two steps.
        let mut names: Vec<&str> = HARNESSES.iter().map(|h| h.0).collect();
        names.extend(["tests", "replay"]);
        names.sort_unstable();
        let repeated: Vec<&str> = names
            .windows(2)
            .filter(|w| w[0] == w[1])
            .map(|w| w[0])
            .collect();
        assert!(repeated.is_empty(), "step names repeat: {repeated:?}");
    }
}
