//! The platform contract's conformance suite on winit, with real windows
//! (rung 1b, T10: "the conformance suite passes where a display exists").
//!
//! winit's event loop must run on the main thread (macOS insists) and can
//! be made once per process, so this file is its own harness (`harness =
//! false` in `Cargo.toml`): `main` makes one platform and runs the checks
//! on it in order. It speaks the part of libtest's command line that
//! `cargo test` and cargo-nextest use: `--list` (with `--format terse` and
//! `--ignored`), name filters, `--exact` and `--skip`; other flags are
//! ignored. nextest runs each check in a process of its own.
//!
//! Without a display, as in CI's test job, every check that needs one
//! skips and says why, unless `ENG_REQUIRE_DISPLAY=1` makes it fail.
//!
//! The suite cannot end a run that the platform never ends (a platform
//! that stops calling the app but keeps its event loop going), so a
//! watchdog fails the harness after [`WATCHDOG`] instead of letting it
//! hang.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use eng_platform_api::window::Window;
use eng_platform_api::{
    App, Control, Instant, PhysicalSize, Platform, PlatformError, Wake, conformance,
};
use eng_platform_winit::{WinitPlatform, display_required};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

const TITLE: &str = "eng-platform-winit conformance";

/// Far longer than the checks take (about a second on a desktop).
const WATCHDOG: Duration = Duration::from_secs(60);

/// A check, and whether it needs the platform (a display).
enum Check {
    Windowed(fn(&mut WinitPlatform)),
    #[cfg(all(unix, not(target_os = "macos")))]
    Anywhere(fn()),
}

const CHECKS: &[(&str, Check)] = &[
    ("conformance", Check::Windowed(passes_the_suite)),
    ("window_seam", Check::Windowed(hands_over_a_real_window)),
    (
        "clock_restarts",
        Check::Windowed(each_run_starts_its_clock_at_zero),
    ),
    ("timers", Check::Windowed(timed_wakes_come_without_events)),
    (
        "one_event_loop_per_process",
        Check::Windowed(a_second_platform_is_unavailable),
    ),
    #[cfg(all(unix, not(target_os = "macos")))]
    (
        "no_display",
        Check::Anywhere(without_a_display_it_skips_unless_required),
    ),
];

/// T10's done-when: the suite's three runs (timed wakes, one in the past;
/// exit in `start`; running again) on real windows.
fn passes_the_suite(platform: &mut WinitPlatform) {
    conformance::check(platform);
}

/// What the app saw of the window it was handed.
struct Seen {
    size: PhysicalSize,
    window: RawWindowHandle,
    display: RawDisplayHandle,
}

/// Takes the window, looks at it, and exits in its first frame.
#[derive(Default)]
struct Seam {
    seen: Option<Seen>,
    frames: usize,
    stops: usize,
}

impl App for Seam {
    fn window(&mut self, _ctx: &mut dyn Control, window: Arc<dyn Window>) {
        // Callbacks run on the main thread, where winit gives out handles.
        let handle = window.window_handle().expect("a window handle");
        let display = window.display_handle().expect("a display handle");
        self.seen = Some(Seen {
            size: window.size(),
            window: handle.as_raw(),
            display: display.as_raw(),
        });
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        self.frames += 1;
        ctx.exit();
        Wake::OnEvent
    }

    fn stop(&mut self, _ctx: &dyn Control) {
        self.stops += 1;
    }
}

/// ADR-0016's seam with a real window: the app gets the OS's own window
/// and display handles, the kind wgpu makes a surface from, and a size.
fn hands_over_a_real_window(platform: &mut WinitPlatform) {
    let mut app = Seam::default();
    assert_eq!(platform.run(&mut app), Ok(()));
    assert_eq!((app.frames, app.stops), (1, 1));
    let seen = app.seen.expect("the app got the window");
    assert!(
        seen.size.width > 0 && seen.size.height > 0,
        "{:?}",
        seen.size
    );
    let native = if cfg!(target_os = "macos") {
        matches!(seen.window, RawWindowHandle::AppKit(_))
            && matches!(seen.display, RawDisplayHandle::AppKit(_))
    } else if cfg!(windows) {
        matches!(seen.window, RawWindowHandle::Win32(_))
            && matches!(seen.display, RawDisplayHandle::Windows(_))
    } else {
        matches!(
            (seen.window, seen.display),
            (RawWindowHandle::Xlib(_), RawDisplayHandle::Xlib(_))
                | (RawWindowHandle::Xcb(_), RawDisplayHandle::Xcb(_))
                | (RawWindowHandle::Wayland(_), RawDisplayHandle::Wayland(_))
        )
    };
    assert!(native, "{:?} on {:?}", seen.window, seen.display);
}

/// Notes the clock at `start`, the first frame (which lingers before it
/// exits) and `stop`.
#[derive(Default)]
struct Restart {
    start: Option<Instant>,
    first_frame: Option<Instant>,
    stop: Option<Instant>,
    linger: Duration,
}

impl App for Restart {
    fn start(&mut self, ctx: &mut dyn Control) {
        self.start = Some(ctx.now());
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        self.first_frame.get_or_insert(ctx.now());
        std::thread::sleep(self.linger);
        ctx.exit();
        Wake::OnEvent
    }

    fn stop(&mut self, ctx: &dyn Control) {
        self.stop = Some(ctx.now());
    }
}

/// The contract: each run's clock starts at zero. `start` reads zero, as
/// on the headless platform, and a second run's first frame comes before
/// the time the first run's clock had reached at its end, which a clock
/// going on from the first run could not do (the first run lingers a
/// second, so a busy machine has that long to open the second window).
/// The suite runs a platform three times but does not check this.
fn each_run_starts_its_clock_at_zero(platform: &mut WinitPlatform) {
    let mut first = Restart {
        linger: Duration::from_secs(1),
        ..Restart::default()
    };
    assert_eq!(platform.run(&mut first), Ok(()));
    let mut second = Restart::default();
    assert_eq!(platform.run(&mut second), Ok(()));
    assert_eq!(first.start, Some(Instant::ZERO), "the first run's start");
    assert_eq!(second.start, Some(Instant::ZERO), "the second run's start");
    let end = first.stop.expect("the first run stopped");
    let frame = second.first_frame.expect("the second run framed");
    assert!(
        frame < end,
        "the second run's first frame came at {frame}, after the first run ended at {end}: \
         its clock went on from the first run's"
    );
}

/// Asks for a frame every 10 ms and exits at the [`Ticks::FRAMES`]th.
#[derive(Default)]
struct Ticks {
    /// When the next frame was asked for, unless an event came since.
    asked: Option<Instant>,
    /// Each frame's time, and the time it was asked for.
    frames: Vec<(Instant, Option<Instant>)>,
}

impl Ticks {
    const FRAMES: usize = 20;
}

impl App for Ticks {
    fn event(&mut self, _ctx: &mut dyn Control, _event: eng_platform_api::PlatformEvent) {
        // An event's batch may bring the frame sooner.
        self.asked = None;
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        let now = ctx.now();
        self.frames.push((now, self.asked));
        if self.frames.len() == Self::FRAMES {
            ctx.exit();
            return Wake::OnEvent;
        }
        let next = now + Duration::from_millis(10);
        self.asked = Some(next);
        Wake::At(next)
    }
}

/// The platform's own timer wakes the app: 20 frames 10 ms apart, each on
/// time or late, never early. The suite's clock run asks for three timed
/// frames, which the events of a window opening can bring as well; a
/// platform that never armed its timer would pass it on a desktop, and
/// here, with no input, hangs until the watchdog fails it.
fn timed_wakes_come_without_events(platform: &mut WinitPlatform) {
    let mut app = Ticks::default();
    assert_eq!(platform.run(&mut app), Ok(()));
    assert_eq!(app.frames.len(), Ticks::FRAMES);
    for &(now, asked) in &app.frames {
        if let Some(at) = asked {
            assert!(now >= at, "a frame asked for at {at} came at {now}");
        }
    }
}

/// winit makes one event loop per process; a second platform says so
/// instead of panicking.
fn a_second_platform_is_unavailable(_platform: &mut WinitPlatform) {
    match WinitPlatform::new("second") {
        Err(PlatformError::Unavailable(why)) => {
            assert!(why.contains("once per process"), "{why}");
        }
        Err(other) => panic!("not Unavailable: {other:?}"),
        Ok(_) => panic!("a second event loop was made"),
    }
}

/// On Linux and the BSDs a display is `DISPLAY` or `WAYLAND_DISPLAY`: this
/// harness, run again without them, skips, and fails with
/// `ENG_REQUIRE_DISPLAY=1`.
#[cfg(all(unix, not(target_os = "macos")))]
fn without_a_display_it_skips_unless_required() {
    use std::process::Command;

    let run = |required: bool| {
        let mut cmd = Command::new(std::env::current_exe().expect("this harness"));
        cmd.args(["--exact", "conformance"]);
        for var in ["DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET"] {
            cmd.env_remove(var);
        }
        if required {
            cmd.env("ENG_REQUIRE_DISPLAY", "1");
        } else {
            cmd.env_remove("ENG_REQUIRE_DISPLAY");
        }
        let out = cmd.output().expect("the harness runs");
        let text = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        (out.status.success(), text)
    };
    let (ok, text) = run(false);
    assert!(ok && text.contains("skipped"), "{text}");
    let (ok, text) = run(true);
    assert!(!ok && text.contains("[E-NO-DISPLAY]"), "{text}");
}

/// The part of libtest's command line that cargo and nextest use.
#[derive(Default)]
struct Cli {
    list: bool,
    ignored: bool,
    exact: bool,
    filters: Vec<String>,
    skips: Vec<String>,
}

impl Cli {
    fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut cli = Cli::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--list" => cli.list = true,
                "--ignored" => cli.ignored = true,
                "--exact" => cli.exact = true,
                "--skip" => cli.skips.extend(args.next()),
                // libtest's flags that take a value; the value means nothing here.
                "--format" | "--color" | "--test-threads" | "--logfile" | "--shuffle-seed"
                | "-Z" => {
                    args.next();
                }
                flag if flag.starts_with('-') => {}
                filter => cli.filters.push(filter.to_string()),
            }
        }
        cli
    }

    fn selects(&self, name: &str) -> bool {
        let hit = |f: &String| {
            if self.exact {
                name == f
            } else {
                name.contains(f.as_str())
            }
        };
        (self.filters.is_empty() || self.filters.iter().any(hit)) && !self.skips.iter().any(hit)
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse(std::env::args().skip(1));
    if cli.list {
        // No check is ignored.
        if !cli.ignored {
            for (name, _) in CHECKS {
                println!("{name}: test");
            }
        }
        return ExitCode::SUCCESS;
    }
    std::thread::spawn(|| {
        std::thread::sleep(WATCHDOG);
        eprintln!("[E-TIMEOUT] the windowed checks took more than {WATCHDOG:?}: a run never ended");
        std::process::exit(1);
    });
    let chosen: Vec<_> = CHECKS
        .iter()
        .filter(|(name, _)| !cli.ignored && cli.selects(name))
        .collect();
    let mut platform = None;
    if chosen.iter().any(|(_, c)| matches!(c, Check::Windowed(_))) {
        match WinitPlatform::new(TITLE) {
            Ok(made) => platform = Some(made),
            Err(e) if display_required() => {
                eprintln!("[E-NO-DISPLAY] {e}; ENG_REQUIRE_DISPLAY=1 needs a display");
                return ExitCode::FAILURE;
            }
            Err(e) => println!("no display, so the windowed checks skip: {e}"),
        }
    }
    for (name, check) in &chosen {
        match (check, platform.as_mut()) {
            (Check::Windowed(_), None) => println!("test {name} ... skipped"),
            (Check::Windowed(run), Some(platform)) => {
                run(platform);
                println!("test {name} ... ok");
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            (Check::Anywhere(run), _) => {
                run();
                println!("test {name} ... ok");
            }
        }
    }
    println!("test result: ok. {} checks", chosen.len());
    ExitCode::SUCCESS
}
