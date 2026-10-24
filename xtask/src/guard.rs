//! The Claude Code `PreToolUse` guard (spec 7.3).
//!
//! - Blocks Edit/Write/MultiEdit/NotebookEdit on protected paths, and edits
//!   that change the protected tables of the root `Cargo.toml`.
//! - Refuses Bash commands that write to protected paths or "bless"
//!   snapshots and expected output (`guard_bash.rs`, `guard_tools.rs`,
//!   `guard_code.rs`), that bring a protected file back from another commit
//!   (`guard_vcs.rs`), that set git up to run other programs
//!   (`guard_settings.rs`), or that only the owner runs: pushes, GitHub
//!   writes, jj history (`guard_owner.rs`).
//! - Fails closed: any error exits with 2, which blocks the tool call.
//!
//! With commits straight to `main` (ADR-0014, ADR-0015) this is the only
//! local gate; CI's `oracle-check` applies to pull requests alone. It runs as
//! the agent's own OS user, so a determined agent can defeat it: it catches
//! accidents and the common forms.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use serde::Deserialize;
use serde_json::Value;

use crate::guard_vcs::git_out;
use crate::{glob, guard_snapshot, util};

pub const PROTECTED_TOML: &str = include_str!("../../protected.toml");
pub const UNLOCK_FILE: &str = ".claude/guard-unlock";

/// Paths the guard protects for its own sake: its binary, the sources it is
/// built from, the owner's unlock file, and git's settings and hooks, which
/// make git run other programs.
const GUARD_PATHS: [&str; 10] = [
    ".git/eng-hooks/**",
    "target/hooks-src/**",
    "target/hooks-build/**",
    // Cargo reads config from every parent of the hook build.
    "target/.cargo/**",
    ".claude/guard-unlock",
    ".git/hooks/**",
    ".git/config",
    ".git/config.worktree",
    ".git/worktrees/*/config.worktree",
    ".jj/repo/config.toml",
];

/// How an MCP tool's name starts when it only reads: `read_file`, `list_dir`.
const MCP_READS: [&str; 13] = [
    "read", "get", "list", "search", "find", "query", "view", "show", "describe", "grep", "glob",
    "stat", "inspect",
];

/// Files outside the repo through which an agent could switch the guard off
/// or have Claude Code, git, jj or the shell run its programs: Claude Code's
/// settings and plugins, git's and jj's user config (jj keeps each repo's
/// config there too), and shell startup files.
fn outside_paths() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".config")));
    let claude = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .into_iter()
        .chain(home.as_ref().map(|h| h.join(".claude")));
    let mut out = Vec::new();
    for dir in claude {
        for name in [
            "settings.json",
            "settings.local.json",
            "managed-settings.json",
            ".claude.json",
            "plugins",
            "hooks",
        ] {
            out.push(dir.join(name));
        }
    }
    if let Some(home) = &home {
        for name in [
            ".claude.json",
            ".gitconfig",
            ".zshenv",
            ".zshrc",
            ".zprofile",
            ".zlogin",
            ".zlogout",
            ".bashrc",
            ".bash_profile",
            ".bash_login",
            ".bash_logout",
            ".profile",
        ] {
            out.push(home.join(name));
        }
    }
    out.extend(config.iter().flat_map(|c| [c.join("git"), c.join("jj")]));
    out
}

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
            eprintln!(
                "[E-GUARD] guard error, failing closed: {error}\nIf this repeats, stop and tell \
                 the owner: they can fix it, or pause the guard with .claude/guard-unlock."
            );
            ExitCode::from(2)
        }
    }
}

/// Whether `tool` is an MCP tool whose name says it only reads.
fn mcp_read(tool: &str) -> bool {
    let Some(rest) = tool.strip_prefix("mcp__") else {
        return false;
    };
    let action = rest.rsplit("__").next().unwrap_or(rest).to_lowercase();
    MCP_READS
        .iter()
        .any(|v| action == *v || action.starts_with(&format!("{v}_")))
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
    let rules = protected()?;
    let decision = decide(&rules, &root, &cwd, &input, &unlocked)?;
    if decision == Decision::Allow {
        let ctx = Ctx {
            rules: &rules,
            root: &root,
            cwd: &cwd,
            unlocked: &unlocked,
        };
        guard_snapshot::record(&ctx, &input)?;
    }
    Ok(decision)
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
        _ if mcp_read(tool) => Ok(Decision::Allow),
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

    /// The guarded path `raw` names from `cwd`, if any: repo-relative, or
    /// one of the files outside the repo that the guard watches.
    pub fn guarded_path(&self, cwd: &Path, raw: &str) -> Option<String> {
        match self.resolve_in(cwd, raw) {
            Some(rel) => self.guarded(&rel).then_some(rel),
            None => self.outside_guarded(&cwd.join(expand_home(raw))),
        }
    }

    /// The watched file outside the repo that `path` is or lies under.
    fn outside_guarded(&self, path: &Path) -> Option<String> {
        let real = |p: &Path| {
            let p = normalize(p);
            fs::canonicalize(&p)
                .ok()
                .or_else(|| Some(fs::canonicalize(p.parent()?).ok()?.join(p.file_name()?)))
                .unwrap_or(p)
        };
        let path = real(path);
        outside_paths()
            .iter()
            .map(|p| real(p))
            .find(|p| path.starts_with(p))
            .map(|p| p.display().to_string())
    }

    /// Whether removing or moving the directory `rel` would take guarded
    /// files with it.
    pub fn guards_below(&self, rel: &str) -> bool {
        let prefix = match rel.trim_end_matches('/') {
            "" | "." => String::new(),
            dir => format!("{dir}/"),
        };
        if GUARD_PATHS
            .iter()
            .any(|p| p.to_lowercase().starts_with(&prefix.to_lowercase()))
        {
            return true;
        }
        let spec = if prefix.is_empty() {
            "."
        } else {
            prefix.as_str()
        };
        match git_out(self.root, &["ls-files", "-z", "--", spec]) {
            Some(files) => files.split('\0').any(|f| !f.is_empty() && self.guarded(f)),
            // Without git, assume a protected file may be anywhere below.
            None => true,
        }
    }

    fn file_write(&self, tool: &str, path: &str, args: &Value) -> Result<Decision, String> {
        let Some(rel) = self.resolve_in(self.cwd, path) else {
            return Ok(
                match self.outside_guarded(&self.cwd.join(expand_home(path))) {
                    Some(watched) => Decision::Block(format!(
                        "{tool} on {watched}, which can switch the guard off or make tools run programs"
                    )),
                    None => Decision::Allow,
                },
            );
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

#[cfg(test)]
#[path = "guard_vcs_tests.rs"]
mod vcs_tests;
