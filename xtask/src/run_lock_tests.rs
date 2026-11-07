//! Tests for the run lock (amendment 98). Each uses a lock file of its own
//! under the system's temp directory, never the checkout's: `quick` runs
//! these tests while it holds that one.

use std::fs::{self, OpenOptions, TryLockError};
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use std::ffi::OsStr;

use super::{acquire, hold, holder, holds, nested, try_acquire};

/// A lock path of the test's own, in a directory that does not exist yet.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-run-lock-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir.join("xtask").join("run.lock")
}

/// Whether another open of `path` could take the lock right now.
fn is_free(path: &Path) -> bool {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("the lock file exists");
    match file.try_lock() {
        Ok(()) => true,
        Err(TryLockError::WouldBlock) => false,
        Err(TryLockError::Error(e)) => panic!("try_lock {}: {e}", path.display()),
    }
}

/// The scratch directory `path` sits two levels below.
fn clean(path: &Path) {
    let _ = fs::remove_dir_all(path.parent().and_then(Path::parent).expect("scratch"));
}

#[test]
fn the_line_names_the_file_and_the_holder() {
    let path = Path::new("target/xtask/run.lock");
    assert_eq!(
        holds(path, Some(42)),
        "another xtask run (pid 42) holds target/xtask/run.lock"
    );
    assert_eq!(
        holds(path, None),
        "another xtask run holds target/xtask/run.lock"
    );
}

/// Two holders in one process: the second says once that it waits, and
/// runs only after the first lets go. No sleeps: the second's line arrives
/// only once it has found the lock taken.
#[test]
fn a_second_holder_waits_for_the_first() {
    let path = scratch("two");
    let first = acquire(&path, |line| panic!("the lock was free: {line}"));
    let released = Arc::new(AtomicBool::new(false));
    let (said, heard) = mpsc::channel();
    let second = thread::spawn({
        let (path, released) = (path.clone(), Arc::clone(&released));
        move || {
            let say = |line: &str| said.send(line.to_string()).expect("the test listens");
            hold(acquire(&path, say), || released.load(Ordering::SeqCst))
        }
    });
    let line = heard.recv().expect("the second holder says it waits");
    // Windows refuses to read a locked file, so only Unix names the PID.
    let pid = cfg!(unix).then(std::process::id);
    assert_eq!(line, format!("waiting: {}", holds(&path, pid)));
    assert!(!is_free(&path), "the first still holds it");
    released.store(true, Ordering::SeqCst);
    drop(first);
    let ran_after = second.join().expect("the second holder did not panic");
    assert!(ran_after, "the second ran before the first let go");
    assert!(heard.try_recv().is_err(), "more than one waiting line");
    assert!(is_free(&path), "the second let go too");
    clean(&path);
}

/// What the Stop hook's `quick` uses: a held lock is reported at once,
/// never waited for, and a free one is taken.
#[test]
fn try_acquire_never_waits() {
    let path = scratch("try");
    let first = try_acquire(&path, |line| panic!("unexpected: {line}"));
    assert!(first.is_ok(), "the lock was free");
    let busy = try_acquire(&path, |line| panic!("unexpected: {line}"));
    let pid = cfg!(unix).then(std::process::id);
    assert_eq!(busy.err(), Some(pid), "a held lock, and its holder");
    drop(first);
    let again = try_acquire(&path, |line| panic!("unexpected: {line}"));
    assert!(again.is_ok(), "free again once the first let go");
    drop(again);
    clean(&path);
}

/// An xtask started by a run that holds the same lock fails at once; one
/// under another checkout's run, or under none, goes ahead.
#[test]
fn only_a_run_under_its_own_lock_is_nested() {
    let path = Path::new("/repo/target/xtask/run.lock");
    assert_eq!(nested(None, path), None);
    let other = OsStr::new("/other/target/xtask/run.lock");
    assert_eq!(nested(Some(other), path), None);
    let error = nested(Some(path.as_os_str()), path).expect("nested");
    assert!(
        error.starts_with("[E-LOCK-NESTED]") && error.contains(&path.display().to_string()),
        "{error}"
    );
}

#[test]
fn the_lock_is_held_while_the_closure_runs_and_free_after_it_returns() {
    let path = scratch("returns");
    let held = acquire(&path, |line| panic!("the lock was free: {line}"));
    let (free, pid) = hold(held, || (is_free(&path), holder(&path)));
    assert!(!free, "the lock is held while the closure runs");
    assert!(is_free(&path), "and free once it returns");
    #[cfg(unix)]
    {
        assert_eq!(pid, Some(std::process::id()), "the file names the holder");
        assert_eq!(holder(&path), None, "and is cleared when it lets go");
    }
    #[cfg(not(unix))]
    let _ = pid;
    clean(&path);
}

/// Closing the file would release the lock without `Held`'s `Drop`; the
/// PID cleared shows that `Drop` ran on the way out of the panic.
#[test]
fn the_lock_is_free_after_the_closure_panics() {
    let path = scratch("panics");
    let caught = panic::catch_unwind(|| {
        hold::<()>(acquire(&path, |_| {}), || {
            assert!(!is_free(&path), "held inside");
            panic!("a step panicked")
        })
    });
    assert!(caught.is_err(), "the panic reaches the caller");
    assert!(is_free(&path), "and the lock is released on the way out");
    #[cfg(unix)]
    assert_eq!(
        holder(&path),
        None,
        "by `Held`'s `Drop`, which clears the PID"
    );
    clean(&path);
}

/// Where the lock file cannot be made (here: a file stands where its
/// directory goes), the run goes ahead unlocked, as before amendment 98,
/// and says so. A file system without locks takes the same way out of
/// `try_lock`, which this test cannot make fail.
#[test]
fn a_lock_file_that_cannot_be_made_says_so_and_runs_anyway() {
    let path = scratch("blocked");
    let dir = path.parent().expect("has a directory");
    fs::create_dir_all(dir.parent().expect("scratch")).expect("scratch directory");
    fs::write(dir, "not a directory").expect("blocking file");
    let mut said = Vec::new();
    let held = acquire(&path, |line| said.push(line.to_string()));
    let ran = hold(held, || true);
    assert!(ran, "the closure ran");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with(&format!("cannot lock {}", path.display()))
            && said[0].ends_with("running without the lock"),
        "{said:?}"
    );
    let _ = fs::remove_dir_all(dir.parent().expect("scratch"));
}

/// The lock file, for the child process of `a_killed_holder_leaves_no_stale_lock`.
#[cfg(unix)]
const CHILD_LOCK: &str = "XTASK_RUN_LOCK_TEST_CHILD";
/// What that child prints once it holds the lock.
#[cfg(unix)]
const CHILD_HELD: &str = "run-lock-test: held";

/// A holder killed in the middle of its run leaves no stale lock: the OS
/// releases it with the process. Its PID stays in the file, where only a
/// waiting line would read it. Unix only: Windows releases a dead
/// process's locks too, but documents no bound on when.
#[cfg(unix)]
#[test]
fn a_killed_holder_leaves_no_stale_lock() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let path = scratch("killed");
    let mut child = Command::new(std::env::current_exe().expect("the test binary"))
        .args(["--exact", "run_lock::tests::child_holds_the_lock"])
        .args(["--ignored", "--nocapture"])
        .env(CHILD_LOCK, &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the test binary starts");
    let mut lines = BufReader::new(child.stdout.take().expect("piped")).lines();
    let held = lines
        .by_ref()
        .map_while(Result::ok)
        .any(|line| line.contains(CHILD_HELD));
    assert!(held, "the child never said it holds the lock");
    assert!(!is_free(&path), "the child holds it");
    assert_eq!(holder(&path), Some(child.id()));
    child.kill().expect("kill the child");
    child.wait().expect("reap the child");
    assert!(is_free(&path), "the lock went with the killed process");
    assert_eq!(holder(&path), Some(child.id()), "its PID stays behind");
    drop(lines);
    clean(&path);
}

/// Run by `a_killed_holder_leaves_no_stale_lock` in a child process; does
/// nothing without `CHILD_LOCK`.
#[cfg(unix)]
#[test]
#[ignore = "a helper: a_killed_holder_leaves_no_stale_lock runs it in a child process"]
fn child_holds_the_lock() {
    use std::io::{Read, Write};

    let Some(path) = std::env::var_os(CHILD_LOCK) else {
        return;
    };
    let _held = acquire(Path::new(&path), |line| panic!("{line}"));
    println!("\n{CHILD_HELD}");
    let _ = std::io::stdout().flush();
    // Held until killed, or until the parent's end of stdin closes.
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
}
