//! The Claude Code `PreToolUse` guard (spec 7.3).
//!
//! - Blocks Edit/Write/MultiEdit/NotebookEdit on protected paths, and edits
//!   that change the protected tables of the root `Cargo.toml`.
//! - Refuses Bash commands that write to protected paths or "bless"
//!   snapshots and expected output (`guard_bash.rs`).
//! - Fails closed: any error exits with 2, which blocks the tool call.
//!
//! This is defense in depth. It runs as the agent's own OS user, so a
//! determined agent can defeat it; CI's `oracle-check`, which runs only
//! base-branch code, is the gate. It catches accidents and the common forms.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use serde::Deserialize;
use serde_json::Value;

use crate::{glob, util};

pub const PROTECTED_TOML: &str = include_str!("../../protected.toml");
pub const UNLOCK_FILE: &str = ".claude/guard-unlock";

/// Paths the guard protects for its own sake: its binary, the sources it is
/// built from, and the owner's unlock file.
const GUARD_PATHS: [&str; 5] = [
    ".git/eng-hooks/**",
    "target/hooks-src/**",
    "target/hooks-build/**",
    // Cargo reads config from every parent of the hook build.
    "target/.cargo/**",
    ".claude/guard-unlock",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protected {
    #[serde(rename = "version")]
    _version: u32,
    pub owner_github: String,
    pub paths: Vec<String>,
    pub cargo_toml_tables: Vec<String>,
}

pub fn protected() -> Result<Protected, String> {
    toml::from_str(PROTECTED_TOML).map_err(|e| format!("protected.toml: {e}"))
}

impl Protected {
    /// Case-insensitive, because macOS and Windows file systems are.
    pub fn matches(&self, rel: &str) -> bool {
        let rel = rel.to_lowercase();
        self.paths
            .iter()
            .any(|p| glob::matches(&p.to_lowercase(), &rel))
    }

    /// Text that names a protected path inside code the guard cannot parse:
    /// file names (`deny.toml`, `.assert.toml`) and protected directories
    /// (`xtask/`, `.github/workflows/`).
    pub fn fragments(&self) -> Vec<String> {
        self.paths
            .iter()
            .map(String::as_str)
            .chain(GUARD_PATHS)
            .map(|p| {
                let p = p.strip_prefix("**/").unwrap_or(p).to_lowercase();
                match p.strip_suffix("/**") {
                    Some(dir) => format!("{dir}/"),
                    None => p
                        .rsplit('/')
                        .next()
                        .unwrap_or(&p)
                        .trim_start_matches('*')
                        .to_string(),
                }
            })
            .filter(|f| f.len() >= 4)
            .collect()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Block(String),
}

pub fn main() -> ExitCode {
    match run() {
        Ok(Decision::Allow) => ExitCode::SUCCESS,
        Ok(Decision::Block(reason)) => {
            eprintln!(
                "[E-GUARD] {reason}\nProtected oracles and check tooling change only with the owner \
                 (AGENTS.md, \"Protected paths\"). Propose the change in your summary instead."
            );
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("[E-GUARD] guard error, failing closed: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<Decision, String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("reading stdin: {e}"))?;
    let input: Value = serde_json::from_str(&input).map_err(|e| format!("hook input: {e}"))?;
    let cwd = input["cwd"]
        .as_str()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("no cwd")?;
    let root = match std::env::var_os("CLAUDE_PROJECT_DIR") {
        Some(dir) => util::find_root(Path::new(&dir))?,
        None => util::find_root(&cwd)?,
    };
    let unlocked: Vec<String> = fs::read_to_string(root.join(UNLOCK_FILE))
        .map(|t| {
            t.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    decide(&protected()?, &root, &cwd, &input, &unlocked)
}

pub fn decide(
    rules: &Protected,
    root: &Path,
    cwd: &Path,
    input: &Value,
    unlocked: &[String],
) -> Result<Decision, String> {
    let tool = input["tool_name"].as_str().ok_or("no tool_name")?;
    let args = &input["tool_input"];
    let ctx = Ctx {
        rules,
        root,
        cwd,
        unlocked,
    };
    match tool {
        "Bash" => Ok(ctx.bash(args["command"].as_str().ok_or("Bash without command")?)),
        "Read" | "Grep" | "Glob" | "LS" | "WebFetch" | "WebSearch" | "TodoWrite" => {
            Ok(Decision::Allow)
        }
        _ => {
            let path = ["file_path", "notebook_path", "path"]
                .iter()
                .find_map(|k| args[*k].as_str());
            match path {
                Some(path) => ctx.file_write(tool, path, args),
                None if matches!(tool, "Edit" | "Write" | "MultiEdit" | "NotebookEdit") => {
                    Err(format!("{tool} without a path"))
                }
                None => Ok(Decision::Allow),
            }
        }
    }
}

pub(crate) struct Ctx<'a> {
    pub rules: &'a Protected,
    pub root: &'a Path,
    pub cwd: &'a Path,
    pub unlocked: &'a [String],
}

pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// `~` and `~/x` with `$HOME`.
pub(crate) fn expand_home(text: &str) -> String {
    match (text.strip_prefix('~'), std::env::var("HOME")) {
        (Some(rest), Ok(home)) if rest.is_empty() || rest.starts_with('/') => {
            format!("{home}{rest}")
        }
        _ => text.to_string(),
    }
}

impl Ctx<'_> {
    /// The repo-relative path `raw` names from `cwd`, after symlinks; `None`
    /// when it lies outside the repo.
    pub fn resolve_in(&self, cwd: &Path, raw: &str) -> Option<String> {
        let joined = normalize(&cwd.join(expand_home(raw)));
        let real = fs::canonicalize(&joined).ok().or_else(|| {
            let parent = fs::canonicalize(joined.parent()?).ok()?;
            Some(parent.join(joined.file_name()?))
        });
        let root = fs::canonicalize(self.root).unwrap_or_else(|_| self.root.to_path_buf());
        let path = real.unwrap_or(joined);
        let rel = path.strip_prefix(&root).ok()?;
        Some(rel.to_string_lossy().replace('\\', "/"))
    }

    pub fn all_unlocked(&self) -> bool {
        self.unlocked.iter().any(|l| l == "*")
    }

    fn unlocked(&self, rel: &str) -> bool {
        let rel = rel.to_lowercase();
        self.unlocked
            .iter()
            .any(|p| p == "*" || glob::matches(&p.to_lowercase(), &rel))
    }

    /// Protected and not unlocked.
    pub fn guarded(&self, rel: &str) -> bool {
        let lower = rel.to_lowercase();
        let protected =
            self.rules.matches(rel) || GUARD_PATHS.iter().any(|p| glob::matches(p, &lower));
        protected && !self.unlocked(rel)
    }

    /// The guarded repo-relative path `raw` names from `cwd`, if any.
    pub fn guarded_path(&self, cwd: &Path, raw: &str) -> Option<String> {
        self.resolve_in(cwd, raw).filter(|rel| self.guarded(rel))
    }

    fn file_write(&self, tool: &str, path: &str, args: &Value) -> Result<Decision, String> {
        let Some(rel) = self.resolve_in(self.cwd, path) else {
            return Ok(Decision::Allow);
        };
        if rel.eq_ignore_ascii_case("Cargo.toml") && !self.unlocked(&rel) {
            return self.root_manifest(tool, args);
        }
        Ok(if self.guarded(&rel) {
            Decision::Block(format!("{tool} on protected path {rel}"))
        } else {
            Decision::Allow
        })
    }

    /// Allows edits to the root Cargo.toml that leave its protected tables alone.
    fn root_manifest(&self, tool: &str, args: &Value) -> Result<Decision, String> {
        let old = fs::read_to_string(self.root.join("Cargo.toml")).unwrap_or_default();
        let new = match tool {
            "Write" => args["content"]
                .as_str()
                .ok_or("Write without content")?
                .to_string(),
            "Edit" => apply_edit(&old, args),
            "MultiEdit" => args["edits"]
                .as_array()
                .ok_or("MultiEdit without edits")?
                .iter()
                .fold(old.clone(), |text, edit| apply_edit(&text, edit)),
            _ => return Ok(Decision::Block(format!("{tool} on Cargo.toml"))),
        };
        Ok(
            match (
                protected_tables(self.rules, &old),
                protected_tables(self.rules, &new),
            ) {
                (Some(a), Some(b)) if a == b => Decision::Allow,
                (_, None) => {
                    Decision::Block("Cargo.toml would not parse; refusing to guess".into())
                }
                _ => Decision::Block(format!(
                    "{tool} changes a protected table of Cargo.toml ({})",
                    self.rules.cargo_toml_tables.join(", ")
                )),
            },
        )
    }
}

/// The protected tables of a Cargo.toml, or `None` if it does not parse.
pub fn protected_tables(rules: &Protected, text: &str) -> Option<Vec<Option<toml::Value>>> {
    let table: toml::Table = text.parse().ok()?;
    Some(
        rules
            .cargo_toml_tables
            .iter()
            .map(|dotted| {
                dotted
                    .split('.')
                    .try_fold(toml::Value::Table(table.clone()), |v, key| {
                        v.get(key).cloned()
                    })
            })
            .collect(),
    )
}

fn apply_edit(text: &str, edit: &Value) -> String {
    let old = edit["old_string"].as_str().unwrap_or("");
    let new = edit["new_string"].as_str().unwrap_or("");
    if old.is_empty() {
        return text.to_string();
    }
    if edit["replace_all"].as_bool() == Some(true) {
        text.replace(old, new)
    } else {
        text.replacen(old, new, 1)
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
