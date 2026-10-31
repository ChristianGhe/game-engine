//! The platform contract's conformance suite against the headless platform
//! (rung 1b, T3: "the suite passes for headless, including a scripted
//! close request reaching the app"), and against broken platforms made
//! from it: each breaks one rule, and the suite must name that rule.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use eng_platform_api::{
    App, Control, Instant, Platform, PlatformError, PlatformEvent, Wake, conformance,
};
use eng_platform_headless::HeadlessPlatform;

#[test]
fn headless_passes_the_lifecycle_and_clock_rules() {
    conformance::check(&mut HeadlessPlatform::new());
}

#[test]
fn headless_passes_the_scripted_rules_including_a_close_request() {
    conformance::check_scripted(|script| HeadlessPlatform::new().script(script.iter().copied()));
}

/// One way to break the contract, put between the headless platform and
/// the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    StartTwice,
    FrameBeforeStart,
    NoStop,
    StopTwice,
    StopClockBehind,
    WakesEarly,
    WakesLate,
    IgnoresExitOnce,
    ErrAfterExit,
    DropsAnEvent,
    RepeatsAnEvent,
    SwapsEvents,
    StopsOnCloseRequest,
    NoFrameAfterEvents,
    ClockAheadAfterEvents,
    WakesWithoutEvent,
    /// Drops every event, and from then on wakes the app at once, so its
    /// clock stands still: the suite must give up, not spin.
    LosesEventsAndStalls,
    /// Never passes the app's exit on: the suite must fail, not spin.
    IgnoresExit,
    /// Delivers one event of each batch per frame.
    SplitsBatches,
    /// Ends the run itself at the app's second frame.
    EndsEarly,
    /// Holds the first frame back 1 ms when no event comes before it.
    FirstFrameLate,
    /// Lets a wake asked for in the past set the clock back.
    PastWakeGoesBack,
}

/// Each fault and the rule the suite must name for it.
const FAULTS: [(Fault, &str); 22] = [
    (
        Fault::LosesEventsAndStalls,
        "every scripted event arrives once, in order",
    ),
    (Fault::IgnoresExit, "nothing comes after exit but stop"),
    (
        Fault::SplitsBatches,
        "every event due comes before the frame",
    ),
    (Fault::EndsEarly, "the run lasts until the app exits"),
    (Fault::FirstFrameLate, "the first frame comes at once"),
    (Fault::PastWakeGoesBack, "the clock never goes back"),
    (Fault::StartTwice, "start comes first, once"),
    (Fault::FrameBeforeStart, "start comes first, once"),
    (Fault::NoStop, "stop comes last, once"),
    (Fault::StopTwice, "stop comes last, once"),
    (Fault::StopClockBehind, "the clock never goes back"),
    (Fault::WakesEarly, "a timed wake is never early"),
    (Fault::WakesLate, "a timed wake comes on time"),
    (Fault::IgnoresExitOnce, "nothing comes after exit but stop"),
    (Fault::ErrAfterExit, "run returns Ok after the app exits"),
    (
        Fault::DropsAnEvent,
        "every scripted event arrives once, in order",
    ),
    (
        Fault::RepeatsAnEvent,
        "every scripted event arrives once, in order",
    ),
    (
        Fault::SwapsEvents,
        "every scripted event arrives once, in order",
    ),
    (
        Fault::StopsOnCloseRequest,
        "a close request is the app's to handle",
    ),
    (
        Fault::NoFrameAfterEvents,
        "a frame follows each batch of events",
    ),
    (Fault::ClockAheadAfterEvents, "an event arrives at its time"),
    (
        Fault::WakesWithoutEvent,
        "a wait for events ends only with an event",
    ),
];

/// The whole suite, as an implementation's tests would run it.
fn suite(fault: Fault) {
    conformance::check(&mut Faulty::new(HeadlessPlatform::new(), fault));
    conformance::check_scripted(|script| {
        Faulty::new(
            HeadlessPlatform::new().script(script.iter().copied()),
            fault,
        )
    });
}

#[test]
fn the_fault_injector_alone_breaks_nothing() {
    suite(Fault::None);
}

#[test]
fn the_suite_rejects_each_broken_rule() {
    for (fault, rule) in FAULTS {
        let caught = catch_unwind(AssertUnwindSafe(|| suite(fault)));
        let message = match caught {
            Ok(()) => panic!("{fault:?} passed the suite"),
            Err(payload) => panic_message(payload),
        };
        assert!(message.contains(rule), "{fault:?}: {message}");
        assert!(message.contains("platform conformance"), "{message}");
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => payload
            .downcast_ref::<&str>()
            .map_or_else(|| "(not a string)".to_string(), |s| (*s).to_string()),
    }
}

/// A platform that runs the headless one with `fault` applied.
struct Faulty {
    inner: HeadlessPlatform,
    fault: Fault,
}

impl Faulty {
    fn new(inner: HeadlessPlatform, fault: Fault) -> Self {
        Self { inner, fault }
    }
}

impl Platform for Faulty {
    fn name(&self) -> &'static str {
        "faulty"
    }

    fn run(&mut self, app: &mut dyn App) -> Result<(), PlatformError> {
        let mut shim = Shim {
            app,
            fault: self.fault,
            state: State::default(),
        };
        let result = self.inner.run(&mut shim);
        if self.fault == Fault::ErrAfterExit && result.is_ok() {
            return Err(PlatformError::Failed("made up".into()));
        }
        result
    }
}

/// What the faults remember during one run.
#[derive(Default)]
struct State {
    /// Events the headless platform delivered so far.
    events: usize,
    /// Events since the last frame.
    since_frame: usize,
    /// `SwapsEvents`: the first event, held back.
    held: Option<PlatformEvent>,
    /// `IgnoresExitOnce`: the app's first exit was kept from the platform,
    swallowed: bool,
    /// during this callback,
    pending: bool,
    /// and reaches it at the end of the next one.
    owed: bool,
    /// `NoFrameAfterEvents`: the app's last answer.
    last_wake: Option<Wake>,
    /// `ClockAheadAfterEvents`, `LosesEventsAndStalls`: an event has come.
    had_event: bool,
    /// The app has called `exit`.
    exited: bool,
    /// Frames the app has had.
    frames: usize,
    /// `FirstFrameLate`: the first frame has been held back.
    delayed: bool,
    /// `SplitsBatches`: the rest of the batch, and that the next frame is
    /// the platform waking at once to deliver the next of them.
    split: VecDeque<PlatformEvent>,
    rewake: bool,
    /// `PastWakeGoesBack`: the clock the app sees in its next callback.
    back_to: Option<Instant>,
}

/// Sits between the headless platform and the app.
struct Shim<'a> {
    app: &'a mut dyn App,
    fault: Fault,
    state: State,
}

/// The platform's controls as the app sees them, with the fault applied.
struct Lens<'a> {
    inner: &'a mut dyn Control,
    fault: Fault,
    state: &'a mut State,
}

impl Control for Lens<'_> {
    fn now(&self) -> Instant {
        let now = self.inner.now();
        if let Some(back) = self.state.back_to {
            return back;
        }
        if self.fault == Fault::ClockAheadAfterEvents && self.state.had_event {
            return now + Duration::from_nanos(1);
        }
        now
    }

    fn exit(&mut self) {
        self.state.exited = true;
        match self.fault {
            Fault::IgnoresExit => {}
            Fault::IgnoresExitOnce if !self.state.swallowed => {
                self.state.swallowed = true;
                self.state.pending = true;
            }
            _ => self.inner.exit(),
        }
    }
}

/// The clock `stop` sees: `shift` nanoseconds off the platform's.
struct StopLens<'a> {
    inner: &'a dyn Control,
    shift: i8,
}

impl Control for StopLens<'_> {
    fn now(&self) -> Instant {
        let now = self.inner.now().as_nanos();
        Instant::from_nanos(now.saturating_add_signed(i64::from(self.shift)))
    }

    fn exit(&mut self) {}
}

impl Shim<'_> {
    fn deliver(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        let mut lens = Lens {
            inner: ctx,
            fault: self.fault,
            state: &mut self.state,
        };
        self.app.event(&mut lens, event);
    }

    /// Ends every callback: a swallowed exit reaches the platform one
    /// callback late, after the app has had a callback it should not; a
    /// clock set back lasts one callback.
    fn settle(&mut self, ctx: &mut dyn Control) {
        if std::mem::take(&mut self.state.owed) {
            ctx.exit();
        }
        if std::mem::take(&mut self.state.pending) {
            self.state.owed = true;
        }
        self.state.back_to = None;
    }
}

impl App for Shim<'_> {
    fn start(&mut self, ctx: &mut dyn Control) {
        let mut lens = Lens {
            inner: ctx,
            fault: self.fault,
            state: &mut self.state,
        };
        if self.fault == Fault::FrameBeforeStart {
            let _ = self.app.frame(&mut lens);
        }
        self.app.start(&mut lens);
        if self.fault == Fault::StartTwice {
            self.app.start(&mut lens);
        }
        self.settle(ctx);
    }

    fn event(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        self.state.events += 1;
        self.state.since_frame += 1;
        match (self.fault, self.state.events) {
            (Fault::DropsAnEvent, 3) => {}
            (Fault::RepeatsAnEvent, 3) => {
                self.deliver(ctx, event);
                self.deliver(ctx, event);
            }
            (Fault::SwapsEvents, 1) => self.state.held = Some(event),
            (Fault::SwapsEvents, 2) => {
                self.deliver(ctx, event);
                let held = self.state.held.take().unwrap();
                self.deliver(ctx, held);
            }
            (Fault::StopsOnCloseRequest, _) if event == PlatformEvent::CloseRequested => {
                self.deliver(ctx, event);
                ctx.exit();
            }
            (Fault::LosesEventsAndStalls, _) => {}
            (Fault::SplitsBatches, _) if self.state.since_frame > 1 => {
                self.state.split.push_back(event);
            }
            _ => self.deliver(ctx, event),
        }
        self.state.had_event = true;
        self.settle(ctx);
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        let after_events = std::mem::take(&mut self.state.since_frame) > 0;
        if std::mem::take(&mut self.state.rewake)
            && let Some(event) = self.state.split.pop_front()
        {
            self.deliver(ctx, event);
            if self.state.exited {
                self.settle(ctx);
                return Wake::OnEvent;
            }
        }
        if self.fault == Fault::NoFrameAfterEvents
            && after_events
            && let Some(wake) = self.state.last_wake
        {
            self.settle(ctx);
            return wake;
        }
        if self.fault == Fault::FirstFrameLate
            && self.state.frames == 0
            && !after_events
            && !std::mem::replace(&mut self.state.delayed, true)
        {
            self.settle(ctx);
            return Wake::At(ctx.now() + Duration::from_millis(1));
        }
        let mut lens = Lens {
            inner: ctx,
            fault: self.fault,
            state: &mut self.state,
        };
        let asked = self.app.frame(&mut lens);
        self.state.frames += 1;
        self.state.last_wake = Some(asked);
        let wake = match (self.fault, asked) {
            (Fault::WakesEarly, Wake::At(at)) if at.as_nanos() > 0 => {
                Wake::At(Instant::from_nanos(at.as_nanos() - 1))
            }
            (Fault::WakesLate, Wake::At(at)) => Wake::At(at + Duration::from_millis(1)),
            (Fault::WakesWithoutEvent, Wake::OnEvent) => {
                Wake::At(ctx.now() + Duration::from_millis(1))
            }
            (Fault::LosesEventsAndStalls, _) if self.state.had_event => Wake::At(ctx.now()),
            (Fault::SplitsBatches, _) if !self.state.split.is_empty() => {
                self.state.rewake = true;
                Wake::At(ctx.now())
            }
            _ => asked,
        };
        if self.fault == Fault::EndsEarly && self.state.frames == 2 {
            ctx.exit();
        }
        self.settle(ctx);
        if self.fault == Fault::PastWakeGoesBack
            && let Wake::At(at) = asked
            && at < ctx.now()
        {
            self.state.back_to = Some(at);
        }
        wake
    }

    fn stop(&mut self, ctx: &dyn Control) {
        if self.fault == Fault::NoStop {
            return;
        }
        let shift = match self.fault {
            Fault::StopClockBehind => -1,
            Fault::ClockAheadAfterEvents if self.state.had_event => 1,
            _ => 0,
        };
        let lens = StopLens { inner: ctx, shift };
        self.app.stop(&lens);
        if self.fault == Fault::StopTwice {
            self.app.stop(&lens);
        }
    }
}
