//! The guard's checks for git settings and variables: those that make git
//! run another program, write a file or work on another repository are the
//! owner's to set (ADR-0015).

use crate::guard::Decision;
use crate::guard_bash::Check;
use crate::guard_vcs::{operands, value};
use crate::guard_words::Var;
use crate::shell::Word;

/// git settings an agent may give with `-c` or `git config`: none runs a
/// program, writes a file or points git elsewhere.
const SAFE_SETTINGS: [&str; 22] = [
    "user.name",
    "user.email",
    "color.*",
    "core.quotepath",
    "core.autocrlf",
    "core.safecrlf",
    "commit.gpgsign",
    "tag.gpgsign",
    "advice.*",
    "init.defaultbranch",
    "log.*",
    "format.pretty",
    "diff.renames",
    "diff.algorithm",
    "diff.noprefix",
    "merge.conflictstyle",
    "pull.rebase",
    "pull.ff",
    "rebase.autostash",
    "rebase.autosquash",
    "status.*",
    "fetch.prune",
];
/// Settings that name a program: only a trivial one is safe.
const PROGRAM_SETTINGS: [&str; 4] = ["core.pager", "core.editor", "sequence.editor", "pager.*"];
/// Variables git may be given freely.
const SAFE_VARS: [&str; 10] = [
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_AUTHOR_DATE",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
    "GIT_COMMITTER_DATE",
    "GIT_TERMINAL_PROMPT",
    "GIT_OPTIONAL_LOCKS",
    "GIT_MERGE_AUTOEDIT",
    "GIT_PROGRESS_DELAY",
];
/// Variables that name a program or a trace file: only a trivial value is
/// safe (`GIT_TRACE*` counts too).
const PROGRAM_VARS: [&str; 6] = [
    "GIT_PAGER",
    "PAGER",
    "GIT_EDITOR",
    "GIT_SEQUENCE_EDITOR",
    "EDITOR",
    "VISUAL",
];

impl Check<'_> {
    /// Refuses git given variables that make it run other programs, write
    /// files or work on another repository: `GIT_DIR`, `GIT_EXTERNAL_DIFF`,
    /// `GIT_CONFIG_*`, a `HOME` with another `.gitconfig`, and the like.
    pub(crate) fn git_env(&self, assigns: &[&Word]) -> Option<Decision> {
        let given = assigns
            .iter()
            .filter_map(|w| w.text.split_once('='))
            .map(|(k, v)| (k.to_string(), Some(v.to_string())))
            .chain(self.vars.iter().map(|(k, v)| {
                let value = match v {
                    Var::Value(v) => Some(v.clone()),
                    _ => None,
                };
                (k.clone(), value)
            }));
        for (name, value) in given {
            let ok = if SAFE_VARS.contains(&name.as_str()) {
                true
            } else if PROGRAM_VARS.contains(&name.as_str()) || name.starts_with("GIT_TRACE") {
                value.as_deref().is_some_and(trivial)
            } else {
                !name.starts_with("GIT_") && !matches!(name.as_str(), "HOME" | "XDG_CONFIG_HOME")
            };
            if !ok {
                return Some(Decision::Block(format!(
                    "git with {name} set can run other programs, write files or work on another repository"
                )));
            }
        }
        None
    }

    /// `git config`: reads pass; writes only of safe settings.
    pub(crate) fn config(&self, args: &[&Word]) -> Option<Decision> {
        let with_value = [
            "--file",
            "-f",
            "--blob",
            "--type",
            "--default",
            "--comment",
            "--value",
        ];
        let (flags, ops, _) = operands(args, &with_value);
        let reads = [
            "--get",
            "--get-all",
            "--get-regexp",
            "--get-urlmatch",
            "--get-color",
            "--get-colorbool",
            "--list",
            "-l",
        ];
        let ops: Vec<&str> = ops.iter().map(|w| w.text.as_str()).collect();
        if flags.iter().any(|f| reads.contains(f)) || matches!(ops.first(), Some(&("get" | "list")))
        {
            return None;
        }
        if flags.iter().any(|f| matches!(*f, "--edit" | "-e")) || ops.first() == Some(&"edit") {
            return Some(Decision::Block(
                "git config --edit opens an editor on git's settings".into(),
            ));
        }
        if let Some(file) = value(args, "--file").or_else(|| value(args, "-f"))
            && let Some(what) = self.target(&Word {
                text: file,
                quoted: false,
                dynamic: false,
                op: false,
            })
        {
            return Some(Decision::Block(format!(
                "git config would write protected path {what}"
            )));
        }
        let changes = flags.iter().any(|f| {
            f.starts_with("--unset") || matches!(*f, "--rename-section" | "--remove-section")
        });
        match ops.as_slice() {
            ["set", key, value, ..] => setting(key, Some(value)),
            [
                "set" | "unset" | "rename-section" | "remove-section",
                key,
                ..,
            ] => setting(key, None),
            [_] if !changes => None, // `git config <key>` reads it
            [key, value, ..] => setting(key, Some(value)),
            [key] => setting(key, None),
            [] => None,
        }
    }
}

/// Refuses a git setting not known to be safe.
pub(crate) fn setting(key: &str, value: Option<&str>) -> Option<Decision> {
    let key = key.to_ascii_lowercase();
    let listed = |list: &[&str]| {
        list.iter().any(|p| {
            p.strip_suffix('*')
                .map_or(key == *p, |prefix| key.starts_with(prefix))
        })
    };
    let ok = listed(&SAFE_SETTINGS) || (listed(&PROGRAM_SETTINGS) && value.is_some_and(trivial));
    (!ok).then(|| {
        Decision::Block(format!(
            "git setting {key} can run a program, write files or point git elsewhere; the owner sets it"
        ))
    })
}

/// A program or trace value that runs nothing and writes no file.
fn trivial(value: &str) -> bool {
    matches!(
        value,
        "" | "cat" | "true" | "false" | ":" | "less" | "less -R" | "more"
    ) || value.chars().all(|c| c.is_ascii_digit())
}
