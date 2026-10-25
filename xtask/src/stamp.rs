//! The version stamp's source-tree values (spec 6.13, rung 1b patch X1).
//!
//! `crates/eng-cli/src/version.rs` reads four variables at compile time,
//! and only xtask knows their values, so every cargo command it runs sets
//! them ([`crate::util::cargo`]):
//!
//! | Variable | Value |
//! |---|---|
//! | `ENG_ENGINE_SHA` | `git rev-parse HEAD` |
//! | `ENG_DIRTY` | `1` if `git status --porcelain` lists anything, untracked files included, else `0` |
//! | `SOURCE_DATE_EPOCH` | `HEAD`'s committer time, in Unix seconds |
//! | `ENG_CI_RUN` | `github-<GITHUB_RUN_ID>/<GITHUB_RUN_ATTEMPT>` on GitHub Actions, else `local` |
//!
//! - A non-empty value already in the environment wins, as it is: CI's
//!   workflow sets `ENG_ENGINE_SHA`, and a container can pass in what git
//!   cannot see. An empty one counts as unset, as it does for `version.rs`.
//! - A value git cannot give (no git, no repository, no commit) stays
//!   unset, so the stamp says `unknown` or `null` for it instead of the
//!   build failing.
//! - The values are worked out once per process. Every build in one xtask
//!   run gets the same ones, so its steps do not rebuild `eng-cli` for
//!   each other, and the binaries they leave behind agree.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// The variable names, in `version.rs`'s order.
pub const NAMES: [&str; 4] = [
    "ENG_ENGINE_SHA",
    "ENG_DIRTY",
    "SOURCE_DATE_EPOCH",
    "ENG_CI_RUN",
];

/// The values for every cargo command this process runs. The first call
/// reads the source tree at `root`; scratch workspaces under `target/`
/// belong to the same tree.
pub fn vars(root: &Path) -> &'static [(&'static str, String)] {
    static VARS: OnceLock<Vec<(&'static str, String)>> = OnceLock::new();
    VARS.get_or_init(|| compute(&|name| std::env::var(name).ok(), &|args| git(root, args)))
}

/// The values from `env` (a variable's value) and `git` (a git command's
/// stdout, or `None` if it failed), in [`NAMES`] order. A variable with no
/// value is left out.
fn compute(
    env: &dyn Fn(&str) -> Option<String>,
    git: &dyn Fn(&[&str]) -> Option<String>,
) -> Vec<(&'static str, String)> {
    let derive = |name: &str| match name {
        "ENG_ENGINE_SHA" => commit(git),
        "ENG_DIRTY" => dirty(git),
        "SOURCE_DATE_EPOCH" => commit_time(git),
        _ => Some(ci_run(env)),
    };
    NAMES
        .into_iter()
        .filter_map(|name| {
            let value = set(env, name).or_else(|| derive(name))?;
            Some((name, value))
        })
        .collect()
}

/// `name`'s value, if it is set and not empty.
fn set(env: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    env(name).filter(|value| !value.is_empty())
}

/// `HEAD`'s full commit hash: SHA-1 or SHA-256, in hex.
fn commit(git: &dyn Fn(&[&str]) -> Option<String>) -> Option<String> {
    let out = git(&["rev-parse", "--verify", "HEAD"])?;
    let sha = out.trim();
    let hex = sha.bytes().all(|b| b.is_ascii_hexdigit());
    (hex && matches!(sha.len(), 40 | 64)).then(|| sha.to_ascii_lowercase())
}

/// `0` or `1`. `--no-optional-locks` keeps a status check from taking the
/// index lock that a git or jj command running alongside may need.
fn dirty(git: &dyn Fn(&[&str]) -> Option<String>) -> Option<String> {
    let out = git(&[
        "--no-optional-locks",
        "status",
        "--porcelain",
        "--untracked-files=normal",
    ])?;
    Some(if out.trim().is_empty() { "0" } else { "1" }.into())
}

/// `HEAD`'s commit time in whole Unix seconds. `version.rs` refuses
/// anything but digits, so other output leaves the value unset.
fn commit_time(git: &dyn Fn(&[&str]) -> Option<String>) -> Option<String> {
    let out = git(&["log", "-1", "--no-show-signature", "--format=%ct", "HEAD"])?;
    let seconds = out.trim();
    let digits = !seconds.is_empty() && seconds.bytes().all(|b| b.is_ascii_digit());
    (digits && seconds.parse::<u64>().is_ok()).then(|| seconds.to_string())
}

fn ci_run(env: &dyn Fn(&str) -> Option<String>) -> String {
    match (set(env, "GITHUB_RUN_ID"), set(env, "GITHUB_RUN_ATTEMPT")) {
        (Some(id), Some(attempt)) => format!("github-{id}/{attempt}"),
        _ => "local".into(),
    }
}

/// Runs git in `root` and returns its stdout if it succeeded.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
#[path = "stamp_tests.rs"]
mod tests;
