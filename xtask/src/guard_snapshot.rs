//! The guard's second line (ADR-0015). Before a tool call it records the
//! protected files in the repo; after the call, whether it worked or failed,
//! it puts back any the call changed, created or deleted, whatever did it: a
//! command the first line could not read, a test, a build script. A file
//! that now matches `main` as it was when the call began is left alone,
//! since only the owner approves what is on `main`. What the call wrote, and
//! what an approved change replaced, is kept under `target/guard-quarantine/`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::guard::{Ctx, UNLOCK_FILE, protected};
use crate::guard_vcs::git_out;
use crate::util;

#[path = "guard_snapshot_files.rs"]
mod files;

use files::{blob, put_back_as, states, write_blob};

/// Ignored by git, so listed by hand. Claude Code writes the owner's
/// permission choices here.
const LOCAL_SETTINGS: &str = ".claude/settings.local.json";

/// What a protected path held.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum State {
    /// A file: its blob id and whether it is executable.
    File(String, bool),
    /// A symbolic link, and where it points.
    Link(PathBuf),
    /// A file git could not read.
    Unreadable,
}

/// Each protected path before a call (`None` where absent), and `main`'s
/// commit then, whose versions are approved.
#[derive(Serialize, Deserialize)]
struct Record {
    main: Option<String>,
    files: BTreeMap<String, Option<State>>,
}

/// Records the protected files before an allowed tool call runs. Skipped
/// while the owner's unlock file exists, and outside a git repository.
pub fn record(ctx: &Ctx, input: &Value) -> Result<(), String> {
    let (Some(id), Some(dir)) = (call_id(input), snapshots(ctx.root)) else {
        return Ok(());
    };
    if !ctx.unlocked.is_empty() || ctx.root.join(UNLOCK_FILE).is_file() {
        return Ok(());
    }
    let files = protected_files(ctx)?;
    let record = Record {
        main: git_out(
            ctx.root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/heads/main^{commit}",
            ],
        )
        .map(|s| s.trim().to_string()),
        files: states(ctx.root, &files, true),
    };
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    prune(&dir);
    let text = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{id}.json")), text).map_err(|e| format!("snapshot: {e}"))
}

/// `cargo xtask hook verify`, the `PostToolUse` and `PostToolUseFailure`
/// hook.
pub fn verify() -> ExitCode {
    let result = (|| {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| format!("reading stdin: {e}"))?;
        let input: Value = serde_json::from_str(&input).map_err(|e| format!("hook input: {e}"))?;
        let root = match std::env::var_os("CLAUDE_PROJECT_DIR") {
            Some(dir) => util::find_root(Path::new(&dir))?,
            None => util::find_root(&std::env::current_dir().map_err(|e| e.to_string())?)?,
        };
        restore(&root, &input)
    })();
    match result {
        Ok(None) => ExitCode::SUCCESS,
        Ok(Some(report)) => {
            eprintln!("[E-GUARD] {report}");
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("[E-GUARD] guard verify error: {error}");
            ExitCode::from(2)
        }
    }
}

/// Puts back what the tool call in `input` changed; `Some` reports it.
pub fn restore(root: &Path, input: &Value) -> Result<Option<String>, String> {
    let (Some(id), Some(dir)) = (call_id(input), snapshots(root)) else {
        return Ok(None);
    };
    let file = dir.join(format!("{id}.json"));
    let Ok(text) = fs::read_to_string(&file) else {
        // Not recorded: the call was refused, unlocked, or not checked.
        return Ok(None);
    };
    let record: Record = serde_json::from_str(&text).map_err(|e| format!("snapshot: {e}"))?;
    let rules = protected()?;
    let ctx = Ctx {
        rules: &rules,
        root,
        cwd: root,
        unlocked: &[],
    };
    let mut files: BTreeSet<String> = record.files.keys().cloned().collect();
    files.extend(protected_files(&ctx)?);
    let files: Vec<String> = files.into_iter().collect();
    let now = states(root, &files, false);
    let before = |f: &str| record.files.get(f).cloned().flatten();
    let after = |f: &str| now.get(f).cloned().flatten();
    let changed: Vec<&String> = files.iter().filter(|f| before(f) != after(f)).collect();
    let main = main_versions(root, record.main.as_deref(), &changed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let kept = format!("target/guard-quarantine/{stamp}-{id}");
    let quarantine = root.join(&kept);
    let (mut put_back, mut failed) = (Vec::new(), Vec::new());
    for f in changed {
        let (was, is) = (before(f), after(f));
        if f == LOCAL_SETTINGS && permissions_only(root, was.as_ref(), is.as_ref()) {
            continue;
        }
        if approved(f, is.as_ref(), &main) {
            // Perhaps the owner's work: keep the version this replaced.
            if let Some(State::File(oid, _)) = &was {
                let _ = write_blob(root, &quarantine.join("replaced").join(f), oid, false);
            }
            continue;
        }
        if matches!(is, Some(State::File(..))) {
            let copy = quarantine.join(f);
            if let Some(parent) = copy.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(root.join(f), &copy);
        }
        match put_back_as(root, f, was.as_ref()) {
            Ok(()) => put_back.push(f.clone()),
            Err(e) => failed.push(format!("{f} ({e})")),
        }
    }
    let _ = fs::remove_file(&file);
    let tool = input["tool_name"].as_str().unwrap_or("the tool call");
    let mut report = Vec::new();
    if !put_back.is_empty() {
        report.push(format!(
            "{tool} changed protected {} {}; the guard put back the version from before the call \
             (what it wrote is in {kept}/). Protected files change only with the owner \
             (AGENTS.md, \"Protected paths\").",
            if put_back.len() == 1 { "path" } else { "paths" },
            put_back.join(", ")
        ));
    }
    if !failed.is_empty() {
        report.push(format!(
            "{tool} changed protected paths the guard could not put back: {}. Stop and tell the owner.",
            failed.join(", ")
        ));
    }
    Ok((!report.is_empty()).then(|| report.join("\n")))
}

/// The tool call's id, safe as a file name.
fn call_id(input: &Value) -> Option<String> {
    let id = input["tool_use_id"].as_str()?;
    let safe: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .collect();
    (!safe.is_empty()).then_some(safe)
}

/// Where snapshots live: beside the guard binary, which agents cannot write.
fn snapshots(root: &Path) -> Option<PathBuf> {
    let common = git_out(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    Some(PathBuf::from(common.trim_end()).join("eng-hooks/snapshots"))
}

/// Snapshots older than a day belong to calls that never finished.
fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let day = Duration::from_secs(24 * 60 * 60);
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > day);
        if old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// The protected files now: tracked or new (not ignored) ones, the owner's
/// unlock file and local settings, and git's hooks.
fn protected_files(ctx: &Ctx) -> Result<Vec<String>, String> {
    let listed = git_out(
        ctx.root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )
    .ok_or("git ls-files failed")?;
    let mut files: Vec<String> = listed
        .split('\0')
        .filter(|f| !f.is_empty() && ctx.guarded(f))
        .map(String::from)
        .collect();
    files.push(UNLOCK_FILE.to_string());
    files.push(LOCAL_SETTINGS.to_string());
    if let Ok(entries) = fs::read_dir(ctx.root.join(".git/hooks")) {
        for entry in entries.flatten() {
            files.push(format!(
                ".git/hooks/{}",
                entry.file_name().to_string_lossy()
            ));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Blob ids of the `files` on commit `main`; a file it lacks maps to `None`.
fn main_versions<'a>(
    root: &Path,
    main: Option<&str>,
    files: &[&'a String],
) -> BTreeMap<&'a str, Option<String>> {
    let Some(main) = main else {
        return BTreeMap::new(); // no main: nothing is approved
    };
    let mut args = vec!["ls-tree", "-z", main, "--"];
    args.extend(files.iter().map(|f| f.as_str()));
    let Some(listed) = git_out(root, &args) else {
        return BTreeMap::new();
    };
    let mut found: BTreeMap<&str, Option<String>> =
        files.iter().map(|f| (f.as_str(), None)).collect();
    for entry in listed.split('\0') {
        if let Some((meta, path)) = entry.split_once('\t')
            && let Some(oid) = meta.split_whitespace().nth(2)
            && let Some(slot) = found.get_mut(path)
        {
            *slot = Some(oid.to_string());
        }
    }
    found
}

/// Whether `f` is now as it was on `main` when the call began. Files git
/// does not track (the unlock file, git's hooks, the local settings) never
/// are.
fn approved(f: &str, is: Option<&State>, main: &BTreeMap<&str, Option<String>>) -> bool {
    if f.starts_with(".git/") || f == UNLOCK_FILE || f == LOCAL_SETTINGS {
        return false;
    }
    match (main.get(f), is) {
        (Some(None), None) => true,
        (Some(Some(oid)), Some(State::File(now, _))) => oid == now,
        _ => false,
    }
}

/// Whether the local settings changed only in their permission rules, as
/// when the owner answers a prompt with "don't ask again". Those rules
/// cannot switch the guard off; hooks and other settings can.
fn permissions_only(root: &Path, was: Option<&State>, is: Option<&State>) -> bool {
    let text = |state: Option<&State>, now: bool| match state {
        None => Some("{}".to_string()),
        Some(State::File(_, _)) if now => fs::read_to_string(root.join(LOCAL_SETTINGS)).ok(),
        Some(State::File(oid, _)) => blob(root, oid)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned()),
        Some(_) => None,
    };
    let rest = |text: Option<String>| {
        let mut settings: Value = serde_json::from_str(&text?).ok()?;
        settings.as_object_mut()?.remove("permissions");
        Some(settings)
    };
    matches!((rest(text(was, false)), rest(text(is, true))), (Some(a), Some(b)) if a == b)
}

#[cfg(test)]
#[path = "guard_snapshot_tests.rs"]
mod tests;
