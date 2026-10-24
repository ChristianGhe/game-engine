//! The guard's checks for programs with their own rules: git, patch, find
//! and xargs.

use std::path::PathBuf;

use crate::guard::{Decision, normalize};
use crate::guard_bash::{Check, writer};
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
                "-c" => i += 2,
                t if t.starts_with("--work-tree=") => {
                    cwd = cwd.map(|c| normalize(&c.join(&t["--work-tree=".len()..])));
                    i += 1;
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
            "rm" | "mv" => rest
                .iter()
                .filter(|w| !w.text.starts_with('-'))
                .find_map(guarded)
                .and_then(blocked),
            "checkout" | "restore" => {
                let (source, paths, worktree) = checkout_parts(sub, rest, &cwd);
                if !worktree {
                    return None;
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
            "branch" | "update-ref" | "push"
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
                    }))
                    && (sub != "push" || rest.iter().any(|w| w.text == ".")) =>
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
