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

fn tests(root: &Path, filter: Option<(&str, &str)>) -> Result<String, String> {
    let nextest = has_tool(root, &["cargo", "nextest", "--version"]);
    let mut args: Vec<&str> = match (nextest, filter) {
        (true, None) => vec!["nextest", "run", "--workspace", "--no-fail-fast"],
        (false, None) => vec!["test", "--workspace", "--no-fail-fast"],
        (_, Some((package, test))) => vec!["test", "-p", package, "--test", test],
    };
    if nextest && filter.is_none() && util::on_ci() {
        args.extend(["--profile", "ci"]);
    }
    cargo_step(root, "tests", &args)?;
    if filter.is_none() && nextest {
        cargo_step(root, "doctests", &["test", "--workspace", "--doc"])?;
    }
    Ok(if nextest || filter.is_some() {
        String::new()
    } else {
        "cargo-nextest not installed; ran cargo test".into()
    })
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
    steps.run("tests", || tests(&root, None));
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
    steps.run("tests", || tests(root, None));
    steps.run("known-answers", || {
        tests(root, Some(("eng-math", "known_answers")))
    });
    steps.run("exit-codes", || {
        tests(root, Some(("game-testbed", "exit_codes")))
    });
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
