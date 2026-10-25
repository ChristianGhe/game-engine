//! Unit tests for the version stamp's source-tree values. None reads this
//! process's own environment: `cargo xtask` sets the four variables for
//! the tests too.

use std::fs;
use std::path::PathBuf;

use super::*;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// An environment holding `pairs` and nothing else.
fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.to_string())
    }
}

/// A git whose `rev-parse`, `status` and `log` print these, or fail on `None`.
fn fake_git(
    sha: Option<&str>,
    status: Option<&str>,
    time: Option<&str>,
) -> impl Fn(&[&str]) -> Option<String> + use<> {
    let (sha, status, time) = (
        sha.map(String::from),
        status.map(String::from),
        time.map(String::from),
    );
    move |args| match args {
        ["rev-parse", ..] => sha.clone(),
        [.., "status", "--porcelain", "--untracked-files=normal"] => status.clone(),
        ["log", ..] => time.clone(),
        other => panic!("unexpected git {other:?}"),
    }
}

fn owned(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    pairs.iter().map(|(k, v)| (*k, v.to_string())).collect()
}

#[test]
fn all_four_come_from_git_and_github() {
    let env = env_of(&[("GITHUB_RUN_ID", "18999"), ("GITHUB_RUN_ATTEMPT", "2")]);
    let git = fake_git(Some(&format!("{SHA}\n")), Some(""), Some("1761350400\n"));
    assert_eq!(
        compute(&env, &git),
        owned(&[
            ("ENG_ENGINE_SHA", SHA),
            ("ENG_DIRTY", "0"),
            ("SOURCE_DATE_EPOCH", "1761350400"),
            ("ENG_CI_RUN", "github-18999/2"),
        ])
    );
}

#[test]
fn values_already_set_win_as_they_are() {
    let preset = [
        ("ENG_ENGINE_SHA", "set-by-the-workflow"),
        ("ENG_DIRTY", "1"),
        ("SOURCE_DATE_EPOCH", "5"),
        ("ENG_CI_RUN", "docker-lavapipe"),
    ];
    let mut pairs = preset.to_vec();
    pairs.extend([("GITHUB_RUN_ID", "7"), ("GITHUB_RUN_ATTEMPT", "1")]);
    let git = |args: &[&str]| -> Option<String> {
        panic!("git {args:?} ran although every value was set")
    };
    // Kept even where `version.rs` would refuse them: the build says so.
    assert_eq!(compute(&env_of(&pairs), &git), owned(&preset));
    let odd = [("ENG_DIRTY", "yes")];
    let git = fake_git(Some(SHA), None, None);
    assert_eq!(
        compute(&env_of(&odd), &git),
        owned(&[
            ("ENG_ENGINE_SHA", SHA),
            ("ENG_DIRTY", "yes"),
            ("ENG_CI_RUN", "local"),
        ])
    );
}

#[test]
fn an_empty_value_counts_as_unset() {
    let env = env_of(&[
        ("ENG_ENGINE_SHA", ""),
        ("ENG_DIRTY", ""),
        ("SOURCE_DATE_EPOCH", ""),
        ("ENG_CI_RUN", ""),
        ("GITHUB_RUN_ID", ""),
        ("GITHUB_RUN_ATTEMPT", ""),
    ]);
    let git = fake_git(Some(SHA), Some(" M src/lib.rs\n"), Some("1"));
    assert_eq!(
        compute(&env, &git),
        owned(&[
            ("ENG_ENGINE_SHA", SHA),
            ("ENG_DIRTY", "1"),
            ("SOURCE_DATE_EPOCH", "1"),
            ("ENG_CI_RUN", "local"),
        ])
    );
}

#[test]
fn without_git_only_the_ci_run_is_set() {
    let git = |_: &[&str]| None;
    assert_eq!(
        compute(&env_of(&[]), &git),
        owned(&[("ENG_CI_RUN", "local")])
    );
}

#[test]
fn output_that_version_rs_would_refuse_is_dropped() {
    let (not_hex, too_long) = ("g".repeat(40), format!("{SHA}0"));
    let overflow = "9".repeat(21);
    for (sha, time) in [
        ("HEAD", "-1"),
        ("0123abc", "1761350400.5"),
        (not_hex.as_str(), overflow.as_str()),
        (too_long.as_str(), "17613504OO"),
        ("", ""),
    ] {
        let git = fake_git(Some(sha), Some(""), Some(time));
        assert_eq!(
            compute(&env_of(&[]), &git),
            owned(&[("ENG_DIRTY", "0"), ("ENG_CI_RUN", "local")]),
            "{sha:?} {time:?}"
        );
    }
    // A SHA-256 repository's commits are 64 hex digits.
    let sha256 = "AB".repeat(32);
    let git = fake_git(Some(&sha256), None, None);
    assert_eq!(
        compute(&env_of(&[]), &git)[0],
        ("ENG_ENGINE_SHA", sha256.to_ascii_lowercase())
    );
}

#[test]
fn a_github_run_needs_its_id_and_attempt() {
    for pairs in [
        &[("GITHUB_RUN_ID", "7")][..],
        &[("GITHUB_RUN_ATTEMPT", "1")],
        &[("GITHUB_RUN_ID", "7"), ("GITHUB_RUN_ATTEMPT", "")],
    ] {
        assert_eq!(ci_run(&env_of(pairs)), "local", "{pairs:?}");
    }
}

/// A fresh directory per call: tests run in parallel.
fn temp(what: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("xtask-{what}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(dir).unwrap()
}

fn git_ok(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(root)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_DATE", "1761350400 +0000")
        .env("GIT_COMMITTER_DATE", "1761350400 +0000")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {err}");
}

#[test]
fn a_real_repository() {
    let root = temp("stamp");
    let real = |args: &[&str]| git(&root, args);
    git_ok(&root, &["init", "-q"]);
    // No commit yet: only the dirty flag and the CI run.
    fs::write(root.join("a.txt"), "a\n").unwrap();
    assert_eq!(
        compute(&env_of(&[]), &real),
        owned(&[("ENG_DIRTY", "1"), ("ENG_CI_RUN", "local")])
    );
    git_ok(&root, &["add", "-A"]);
    git_ok(&root, &["commit", "-qm", "start"]);
    let head = git(&root, &["rev-parse", "HEAD"]).unwrap();
    let clean = owned(&[
        ("ENG_ENGINE_SHA", head.trim()),
        ("ENG_DIRTY", "0"),
        ("SOURCE_DATE_EPOCH", "1761350400"),
        ("ENG_CI_RUN", "local"),
    ]);
    assert_eq!(compute(&env_of(&[]), &real), clean);
    let mut dirty = clean.clone();
    dirty[1].1 = "1".into();
    // An untracked file, then an edit to a tracked one.
    fs::write(root.join("new.txt"), "new\n").unwrap();
    assert_eq!(compute(&env_of(&[]), &real), dirty);
    fs::remove_file(root.join("new.txt")).unwrap();
    fs::write(root.join("a.txt"), "b\n").unwrap();
    assert_eq!(compute(&env_of(&[]), &real), dirty);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn every_cargo_command_carries_them() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace root");
    let cmd = crate::util::cargo(root);
    let envs: Vec<(String, String)> = cmd
        .get_envs()
        .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
        .collect();
    let values = vars(root);
    // The CI run always has a value.
    assert!(values.iter().any(|(name, _)| *name == "ENG_CI_RUN"));
    for (name, value) in values {
        assert!(NAMES.contains(name));
        assert!(
            envs.contains(&(name.to_string(), value.clone())),
            "{name} is not set on cargo: {envs:?}"
        );
    }
}
