//! `cargo xtask quick` and `cargo xtask ci --job <job>` (spec 8.2).
//!
//! Each job prints one line per step and writes `target/ci/report.json`
//! (plus a copy under `target/ci/reports/`), which `acceptance` reads.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::util::{self, Steps};
use crate::{arch, ci_durations, ci_native, ci_report, ci_verdicts, docs, replay, selftest, size};

/// The jobs `cargo xtask ci --job` runs, as its error and the usage text
/// (`main.rs`) list them.
pub const JOBS: [&str; 8] = [
    "lint", "test", "riscv", "wasm", "selftest", "native", "summary", "all",
];

pub const RISCV: &str = "riscv64gc-unknown-linux-gnu";
pub const WASM: &str = "wasm32-unknown-unknown";

/// `cargo <args>` in `root`, with `--locked` on CI.
pub(crate) fn cargo_command(root: &Path, args: &[&str]) -> Command {
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
    cmd
}

pub(crate) fn cargo_step(root: &Path, name: &str, args: &[&str]) -> Result<String, String> {
    cargo_step_env(root, name, args, &[])
}

/// [`cargo_step`] with `env` added to cargo's environment.
pub(crate) fn cargo_step_env(
    root: &Path,
    name: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<String, String> {
    let mut cmd = cargo_command(root, args);
    cmd.envs(env.iter().copied());
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
    tests_env(root, &[])
}

/// Every test and doctest, with `env` added (the native job requires a GPU
/// and a display).
pub(crate) fn tests_env(root: &Path, env: &[(&str, &str)]) -> Result<String, String> {
    let cargo_step = |root: &Path, name: &str, args: &[&str]| cargo_step_env(root, name, args, env);
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
/// the test files it runs, each as a package and a test target
/// (`crates/<package>/tests/<target>.rs`). `tests` runs them as well; a
/// step of their own gives each its own entry in the job's report, where
/// acceptance checks find steps by name, and its own log. They build on the
/// dev profile: `crash_json` looks for the `eng_cli::run::run` frame, which
/// a release build without debug info can inline away. Like every cargo
/// command here, they get the stamp's four values from `util::cargo`
/// (`stamp.rs`), so `version` checks a stamp with all of them set.
const HARNESSES: [(&str, &[(&str, &str)]); 7] = [
    ("known-answers", &[("eng-math", "known_answers")]),
    ("exit-codes", &[("game-testbed", "exit_codes")]),
    ("crash-json", &[("game-testbed", "crash_json")]),
    ("version", &[("game-testbed", "version")]),
    ("live-input", &[("eng-input", "live_input")]),
    // Criterion 11: the wire format's golden vectors, decoders that never
    // panic, and a session that hashes the same as stepping the sim.
    (
        "wire-session",
        &[
            ("eng-net-wire", "golden"),
            ("eng-net-wire", "never_panic"),
            ("eng-session", "hash_equality"),
        ],
    ),
    // Criterion 10: every WGSL file validates and translates for each
    // backend.
    ("shaders", &[("eng-gpu", "shaders")]),
];

/// One step from `HARNESSES`, logged under the step's name: a single
/// `cargo test` that names each of its packages and test targets.
fn harness(root: &Path, step: &str, files: &[(&str, &str)]) -> Result<String, String> {
    let mut args = vec!["test"];
    for &(package, target) in files {
        args.extend(["-p", package, "--test", target]);
    }
    cargo_step(root, step, &args)
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
    ci_report::finish(&root, "quick", steps, started, serde_json::Map::new())
}

/// Whether `name` is one of [`JOBS`]: checked before `main` takes the run
/// lock, so a mistyped job never waits.
pub fn known_job(name: &str) -> Result<(), String> {
    if JOBS.contains(&name) {
        Ok(())
    } else {
        Err(format!("unknown job {name:?}: {}", JOBS.join(" ")))
    }
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
        "native" => {
            ci_native::steps(&root, &mut steps);
            return ci_report::finish(&root, name, steps, started, ci_native::extra(&root));
        }
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
            return Err(format!("unknown job {other:?}: {}", JOBS.join(" ")));
        }
    }
    ci_report::finish(&root, name, steps, started, serde_json::Map::new())
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
    for (step, files) in HARNESSES {
        steps.run(step, || harness(root, step, files));
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

/// The last job: cross-architecture equality of the downloaded hash streams,
/// the PR tier's and the native jobs', each against the committed streams
/// (criterion 7), and the measured durations of the PR tier and of the
/// native jobs (criterion 13).
fn summary(root: &Path, steps: &mut Steps) {
    let incoming = root.join("target/ci/in");
    steps.run("compare-hashes", || {
        let mut found: Vec<(String, String)> = fs::read_dir(&incoming)
            .map_err(|e| format!("{}: {e}", incoming.display()))?
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .filter(|(name, _)| name.starts_with("hashes-"))
            .map(|(name, path)| (name, stream_dir(&path).to_string_lossy().into_owned()))
            .collect();
        found.sort();
        let names: Vec<String> = found.iter().map(|(name, _)| name.clone()).collect();
        let missing = ci_verdicts::missing_streams(&names);
        if !missing.is_empty() {
            return Err(format!(
                "[E-CROSS-ARCH] no hash streams from {}; found {names:?}",
                missing.join(", ")
            ));
        }
        // The committed streams first, so every runner is held to them.
        let mut dirs = vec![replay::COMMITTED.to_string()];
        dirs.extend(found.into_iter().map(|(_, dir)| dir));
        let refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
        replay::compare_dirs(root, &refs)
    });
    steps.run("pr-duration", || ci_durations::pr_tier(root));
    steps.run("native-duration", || ci_durations::native_tier(root));
}

/// An artifact holds `<triple>/*.txt`; use the directory with the streams.
fn stream_dir(artifact: &Path) -> std::path::PathBuf {
    util::walk(artifact)
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "txt"))
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| artifact.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::HARNESSES;

    /// Whether `package` has a test target named `target`: a file
    /// `tests/<target>.rs` or `tests/<target>/main.rs`, or a `[[test]]` of
    /// that name in its manifest.
    fn has_test_target(root: &Path, package: &str, target: &str) -> bool {
        let dir = root.join("crates").join(package);
        let tests = dir.join("tests");
        if tests.join(format!("{target}.rs")).is_file()
            || tests.join(target).join("main.rs").is_file()
        {
            return true;
        }
        let manifest: toml::Table = std::fs::read_to_string(dir.join("Cargo.toml"))
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or_default();
        manifest
            .get("test")
            .and_then(toml::Value::as_array)
            .is_some_and(|list| {
                list.iter()
                    .any(|t| t.get("name").and_then(toml::Value::as_str) == Some(target))
            })
    }

    /// Every named step runs test files that exist, under a name of its
    /// own, so renaming a harness fails here and not only on CI. A step's
    /// one `cargo test` runs a target in each package it names that has
    /// one, so a step's target names are each in one of its packages only.
    #[test]
    fn every_harness_step_names_existing_test_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits in the repo root");
        for (step, files) in HARNESSES {
            assert!(!files.is_empty(), "step {step} runs nothing");
            for &(package, target) in files {
                let path = root
                    .join("crates")
                    .join(package)
                    .join("tests")
                    .join(format!("{target}.rs"));
                assert!(path.is_file(), "step {step}: no {}", path.display());
                for &(other, _) in files.iter().filter(|f| f.0 != package) {
                    assert!(
                        !has_test_target(root, other, target),
                        "step {step} also runs {other}'s test target {target}"
                    );
                }
            }
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
