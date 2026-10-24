//! Claude Code hooks (spec 7.5) and `install-hooks`.
//!
//! `.claude/settings.json` runs these through a prebuilt copy of xtask built
//! from `main` and kept in `<git dir>/eng-hooks/`, so they work without
//! compiling and editing xtask in the working tree does not change the
//! running guard. An agent with the owner's OS user can still replace the
//! binary; CI is the gate (see `guard.rs`).

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use serde_json::Value;

use crate::{acceptance, util};

fn stdin_json() -> Value {
    let mut text = String::new();
    let _ = std::io::stdin().read_to_string(&mut text);
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

fn hook_root(input: &Value) -> Result<PathBuf, String> {
    match std::env::var_os("CLAUDE_PROJECT_DIR") {
        Some(dir) => util::find_root(Path::new(&dir)),
        None => util::find_root(Path::new(input["cwd"].as_str().unwrap_or("."))),
    }
}

/// `PostToolUse`: rustfmt the edited Rust file. Never blocks.
pub fn fmt() -> Result<String, String> {
    let input = stdin_json();
    let Some(path) = input["tool_input"]["file_path"].as_str() else {
        return Ok(String::new());
    };
    if !path.ends_with(".rs") || !Path::new(path).is_file() {
        return Ok(String::new());
    }
    if let Ok(root) = hook_root(&input) {
        let _ = Command::new("rustfmt")
            .current_dir(&root)
            .args(["--edition", "2024", path])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    Ok(String::new())
}

/// Size and mtime of every Rust source and manifest: cheap change detection.
fn fingerprint(root: &Path) -> String {
    let mut entries = BTreeMap::new();
    let mut files = util::walk(&root.join("crates"));
    files.extend(util::walk(&root.join("xtask")));
    files.extend(["Cargo.toml", "Cargo.lock"].map(|f| root.join(f)));
    for file in files {
        let name = file.to_string_lossy();
        if !(name.ends_with(".rs") || name.ends_with(".toml") || name.ends_with(".lock")) {
            continue;
        }
        if let Ok(meta) = fs::metadata(&file) {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            entries.insert(util::rel(root, &file), (meta.len(), mtime));
        }
    }
    format!("{entries:?}")
}

/// `Stop`: run `quick` when Rust files changed since the last green run.
/// Blocks the stop once (exit 2) on failure; honours `stop_hook_active`.
pub fn stop() -> ExitCode {
    let input = stdin_json();
    if input["stop_hook_active"] == true {
        return ExitCode::SUCCESS;
    }
    let Ok(root) = hook_root(&input) else {
        return ExitCode::SUCCESS;
    };
    let stamp = root.join("target/xtask/last-green");
    let now = fingerprint(&root);
    if fs::read_to_string(&stamp).is_ok_and(|s| s == now) {
        return ExitCode::SUCCESS;
    }
    // The working tree's own `quick`, not this prebuilt copy's: a branch may
    // have changed the check tooling, and only the guard must come from main.
    let mut cmd = util::cargo(&root);
    cmd.args(["xtask", "quick"]);
    match util::run_quiet(&root, "stop-quick", &mut cmd) {
        Ok(_) => {
            let _ = fs::create_dir_all(stamp.parent().expect("has parent"));
            let _ = fs::write(&stamp, fingerprint(&root));
            ExitCode::SUCCESS
        }
        Err(summary) => {
            eprintln!(
                "`cargo xtask quick` failed after your changes:\n{summary}\n\
                 Fix it, or say in your summary why it is red. Green is required at push."
            );
            ExitCode::from(2)
        }
    }
}

/// `SessionStart`: the rung, its acceptance status and the last CI result.
pub fn session_start() -> Result<String, String> {
    let input = stdin_json();
    let root = hook_root(&input)?;
    let rung = acceptance::current_rung(&root).unwrap_or_else(|_| "unknown".into());
    let mut lines = vec![format!(
        "Engine repo. Current rung: {rung} (docs/rungs/{rung}/). Read AGENTS.md first."
    )];
    let computed = fs::read_to_string(root.join("target/ci/acceptance.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    match computed {
        Some(doc) => {
            let features = doc["features"].as_array().cloned().unwrap_or_default();
            let failing: Vec<String> = features
                .iter()
                .filter(|f| f["passes"] != true)
                .map(|f| f["id"].as_str().unwrap_or("?").to_string())
                .collect();
            lines.push(format!(
                "Acceptance (last computed locally): {}/{} pass{}",
                features.len() - failing.len(),
                features.len(),
                if failing.is_empty() {
                    String::new()
                } else {
                    format!("; failing: {}", failing.join(", "))
                }
            ));
        }
        None => {
            lines.push("Acceptance: not computed locally; CI's summary job computes it.".into())
        }
    }
    if let Ok(text) = fs::read_to_string(root.join("target/ci/report.json"))
        && let Ok(report) = serde_json::from_str::<Value>(&text)
    {
        lines.push(format!(
            "Last local CI job: {} on {}: {}",
            report["job"].as_str().unwrap_or("?"),
            report["target"].as_str().unwrap_or("?"),
            if report["ok"] == true { "green" } else { "RED" }
        ));
    }
    lines.push(
        "Before pushing: `cargo xtask quick`. Protected paths need the owner (AGENTS.md).".into(),
    );
    Ok(lines.join("\n"))
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(root).args(args);
    util::run_quiet(root, "git", &mut cmd).map(|s| s.trim().to_string())
}

/// Builds xtask from `origin/main` (or, with no remote yet, the local
/// `main`), never the working tree, into `<git dir>/eng-hooks/`.
///
/// Remote `main` is guarded by branch protection; local refs are not, and an
/// agent running as the owner's OS user could still swap the binary itself.
/// A separate OS user for agents (spec P7) closes that gap.
pub fn install(if_stale: bool) -> Result<String, String> {
    let root = util::root()?;
    let common = PathBuf::from(git(
        &root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let dest = common.join("eng-hooks");
    let exe = format!("xtask{}", std::env::consts::EXE_SUFFIX);
    let has_remote = git(&root, &["remote", "get-url", "origin"]).is_ok();
    if has_remote {
        // Best effort: offline sessions use the last fetched origin/main.
        let _ = git(&root, &["fetch", "--quiet", "origin", "main"]);
    }
    let main = if has_remote {
        "refs/remotes/origin/main^{commit}"
    } else {
        "main^{commit}"
    };
    let Ok(sha) = git(&root, &["rev-parse", "--verify", main]) else {
        if has_remote {
            return Err("origin/main is not fetched yet; run `git fetch origin main`".into());
        }
        // Bootstrap: before the first commit there is no main to build from.
        // Install from the working tree so the guard exists (it fails closed
        // without a binary); the first run after main exists replaces it.
        if if_stale && dest.join(&exe).is_file() {
            return Ok("hooks: no main yet; keeping the bootstrap binary".into());
        }
        return build_and_copy(&root, &root, &dest, &exe, "working-tree (no main yet)");
    };
    if if_stale
        && dest.join(&exe).is_file()
        && fs::read_to_string(dest.join("SOURCE")).is_ok_and(|s| s.trim() == sha)
    {
        return Ok(format!("hooks up to date (main@{})", &sha[..12]));
    }
    // Always extract afresh: a cached tree could have been edited since.
    let src = root.join("target/hooks-src").join(&sha);
    {
        let _ = fs::remove_dir_all(&src);
        fs::create_dir_all(&src).map_err(|e| e.to_string())?;
        let archive = Command::new("git")
            .current_dir(&root)
            .args(["archive", "--format=tar", &sha])
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("git archive: {e}"))?;
        let status = Command::new("tar")
            .arg("-x")
            .arg("-C")
            .arg(&src)
            .stdin(archive.stdout.expect("piped"))
            .status()
            .map_err(|e| format!("tar: {e}"))?;
        if !status.success() {
            return Err(format!("extracting main@{sha} failed"));
        }
    }
    build_and_copy(&root, &src, &dest, &exe, &sha)
}

fn build_and_copy(
    root: &Path,
    src: &Path,
    dest: &Path,
    exe: &str,
    source: &str,
) -> Result<String, String> {
    // A fresh target directory too: cached build artifacts are not trusted.
    let _ = fs::remove_dir_all(root.join("target/hooks-build"));
    let mut build = util::cargo(src);
    build
        .args([
            "build",
            "--release",
            "--locked",
            "-p",
            "xtask",
            "--target-dir",
        ])
        .arg(root.join("target/hooks-build"));
    util::run_quiet(root, "install-hooks", &mut build)?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    fs::copy(
        root.join("target/hooks-build/release").join(exe),
        dest.join(exe),
    )
    .map_err(|e| format!("copying the hook binary: {e}"))?;
    fs::write(dest.join("SOURCE"), source).map_err(|e| e.to_string())?;
    Ok(format!(
        "installed hooks from {source} into {}",
        dest.display()
    ))
}
