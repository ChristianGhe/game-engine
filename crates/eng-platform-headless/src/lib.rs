//! The headless platform (spec 6.10): the platform contract
//! (`eng-platform-api`) with no window, no devices and no winit.
//!
//! - **A virtual clock.** Time starts at zero and jumps straight to each
//!   wake, so a run takes no real time and repeats exactly on every
//!   machine. `run --headless` paces its ticks on it.
//! - **Scripted events.** [`HeadlessPlatform::script`] gives the events a
//!   run delivers, each at its time on that clock: keys, a close request,
//!   whatever a test needs the "user" to do. Events due together are one
//!   batch, followed by one frame.
//! - **Ends that tests can see.** A run whose app waits for an event when
//!   none is left fails with [`PlatformError::Idle`]; one that passes its
//!   [`time_limit`](HeadlessPlatform::time_limit) fails with
//!   [`PlatformError::TimeLimit`]; and one whose app keeps asking to be
//!   woken at once, so that the clock never moves, fails with
//!   [`PlatformError::Stalled`] after
//!   [`STALL_FRAMES`](HeadlessPlatform::STALL_FRAMES) frames. None of
//!   them hangs.
//! - **Requests recorded** (ADR-0022). Cursor grabs and fullscreen requests
//!   change nothing here, as on any platform without a window: they answer
//!   [`CursorGrab::None`] and `false`. Each is kept with the clock's time,
//!   in order, and [`HeadlessPlatform::requests`] reads them after the run,
//!   so a test sees what an app asked for (that it frees the cursor on
//!   `FocusLost`, say) without a screen.
//!
//! It passes the contract's conformance suite, scripted parts included,
//! and has no window, so it stays outside the window seam (ADR-0016): it
//! never names `raw-window-handle`'s traits and never calls
//! [`App::window`].
#![forbid(unsafe_code)]

use eng_platform_api::{
    App, Control, CursorGrab, Instant, Platform, PlatformError, PlatformEvent, Request, Wake,
};

/// The platform with no window: a virtual clock and scripted events.
///
/// Configure it, then [`run`](Platform::run) apps on it; each run starts
/// the clock and the script again from zero, so a run can be repeated.
#[derive(Debug, Clone, Default)]
pub struct HeadlessPlatform {
    script: Vec<(Instant, PlatformEvent)>,
    limit: Option<Instant>,
    /// The last run's requests.
    requests: Vec<(Instant, Request)>,
}

impl HeadlessPlatform {
    /// Frames in a row at one instant, with no event delivered, after
    /// which a run ends with [`PlatformError::Stalled`]: on a virtual
    /// clock nothing can change while the app keeps asking for a time not
    /// after now.
    pub const STALL_FRAMES: u32 = 10_000;

    /// No events and no time limit.
    pub const fn new() -> Self {
        Self {
            script: Vec::new(),
            limit: None,
            requests: Vec::new(),
        }
    }

    /// The events each run delivers, each when the clock reaches its time,
    /// in this order. Replaces any earlier script.
    ///
    /// # Panics
    /// If the times decrease.
    pub fn script(mut self, events: impl IntoIterator<Item = (Instant, PlatformEvent)>) -> Self {
        self.script = events.into_iter().collect();
        if let Some(pair) = self.script.windows(2).find(|w| w[1].0 < w[0].0) {
            panic!(
                "a headless script must be in time order: an event at {} follows one at {}",
                pair[1].0, pair[0].0
            );
        }
        self
    }

    /// Nothing happens after `limit`: a run whose next wake or event would
    /// come later ends there with [`PlatformError::TimeLimit`]. A wake or
    /// event exactly at the limit still comes.
    pub const fn time_limit(mut self, limit: Instant) -> Self {
        self.limit = Some(limit);
        self
    }

    /// The requests the last run's app made, each with the clock's time
    /// when it asked, in order. Empty before the first run; each run starts
    /// the record again.
    pub fn requests(&self) -> &[(Instant, Request)] {
        &self.requests
    }
}

/// What the app sees during a callback.
struct Clock {
    now: Instant,
    exit: bool,
    requests: Vec<(Instant, Request)>,
}

impl Control for Clock {
    fn now(&self) -> Instant {
        self.now
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    /// Recorded; there is no window to grab.
    fn grab_cursor(&mut self, grab: bool) -> CursorGrab {
        self.requests.push((self.now, Request::GrabCursor(grab)));
        CursorGrab::None
    }

    /// Recorded; there is no window to make fullscreen.
    fn set_fullscreen(&mut self, on: bool) -> bool {
        self.requests.push((self.now, Request::Fullscreen(on)));
        false
    }
}

impl Platform for HeadlessPlatform {
    fn name(&self) -> &'static str {
        "headless"
    }

    fn run(&mut self, app: &mut dyn App) -> Result<(), PlatformError> {
        let mut ctx = Clock {
            now: Instant::ZERO,
            exit: false,
            requests: Vec::new(),
        };
        app.start(&mut ctx);
        let result = self.batches(app, &mut ctx);
        app.stop(&ctx);
        self.requests = ctx.requests;
        result
    }
}

impl HeadlessPlatform {
    /// Everything between `start` and `stop`.
    fn batches(&self, app: &mut dyn App, ctx: &mut Clock) -> Result<(), PlatformError> {
        let mut next = 0;
        // Frames in a row at this instant with no event delivered.
        let mut still = 0;
        while !ctx.exit {
            let delivered = next;
            // The events due, in order; an exit drops the rest.
            while let Some(&(at, event)) = self.script.get(next)
                && at <= ctx.now
                && !ctx.exit
            {
                next += 1;
                app.event(ctx, event);
            }
            if ctx.exit {
                break;
            }
            let timer = match app.frame(ctx) {
                _ if ctx.exit => break,
                Wake::At(at) => Some(at.max(ctx.now)),
                Wake::OnEvent => None,
            };
            let event = self.script.get(next).map(|&(at, _)| at);
            let wake = match (timer, event) {
                (Some(timer), Some(event)) => timer.min(event),
                (Some(at), None) | (None, Some(at)) => at,
                (None, None) => return Err(PlatformError::Idle { at: ctx.now }),
            };
            // Every event due has come, so only a timer at now keeps the
            // clock still.
            if next == delivered && wake == ctx.now {
                still += 1;
                if still == Self::STALL_FRAMES {
                    return Err(PlatformError::Stalled { at: ctx.now });
                }
            } else {
                still = 0;
            }
            if let Some(limit) = self.limit
                && wake > limit
            {
                return Err(PlatformError::TimeLimit { limit });
            }
            ctx.now = wake;
        }
        Ok(())
    }
}
