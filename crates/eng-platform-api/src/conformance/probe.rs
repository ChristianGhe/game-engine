//! The probe app: it follows a plan and records every callback.

use std::sync::Arc;
use std::time::Duration;

use crate::window::Window;
use crate::{App, Control, Instant, PlatformEvent, Wake};

/// A probe gives up and exits after this many frames, so a platform whose
/// clock stands still cannot keep the suite running. A run of the suite
/// needs a few dozen.
const MAX_FRAMES: usize = 1_000;

/// The scripted runs' probes also give up once their clock reaches this,
/// so a platform that loses events cannot keep the suite running. Their
/// clocks are virtual; the clock run's is not, so it has no time limit.
const GIVE_UP: Instant = Instant::from_millis(100);

/// Callbacks after the probe's exit before it fails from inside one: a
/// platform that ignores `exit` would otherwise never end the run.
const LATE_CALLS: usize = 3;

/// The timers run's period: it asks for every multiple of 5 ms.
const GRID: u64 = 5_000_000;

/// What a probe does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Plan {
    /// Asks for +2 ms, then a time in the past (zero), then +1 ms, and
    /// exits (twice) in its fourth frame.
    Clock,
    /// Exits in `start`.
    ExitInStart,
    /// Asks for the next multiple of 5 ms; ignores the first close request
    /// and exits on the second.
    Timers,
    /// Waits for events; exits on a close request.
    Events,
}

/// One callback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Call {
    Start,
    Window,
    Event(PlatformEvent),
    /// A frame, and what the probe answered.
    Frame(Wake),
    Stop,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Seen {
    pub(super) now: Instant,
    pub(super) call: Call,
    /// The probe had exited before this callback.
    pub(super) after_exit: bool,
}

/// An app that records what the platform does.
pub(super) struct Probe {
    plan: Plan,
    /// The platform and the run, for a failure from inside a callback.
    platform: &'static str,
    run: &'static str,
    pub(super) seen: Vec<Seen>,
    /// The callback in which the probe exited.
    pub(super) exited_in: Option<usize>,
    /// The first close request the probe ignored.
    pub(super) ignored_close: Option<usize>,
    /// Callbacks other than `stop` since the probe exited.
    late: usize,
}

impl Probe {
    pub(super) const fn new(plan: Plan, platform: &'static str, run: &'static str) -> Self {
        Self {
            plan,
            platform,
            run,
            seen: Vec::new(),
            exited_in: None,
            ignored_close: None,
            late: 0,
        }
    }

    /// Records a callback; returns its index. Fails, from inside the
    /// callback, once a platform keeps calling after the exit.
    fn record(&mut self, now: Instant, call: Call) -> usize {
        let after_exit = self.exited_in.is_some();
        self.seen.push(Seen {
            now,
            call,
            after_exit,
        });
        if after_exit && call != Call::Stop {
            self.late += 1;
            if self.late >= LATE_CALLS {
                let what = format!(
                    "{call:?} at {now} was callback {} since the app exited; the platform \
                     ignores the exit",
                    self.late
                );
                super::fail(self.platform, self.run, super::AFTER_EXIT, &what);
            }
        }
        self.seen.len() - 1
    }

    fn exit(&mut self, ctx: &mut dyn Control, during: usize) {
        self.exited_in.get_or_insert(during);
        ctx.exit();
    }

    fn frames(&self) -> usize {
        let frames = self
            .seen
            .iter()
            .filter(|s| matches!(s.call, Call::Frame(_)));
        frames.count()
    }
}

impl App for Probe {
    fn start(&mut self, ctx: &mut dyn Control) {
        let i = self.record(ctx.now(), Call::Start);
        if self.plan == Plan::ExitInStart {
            self.exit(ctx, i);
        }
    }

    fn window(&mut self, ctx: &mut dyn Control, window: Arc<dyn Window>) {
        let _ = window.size();
        self.record(ctx.now(), Call::Window);
    }

    fn event(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        let i = self.record(ctx.now(), Call::Event(event));
        if event != PlatformEvent::CloseRequested || self.exited_in.is_some() {
            return;
        }
        match (self.plan, self.ignored_close) {
            (Plan::Timers, None) => self.ignored_close = Some(i),
            (Plan::Timers | Plan::Events, _) => self.exit(ctx, i),
            _ => {}
        }
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        let now = ctx.now();
        let number = self.frames() + 1;
        let i = self.record(now, Call::Frame(Wake::OnEvent));
        let scripted = matches!(self.plan, Plan::Timers | Plan::Events);
        let wake = if self.exited_in.is_some() {
            // Already exited: a platform that ignores it comes back at once
            // and is caught.
            Wake::At(now)
        } else if number > MAX_FRAMES || (scripted && now >= GIVE_UP) {
            self.exit(ctx, i);
            Wake::At(now)
        } else {
            match self.plan {
                Plan::Clock => match number {
                    1 => Wake::At(now + Duration::from_millis(2)),
                    2 => Wake::At(Instant::ZERO),
                    3 => Wake::At(now + Duration::from_millis(1)),
                    _ => {
                        self.exit(ctx, i);
                        ctx.exit();
                        Wake::At(now)
                    }
                },
                Plan::Timers => Wake::At(Instant::from_nanos((now.as_nanos() / GRID + 1) * GRID)),
                Plan::ExitInStart | Plan::Events => Wake::OnEvent,
            }
        };
        self.seen[i].call = Call::Frame(wake);
        wake
    }

    fn stop(&mut self, ctx: &dyn Control) {
        self.record(ctx.now(), Call::Stop);
    }
}
