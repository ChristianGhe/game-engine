//! The guard's checks for programs with their own rules: git, patch, find
//! and xargs. History and git settings are in `guard_vcs` and
//! `guard_settings`; pushes, jj and gh in `guard_owner`; shells,
//! interpreters, downloads and archives in `guard_code`.

use std::path::PathBuf;

use crate::guard::{Decision, normalize};
use crate::guard_bash::{Check, writer};
use crate::guard_owner::OWNER_ONLY;
use crate::guard_settings::setting;
use crate::shell::Word;

impl Check<'_> {
    pub(crate) fn git(&self, args: &[&Word], heredoc: &str) -> Option<Decision> {
        let mut cwd = self.cwd.clone();
        let mut i = 0;
        while i < args.len() && args[i].text.starts_with('-') {
            match args[i].text.as_str() {
                "-C" => {
                    cwd = cwd
                        .zip(args.get(i + 1))
                        .map(|(c, d)| normalize(&c.join(&d.text)));
                    i += 2;
                }
                "-c" => {
                    let set = args.get(i + 1).map_or("", |w| w.text.as_str());
                    let (key, val) = set
                        .split_once('=')
                        .map_or((set, None), |(k, v)| (k, Some(v)));
                    if let Some(block) = setting(key, val) {
                        return Some(block);
                    }
                    i += 2;
                }
                t if t.starts_with("--config-env") => {
                    // The value comes from the environment, unseen.
                    let set = t
                        .strip_prefix("--config-env=")
                        .or_else(|| args.get(i + 1).map(|w| w.text.as_str()))
                        .unwrap_or("");
                    if let Some(block) = setting(set.split('=').next().unwrap_or(""), None) {
                        return Some(block);
                    }
                    i += if t.contains('=') { 1 } else { 2 };
                }
                "--namespace" => i += 2,
                t if matches!(t, "--git-dir" | "--work-tree")
                    || ["--git-dir=", "--work-tree=", "--exec-path="]
                        .iter()
                        .any(|o| t.starts_with(o)) =>
                {
                    return Some(Decision::Block(format!(
                        "git {t} points git at another repository, work tree or program; use -C"
                    )));
                }
                _ => i += 1,
            }
        }
        let sub = args.get(i)?.text.as_str();
        let rest = &args[i + 1..];
        let here = Check {
            cwd: cwd.clone(),
            ..*self
        };
        let guarded = |w: &&Word| here.target(w);
        let blocked = |what: String| {
            Some(Decision::Block(format!(
                "git {sub} would write protected path {what}"
            )))
        };
        match sub {
            "rm" | "mv" => {
                let paths: Vec<&&Word> = rest.iter().filter(|w| !w.text.starts_with('-')).collect();
                if let Some(what) = paths.iter().copied().find_map(guarded) {
                    return blocked(what);
                }
                // A directory takes what is under it; `git mv` moves its sources.
                let taken = match sub {
                    "mv" => &paths[..paths.len().saturating_sub(1)],
                    _ => &paths[..],
                };
                taken.iter().find_map(|w| here.directory(w)).and_then(blocked)
            }
            "checkout" | "restore" => {
                let (source, paths, worktree) = checkout_parts(sub, rest, &cwd);
                if !worktree {
                    return None;
                }
                if sub == "checkout" && paths.is_empty() {
                    // Switching commits: whatever the target's tree changes.
                    let target = source.or_else(|| {
                        rest.iter()
                            .any(|w| w.text == "-")
                            .then(|| "@{-1}".to_string())
                    });
                    return target
                        .and_then(|t| here.tree_change(sub, &["HEAD", &t], &[], Some(&t)));
                }
                if let Some(what) = paths.iter().find_map(guarded) {
                    return blocked(what);
                }
                // Restoring from HEAD or the index brings back approved content;
                // from any other revision, only explicitly named files.
                let broad = paths.iter().any(|w| {
                    w.text.contains(['*', '?', '['])
                        || w.text.starts_with(':')
                        || cwd.as_ref().is_some_and(|c| c.join(&w.text).is_dir())
                });
                (broad && source.as_deref().is_some_and(|s| s != "HEAD")).then(|| {
                    Decision::Block(format!(
                        "git {sub} from {} of a directory or pattern can restore protected files; name the files",
                        source.unwrap_or_default()
                    ))
                })
            }
            "apply" | "am" => here.patch_text(rest, heredoc),
            "checkout-index"
                if rest
                    .iter()
                    .any(|w| matches!(w.text.as_str(), "-a" | "--all")) =>
            {
                Some(Decision::Block(
                    "git checkout-index --all can rewrite protected files".into(),
                ))
            }
            "checkout-index" | "update-index" => rest
                .iter()
                .flat_map(|w| {
                    w.text
                        .split(',')
                        .map(|part| Word {
                            text: part.to_string(),
                            quoted: false,
                            dynamic: false,
                            op: false,
                        })
                        .collect::<Vec<_>>()
                })
                .find_map(|w| here.target(&w))
                .and_then(blocked),
            "read-tree" if rest.iter().any(|w| w.text == "-u") => Some(Decision::Block(
                "git read-tree -u rewrites the working tree".into(),
            )),
            "push" | "send-pack" | "http-push" => {
                Some(Decision::Block(format!("git {sub}: {OWNER_ONLY}")))
            }
            "revert" | "cherry-pick" => here.picks(sub, rest),
            "reset" | "switch" | "merge" | "rebase" | "pull" | "stash" => here.moves(sub, rest),
            "config" => here.config(rest),
            "fetch"
                if rest.iter().any(|w| {
                    let to = w.text.rsplit(':').next().unwrap_or("");
                    w.text.contains(':') && matches!(to, "main" | "refs/heads/main")
                }) =>
            {
                Some(Decision::Block(
                    "git fetch into main would move the local main branch, which only the owner moves"
                        .into(),
                ))
            }
            // Commands that run a command of the agent's.
            "bisect" if rest.first().is_some_and(|w| w.text == "run") => {
                here.nested(&quoted(&rest[1..]))
            }
            "submodule" => rest.iter().position(|w| w.text == "foreach").and_then(|at| {
                let command: Vec<&str> = rest[at + 1..]
                    .iter()
                    .map(|w| w.text.as_str())
                    .filter(|t| !matches!(*t, "--recursive" | "-q" | "--quiet"))
                    .collect();
                here.nested(&command.join(" "))
            }),
            "difftool" | "mergetool" | "filter-branch" | "filter-repo" => Some(Decision::Block(
                format!("git {sub} runs other programs over the tree; the owner runs it"),
            )),
            "branch" | "update-ref"
                if rest.iter().any(|w| {
                    w.text == "main"
                        || w.text.ends_with(":main")
                        || w.text.ends_with("refs/heads/main")
                }) && (sub != "branch"
                    || rest.iter().any(|w| {
                        matches!(
                            w.text.as_str(),
                            "-f" | "--force" | "-D" | "-M" | "-m" | "-C" | "-c"
                        )
                    })) =>
            {
                Some(Decision::Block(format!(
                    "git {sub} would rewrite the local main branch, which agent hooks are built from"
                )))
            }
            _ => None,
        }
    }

    /// Patch files, heredocs and `< file` input: refused if any diff header
    /// names a protected path.
    pub(crate) fn patch_text(&self, args: &[&Word], heredoc: &str) -> Option<Decision> {
        let mut text = String::from(heredoc);
        for w in args.iter().filter(|w| !w.text.starts_with('-')) {
            text.push_str(&self.read_file(&w.text).unwrap_or_default());
        }
        let cwd = self
            .cwd
            .clone()
            .unwrap_or_else(|| self.ctx.root.to_path_buf());
        for line in text.lines() {
            let path = ["+++ ", "--- ", "rename to ", "copy to ", "rename from "]
                .iter()
                .find_map(|p| line.strip_prefix(p))
                .or_else(|| {
                    line.strip_prefix("diff --git ")
                        .and_then(|l| l.split_whitespace().last())
                });
            let Some(path) = path else {
                continue;
            };
            let path = path.split('\t').next().unwrap_or(path);
            let path = path
                .strip_prefix("a/")
                .or_else(|| path.strip_prefix("b/"))
                .unwrap_or(path);
            if let Some(rel) = self.ctx.guarded_path(&cwd, path) {
                return Some(Decision::Block(format!(
                    "the patch changes protected path {rel}"
                )));
            }
        }
        None
    }

    pub(crate) fn find(&self, args: &[&Word]) -> Option<Decision> {
        if self.lenient {
            return None;
        }
        let starts: Vec<&str> = args
            .iter()
            .map(|w| w.text.as_str())
            .take_while(|t| !t.starts_with(['-', '(', '!']))
            .collect();
        let starts = if starts.is_empty() { vec!["."] } else { starts };
        let exec = args
            .iter()
            .position(|w| matches!(w.text.as_str(), "-exec" | "-execdir" | "-ok" | "-okdir"));
        let deletes = args.iter().any(|w| w.text == "-delete");
        let exec_writes = exec.is_some_and(|i| {
            args.get(i + 1)
                .is_some_and(|w| writer(&w.text, &args[i + 1..]))
        });
        ((deletes || exec_writes) && starts.iter().any(|s| self.inside_repo(s))).then(|| {
            Decision::Block(
                "find that deletes or runs a writer inside the repo can reach protected files"
                    .into(),
            )
        })
    }

    pub(crate) fn xargs(&self, args: &[&Word]) -> Option<Decision> {
        if self.lenient {
            return None;
        }
        let with_value = ["-n", "-I", "-L", "-P", "-d", "-E", "-s", "-a", "-i"];
        let mut i = 0;
        while i < args.len() && args[i].text.starts_with('-') {
            i += if with_value.contains(&args[i].text.as_str()) {
                2
            } else {
                1
            };
        }
        let program = args.get(i)?;
        (writer(&program.text, &args[i + 1..]) && self.inside_repo(".")).then(|| {
            Decision::Block(format!(
                "xargs feeds {} paths the guard cannot see",
                program.text
            ))
        })
    }
}
/// The words as one command line, each quoted when it needs it.
fn quoted(words: &[&Word]) -> String {
    words
        .iter()
        .map(|w| {
            let plain = w
                .text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./=:,@%+".contains(c));
            if plain && !w.text.is_empty() {
                w.text.clone()
            } else {
                format!("'{}'", w.text.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
/// `(source revision, paths, touches the working tree)` for checkout/restore.
fn checkout_parts<'a>(
    sub: &str,
    args: &[&'a Word],
    cwd: &Option<PathBuf>,
) -> (Option<String>, Vec<&'a Word>, bool) {
    let with_value = [
        "-b",
        "-B",
        "--orphan",
        "-s",
        "--source",
        "--conflict",
        "--pathspec-from-file",
    ];
    let mut source = None;
    let (mut staged, mut worktree) = (false, false);
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut seen_dashes = false;
    let mut i = 0;
    while i < args.len() {
        let t = args[i].text.as_str();
        if seen_dashes {
            after.push(args[i]);
        } else if t == "--" {
            seen_dashes = true;
        } else if let Some(s) = t.strip_prefix("--source=") {
            source = Some(s.to_string());
        } else if matches!(t, "-s" | "--source") {
            source = args.get(i + 1).map(|w| w.text.clone());
            i += 1;
        } else if matches!(t, "--staged" | "-S") {
            staged = true;
        } else if matches!(t, "--worktree" | "-W") {
            worktree = true;
        } else if with_value.contains(&t) {
            i += 1;
        } else if !t.starts_with('-') {
            before.push(args[i]);
        }
        i += 1;
    }
    if sub == "restore" {
        let mut paths = before;
        paths.extend(after);
        return (source, paths, worktree || !staged);
    }
    if seen_dashes {
        let rev = before.first().map(|w| w.text.clone());
        return (rev, after, true);
    }
    // No `--`: a leading existing path means paths only; otherwise a revision.
    let is_path = |w: &&Word| cwd.as_ref().is_some_and(|c| c.join(&w.text).exists());
    match before.first() {
        Some(first) if is_path(first) => (None, before, true),
        Some(first) => (Some(first.text.clone()), before[1..].to_vec(), true),
        None => (None, Vec::new(), true),
    }
}
