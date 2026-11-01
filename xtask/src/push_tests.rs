//! Tests for `cargo xtask push` (ADR-0018): real pushes, to a local bare
//! repository that stands in for GitHub.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use super::push;

const CARGO_TOML: &str = "[workspace]\nmembers = []\n[workspace.dependencies]\nx = \"1\"\n[workspace.lints.clippy]\ntodo = \"warn\"\n";

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A bare repository standing in for GitHub, whose `main` is the first
/// commit of a clone, and that clone.
fn setup() -> (PathBuf, PathBuf) {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("xtask-push-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("work")).unwrap();
    let dir = fs::canonicalize(dir).unwrap();
    // Without Windows' `\\?\`: git would read such a path as a host name.
    let dir = PathBuf::from(dir.to_str().unwrap().trim_start_matches(r"\\?\"));
    let (hub, work) = (dir.join("hub.git"), dir.join("work"));
    // `-b`, because a clone checks out what HEAD names and git's default
    // branch depends on the machine: `main` from Xcode's git, `master` on CI.
    git(&dir, &["init", "-q", "--bare", "-b", "main", "hub.git"]);
    git(&work, &["init", "-q", "-b", "main"]);
    fs::write(work.join(".gitignore"), "target/\n").unwrap();
    fs::write(work.join("deny.toml"), "").unwrap();
    fs::write(work.join("Cargo.toml"), CARGO_TOML).unwrap();
    commit(&work, "notes.md", "a\n");
    git(&work, &["push", "-q", hub.to_str().unwrap(), "main"]);
    (hub, work)
}

/// Commits `text` as `file` and returns the commit.
fn commit(work: &Path, file: &str, text: &str) -> String {
    fs::write(work.join(file), text).unwrap();
    git(work, &["add", "-A"]);
    git(work, &["commit", "-q", "-m", file]);
    git(work, &["rev-parse", "HEAD"])
}

fn url(hub: &Path) -> &str {
    hub.to_str().unwrap()
}

/// The commit `branch` names in the stand-in for GitHub, if any.
fn at(hub: &Path, branch: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("--git-dir")
        .arg(hub)
        .args(["rev-parse", "--verify", "--quiet", branch])
        .output()
        .unwrap();
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[test]
fn sends_head_to_an_agent_branch() {
    let (hub, work) = setup();
    let head = commit(&work, "notes.md", "b\n");
    let said = push(&work, url(&hub), "t16/notes", false).unwrap();
    assert!(said.contains("agent/t16/notes"), "{said}");
    assert_eq!(at(&hub, "refs/heads/agent/t16/notes"), Some(head));
    // A detached HEAD, as jj leaves it, and a new branch.
    git(&work, &["checkout", "-q", "--detach"]);
    let head = commit(&work, "notes.md", "c\n");
    push(&work, url(&hub), "detached", false).unwrap();
    assert_eq!(at(&hub, "refs/heads/agent/detached"), Some(head));
    // A dry run sends nothing; main never moves.
    push(&work, url(&hub), "dry", true).unwrap();
    assert_eq!(at(&hub, "refs/heads/agent/dry"), None);
    assert_ne!(
        at(&hub, "refs/heads/main"),
        at(&hub, "refs/heads/agent/detached")
    );
}

#[test]
fn refuses_protected_changes() {
    let (hub, work) = setup();
    commit(&work, "deny.toml", "[bans]\n");
    let why = push(&work, url(&hub), "deny", false).unwrap_err();
    assert!(why.contains("deny.toml"), "{why}");
    assert_eq!(at(&hub, "refs/heads/agent/deny"), None);
    // Other tables of the root Cargo.toml are not protected; its lints are.
    commit(&work, "deny.toml", "");
    commit(&work, "Cargo.toml", &CARGO_TOML.replace("\"1\"", "\"2\""));
    push(&work, url(&hub), "deps", false).unwrap();
    commit(
        &work,
        "Cargo.toml",
        &CARGO_TOML.replace("\"warn\"", "\"allow\""),
    );
    let why = push(&work, url(&hub), "lints", false).unwrap_err();
    assert!(why.contains("Cargo.toml"), "{why}");
}

#[test]
fn compares_with_githubs_main_as_it_is_now() {
    let (hub, work) = setup();
    let old = git(&work, &["rev-parse", "HEAD"]);
    // The owner pushes a protected change to main.
    commit(&work, "deny.toml", "[bans]\n");
    git(&work, &["push", "-q", url(&hub), "main"]);
    // A commit from before it would take it back out.
    git(&work, &["checkout", "-q", &old]);
    let why = push(&work, url(&hub), "stale", false).unwrap_err();
    assert!(why.contains("deny.toml"), "{why}");
    git(&work, &["checkout", "-q", "main"]);
    push(&work, url(&hub), "fresh", false).unwrap();
}

#[test]
fn reads_main_past_git_settings_in_the_repository() {
    let (hub, work) = setup();
    // Another repository whose main already has the protected change.
    let evil = hub.with_file_name("evil.git");
    git(hub.parent().unwrap(), &["init", "-q", "--bare", "evil.git"]);
    commit(&work, "deny.toml", "[bans]\n");
    git(&work, &["push", "-q", url(&evil), "HEAD:refs/heads/main"]);
    // This repository's settings send the stand-in's URL there.
    let key = format!("url.{}.insteadOf", url(&evil));
    git(&work, &["config", &key, url(&hub)]);
    let why = push(&work, url(&hub), "evil", false).unwrap_err();
    assert!(why.contains("deny.toml"), "{why}");
}

#[test]
fn sends_the_checked_commit_not_a_ref_named_like_it() {
    let (hub, work) = setup();
    let head = commit(&work, "notes.md", "b\n");
    // A protected change, behind a branch, then a tag, named after HEAD's
    // id: git would send it in place of HEAD's commit without `^0`.
    git(&work, &["checkout", "-q", "-b", "side"]);
    let side = commit(&work, "deny.toml", "[bans]\n");
    git(&work, &["checkout", "-q", "main"]);
    git(&work, &["branch", &head, &side]);
    push(&work, url(&hub), "branch", false).unwrap();
    assert_eq!(at(&hub, "refs/heads/agent/branch"), Some(head.clone()));
    git(&work, &["branch", "-D", &head]);
    git(&work, &["tag", &head, &side]);
    push(&work, url(&hub), "tag", false).unwrap();
    assert_eq!(at(&hub, "refs/heads/agent/tag"), Some(head));
}

#[test]
fn fetches_githubs_main_when_it_is_missing() {
    let (hub, work) = setup();
    // The owner moves main from another clone, which this one has not seen.
    git(hub.parent().unwrap(), &["clone", "-q", url(&hub), "other"]);
    let other = hub.with_file_name("other");
    let moved = commit(&other, "notes.md", "from elsewhere\n");
    git(&other, &["push", "-q", "origin", "main"]);
    let has = |id: &str| {
        Command::new("git")
            .arg("-C")
            .arg(&work)
            .args(["cat-file", "-e", &format!("{id}^{{commit}}")])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    };
    assert!(!has(&moved));
    push(&work, url(&hub), "behind", false).unwrap();
    assert!(has(&moved));
}

#[test]
fn sends_no_tags_and_never_forces() {
    let (hub, work) = setup();
    commit(&work, "notes.md", "b\n");
    git(&work, &["tag", "-a", "-m", "t", "v1"]);
    git(&work, &["config", "push.followTags", "true"]);
    push(&work, url(&hub), "tags", false).unwrap();
    assert_eq!(at(&hub, "refs/tags/v1"), None);
    // A commit that does not descend from the branch is refused.
    git(&work, &["checkout", "-q", "--detach", "HEAD~1"]);
    commit(&work, "notes.md", "c\n");
    let why = push(&work, url(&hub), "tags", false).unwrap_err();
    assert!(why.contains("push"), "{why}");
}

#[test]
fn refuses_names_that_are_not_plain_branches() {
    let (hub, work) = setup();
    for name in [
        "", "/x", "x/", "a//b", "../main", "a/../b", ".x", "a/.x", "x.lock", "a b", "x:y", "x~1",
        "x^", "@{1}", "x*", "-x", "x\\y", "é",
    ] {
        let why = push(&work, url(&hub), name, false).unwrap_err();
        assert!(why.contains("[E-PUSH]"), "{name:?}: {why}");
    }
    assert_eq!(git(&hub, &["for-each-ref"]).lines().count(), 1, "only main");
}
