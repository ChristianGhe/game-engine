//! The second line's file work: what protected paths hold, and putting
//! them back without following links out of the repo.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::State;

/// What each of `files` (relative to `root`) holds; `write` stores the
/// blobs, so they can be put back. A path reached through a link, or with a
/// file where one of its folders should be, counts as absent.
pub(super) fn states(
    root: &Path,
    files: &[String],
    write: bool,
) -> BTreeMap<String, Option<State>> {
    let mut out = BTreeMap::new();
    let mut plain = Vec::new();
    for f in files {
        let path = root.join(f);
        let state = match fs::symlink_metadata(&path) {
            _ if !folders_plain(root, f) => None,
            Ok(m) if m.is_file() => {
                plain.push(f);
                continue;
            }
            Ok(m) if m.file_type().is_symlink() => {
                Some(State::Link(fs::read_link(&path).unwrap_or_default()))
            }
            _ => None,
        };
        out.insert(f.clone(), state);
    }
    for (f, id) in hashes(root, &plain, write) {
        let state = match id {
            Some(id) => State::File(id, executable(&root.join(f))),
            None => State::Unreadable,
        };
        out.insert(f.clone(), Some(state));
    }
    out
}

/// Whether every folder on the way from `root` to `rel` is a real folder.
fn folders_plain(root: &Path, rel: &str) -> bool {
    let mut at = root.to_path_buf();
    let mut parts: Vec<&str> = rel.split('/').collect();
    parts.pop();
    parts.into_iter().all(|part| {
        at.push(part);
        fs::symlink_metadata(&at).is_ok_and(|m| m.is_dir())
    })
}

/// Blob ids of `files`, `None` for one git cannot read.
fn hashes<'a>(root: &Path, files: &[&'a String], write: bool) -> Vec<(&'a String, Option<String>)> {
    let mut args = vec!["hash-object", "--no-filters"];
    if write {
        args.push("-w");
    }
    // One git for the usual names: `--stdin-paths` reads one per line.
    let (batch, mut single): (Vec<&String>, Vec<&String>) =
        files.iter().copied().partition(|f| !f.contains('\n'));
    let names: String = batch.iter().map(|f| format!("{f}\n")).collect();
    let ids = (!batch.is_empty())
        .then(|| {
            git_with_input(
                root,
                &[&args[..], &["--stdin-paths"]].concat(),
                names.as_bytes(),
            )
        })
        .and_then(Result::ok)
        .map(|out| {
            String::from_utf8_lossy(&out)
                .lines()
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .filter(|ids| ids.len() == batch.len());
    let mut out = Vec::new();
    match ids {
        Some(ids) => out.extend(batch.into_iter().zip(ids.into_iter().map(Some))),
        // An unreadable file fails the batch: ask about each in turn.
        None => single.extend(batch),
    }
    for f in single {
        let id = git_with_input(root, &[&args[..], &["--", f.as_str()]].concat(), b"")
            .ok()
            .map(|out| String::from_utf8_lossy(&out).trim().to_string())
            .filter(|id| !id.is_empty());
        out.push((f, id));
    }
    out
}

/// Puts `f` back as it `was` before the call.
pub(super) fn put_back_as(root: &Path, f: &str, was: Option<&State>) -> Result<(), String> {
    let path = root.join(f);
    match was {
        Some(State::Unreadable) => Err("it could not be read before the call".into()),
        None => clear(root, f),
        Some(State::File(oid, executable)) => {
            clear(root, f)?;
            write_blob(root, &path, oid, *executable)
        }
        Some(State::Link(target)) => {
            clear(root, f)?;
            link(target, &path)
        }
    }
}

/// Removes what stands at `rel`, and any link or file where one of its
/// folders should be, so that writing it cannot reach outside the repo.
fn clear(root: &Path, rel: &str) -> Result<(), String> {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut at = root.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        at.push(part);
        let removed = match fs::symlink_metadata(&at) {
            Err(_) => return Ok(()),
            Ok(m) if m.is_dir() && i + 1 < parts.len() => continue,
            Ok(m) if m.is_dir() => fs::remove_dir_all(&at),
            // A link is removed itself, never what it points to.
            Ok(_) => fs::remove_file(&at),
        };
        removed.map_err(|e| format!("{}: {e}", at.display()))?;
    }
    Ok(())
}

pub(super) fn blob(root: &Path, oid: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "blob", oid])
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("git cat-file: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!("cannot read back blob {oid}"))
    }
}

/// Writes blob `oid` to `path`.
pub(super) fn write_blob(
    root: &Path,
    path: &Path,
    oid: &str,
    executable: bool,
) -> Result<(), String> {
    let content = blob(root, oid)?;
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    set_executable(path, executable);
    Ok(())
}

fn git_with_input(root: &Path, args: &[&str], input: &[u8]) -> Result<Vec<u8>, String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("git: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("git stdin")?
        .write_all(input)
        .map_err(|e| format!("git stdin: {e}"))?;
    let out = child.wait_with_output().map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!("git {} failed", args.join(" ")))
    }
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(_: &Path) -> bool {
    false
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        let mode = perms.mode();
        perms.set_mode(if executable {
            mode | 0o755
        } else {
            mode & !0o111
        });
        let _ = fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
fn set_executable(_: &Path, _: bool) {}

#[cfg(unix)]
fn link(target: &Path, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    std::os::unix::fs::symlink(target, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(not(unix))]
fn link(_: &Path, path: &Path) -> Result<(), String> {
    Err(format!("{}: cannot make a link here", path.display()))
}
