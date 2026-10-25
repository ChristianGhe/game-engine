//! Shared helpers: the repo root, running commands quietly, timed steps.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Finds the repo root: the nearest ancestor holding `architecture.toml`.
pub fn find_root(start: &Path) -> Result<PathBuf, String> {
    let start = fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    start
        .ancestors()
        .find(|dir| dir.join("architecture.toml").is_file() && dir.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("no repo root (architecture.toml) above {}", start.display()))
}

pub fn root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("current dir: {e}"))?;
    find_root(&cwd)
}

/// True on CI: adds `--locked` and denies warnings.
pub fn on_ci() -> bool {
    std::env::var_os("CI").is_some()
}

/// `cargo` in `root`, with the version stamp's source-tree values set
/// (`stamp.rs`). Every cargo command xtask runs starts here, so its builds
/// carry the same values and do not rebuild `eng-cli` for each other.
pub fn cargo(root: &Path) -> Command {
    let mut cmd = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cmd.current_dir(root);
    cmd.envs(crate::stamp::vars(root).iter().map(|(k, v)| (k, v)));
    cmd
}

/// Runs `cmd`, keeping its output out of the console unless it fails, and
/// returns its stdout. The full output goes to
/// `target/agent-logs/xtask-<name>.log`.
pub fn run_quiet(root: &Path, name: &str, cmd: &mut Command) -> Result<String, String> {
    let shown = format!("{cmd:?}");
    let out = cmd
        .output()
        .map_err(|e| format!("cannot start {shown}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let text = format!("{stdout}{}", String::from_utf8_lossy(&out.stderr));
    let log_dir = root.join("target/agent-logs");
    let _ = fs::create_dir_all(&log_dir);
    let log = log_dir.join(format!("xtask-{}.log", name.replace([' ', ':', '/'], "-")));
    let _ = fs::write(&log, format!("$ {shown}\n{text}"));
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(format!(
            "{} failed ({}); full log: {}\n{}",
            name,
            out.status,
            log.display(),
            tail(&text, 30)
        ))
    }
}

pub fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// Repo-relative path with forward slashes.
pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every file under `dir` (sorted), skipping nothing but `.git`. For source
/// scans, where a module directory may be called anything.
pub fn walk_all(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if entry.file_name() != ".git" {
                    stack.push(path);
                }
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Every file under `dir` (sorted), skipping `target` and hidden dirs.
pub fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if name != "target" && !name.starts_with('.') {
                    stack.push(path);
                }
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub ok: bool,
    pub seconds: f64,
    pub detail: String,
}

/// Runs named steps, prints one line each, and keeps going after failures.
#[derive(Default)]
pub struct Steps {
    pub results: Vec<StepResult>,
}

impl Steps {
    pub fn run(&mut self, name: &str, f: impl FnOnce() -> Result<String, String>) -> bool {
        let started = Instant::now();
        let outcome = f();
        let seconds = started.elapsed().as_secs_f64();
        let (ok, detail) = match outcome {
            Ok(detail) => (true, detail),
            Err(detail) => (false, detail),
        };
        let status = if ok { "ok" } else { "FAILED" };
        println!("{name:.<40} {status} ({seconds:.1}s)");
        if !ok {
            println!("{detail}");
        }
        self.results.push(StepResult {
            name: name.to_string(),
            ok,
            seconds,
            detail,
        });
        ok
    }

    pub fn ok(&self) -> bool {
        self.results.iter().all(|r| r.ok)
    }
}
