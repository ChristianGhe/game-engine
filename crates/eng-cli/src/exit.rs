use std::process::ExitCode;

/// Probe exit codes (spec 7.1). Each has a self-test in `game-testbed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    /// A semantic assertion failed or never held.
    AssertionFailed = 10,
    /// A replay's state hash diverged from the recording.
    Desync = 11,
    /// A panic; the hook exits with this code.
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

/// Installs a hook that reports any panic and exits with code 12.
/// Rung 1b adds `crash.json`.
pub(crate) fn install_panic_hook(game: &'static str) {
    std::panic::set_hook(Box::new(move |info| {
        let line = format!("[E-PANIC] {game}: {info}");
        eprintln!("{line}");
        crate::log::line("error", &line);
        if let Some(path) = crate::log::path() {
            eprintln!("log: {}", path.display());
        }
        std::process::exit(i32::from(Exit::Panic.code()));
    }));
}
