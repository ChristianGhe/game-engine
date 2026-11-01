//! `cargo xtask push <name>`: how agents push (ADR-0018). It sends the
//! commit at `HEAD` to `refs/heads/agent/<name>` on GitHub, without force,
//! tags or submodules, and only when that commit's protected files are the
//! same as on GitHub's `main`. Agents may not run `git push` itself
//! (`settings.json` and the guard); `main` and tags stay the owner's.
//!
//! - GitHub's `main` is read with `git ls-remote` outside the repository and
//!   without git's settings, so no `insteadOf` can answer for it.
//! - Every git call starts from an empty environment plus `KEPT`, so a
//!   `GIT_*` variable set on the cargo command never reaches git.
//! - What is pushed is the commit that was checked: `<id>^0`, which git
//!   cannot take for the name of a branch or tag.
//!
//! Like the guard, this stops accidents and the obvious forms. `PATH` and
//! `HOME` still come from the caller, and an agent running as the owner has
//! other ways to reach GitHub (ADR-0015).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::guard::{Protected, protected, protected_tables};
use crate::util;

/// Where agents push. A fork changes this line (`xtask/` is protected).
const REPOSITORY: &str = "https://github.com/ChristianGhe/game-engine.git";
const USAGE: &str = "usage: cargo xtask push [--dry-run] <name>    sends HEAD to agent/<name>";
/// The variables git calls keep: where programs are, the owner's settings
/// and credentials (`HOME`), the locale, temporary files, an SSH agent, and
/// what Windows needs to start a program.
const KEPT: [&str; 18] = [
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TMPDIR",
    "SSH_AUTH_SOCK",
    "SYSTEMROOT",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "TEMP",
    "TMP",
    "HOMEDRIVE",
    "HOMEPATH",
    "PATHEXT",
];

pub fn main(args: &[&str]) -> Result<String, String> {
    let (name, dry_run) = match args {
        [name] if !name.starts_with('-') => (*name, false),
        ["--dry-run", name] => (*name, true),
        _ => return Err(USAGE.into()),
    };
    push(&util::root()?, REPOSITORY, name, dry_run)
}

/// Pushes `HEAD` of the repository at `root` to `agent/<name>` at `url`.
pub(crate) fn push(root: &Path, url: &str, name: &str, dry_run: bool) -> Result<String, String> {
    if !branch_name(name) {
        return Err(format!(
            "[E-PUSH] agent/{name}: use letters, digits, '-', '_' and '.', in parts split by '/'"
        ));
    }
    let rules = protected()?;
    let commit = git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let main = github_main(root, url)?;
    if let Some(what) = protected_change(root, &rules, &main, &commit)? {
        return Err(format!(
            "[E-PUSH] {} changes protected {what} compared with GitHub's main ({}); protected changes reach GitHub through the owner (ADR-0018)",
            short(&commit),
            short(&main)
        ));
    }
    let refspec = format!("{commit}^0:refs/heads/agent/{name}");
    let mut cmd = git_command();
    cmd.arg("-C").arg(root);
    cmd.args(["push", "--no-follow-tags", "--recurse-submodules=no"]);
    if dry_run {
        cmd.arg("--dry-run");
    }
    cmd.args([url, refspec.as_str()]);
    util::run_quiet(root, "push", &mut cmd)?;
    let dirty = git(root, &["status", "--porcelain", "--untracked-files=no"])?;
    let note = if dirty.is_empty() {
        ""
    } else {
        "\nnote: the working tree has uncommitted changes; only HEAD was sent"
    };
    let verb = if dry_run { "would push" } else { "pushed" };
    Ok(format!(
        "{verb} {} to agent/{name}; CI: gh run list --branch agent/{name}{note}",
        short(&commit)
    ))
}

/// GitHub's `main`, read without any git settings, and fetched into this
/// repository if it does not have that commit yet.
fn github_main(root: &Path, url: &str) -> Result<String, String> {
    // Outside any repository: the file system's root has no `.git`.
    let outside = if cfg!(windows) {
        std::env::temp_dir()
    } else {
        PathBuf::from("/")
    };
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let out = git_command()
        .current_dir(&outside)
        .env("GIT_CEILING_DIRECTORIES", &outside)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null)
        .args(["ls-remote", url, "refs/heads/main"])
        .output()
        .map_err(|e| format!("git ls-remote: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let main = text
        .lines()
        .find_map(|line| line.strip_suffix("\trefs/heads/main"))
        .filter(|id| id.len() >= 40 && id.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| {
            format!(
                "[E-PUSH] cannot read main at {url}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )
        })?
        .to_string();
    let here = |id: &str| git(root, &["cat-file", "-e", &format!("{id}^{{commit}}")]).is_ok();
    if !here(&main) {
        git(
            root,
            &["fetch", "--quiet", "--no-tags", url, "refs/heads/main"],
        )?;
        if !here(&main) {
            return Err(format!(
                "[E-PUSH] fetching {url} did not bring its main ({})",
                short(&main)
            ));
        }
    }
    Ok(main)
}

/// The first protected path, or protected table of the root `Cargo.toml`,
/// that differs between the trees of `main` and `commit`. A `Cargo.toml`
/// that changed and cannot be read counts as a change.
fn protected_change(
    root: &Path,
    rules: &Protected,
    main: &str,
    commit: &str,
) -> Result<Option<String>, String> {
    let diff = [
        "diff",
        "--name-only",
        "-z",
        "--no-renames",
        "--no-relative",
        "--no-ext-diff",
        main,
        commit,
        "--",
    ];
    let names = git(root, &diff)?;
    let tables = |rev: &str| {
        git(root, &["cat-file", "blob", &format!("{rev}:Cargo.toml")])
            .ok()
            .and_then(|text| protected_tables(rules, &text))
    };
    for name in names.split('\0').filter(|n| !n.is_empty()) {
        if rules.matches(name) {
            return Ok(Some(name.to_string()));
        }
        if name == "Cargo.toml" {
            match (tables(main), tables(commit)) {
                (Some(before), Some(after)) if before == after => {}
                _ => return Ok(Some("tables of Cargo.toml".into())),
            }
        }
    }
    Ok(None)
}

/// A name git takes as a plain branch under `agent/`.
fn branch_name(name: &str) -> bool {
    name.len() <= 100
        && !name.contains("..")
        && name.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with(['.', '-'])
                && !part.ends_with(".lock")
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
}

/// `git` with an empty environment plus `KEPT`, and no prompts.
fn git_command() -> Command {
    let mut cmd = Command::new("git");
    cmd.env_clear();
    for (key, value) in std::env::vars_os() {
        let upper = key.to_string_lossy().to_ascii_uppercase();
        if KEPT.contains(&upper.as_str()) {
            cmd.env(key, value);
        }
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

/// `git -C root` without replace refs, so a commit's id names its content.
fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_command()
        .arg("-C")
        .arg(root)
        .args(["-c", "core.useReplaceRefs=false"])
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "[E-PUSH] git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

fn short(id: &str) -> &str {
    &id[..id.len().min(12)]
}

#[cfg(test)]
#[path = "push_tests.rs"]
mod tests;
