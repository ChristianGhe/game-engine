//! Unit tests for the guard's version-control, GitHub and jj rules (ADR-0015).

use super::tests::{bash, repo};
use super::*;

#[test]
fn pushes_jj_and_github() {
    let root = repo();
    for cmd in [
        "git push",
        "git push origin HEAD",
        "git push --force origin main",
        "git -C . push origin main",
        "git --git-dir .git push",
        "git send-pack origin main",
        "env git push",
        "jj git push",
        "jj git push --bookmark main",
        "jj --no-pager restore --from @- deny.toml",
        "jj abandon",
        "jj -R . new",
        "jj undo",
        "jj op restore abc",
        "jj bookmark set main -r @",
        "jj describe -m x",
        "jj squash",
        "gh api -X PUT repos/o/r/branches/main/protection --input -",
        "gh api repos/o/r/issues -f title=x",
        "gh api --method=DELETE repos/o/r",
        "gh repo delete o/r --yes",
        "gh repo edit --visibility private",
        "gh pr create --fill",
        "gh release create v1",
        "gh secret set X",
        "gh workflow run ci.yml",
        "gh run rerun 1",
    ] {
        assert!(
            matches!(bash(&root, cmd), Decision::Block(_)),
            "{cmd:?} should be blocked"
        );
    }
    for cmd in [
        "git fetch origin",
        "git log origin/main..HEAD",
        "jj",
        "jj log",
        "jj --no-pager log -r '::@'",
        "jj st",
        "jj diff -r @-",
        "jj op log",
        "jj file show -r @- deny.toml",
        "jj bookmark list",
        "jj git remote list",
        "jj resolve --list",
        "gh run list",
        "gh run view 1 --log-failed",
        "gh run watch 1 --exit-status",
        "gh api repos/o/r/actions/runs",
        "gh api -X GET repos/o/r -f per_page=5",
        "gh pr view 3",
        "gh auth status",
        "gh --version",
    ] {
        assert_eq!(
            bash(&root, cmd),
            Decision::Allow,
            "{cmd:?} should be allowed"
        );
    }
}

fn git(root: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
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

/// `repo()` with history: `main` approves `deny.toml` after `old`, `side`
/// changes it differently, and a stash holds a third version.
fn history() -> PathBuf {
    let root = repo();
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "start"]);
    git(&root, &["branch", "old"]);
    fs::write(root.join("deny.toml"), "approved\n").unwrap();
    git(&root, &["commit", "-qam", "approve"]);
    fs::write(root.join("notes.txt"), "x\n").unwrap();
    git(&root, &["add", "notes.txt"]);
    git(&root, &["commit", "-qm", "notes"]);
    git(&root, &["checkout", "-q", "-b", "side", "old"]);
    fs::write(root.join("deny.toml"), "side\n").unwrap();
    git(&root, &["commit", "-qam", "side"]);
    git(&root, &["checkout", "-q", "main"]);
    fs::write(root.join("deny.toml"), "stashed\n").unwrap();
    git(&root, &["stash", "-q"]);
    root
}

#[test]
fn history_that_changes_protected_files() {
    let root = history();
    for cmd in [
        "git revert --no-edit HEAD~1",
        "git revert --no-edit old..HEAD",
        "git cherry-pick side",
        "git reset --hard old",
        "git reset old",
        "git reset old -- deny.toml",
        "git checkout old",
        "git checkout -b again old",
        "git checkout -",
        "git switch side",
        "git switch -d HEAD~2",
        "git switch --orphan empty",
        "git merge side",
        "git rebase side",
        "git stash pop",
        "git revert $(git rev-parse HEAD~1)",
        "git reset --hard no-such-commit",
        "git pull https://example.com/x.git main",
    ] {
        assert!(
            matches!(bash(&root, cmd), Decision::Block(_)),
            "{cmd:?} should be blocked"
        );
    }
    for cmd in [
        "git revert --no-edit HEAD",
        "git cherry-pick --continue",
        "git reset --hard",
        "git reset --hard HEAD",
        "git reset --soft HEAD~2",
        "git reset notes.txt",
        "git reset HEAD~1 -- notes.txt",
        "git checkout main",
        "git checkout -b topic",
        "git switch -c topic",
        "git merge old",
        "git rebase -i HEAD~1",
        "git stash",
        "git log -p old..side",
        "git diff side",
    ] {
        assert_eq!(
            bash(&root, cmd),
            Decision::Allow,
            "{cmd:?} should be allowed"
        );
    }
    // A stash made earlier in the same command holds the agent's own work.
    git(&root, &["stash", "drop", "-q"]);
    assert_eq!(
        bash(&root, "git stash && cargo test && git stash pop"),
        Decision::Allow
    );
    // From an older branch, main's approved version may come forward.
    git(&root, &["checkout", "-q", "old"]);
    for cmd in [
        "git switch main",
        "git merge main",
        "git rebase main",
        "git reset --hard main",
    ] {
        assert_eq!(
            bash(&root, cmd),
            Decision::Allow,
            "{cmd:?} should be allowed"
        );
    }
    assert!(matches!(bash(&root, "git switch side"), Decision::Block(_)));
}

#[test]
fn git_settings_and_commands_that_run_programs() {
    let root = repo();
    for cmd in [
        "git -c core.pager='sh -c x' log",
        "git -c alias.x='!echo x > deny.toml' x",
        "git -c core.hooksPath=/tmp/h commit -m x",
        "git --config-env=core.pager=X log",
        "git --work-tree=/tmp/x status",
        "git --exec-path=/tmp/x status",
        "git -C /tmp/other -c core.worktree=. checkout -f HEAD -- .",
        "GIT_EXTERNAL_DIFF=/tmp/x.sh git diff",
        "GIT_TRACE=deny.toml git status",
        "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.pager GIT_CONFIG_VALUE_0=x git log",
        "HOME=/tmp/fake git log",
        "git config core.pager 'sh -c x'",
        "git config --global alias.x '!sh'",
        "git config set core.hooksPath /tmp/h",
        "git config --edit",
        "git config -f deny.toml user.name x",
        "git fetch . HEAD~1:main",
        "git fetch origin +x:refs/heads/main",
        "git http-push origin main",
        "git rebase --exec 'echo x > deny.toml' HEAD~1",
        "git submodule foreach 'echo x > deny.toml'",
        "git bisect run sh -c 'echo x > deny.toml'",
        "git difftool -y",
        "git filter-branch --tree-filter x",
        "jj log --config 'ui.pager=[\"sh\"]'",
        "jj diff --tool sh",
        "jj --config-toml 'x=1' log",
        "JJ_CONFIG=/tmp/x.toml jj log",
        "gh run download 1",
        "gh release download v1",
    ] {
        assert!(
            matches!(bash(&root, cmd), Decision::Block(_)),
            "{cmd:?} should be blocked"
        );
    }
    for cmd in [
        "git -c color.ui=never log",
        "git -c core.pager=cat log",
        "git --no-pager log",
        "GIT_PAGER=cat git log",
        "GIT_EDITOR=true git rebase --continue",
        "git config user.email",
        "git config --get user.name",
        "git config --list",
        "git config user.name someone",
        "git fetch origin main",
        "git rebase --exec 'cargo test' HEAD~1",
        "git bisect run cargo test",
        "git submodule foreach git status",
        "gh -R o/r run list",
        "gh --repo o/r pr view 3",
    ] {
        assert_eq!(
            bash(&root, cmd),
            Decision::Allow,
            "{cmd:?} should be allowed"
        );
    }
}
