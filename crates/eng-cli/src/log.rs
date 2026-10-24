//! The full agent log: JSON lines under `target/agent-logs/`.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use eng_schema_json::JsonValue;

struct Log {
    path: PathBuf,
    file: File,
}

static CURRENT: Mutex<Option<Log>> = Mutex::new(None);

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Starts a log for this process. Logging is best-effort: if the directory
/// cannot be written, the probe still runs.
pub(crate) fn open(game: &str, command: &str) {
    let dir = std::env::var_os("ENG_AGENT_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/agent-logs"));
    let name = format!(
        "{game}-{}-{}-{}.jsonl",
        command.replace(' ', "-"),
        unix_ms(),
        std::process::id()
    );
    let path = dir.join(name);
    let opened = fs::create_dir_all(&dir).and_then(|()| File::create(&path));
    if let Ok(file) = opened {
        *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Log { path, file });
    }
}

pub(crate) fn path() -> Option<PathBuf> {
    let guard = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map(|log| log.path.clone())
}

/// Appends one line. `message` keeps its `[E-...]` tag for grepping.
pub(crate) fn line(level: &str, message: &str) {
    let mut guard = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(log) = guard.as_mut() {
        let record = JsonValue::object([
            ("ts_ms", JsonValue::U64(unix_ms())),
            ("level", JsonValue::str(level)),
            ("msg", JsonValue::str(message)),
        ]);
        let _ = writeln!(log.file, "{}", record.to_compact());
    }
}
