//! The guard's checks for git history: commands that take a protected file
//! from another commit (ADR-0015).
//!
//! The guard asks git which files a command would change. A protected file
//! may only become the version on `main`, which only the owner moves:
//! switching to `main` passes, reverting an approval commit does not.
//! Plumbing such as `read-tree` or `update-index --index-info` is out of its
//! reach.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::guard::Decision;
use crate::guard_bash::Check;
use crate::shell::Word;

impl Check<'_> {
    /// `git revert` and `git cherry-pick`: the files each named commit changes.
    pub(crate) fn picks(&self, sub: &str, args: &[&Word]) -> Option<Decision> {
        let with_value = [
            "-m",
            "--mainline",
            "-X",
            "--strategy-option",
            "--strategy",
            "--cleanup",
        ];
        let (flags, commits, _) = operands(args, &with_value);
        if flags
            .iter()
            .any(|f| matches!(*f, "--continue" | "--abort" | "--skip" | "--quit"))
        {
            return None;
        }
        if flags.contains(&"--stdin") {
            return Some(unknown_rev(sub, "revisions on stdin"));
        }
        for w in commits.iter().filter(|w| !w.text.starts_with('^')) {
            let Some(rev) = self.expand(w) else {
                return Some(unknown_rev(sub, &w.text));
            };
            let (from, to) = match rev.split_once("..") {
                Some((a, b)) => (side(a), side(b.trim_start_matches('.'))),
                None => (format!("{rev}^"), rev.clone()),
            };
            if let Some(block) = self.tree_change(sub, &[&from, &to], &[], None) {
                return Some(block);
            }
        }
        None
    }

    /// `git reset`, `switch`, `merge`, `rebase`, `pull` and `stash pop|apply`:
    /// what the tree they move to changes.
    pub(crate) fn moves(&self, sub: &str, args: &[&Word]) -> Option<Decision> {
        let with_value: &[&str] = match sub {
            "switch" => &["-c", "-C", "--create", "--force-create", "--orphan"],
            "merge" | "pull" => &[
                "-m",
                "-F",
                "--file",
                "-s",
                "--strategy",
                "-X",
                "--strategy-option",
                "--cleanup",
                "--into-name",
                "--depth",
                "--upload-pack",
            ],
            "rebase" => &[
                "--onto",
                "-s",
                "--strategy",
                "-X",
                "--strategy-option",
                "-x",
                "--exec",
                "-C",
            ],
            _ => &[],
        };
        let (flags, ops, paths) = operands(args, with_value);
        if flags
            .iter()
            .any(|f| matches!(*f, "--continue" | "--abort" | "--skip" | "--quit"))
        {
            return None;
        }
        let rev = |w: &Word| match w.text.as_str() {
            "-" => Ok("@{-1}".to_string()),
            _ => self.expand(w).ok_or_else(|| unknown_rev(sub, &w.text)),
        };
        let targets: Result<Vec<String>, Decision> = match sub {
            "reset" => {
                if flags.contains(&"--soft") {
                    return None; // moves HEAD only: no file changes
                }
                if flags.iter().any(|f| f.starts_with("--pathspec-from-file")) {
                    return Some(unknown_rev(sub, "paths read from a file"));
                }
                // The first operand is a revision unless it names a path.
                let (target, rest) = match ops.split_first() {
                    Some((first, rest)) if !self.exists(first) => (*first, rest),
                    _ => return None,
                };
                let mut pathspec: Vec<String> = rest.iter().map(|w| w.text.clone()).collect();
                pathspec.extend(paths.iter().map(|w| w.text.clone()));
                return match rev(target) {
                    Ok(t) => self.tree_change(sub, &["HEAD", &t], &pathspec, Some(&t)),
                    Err(block) => Some(block),
                };
            }
            "switch" => {
                if flags.iter().any(|f| f.starts_with("--orphan")) {
                    return Some(Decision::Block(
                        "git switch --orphan empties the working tree, protected files included"
                            .into(),
                    ));
                }
                let target = ops.first()?;
                return match rev(target) {
                    Ok(t) => self.tree_change(sub, &["HEAD", &t], &[], Some(&t)),
                    Err(block) => Some(block),
                };
            }
            "stash" => {
                let at = match ops.first().map(|w| w.text.as_str()) {
                    Some("pop" | "apply") => 1,
                    Some("branch") => 2,
                    _ => return None,
                };
                let stash = match ops.get(at).map(|w| self.expand(w)) {
                    Some(None) => return Some(unknown_rev(sub, &ops[at].text)),
                    Some(Some(n)) if n.chars().all(|c| c.is_ascii_digit()) => {
                        format!("stash@{{{n}}}")
                    }
                    Some(Some(s)) => s,
                    None => "stash@{0}".to_string(),
                };
                // A stash made earlier in the same command does not exist yet;
                // it holds the agent's own changes, so it may come back.
                if let Some(cwd) = &self.cwd
                    && git_out(cwd, &["rev-parse", "--verify", "-q", &stash]).is_none()
                    && self.ran_earlier(&["git", "stash"])
                {
                    return None;
                }
                return self.tree_change(sub, &[&format!("{stash}^1"), &stash], &[], None);
            }
            "pull" => match ops.as_slice() {
                [] | [_] => Ok(vec!["@{u}".to_string()]),
                [remote, branches @ ..]
                    if ref_name(&remote.text, false)
                        && branches.iter().all(|b| ref_name(&b.text, true)) =>
                {
                    Ok(branches
                        .iter()
                        .map(|b| format!("{}/{}", remote.text, b.text))
                        .collect())
                }
                _ => {
                    return Some(Decision::Block(
                        "git pull from a URL or refspec: fetch first, then merge, so the guard can see what it brings"
                            .into(),
                    ));
                }
            },
            _ => {
                // merge, rebase: the other side's changes since the merge base.
                if sub == "rebase" {
                    for exec in values(args, &["-x", "--exec"]) {
                        if let Some(block) = self.nested(&exec) {
                            return Some(block);
                        }
                    }
                }
                let onto = (sub == "rebase").then(|| value(args, "--onto")).flatten();
                match (onto, ops.is_empty()) {
                    (None, true) => Ok(vec!["@{u}".to_string()]),
                    (onto, _) => ops.iter().map(|w| rev(w)).chain(onto.map(Ok)).collect(),
                }
            }
        };
        // A merge brings the other side's changes since the merge base. A
        // rebase ends at its target's tree plus HEAD's own commits, and onto
        // an ancestor it can drop commits, so it compares the trees.
        match targets {
            Ok(targets) => targets.iter().find_map(|t| {
                let three_dot = format!("HEAD...{t}");
                let revs = if sub == "rebase" {
                    vec!["HEAD", t.as_str()]
                } else {
                    vec![three_dot.as_str()]
                };
                self.tree_change(sub, &revs, &[], Some(t))
            }),
            Err(block) => Some(block),
        }
    }

    /// Refuses `git <sub>` when `git diff <revs> -- <paths>` names a
    /// protected path, unless `onto` has `main`'s version of it; or when git
    /// cannot tell.
    pub(crate) fn tree_change(
        &self,
        sub: &str,
        revs: &[&str],
        paths: &[String],
        onto: Option<&str>,
    ) -> Option<Decision> {
        match self.protected_change(revs, paths, onto) {
            Ok(None) => None,
            Ok(Some(what)) => Some(changes(sub, &what, &revs.join(" "))),
            Err(()) => Some(unknown_rev(sub, &revs.join(" "))),
        }
    }

    /// The first protected path that differs between `revs` and, if `onto`
    /// is given, also between `onto` and `main`. `Err` if git cannot say.
    fn protected_change(
        &self,
        revs: &[&str],
        paths: &[String],
        onto: Option<&str>,
    ) -> Result<Option<String>, ()> {
        let cwd = self.cwd.as_ref().ok_or(())?;
        if revs.iter().chain(&onto).any(|r| r.starts_with('-')) {
            return Err(());
        }
        let Some(top) = git_out(cwd, &["rev-parse", "--show-toplevel"]) else {
            // Outside a repository git refuses the command itself; inside
            // this one, a failing git is no reason to let it through.
            let here = self.ctx.root.join(".git").exists() && self.inside_repo(".");
            return if here { Err(()) } else { Ok(None) };
        };
        let top = PathBuf::from(top.trim_end_matches('\n'));
        let mut args = vec!["diff", "--name-only", "-z", "--no-renames", "--no-relative"];
        args.push("--no-ext-diff");
        args.extend(revs);
        args.push("--");
        args.extend(paths.iter().map(String::as_str));
        let names = git_out(cwd, &args).ok_or(())?;
        let mut hits: Vec<(&str, String)> = names
            .split('\0')
            .filter(|n| !n.is_empty())
            .filter_map(|n| self.ctx.guarded_path(&top, n).map(|what| (n, what)))
            .collect();
        if let Some(onto) = onto
            && !hits.is_empty()
        {
            // Bringing in main's version is how approved changes arrive.
            let specs: Vec<String> = hits.iter().map(|(n, _)| format!(":(literal){n}")).collect();
            let mut args = vec!["diff", "--name-only", "-z", "--no-renames", "--no-relative"];
            args.extend(["--no-ext-diff", "refs/heads/main", onto, "--"]);
            args.extend(specs.iter().map(String::as_str));
            if let Some(differ) = git_out(&top, &args) {
                hits.retain(|(n, _)| differ.split('\0').any(|d| d == *n));
            }
        }
        Ok(hits.into_iter().next().map(|(_, what)| what))
    }

    fn exists(&self, w: &Word) -> bool {
        self.cwd.as_ref().is_some_and(|c| c.join(&w.text).exists())
    }
}

/// `(flags, operands, paths after --)`; a flag in `with_value` takes the
/// next word as its value.
pub(crate) fn operands<'a>(
    args: &[&'a Word],
    with_value: &[&str],
) -> (Vec<&'a str>, Vec<&'a Word>, Vec<&'a Word>) {
    let (mut flags, mut ops, mut paths) = (Vec::new(), Vec::new(), Vec::new());
    let mut dashes = false;
    let mut i = 0;
    while i < args.len() {
        let t = args[i].text.as_str();
        if dashes {
            paths.push(args[i]);
        } else if t == "--" {
            dashes = true;
        } else if t.starts_with('-') && t != "-" {
            flags.push(t);
            if with_value.contains(&t) {
                i += 1;
            }
        } else {
            ops.push(args[i]);
        }
        i += 1;
    }
    (flags, ops, paths)
}

/// The value of flag `name`, given as `name value` or `name=value`.
pub(crate) fn value(args: &[&Word], name: &str) -> Option<String> {
    values(args, &[name]).into_iter().next()
}

/// Every value of the flags in `names`.
pub(crate) fn values(args: &[&Word], names: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, w) in args.iter().enumerate() {
        for name in names {
            if w.text == *name {
                out.extend(args.get(i + 1).map(|v| v.text.clone()));
            } else if let Some(v) = w.text.strip_prefix(name).and_then(|r| r.strip_prefix('=')) {
                out.push(v.to_string());
            }
        }
    }
    out
}

/// `a..b` with an empty side means HEAD.
fn side(rev: &str) -> String {
    let rev = if rev.is_empty() { "HEAD" } else { rev };
    rev.to_string()
}

/// A remote or branch name, not a URL or refspec.
fn ref_name(text: &str, slashes: bool) -> bool {
    !text.is_empty()
        && !text.starts_with(['-', '/'])
        && !text.contains("..")
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c) || (slashes && c == '/'))
}

fn changes(sub: &str, what: &str, from: &str) -> Decision {
    Decision::Block(format!(
        "git {sub} would change protected path {what} (from {from}) to a version that is not main's"
    ))
}

fn unknown_rev(sub: &str, rev: &str) -> Decision {
    Decision::Block(format!(
        "git {sub}: the guard cannot tell what {rev} would change (an unknown revision, or the first commit); name an existing commit"
    ))
}

/// A read-only git command in `dir`: its output, or `None` if it fails.
pub(crate) fn git_out(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}
