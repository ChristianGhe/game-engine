use std::backtrace::Backtrace;
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::Duration;

use eng_core::Tick;
use eng_schema_json::JsonValue;

use crate::GameDef;

/// Probe exit codes (spec 7.1). Each has a self-test in `game-testbed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    /// A semantic assertion failed or never held.
    AssertionFailed = 10,
    /// A replay's state hash diverged from the recording.
    Desync = 11,
    /// A panic; the hook writes `crash.json` and exits with this code.
    Panic = 12,
    /// A budget in the budgets file was exceeded.
    BudgetExceeded = 13,
    /// Bad arguments or unusable input files.
    Usage = 64,
}

impl Exit {
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Exit::Ok => "ok",
            Exit::AssertionFailed => "assertion-failed",
            Exit::Desync => "desync",
            Exit::Panic => "panic",
            Exit::BudgetExceeded => "budget-exceeded",
            Exit::Usage => "usage",
        }
    }
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        ExitCode::from(exit.code())
    }
}

/// Prints a usage error and returns [`Exit::Usage`].
pub(crate) fn usage(game: &str, message: &str) -> Exit {
    let line = format!("[E-USAGE] {message}");
    eprintln!("{line}");
    eprintln!("run `{game} help` for the command list");
    crate::log::line("error", &line);
    Exit::Usage
}

/// The `crash.json` format. Bump it when a field changes meaning.
pub(crate) const CRASH_VERSION: u64 = 1;

/// Deep recursion can make backtraces huge; longer ones are cut here.
const MAX_BACKTRACE_LINES: usize = 400;

/// What the running command is doing, for `crash.json`.
struct Context {
    args: Vec<String>,
    seed: Option<u64>,
}

static CONTEXT: Mutex<Context> = Mutex::new(Context {
    args: Vec::new(),
    seed: None,
});

/// The tick being simulated, or [`NO_TICK`]. An atomic, since it changes
/// every tick.
static TICK: AtomicU64 = AtomicU64::new(NO_TICK);
const NO_TICK: u64 = u64::MAX;

/// Starts a command: keeps its arguments and forgets the previous seed and
/// tick.
pub(crate) fn begin(args: &[String]) {
    let mut context = CONTEXT.lock().unwrap_or_else(PoisonError::into_inner);
    context.args = args.to_vec();
    context.seed = None;
    TICK.store(NO_TICK, Ordering::Relaxed);
}

/// Records the seed of the simulation being built.
pub(crate) fn note_seed(seed: u64) {
    CONTEXT.lock().unwrap_or_else(PoisonError::into_inner).seed = Some(seed);
}

/// Records the tick of the latest state: 0 once the sim is built, N from
/// the moment the commands for tick N start being applied. It stays at the
/// last tick while a command writes its outputs.
pub(crate) fn note_tick(tick: Tick) {
    TICK.store(tick.0, Ordering::Relaxed);
}

/// Takes `mutex` from the panic hook. The hook may run on a thread that
/// panicked while holding the lock, where `lock()` would deadlock, so this
/// gives up after about 100 ms. A poisoned lock is used anyway.
pub(crate) fn lock_on_panic<T>(mutex: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    for _ in 0..100 {
        match mutex.try_lock() {
            Ok(guard) => return Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => return Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(1)),
        }
    }
    None
}

/// Set by the first panic. Only that panic writes the crash file and exits.
/// A panic on another thread meanwhile is printed and then waits for the
/// exit, so two writers never truncate each other's file.
static CRASHING: AtomicBool = AtomicBool::new(false);

/// Installs a hook that reports any panic, writes
/// `crash-<game>-<pid>.json` beside the agent log (spec 6.13) and exits
/// with code 12. Nothing in the hook may panic: a panic inside a panic hook
/// aborts the process instead of exiting 12. The one known exception is
/// `std::thread::current()` for a panic in a thread-local destructor, after
/// the thread's own data is gone; std's default hook avoids that with an
/// unstable API.
pub(crate) fn install_panic_hook(game: &GameDef) {
    let name = game.name;
    let version = crate::report::build_info(game);
    std::panic::set_hook(Box::new(move |info| on_panic(name, &version, info)));
}

fn on_panic(game: &str, version: &JsonValue, info: &PanicHookInfo<'_>) -> ! {
    let line = format!("[E-PANIC] {game}: {info}");
    // `eprintln!` panics if stderr is closed, so write and ignore errors.
    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "{line}");
    if CRASHING.swap(true, Ordering::SeqCst) {
        // Another thread is writing the crash file and will exit.
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let backtrace = Backtrace::force_capture();
    let (log_path, last_log_lines) = crate::log::on_panic(&line);
    let crash = crash_json(Crash {
        game,
        version,
        info,
        backtrace: &backtrace,
        log_path: log_path.as_deref(),
        last_log_lines,
    });
    let path = crate::log::dir().join(format!("crash-{game}-{}.json", std::process::id()));
    match crate::run::write_file(&path, crash.to_pretty().as_bytes()) {
        Ok(()) => {
            let _ = writeln!(stderr, "crash: {}", path.display());
        }
        Err(e) => {
            let _ = writeln!(stderr, "[E-CRASH-JSON] {e}");
        }
    }
    if let Some(path) = log_path {
        let _ = writeln!(stderr, "log: {}", path.display());
    }
    std::process::exit(i32::from(Exit::Panic.code()))
}

struct Crash<'a> {
    game: &'a str,
    version: &'a JsonValue,
    info: &'a PanicHookInfo<'a>,
    backtrace: &'a Backtrace,
    log_path: Option<&'a Path>,
    last_log_lines: Vec<JsonValue>,
}

/// The fields are spec 6.13's (version, thread, backtrace, last log lines,
/// seed) plus the 1b plan's tick and command line, the panic itself and the
/// log file. Unknown values are `null`.
fn crash_json(c: Crash<'_>) -> JsonValue {
    let (args, seed) = match lock_on_panic(&CONTEXT) {
        Some(context) => (context.args.clone(), context.seed),
        None => (Vec::new(), None),
    };
    let tick = TICK.load(Ordering::Relaxed);
    let thread = std::thread::current();
    let location = c.info.location().map_or(JsonValue::Null, |l| {
        JsonValue::str(format!("{}:{}:{}", l.file(), l.line(), l.column()))
    });
    JsonValue::object([
        ("crash_version", JsonValue::U64(CRASH_VERSION)),
        ("game", JsonValue::str(c.game)),
        ("version", c.version.clone()),
        (
            "args",
            JsonValue::Array(args.into_iter().map(JsonValue::Str).collect()),
        ),
        ("seed", seed.map_or(JsonValue::Null, JsonValue::U64)),
        (
            "tick",
            if tick == NO_TICK {
                JsonValue::Null
            } else {
                JsonValue::U64(tick)
            },
        ),
        (
            "thread",
            JsonValue::object([
                (
                    "name",
                    thread.name().map_or(JsonValue::Null, JsonValue::str),
                ),
                ("id", JsonValue::str(format!("{:?}", thread.id()))),
            ]),
        ),
        (
            "panic",
            JsonValue::object([
                (
                    "message",
                    c.info
                        .payload_as_str()
                        .map_or(JsonValue::Null, JsonValue::str),
                ),
                ("location", location),
            ]),
        ),
        ("backtrace", backtrace_lines(&c.backtrace.to_string())),
        ("last_log_lines", JsonValue::Array(c.last_log_lines)),
        (
            "log_file",
            c.log_path
                .map_or(JsonValue::Null, |p| JsonValue::str(p.display().to_string())),
        ),
    ])
}

/// One string per line, cut to [`MAX_BACKTRACE_LINES`].
fn backtrace_lines(text: &str) -> JsonValue {
    let total = text.lines().count();
    let mut lines: Vec<JsonValue> = text
        .lines()
        .take(MAX_BACKTRACE_LINES)
        .map(|l| JsonValue::str(l.trim_end()))
        .collect();
    if total > MAX_BACKTRACE_LINES {
        lines.push(JsonValue::str(format!(
            "... {} more lines",
            total - MAX_BACKTRACE_LINES
        )));
    }
    JsonValue::Array(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_backtraces_are_cut() {
        let text: String = (0..MAX_BACKTRACE_LINES + 3)
            .map(|i| format!("  {i}: frame  \n"))
            .collect();
        let JsonValue::Array(lines) = backtrace_lines(&text) else {
            panic!("expected an array");
        };
        assert_eq!(lines.len(), MAX_BACKTRACE_LINES + 1);
        assert_eq!(lines[0], JsonValue::str("  0: frame"));
        assert_eq!(
            lines[MAX_BACKTRACE_LINES],
            JsonValue::str("... 3 more lines")
        );
        assert_eq!(backtrace_lines(""), JsonValue::Array(Vec::new()));
    }

    #[test]
    fn lock_on_panic_gives_up_instead_of_deadlocking() {
        let mutex = Mutex::new(1);
        let held = mutex.lock().unwrap();
        assert!(lock_on_panic(&mutex).is_none());
        drop(held);
        assert_eq!(lock_on_panic(&mutex).map(|g| *g), Some(1));
    }

    #[test]
    fn lock_on_panic_uses_a_poisoned_lock() {
        let mutex = Mutex::new(2);
        let _ = std::thread::scope(|s| {
            s.spawn(|| {
                let _held = mutex.lock().unwrap();
                panic!("poison the lock");
            })
            .join()
        });
        assert!(mutex.is_poisoned());
        assert_eq!(lock_on_panic(&mutex).map(|g| *g), Some(2));
    }
}
