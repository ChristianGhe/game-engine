//! One xtask run at a time in a checkout (rung 1b, amendment 98).
//!
//! Sessions working in one checkout share `target/`. On 2026-11-01 one
//! session's build replaced `target/debug/game-testbed` while another's
//! `quick` was running its tests, and `quick` went red for no code reason.
//! The commands that build into `target/` and run what they built
//! (`quick`, `ci --job`, `replay` and `selftest`) therefore hold an
//! exclusive lock on [`PATH`] for their whole run, taken in `main` once
//! the arguments are known to be good. A second run prints one line naming
//! the file, and the holder's PID where the platform lets it read the file
//! (not on Windows), then waits for the first to end. With [`NO_WAIT`] set
//! it exits [`BUSY_EXIT`] instead; the Stop hook (`hooks.rs`) never waits.
//! A plain `cargo` command takes no lock and can still replace a binary
//! under a running xtask.
//!
//! The lock is the operating system's (`File::lock`: `flock` on Unix,
//! `LockFileEx` on Windows) and belongs to the open file, so it ends with
//! the process, however the process ends: a return, a failed step, a
//! panic, Ctrl-C or a kill. It cannot go stale. The file stays behind and
//! holds no lock; a PID a killed run left in it is only ever printed.
//! std opens files close-on-exec (not inheritable on Windows), so the
//! cargo and testbed processes a run starts never hold the lock.
//!
//! Two opens of the file conflict even within one process, so an xtask
//! that a holder starts and that locks too would wait for itself forever.
//! `util::cargo` therefore passes the held lock on in [`HELD_BY`], and a
//! run that finds its own lock there fails at once. The Stop hook takes no
//! lock for the same reason: the `cargo xtask quick` it starts does.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::util;

/// The lock file, under the repo root.
pub const PATH: &str = "target/xtask/run.lock";

/// Set (to anything) for a run that must not wait: the Stop hook's `quick`.
pub const NO_WAIT: &str = "ENG_XTASK_NO_WAIT";

/// The exit code of a run that found the lock held and, under [`NO_WAIT`],
/// did not wait (`EX_TEMPFAIL`).
pub const BUSY_EXIT: u8 = 75;

/// The lock file of the run that started this process, set by
/// `util::cargo` for every cargo command a holder runs.
pub const HELD_BY: &str = "ENG_XTASK_LOCK_HELD";

/// The lock this process holds through [`run`]; `main` exits once `run`
/// returns, so it is never set for longer than the lock is held.
static HOLDING: OnceLock<PathBuf> = OnceLock::new();

/// The lock file this process holds through [`run`], if any.
pub fn holding() -> Option<&'static Path> {
    HOLDING.get().map(PathBuf::as_path)
}

/// `f` under the lock of the checkout around the current directory, with
/// the waiting line on stderr. Under [`NO_WAIT`] a held lock ends the
/// process with [`BUSY_EXIT`] instead of waiting.
pub fn run(f: impl FnOnce() -> Result<String, String>) -> Result<String, String> {
    let path = util::root()?.join(PATH);
    if let Some(error) = nested(std::env::var_os(HELD_BY).as_deref(), &path) {
        return Err(error);
    }
    let say = |line: &str| eprintln!("{line}");
    let held = if std::env::var_os(NO_WAIT).is_some() {
        try_acquire(&path, say).unwrap_or_else(|pid| {
            eprintln!("not waiting ({NO_WAIT} is set): {}", holds(&path, pid));
            std::process::exit(BUSY_EXIT.into())
        })
    } else {
        acquire(&path, say)
    };
    if held.0.is_some() {
        let _ = HOLDING.set(path);
    }
    hold(held, f)
}

/// `f` while `held` is held, released when `f` returns or panics.
pub fn hold<T>(held: Held, f: impl FnOnce() -> T) -> T {
    let _held = held;
    f()
}

/// The lock on a file, released when dropped; `None` if it could not be
/// taken.
pub struct Held(Option<File>);

impl Held {
    /// Writes this process's PID into the locked file. Best effort: the PID
    /// only names the holder in another run's line.
    fn new(mut file: File) -> Self {
        let _ = file
            .set_len(0)
            .and_then(|()| file.rewind())
            .and_then(|()| writeln!(file, "{}", std::process::id()));
        Held(Some(file))
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(file) = &self.0 {
            // Clear the PID, so that no later waiter names this process.
            // Closing the file releases the lock even if `unlock` fails.
            let _ = file.set_len(0);
            let _ = file.unlock();
        }
    }
}

/// Takes the lock on `path`, creating the file and its directory, and
/// waits while another holder has it. `say` gets the line to print if
/// another holder must end first, or if the lock cannot be taken here at
/// all; then the run goes ahead without it, as every run did before the
/// lock existed.
pub fn acquire(path: &Path, mut say: impl FnMut(&str)) -> Held {
    let pid = match try_acquire(path, &mut say) {
        Ok(held) => return held,
        Err(pid) => pid,
    };
    say(&format!("waiting: {}", holds(path, pid)));
    match open(path).and_then(|file| file.lock().map(|()| file)) {
        Ok(file) => Held::new(file),
        Err(e) => unavailable(path, &e, &mut say),
    }
}

/// [`acquire`] without waiting: `Err` with the holder's PID, where the
/// platform lets it be read, if another holder has the lock.
pub fn try_acquire(path: &Path, mut say: impl FnMut(&str)) -> Result<Held, Option<u32>> {
    let file = match open(path) {
        Ok(file) => file,
        Err(e) => return Ok(unavailable(path, &e, &mut say)),
    };
    match file.try_lock() {
        Ok(()) => Ok(Held::new(file)),
        Err(TryLockError::WouldBlock) => Err(holder(path)),
        Err(TryLockError::Error(e)) => Ok(unavailable(path, &e, &mut say)),
    }
}

fn open(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    // Not truncated on open: the holder's PID stays until the lock is ours.
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

/// No lock here (a file system without locks, a `target/` that cannot be
/// written): say so and run without it.
fn unavailable(path: &Path, e: &io::Error, say: &mut impl FnMut(&str)) -> Held {
    say(&format!(
        "cannot lock {} ({e}); running without the lock",
        path.display()
    ));
    Held(None)
}

/// The PID in the lock file, if it holds one. Windows refuses to read a
/// file that another handle has locked, so there it is `None` while held.
pub fn holder(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Who holds the lock on `path`, for the one line a run prints.
pub fn holds(path: &Path, pid: Option<u32>) -> String {
    match pid {
        Some(pid) => format!("another xtask run (pid {pid}) holds {}", path.display()),
        None => format!("another xtask run holds {}", path.display()),
    }
}

/// The error for a run whose lock `path` is `marker`, the lock of the run
/// that started it ([`HELD_BY`]): waiting for its parent would never end.
pub fn nested(marker: Option<&OsStr>, path: &Path) -> Option<String> {
    (Path::new(marker?) == path).then(|| {
        format!(
            "[E-LOCK-NESTED] the xtask run that started this one holds {}, so this one \
             would wait for it forever; call the step's function instead",
            path.display()
        )
    })
}

#[cfg(test)]
#[path = "run_lock_tests.rs"]
mod tests;
