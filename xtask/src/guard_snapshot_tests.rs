//! Unit tests for the guard's second line.

use std::process::Command;

use super::*;
use serde_json::json;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {err}");
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

/// A git repository whose `main` approves `deny.toml`; `old` predates that.
fn repo() -> PathBuf {
    let root = temp("snapshot");
    fs::create_dir_all(root.join(".claude")).unwrap();
    fs::create_dir_all(root.join(".github/workflows")).unwrap();
    fs::write(root.join("architecture.toml"), "").unwrap();
    fs::write(root.join("deny.toml"), "first\n").unwrap();
    fs::write(root.join(".github/workflows/ci.yml"), "ci\n").unwrap();
    fs::write(root.join("notes.txt"), "x\n").unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "start"]);
    git(&root, &["branch", "old"]);
    fs::write(root.join("deny.toml"), "approved\n").unwrap();
    git(&root, &["commit", "-qam", "approve"]);
    root
}

fn call(id: &str) -> Value {
    json!({ "tool_name": "Bash", "tool_use_id": id, "tool_input": { "command": "x" } })
}

fn recorded(root: &Path, id: &str) {
    let rules = protected().unwrap();
    let ctx = Ctx {
        rules: &rules,
        root,
        cwd: root,
        unlocked: &[],
    };
    record(&ctx, &call(id)).unwrap();
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).unwrap()
}

/// The one quarantine folder a restore made.
fn quarantine(root: &Path) -> PathBuf {
    let kept: Vec<_> = fs::read_dir(root.join("target/guard-quarantine"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(kept.len(), 1);
    kept[0].path()
}

#[test]
fn puts_back_what_a_call_changed() {
    let root = repo();
    recorded(&root, "toolu_a");
    fs::write(root.join("deny.toml"), "changed\n").unwrap();
    fs::write(root.join(UNLOCK_FILE), "*\n").unwrap();
    fs::write(root.join("budgets.toml"), "new\n").unwrap();
    fs::write(root.join("notes.txt"), "ordinary edit\n").unwrap();
    let report = restore(&root, &call("toolu_a")).unwrap().expect("a report");
    for path in ["deny.toml", UNLOCK_FILE, "budgets.toml"] {
        assert!(report.contains(path), "{report}");
    }
    assert!(!report.contains("notes.txt"), "{report}");
    assert_eq!(read(root.join("deny.toml")), "approved\n");
    assert!(!root.join(UNLOCK_FILE).exists());
    assert!(!root.join("budgets.toml").exists());
    assert_eq!(read(root.join("notes.txt")), "ordinary edit\n");
    assert_eq!(read(quarantine(&root).join("deny.toml")), "changed\n");
    // The snapshot is used once.
    assert_eq!(restore(&root, &call("toolu_a")).unwrap(), None);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn puts_back_a_deleted_file_and_a_hook() {
    let root = repo();
    fs::create_dir_all(root.join(".git/hooks")).unwrap();
    recorded(&root, "toolu_b");
    fs::remove_file(root.join("deny.toml")).unwrap();
    fs::write(root.join(".git/hooks/pre-commit"), "#!/bin/sh\n").unwrap();
    let report = restore(&root, &call("toolu_b")).unwrap().expect("a report");
    assert!(report.contains(".git/hooks/pre-commit"), "{report}");
    assert_eq!(read(root.join("deny.toml")), "approved\n");
    assert!(!root.join(".git/hooks/pre-commit").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mains_version_is_approved_and_what_it_replaced_is_kept() {
    let root = repo();
    git(&root, &["checkout", "-q", "old"]);
    fs::write(root.join("deny.toml"), "owner draft\n").unwrap();
    recorded(&root, "toolu_c");
    // As `git checkout -f main` would: deny.toml becomes main's version.
    fs::write(root.join("deny.toml"), "approved\n").unwrap();
    assert_eq!(restore(&root, &call("toolu_c")).unwrap(), None);
    assert_eq!(read(root.join("deny.toml")), "approved\n");
    let replaced = quarantine(&root).join("replaced/deny.toml");
    assert_eq!(read(replaced), "owner draft\n");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn main_moved_by_the_call_approves_nothing() {
    let root = repo();
    recorded(&root, "toolu_e");
    // As `<a write the first line missed> && git commit -am x` would.
    fs::write(root.join("deny.toml"), "sneaked in\n").unwrap();
    git(&root, &["commit", "-qam", "x"]);
    let report = restore(&root, &call("toolu_e")).unwrap().expect("a report");
    assert!(report.contains("deny.toml"), "{report}");
    assert_eq!(read(root.join("deny.toml")), "approved\n");
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn never_writes_through_a_link() {
    use std::os::unix::fs::symlink;
    let root = repo();
    let outside = temp("snapshot-outside");
    fs::write(outside.join("rc"), "outside\n").unwrap();
    fs::write(outside.join("ci.yml"), "outside ci\n").unwrap();
    recorded(&root, "toolu_f");
    fs::remove_file(root.join("deny.toml")).unwrap();
    symlink(outside.join("rc"), root.join("deny.toml")).unwrap();
    fs::remove_dir_all(root.join(".github")).unwrap();
    symlink(&outside, root.join(".github")).unwrap();
    symlink(outside.join("rc"), root.join(UNLOCK_FILE)).unwrap();
    let report = restore(&root, &call("toolu_f")).unwrap().expect("a report");
    assert!(report.contains(".github/workflows/ci.yml"), "{report}");
    assert_eq!(read(outside.join("rc")), "outside\n");
    assert_eq!(read(outside.join("ci.yml")), "outside ci\n");
    assert!(
        fs::symlink_metadata(root.join("deny.toml"))
            .unwrap()
            .is_file()
    );
    assert_eq!(read(root.join("deny.toml")), "approved\n");
    assert!(fs::symlink_metadata(root.join(".github")).unwrap().is_dir());
    assert_eq!(read(root.join(".github/workflows/ci.yml")), "ci\n");
    assert!(fs::symlink_metadata(root.join(UNLOCK_FILE)).is_err());
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn local_settings_keep_permission_rules_only() {
    let root = repo();
    let settings = root.join(LOCAL_SETTINGS);
    recorded(&root, "toolu_g");
    // As Claude Code writes when the owner says "don't ask again".
    fs::write(&settings, r#"{"permissions":{"allow":["Bash(ls:*)"]}}"#).unwrap();
    assert_eq!(restore(&root, &call("toolu_g")).unwrap(), None);
    assert!(settings.exists());
    recorded(&root, "toolu_h");
    fs::write(
        &settings,
        r#"{"permissions":{"allow":["Bash(ls:*)"]},"disableAllHooks":true}"#,
    )
    .unwrap();
    let report = restore(&root, &call("toolu_h")).unwrap().expect("a report");
    assert!(report.contains(LOCAL_SETTINGS), "{report}");
    assert_eq!(
        read(&settings),
        r#"{"permissions":{"allow":["Bash(ls:*)"]}}"#
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn nothing_is_recorded_while_unlocked_or_without_an_id() {
    let root = repo();
    fs::write(root.join(UNLOCK_FILE), "deny.toml\n").unwrap();
    recorded(&root, "toolu_d");
    fs::write(root.join("deny.toml"), "owner edit\n").unwrap();
    assert_eq!(restore(&root, &call("toolu_d")).unwrap(), None);
    assert_eq!(read(root.join("deny.toml")), "owner edit\n");
    let no_id = json!({ "tool_name": "Bash", "tool_input": {} });
    assert_eq!(restore(&root, &no_id).unwrap(), None);
    let _ = fs::remove_dir_all(&root);
}
