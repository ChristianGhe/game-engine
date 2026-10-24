//! Unit tests for the guard; the end-to-end cases live in `selftest`.

use super::*;
use serde_json::json;

fn repo() -> PathBuf {
    // One directory per call: tests run in parallel.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("xtask-guard-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("crates/eng-core/src")).unwrap();
    fs::write(dir.join("architecture.toml"), "").unwrap();
    fs::write(dir.join("deny.toml"), "").unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = []\n[workspace.dependencies]\nx = \"1\"\n[workspace.lints.clippy]\ntodo = \"warn\"\n[profile.dev]\nopt-level = 1\n",
    )
    .unwrap();
    fs::canonicalize(dir).unwrap()
}

fn decide_in(root: &Path, tool: &str, args: Value, unlocked: &[&str]) -> Decision {
    let input = json!({ "tool_name": tool, "tool_input": args, "cwd": root });
    let unlocked: Vec<String> = unlocked.iter().map(|s| s.to_string()).collect();
    decide(&protected().unwrap(), root, root, &input, &unlocked).unwrap()
}

fn bash(root: &Path, cmd: &str) -> Decision {
    decide_in(root, "Bash", json!({ "command": cmd }), &[])
}

#[test]
fn file_tools() {
    let root = repo();
    let blocked = |tool: &str, path: &str| {
        matches!(
            decide_in(&root, tool, json!({ "file_path": path }), &[]),
            Decision::Block(_)
        )
    };
    assert!(blocked("Edit", "deny.toml"));
    assert!(blocked(
        "Write",
        root.join("xtask/src/main.rs").to_str().unwrap()
    ));
    assert!(blocked("Edit", "crates/x/scenarios/a.assert.toml"));
    assert!(blocked("Edit", "DENY.TOML"), "case-insensitive");
    assert!(blocked("Edit", "crates/../deny.toml"));
    assert!(!blocked("Edit", "crates/eng-core/src/lib.rs"));
    assert!(!blocked("Write", "/tmp/elsewhere.txt"));
    assert_eq!(
        decide_in(
            &root,
            "Edit",
            json!({ "file_path": "deny.toml" }),
            &["deny.toml"]
        ),
        Decision::Allow,
        "the owner's unlock file"
    );
}

#[test]
fn root_manifest_tables() {
    let root = repo();
    let edit = |old: &str, new: &str| {
        decide_in(
            &root,
            "Edit",
            json!({ "file_path": "Cargo.toml", "old_string": old, "new_string": new }),
            &[],
        )
    };
    assert_eq!(edit("x = \"1\"", "x = \"2\""), Decision::Allow);
    assert!(matches!(
        edit("opt-level = 1", "opt-level = 0"),
        Decision::Block(_)
    ));
    assert!(matches!(
        edit("todo = \"warn\"", "todo = \"allow\""),
        Decision::Block(_)
    ));
}

#[test]
fn symlinks_into_the_repo_are_followed() {
    let root = repo();
    let link = root.join("crates/innocent.toml");
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("deny.toml"), &link).unwrap();
    #[cfg(unix)]
    assert!(matches!(
        decide_in(&root, "Write", json!({ "file_path": link }), &[]),
        Decision::Block(_)
    ));
}

#[test]
fn bash_writes() {
    let root = repo();
    for cmd in [
        "echo x > deny.toml",
        "echo x >deny.toml",
        "cat a >> clippy.toml",
        "sed -i '' 's/a/b/' clippy.toml",
        "perl -pi -e 's/a/b/' architecture.toml",
        "tee .github/workflows/ci.yml < x",
        "cp /tmp/x xtask/src/main.rs",
        "rm budgets.toml",
        "git checkout -- deny.toml",
        "python3 -c \"open('deny.toml','w').write('')\"",
        "cargo build && echo ok > protected.toml",
        // Bypasses found in review.
        "cd .claude && echo '*' > guard-unlock",
        "cd xtask && echo x >> ../deny.toml",
        "cd .github/workflows && sed -i s/a/b/ ci.yml",
        "python3 - <<'EOF'\nopen('deny.toml', 'a').write('x')\nEOF",
        "f=deny.toml; echo x >> $f",
        "echo x >> den?.toml",
        "ln deny.toml d2 && echo x >> d2",
        "git checkout HEAD~1 .",
        "(cd /tmp); echo x > deny.toml",
        "echo $(echo x > deny.toml)",
        "cp /tmp/x .git/eng-hooks/xtask",
        "find . -name '*.toml' -exec sed -i s/a/b/ {} +",
        "git ls-files | xargs sed -i s/a/b/",
        "cat > budgets.toml <<EOF\n[sim]\nEOF",
        // Bypasses found in the second review.
        "echo x>deny.toml",
        "echo x >'deny.toml'",
        "f=deny.toml; echo x >\"$f\"",
        ": 9>deny.toml",
        "{ cd xtask; }; echo x > src/main.rs",
        "if true; then cd xtask; fi; echo x > src/main.rs",
        "git -C . checkout HEAD~1 -- deny.toml",
        "git checkout HEAD~1 -- '*.toml'",
        "git checkout HEAD~1 -- :/",
        "bash -ec 'echo x > deny.toml'",
        "dash -c 'echo x > deny.toml'",
        "env -i bash -c 'echo x > deny.toml'",
        "timeout 5 sh -c 'echo x > deny.toml'",
        "awk 'BEGIN { print \"x\" > \"deny.toml\" }'",
        "python3 -Ic \"open('deny.toml', 'w')\"",
        "cp -l deny.toml /tmp/h",
        "git update-index --cacheinfo 100644,0123,deny.toml",
        "git branch -f main HEAD",
    ] {
        assert!(
            matches!(bash(&root, cmd), Decision::Block(_)),
            "{cmd:?} should be blocked"
        );
    }
    for cmd in [
        "cat deny.toml",
        "cargo test -p xtask 2>&1 | tail -20",
        "cargo run -p xtask -- arch-check",
        "cp deny.toml /tmp/deny-copy.toml",
        "grep -n wgpu deny.toml > /tmp/out.txt",
        "git diff deny.toml",
        "echo '>' deny.toml",
        "cd crates/eng-core && cargo test",
        "echo x > /tmp/$f",
        "git checkout main",
        "git checkout -b feature/x",
        "for f in a b; do echo $f; done",
        "python3 - <<'EOF'\nprint(1)\nEOF",
        "ls ~/x > ~/y.txt",
        "find crates -name '*.rs' | wc -l",
        "(cd /tmp && echo x > out.txt)",
        // False positives found in the second review.
        "bash -c 'cargo test -p eng-math'",
        "sh -c 'cargo xtask quick'",
        "python3 - <<'EOF'\nprint(open('crates/game-testbed/src/main.rs').read())\nEOF",
        "echo x > \"$TMPDIR/x\"",
        "tmp=$(mktemp -d); rm -rf \"$tmp\"",
        "rm -f target/*.json",
        "git restore --staged .",
        "git checkout -- .",
        "find . -name '*.rs' -exec grep -l foo {} +",
        "ls | xargs grep 'rm'",
        "git commit -m 'fix the --accept handling'",
        "git commit -m 'UPDATE_X=1 is documented'",
        "grep guard-unlock docs",
        "cargo test 2>&1 | tail -5",
        "cargo build >/dev/null 2>&1",
    ] {
        assert_eq!(
            bash(&root, cmd),
            Decision::Allow,
            "{cmd:?} should be allowed"
        );
    }
}

#[test]
fn bless_commands() {
    let root = repo();
    for cmd in [
        "cargo insta accept",
        "cargo insta test --accept",
        "INSTA_UPDATE=always cargo test",
        "UPDATE_EXPECT=1 cargo test",
        "cargo test -- --bless",
        "env UPDATE_GOLDENS=1 cargo test",
        "export INSTA_UPDATE=always",
        "cd crates && cargo insta review",
    ] {
        assert!(matches!(bash(&root, cmd), Decision::Block(_)), "{cmd:?}");
    }
    assert_eq!(
        decide_in(
            &root,
            "Bash",
            json!({ "command": "cargo insta accept" }),
            &["bless"]
        ),
        Decision::Allow
    );
    assert_eq!(
        bash(&root, "cargo xtask replay --regen"),
        Decision::Allow,
        "hash streams are not oracles"
    );
}

#[test]
fn fails_closed_on_bad_input() {
    let root = repo();
    let input = json!({ "tool_name": "Edit", "tool_input": {} });
    assert!(decide(&protected().unwrap(), &root, &root, &input, &[]).is_err());
    let input = json!({ "tool_input": {} });
    assert!(decide(&protected().unwrap(), &root, &root, &input, &[]).is_err());
}
