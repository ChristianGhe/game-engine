//! The callbacks: the platform drives, the app answers.

use std::fmt;
use std::sync::Arc;

use crate::clock::Instant;
use crate::event::PlatformEvent;
use crate::window::Window;

/// Runs an app: owns the event loop and calls the app back.
///
/// The headless platform implements this, so nothing here names a window
/// handle; the window seam is [`App::window`].
pub trait Platform {
    /// A short name for reports, such as `"headless"` or `"winit"`.
    fn name(&self) -> &'static str;

    /// Runs `app` until it calls [`Control::exit`], following the rules in
    /// the crate docs. Returns `Ok(())` after an exit the app asked for,
    /// and also when the operating system ended the run (a desktop's quit
    /// command). Every run that called [`App::start`] calls [`App::stop`]
    /// before it returns, whatever it returns.
    ///
    /// `run` may be called again once it has returned, for a new run: the
    /// clock starts again from [`Instant::ZERO`].
    fn run(&mut self, app: &mut dyn App) -> Result<(), PlatformError>;
}

/// What the app may ask of the platform during a callback.
///
/// The cursor and fullscreen requests (ADR-0022) act on the window the
/// platform handed to [`App::window`], at once, from that callback on.
/// Before it, and on a platform without a window, they change nothing and
/// answer [`CursorGrab::None`] or `false`. Their default bodies are that
/// answer, so a platform without a window, and a wrapper that does not
/// forward them, need not implement them; a wrapper meant to pass requests
/// through forwards them to its inner `Control`.
pub trait Control {
    /// The monotonic clock: time since this run started. It never goes
    /// back, and it may stand still within a callback.
    fn now(&self) -> Instant;

    /// Stop the run. After this the app gets no callback but
    /// [`App::stop`]: the rest of the current batch of events is dropped.
    /// Calling it again changes nothing.
    fn exit(&mut self);

    /// Grabs the cursor for mouse look (`true`) or frees it (`false`), and
    /// returns the grab now in force. A grab tries [`CursorGrab::Locked`],
    /// then [`CursorGrab::Confined`], and hides the cursor; freeing shows
    /// it and answers [`CursorGrab::None`].
    ///
    /// A grab never outlives focus: when the window loses it, the platform
    /// frees the cursor before [`PlatformEvent::FocusLost`] arrives. It
    /// never grabs unasked, so an app that wants the grab back asks again
    /// on [`PlatformEvent::FocusGained`]. A grab asked for while the window
    /// lacks focus may wait for it (winit's does, as in `App::window`,
    /// before the window shows), answering `None` until then.
    fn grab_cursor(&mut self, grab: bool) -> CursorGrab {
        let _ = grab;
        CursorGrab::None
    }

    /// Borderless fullscreen on the window's monitor (`true`), or back to
    /// a window (`false`). Returns whether the platform took the request:
    /// the change itself may land later (macOS animates it), and its new
    /// size arrives as [`PlatformEvent::Resized`].
    fn set_fullscreen(&mut self, on: bool) -> bool {
        let _ = on;
        false
    }
}

/// How the cursor is held for mouse look: [`Control::grab_cursor`]'s
/// answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CursorGrab {
    /// Not held: the cursor moves freely and shows.
    #[default]
    None,
    /// Kept inside the window, where the system cannot hold it still
    /// (X11, in winit).
    Confined,
    /// Held still. Raw motion still arrives.
    Locked,
}

impl CursorGrab {
    /// Its name in reports: `"none"`, `"confined"` or `"locked"`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Confined => "confined",
            Self::Locked => "locked",
        }
    }
}

/// A request an app made through [`Control`], as a platform that records
/// them (the headless one) keeps it, with the clock's time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Request {
    /// [`Control::grab_cursor`]: `true` grabs, `false` frees.
    GrabCursor(bool),
    /// [`Control::set_fullscreen`]: `true` asks for fullscreen.
    Fullscreen(bool),
}

/// When the app wants its next [`App::frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    /// At this time, or at once if it is not after now. An event wakes the
    /// app sooner: its batch, then a frame.
    At(Instant),
    /// Only after the next event.
    OnEvent,
}

/// The app's side: everything a platform calls.
///
/// The order is `start`, then `window` on a platform with a window, then
/// any number of batches of `event`s, each followed by one `frame`, then
/// `stop`. The first `frame` comes right after `start` (and `window`),
/// after any events already due.
pub trait App {
    /// The run has started. Always the first callback.
    fn start(&mut self, ctx: &mut dyn Control) {
        let _ = ctx;
    }

    /// The window seam (ADR-0016): a platform with a window hands it over
    /// once, right after `start`, for the GPU to draw into. The headless
    /// platform has no window and never calls this.
    fn window(&mut self, ctx: &mut dyn Control, window: Arc<dyn Window>) {
        let _ = (ctx, window);
    }

    /// One event, in the order the platform saw it.
    fn event(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        let _ = (ctx, event);
    }

    /// The events due have all been delivered and the platform is about to
    /// wait: do the frame's work (run the ticks that are due, draw), then
    /// say when to be woken. Without an event in between, the next frame
    /// never comes before the time asked for.
    fn frame(&mut self, ctx: &mut dyn Control) -> Wake;

    /// The run is over: the last callback, exactly once.
    fn stop(&mut self, ctx: &dyn Control) {
        let _ = ctx;
    }
}

/// Why a run could not start, or ended without the app's exit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlatformError {
    /// The platform cannot run here: no display, or its event loop or
    /// window could not be made. Nothing was called.
    Unavailable(String),
    /// The platform failed while running.
    Failed(String),
    /// Headless: the app waited for an event (`Wake::OnEvent`) at `at`,
    /// and no scripted event was left.
    Idle { at: Instant },
    /// Headless: the next wake was past the time limit.
    TimeLimit { limit: Instant },
    /// Headless: the app kept asking to be woken at once at `at`, with no
    /// event coming, so its virtual clock could never move.
    Stalled { at: Instant },
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(why) => write!(f, "the platform cannot run here: {why}"),
            Self::Failed(why) => write!(f, "the platform failed while running: {why}"),
            Self::Idle { at } => write!(
                f,
                "the app waited for an event at {at}, and none is left to come"
            ),
            Self::TimeLimit { limit } => {
                write!(f, "the app had not exited by the time limit, {limit}")
            }
            Self::Stalled { at } => write!(
                f,
                "the app kept asking to be woken at once at {at}, so the clock never moved"
            ),
        }
    }
}

impl std::error::Error for PlatformError {}
