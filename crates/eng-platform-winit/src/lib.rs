//! The desktop platform (spec 6.10, 9.2): winit windows and input behind
//! the `eng-platform-api` contract. It is the only crate that may import
//! winit (`[floor]` in `architecture.toml`), and no winit type appears in
//! its API: [`WinitPlatform`] and [`display_required`] are all of it.
//!
//! - **One window per run.** Each [`Platform::run`] makes a window with
//!   the platform's title, hidden, then calls `start`, hands the window to
//!   `App::window` (ADR-0016's seam, as an `Arc<dyn Window>`) and shows
//!   it. When the run ends the platform lets go of it. winit carries no
//!   window state from one run to the next, so an app drops its `Arc`,
//!   and anything holding one such as a GPU surface, by `stop`; the window
//!   closes then (on macOS winit closes it at the end of the run anyway).
//!   If the window cannot be made, `run` returns
//!   [`PlatformError::Unavailable`] and calls nothing. It is resizable,
//!   winit's default size, and fullscreen only when the app asks.
//! - **Running again.** winit makes its event loop once per process, so
//!   the platform owns it and each run is winit's `run_app_on_demand`
//!   (Windows, macOS, X11 and Wayland). A second [`WinitPlatform::new`] in
//!   a process is `Unavailable`.
//! - **The main thread.** winit's event loop must be made and run on the
//!   main thread (macOS requires it; elsewhere winit refuses otherwise),
//!   so tests that open windows are `harness = false`
//!   (`tests/conformance.rs`).
//! - **The clock** is the system's monotonic clock from just before
//!   `start`, read once per callback; a reading never goes below the one
//!   before.
//! - **Wakes.** After each batch winit calls `about_to_wait`; the app's
//!   `frame` comes there when it is due, and its answer becomes winit's
//!   `ControlFlow` (`WaitUntil` or `Wait`). winit also wakes for OS events
//!   the contract leaves out, and its timers may fire a little early; then
//!   the frame is not due yet and the loop waits again, so a timed wake is
//!   never early. A wake more than a minute off is looked at again each
//!   minute.
//! - **Exit.** After `Control::exit` the app gets nothing but `stop`:
//!   winit may still deliver the rest of its batch, which is dropped.
//!   `run` returns `Ok(())` after the app's exit, and when the system ended
//!   the run. One exception: on macOS, Quit in the application menu
//!   (Cmd+Q) or the Dock ends the process right after `stop`, and `run`
//!   never returns (AppKit exits), so an app saves what it must in `stop`.
//! - **Events.** Keys by physical code (W3C names; winit's `SuperLeft` is
//!   `MetaLeft`); keys the contract cannot name, OS key repeat and winit's
//!   synthetic presses and releases at focus changes are dropped. Focus;
//!   size in physical pixels; occlusion, where winit reports it (macOS:
//!   minimised or covered; X11: fully covered; on Windows a minimised
//!   window shows as 0 x 0 instead, and Wayland reports neither); and
//!   close requests, which the app handles. Mouse buttons, and the wheel
//!   turned to the contract's signs (winit's say where content moves; on
//!   macOS the platform reads natural scrolling once, when it is made, and
//!   turns it back). Raw mouse motion is winit's device motion, only while
//!   the window has focus, and not on macOS, where winit's deltas are
//!   accelerated and coalesced: there it comes from the GCMouse shim
//!   (rung 2, task T14). No gamepad events until gilrs (rung 6).
//!   `map.rs` has the details.
//! - **Requests** (ADR-0022). Borderless fullscreen on the window's
//!   monitor, at once. A cursor grab hides the cursor, moves it to the
//!   middle of the window and tries winit's `Locked`, then `Confined`
//!   (X11 has no lock), but holds only while the window has focus: asked
//!   for without focus (as in `App::window`, before the window shows) it
//!   waits for focus and answers `None` until then, and focus loss frees
//!   the cursor, so the app asks again on `FocusGained`. The end of a run
//!   frees it too. `request.rs` says why. The window's refresh rate is its
//!   monitor's, as winit reads it.
//! - **Displays.** Linux and the BSDs need `DISPLAY` or `WAYLAND_DISPLAY`;
//!   winit loads X11 or Wayland at run time, so building needs no system
//!   development packages (plan decision 5). Without a display `new` is
//!   `Unavailable`, and the windowed tests skip unless
//!   `ENG_REQUIRE_DISPLAY=1` ([`display_required`]).
//!
//! It passes the contract's conformance suite where a display exists
//! (rung 1b, T10). `examples/events.rs` prints what a real window reports,
//! to check a machine's input by hand; `--grab` and `--fullscreen` make
//! the requests.
#![forbid(unsafe_code)]

mod map;
mod pace;
mod request;
mod run;
mod window;

use eng_platform_api::{App, Platform, PlatformError};
use winit::error::EventLoopError;
use winit::event_loop::EventLoop;
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;

use crate::run::Run;

/// The desktop platform: one window per run, on winit's event loop.
///
/// Make it once, on the main thread, then [`run`](Platform::run) apps on
/// it as often as needed.
pub struct WinitPlatform {
    event_loop: EventLoop<()>,
    title: String,
    /// macOS's natural scrolling, read once when the platform is made.
    natural: bool,
}

impl WinitPlatform {
    /// Makes winit's event loop, for windows titled `title`.
    ///
    /// # Errors
    /// [`PlatformError::Unavailable`] when there is no display (on Linux:
    /// no `DISPLAY` or `WAYLAND_DISPLAY`, or no server behind it), or when
    /// this process has made a platform already.
    ///
    /// # Panics
    /// winit panics off the main thread.
    pub fn new(title: &str) -> Result<Self, PlatformError> {
        let event_loop = EventLoop::new().map_err(|e| {
            PlatformError::Unavailable(match e {
                EventLoopError::RecreationAttempt => {
                    "winit makes its event loop once per process, and this process made it already"
                        .to_string()
                }
                other => format!("winit found no display: {other}"),
            })
        })?;
        Ok(Self {
            event_loop,
            title: title.to_string(),
            natural: natural_scrolling(),
        })
    }
}

/// Whether the system's natural scrolling is on, which reverses winit's
/// wheel deltas. macOS keeps it in a user default that winit does not
/// read; `defaults` reads it (one process, once per platform). Elsewhere
/// the platform cannot tell, and takes the deltas as the system gives
/// them.
fn natural_scrolling() -> bool {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/bin/defaults")
            .args(["read", "-g", "com.apple.swipescrolldirection"])
            .output()
            .ok()
            .filter(|out| out.status.success());
        let text = out.map(|out| String::from_utf8_lossy(&out.stdout).into_owned());
        map::natural_scrolling(text.as_deref())
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

impl Platform for WinitPlatform {
    fn name(&self) -> &'static str {
        "winit"
    }

    fn run(&mut self, app: &mut dyn App) -> Result<(), PlatformError> {
        let attributes = winit::window::Window::default_attributes()
            .with_title(self.title.as_str())
            .with_visible(false);
        let mut run = Run::new(app, attributes, self.natural);
        let result = self.event_loop.run_app_on_demand(&mut run);
        run.finish();
        if let Some(why) = run.unavailable.take() {
            return Err(PlatformError::Unavailable(why));
        }
        match result {
            Ok(()) => Ok(()),
            // The contract: an exit the app asked for is a clean end, even
            // if winit then fails to tear down.
            Err(_) if run.exited() => Ok(()),
            Err(e) => Err(PlatformError::Failed(format!("winit's event loop: {e}"))),
        }
    }
}

/// Whether a missing display fails rather than skips the windowed tests:
/// `ENG_REQUIRE_DISPLAY=1`, for CI jobs that have a display (xvfb on
/// Linux), as `ENG_REQUIRE_GPU=1` does for GPUs.
pub fn display_required() -> bool {
    std::env::var("ENG_REQUIRE_DISPLAY").is_ok_and(|value| value == "1")
}
