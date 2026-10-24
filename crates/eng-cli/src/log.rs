//! The full agent log: JSON lines under `target/agent-logs/`. The last
//! [`TAIL_LINES`] records also stay in memory for `crash.json`.

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use eng_schema_json::JsonValue;

/// How many of the latest log records `crash.json` keeps.
pub(crate) const TAIL_LINES: usize = 50;

struct Log {
    path: PathBuf,
    file: File,
}

struct State {
    /// `None` until [`open`], or when the directory cannot be written.
    log: Option<Log>,
    tail: VecDeque<JsonValue>,
}

impl State {
    fn push(&mut self, level: &str, message: &str) {
        let record = JsonValue::object([
            ("ts_ms", JsonValue::U64(unix_ms())),
            ("level", JsonValue::str(level)),
            ("msg", JsonValue::str(message)),
        ]);
        if let Some(log) = self.log.as_mut() {
            let _ = writeln!(log.file, "{}", record.to_compact());
        }
        if self.tail.len() == TAIL_LINES {
            self.tail.pop_front();
        }
        self.tail.push_back(record);
    }
}

static CURRENT: Mutex<State> = Mutex::new(State {
    log: None,
    tail: VecDeque::new(),
});

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Where logs and crash files go: `$ENG_AGENT_LOG_DIR`, else
/// `target/agent-logs`.
pub(crate) fn dir() -> PathBuf {
    std::env::var_os("ENG_AGENT_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/agent-logs"))
}

/// Starts a log for this process. Logging is best-effort: if the directory
/// cannot be written, the probe still runs, and the tail is still kept.
pub(crate) fn open(game: &str, command: &str) {
    let dir = dir();
    let name = format!(
        "{game}-{}-{}-{}.jsonl",
        command.replace(' ', "-"),
        unix_ms(),
        std::process::id()
    );
    let path = dir.join(name);
    let log = fs::create_dir_all(&dir)
        .and_then(|()| File::create(&path))
        .ok()
        .map(|file| Log { path, file });
    let mut state = CURRENT.lock().unwrap_or_else(PoisonError::into_inner);
    state.log = log;
    state.tail.clear();
}

/// Appends one line. `message` keeps its `[E-...]` tag for grepping.
pub(crate) fn line(level: &str, message: &str) {
    CURRENT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(level, message);
}

/// The panic hook's access to the log: appends `message` as an error, then
/// returns the log file's path and the last [`TAIL_LINES`] records, oldest
/// first. If the lock cannot be taken in time (see
/// [`crate::exit::lock_on_panic`]), nothing is logged and both are empty.
pub(crate) fn on_panic(message: &str) -> (Option<PathBuf>, Vec<JsonValue>) {
    let Some(mut state) = crate::exit::lock_on_panic(&CURRENT) else {
        return (None, Vec::new());
    };
    state.push("error", message);
    let path = state.log.as_ref().map(|log| log.path.clone());
    (path, state.tail.iter().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_latest_lines_in_order() {
        let mut state = State {
            log: None,
            tail: VecDeque::new(),
        };
        for i in 0..TAIL_LINES + 7 {
            state.push("info", &format!("line {i}"));
        }
        assert_eq!(state.tail.len(), TAIL_LINES);
        let msg = |r: &JsonValue| r.get("msg").cloned();
        assert_eq!(msg(&state.tail[0]), Some(JsonValue::str("line 7")));
        assert_eq!(
            msg(&state.tail[TAIL_LINES - 1]),
            Some(JsonValue::str(format!("line {}", TAIL_LINES + 6)))
        );
    }
}
