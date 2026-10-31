//! The headless platform beyond the conformance suite: exact virtual
//! times, its script and time limit, and runs that repeat exactly.

use std::time::Duration;

use eng_platform_api::{App, Control, Instant, Key, Platform, PlatformError, PlatformEvent, Wake};
use eng_platform_headless::HeadlessPlatform;
use proptest::prelude::*;

const fn ms(n: u64) -> Instant {
    Instant::from_millis(n)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Call {
    Start,
    Event(PlatformEvent),
    Frame,
    Stop,
}

/// Records every callback with its time. `next` answers each frame from
/// the frame's time; `None` exits.
struct Recorder<F> {
    calls: Vec<(Instant, Call)>,
    next: F,
    exit_on_close: bool,
}

impl<F: FnMut(Instant) -> Option<Wake>> Recorder<F> {
    fn new(next: F) -> Self {
        Self {
            calls: Vec::new(),
            next,
            exit_on_close: false,
        }
    }

    fn frames(&self) -> Vec<Instant> {
        let frames = self.calls.iter().filter(|(_, c)| *c == Call::Frame);
        frames.map(|&(at, _)| at).collect()
    }
}

impl<F: FnMut(Instant) -> Option<Wake>> App for Recorder<F> {
    fn start(&mut self, ctx: &mut dyn Control) {
        self.calls.push((ctx.now(), Call::Start));
    }

    fn event(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        self.calls.push((ctx.now(), Call::Event(event)));
        if self.exit_on_close && event == PlatformEvent::CloseRequested {
            ctx.exit();
        }
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        // A broken clock that stands still must fail a test, not hang it.
        assert!(self.calls.len() < 100_000, "the run never ended");
        self.calls.push((ctx.now(), Call::Frame));
        (self.next)(ctx.now()).unwrap_or_else(|| {
            ctx.exit();
            Wake::OnEvent
        })
    }

    fn stop(&mut self, ctx: &dyn Control) {
        self.calls.push((ctx.now(), Call::Stop));
    }
}

/// Every `period` from 0, like a fixed-timestep frame loop.
fn every(period: Duration) -> impl FnMut(Instant) -> Option<Wake> {
    move |now| Some(Wake::At(now + period))
}

#[test]
fn it_is_called_headless() {
    assert_eq!(HeadlessPlatform::new().name(), "headless");
}

/// T3's done-when, on its own: a close request scripted at 500 ms reaches
/// a 100 Hz frame loop, which exits; nothing runs after it.
#[test]
fn a_scripted_close_request_reaches_the_app() {
    let close = PlatformEvent::CloseRequested;
    let mut platform = HeadlessPlatform::new().script([(ms(500), close)]);
    let mut app = Recorder::new(every(Duration::from_millis(10)));
    app.exit_on_close = true;
    assert_eq!(platform.run(&mut app), Ok(()));
    let want: Vec<_> = (0..50).map(|i| ms(i * 10)).collect();
    assert_eq!(app.frames(), want, "frames until the close request");
    let end = &app.calls[app.calls.len() - 2..];
    assert_eq!(end, [(ms(500), Call::Event(close)), (ms(500), Call::Stop)]);
}

#[test]
fn time_jumps_straight_to_each_wake() {
    let wakes = [
        ms(3_000),
        Instant::from_nanos(3_000_000_001),
        ms(86_400_000),
    ];
    let mut answers = wakes.into_iter();
    let mut app = Recorder::new(|_| answers.next().map(Wake::At));
    assert_eq!(HeadlessPlatform::new().run(&mut app), Ok(()));
    assert_eq!(app.frames(), [Instant::ZERO, wakes[0], wakes[1], wakes[2]]);
    assert_eq!(app.calls.last(), Some(&(wakes[2], Call::Stop)));
}

#[test]
fn events_due_together_come_before_one_frame() {
    let (w, a) = (
        PlatformEvent::KeyDown(Key::KeyW),
        PlatformEvent::KeyDown(Key::KeyA),
    );
    let script = [
        (Instant::ZERO, PlatformEvent::FocusGained),
        (ms(5), w),
        (ms(5), a),
        (ms(9), PlatformEvent::CloseRequested),
    ];
    let mut platform = HeadlessPlatform::new().script(script);
    // A timer at 9 ms, when the close request is also due: one frame.
    let mut app = Recorder::new(|now| (now < ms(9)).then_some(Wake::At(ms(9))));
    assert_eq!(platform.run(&mut app), Ok(()));
    let want = [
        (Instant::ZERO, Call::Start),
        (Instant::ZERO, Call::Event(PlatformEvent::FocusGained)),
        (Instant::ZERO, Call::Frame),
        (ms(5), Call::Event(w)),
        (ms(5), Call::Event(a)),
        (ms(5), Call::Frame),
        (ms(9), Call::Event(PlatformEvent::CloseRequested)),
        (ms(9), Call::Frame),
        (ms(9), Call::Stop),
    ];
    assert_eq!(app.calls, want);
}

#[test]
fn a_wake_not_after_now_comes_at_once() {
    let mut answers = [Wake::At(ms(40)), Wake::At(ms(10)), Wake::At(ms(40))].into_iter();
    let mut app = Recorder::new(|_| answers.next());
    assert_eq!(HeadlessPlatform::new().run(&mut app), Ok(()));
    assert_eq!(app.frames(), [Instant::ZERO, ms(40), ms(40), ms(40)]);
}

#[test]
fn waiting_for_an_event_when_none_is_left_fails() {
    let script = [(ms(7), PlatformEvent::KeyDown(Key::Space))];
    let mut app = Recorder::new(|_| Some(Wake::OnEvent));
    let result = HeadlessPlatform::new().script(script).run(&mut app);
    assert_eq!(result, Err(PlatformError::Idle { at: ms(7) }));
    assert_eq!(app.frames(), [Instant::ZERO, ms(7)]);
    assert_eq!(
        app.calls.last(),
        Some(&(ms(7), Call::Stop)),
        "stop still comes"
    );
}

#[test]
fn the_time_limit_ends_a_run_that_never_exits() {
    let mut platform = HeadlessPlatform::new().time_limit(ms(1_000));
    let mut app = Recorder::new(every(Duration::from_millis(100)));
    let result = platform.run(&mut app);
    assert_eq!(result, Err(PlatformError::TimeLimit { limit: ms(1_000) }));
    let want: Vec<_> = (0..=10).map(|i| ms(i * 100)).collect();
    assert_eq!(
        app.frames(),
        want,
        "a wake exactly at the limit still comes"
    );
    assert_eq!(app.calls.last(), Some(&(ms(1_000), Call::Stop)));
}

#[test]
fn events_after_the_time_limit_never_arrive() {
    let late = PlatformEvent::KeyDown(Key::KeyQ);
    let mut platform = HeadlessPlatform::new()
        .script([(ms(1_001), late)])
        .time_limit(ms(1_000));
    let mut app = Recorder::new(|_| Some(Wake::OnEvent));
    let result = platform.run(&mut app);
    assert_eq!(result, Err(PlatformError::TimeLimit { limit: ms(1_000) }));
    assert!(!app.calls.contains(&(ms(1_001), Call::Event(late))));
}

/// On a virtual clock nothing can change while the app keeps asking to be
/// woken at once and no event comes, so the run ends instead of spinning.
#[test]
fn an_app_that_never_lets_time_pass_is_stopped() {
    let mut app = Recorder::new(|now| Some(Wake::At(now)));
    let result = HeadlessPlatform::new().run(&mut app);
    assert_eq!(result, Err(PlatformError::Stalled { at: Instant::ZERO }));
    let limit = HeadlessPlatform::STALL_FRAMES as usize;
    assert_eq!(app.frames().len(), limit);
    assert_eq!(app.calls.last(), Some(&(Instant::ZERO, Call::Stop)));
}

#[test]
fn frames_at_one_instant_are_fine_below_the_stall_limit() {
    let limit = u64::from(HeadlessPlatform::STALL_FRAMES);
    let mut n = 0;
    let mut app = Recorder::new(|now| {
        n += 1;
        match n {
            _ if n == 3 * limit => None,
            _ if n % limit == 0 => Some(Wake::At(now + Duration::from_nanos(1))),
            _ => Some(Wake::At(now)),
        }
    });
    assert_eq!(HeadlessPlatform::new().run(&mut app), Ok(()));
    assert_eq!(
        app.calls.last(),
        Some(&(Instant::from_nanos(2), Call::Stop))
    );
}

#[test]
fn exit_in_start_ends_the_run_at_once() {
    struct Leaves(Vec<&'static str>);
    impl App for Leaves {
        fn start(&mut self, ctx: &mut dyn Control) {
            self.0.push("start");
            ctx.exit();
        }
        fn event(&mut self, _: &mut dyn Control, _: PlatformEvent) {
            self.0.push("event");
        }
        fn frame(&mut self, _: &mut dyn Control) -> Wake {
            self.0.push("frame");
            Wake::OnEvent
        }
        fn stop(&mut self, _: &dyn Control) {
            self.0.push("stop");
        }
    }
    let mut platform =
        HeadlessPlatform::new().script([(Instant::ZERO, PlatformEvent::FocusGained)]);
    let mut app = Leaves(Vec::new());
    assert_eq!(platform.run(&mut app), Ok(()));
    assert_eq!(app.0, ["start", "stop"]);
}

#[test]
fn a_run_can_be_repeated_and_repeats_exactly() {
    let script = [
        (ms(3), PlatformEvent::KeyDown(Key::KeyD)),
        (ms(8), PlatformEvent::KeyUp(Key::KeyD)),
        (ms(20), PlatformEvent::CloseRequested),
    ];
    let mut platform = HeadlessPlatform::new().script(script);
    let mut runs = Vec::new();
    for _ in 0..2 {
        let mut app = Recorder::new(every(Duration::from_millis(5)));
        app.exit_on_close = true;
        assert_eq!(platform.run(&mut app), Ok(()));
        runs.push(app.calls);
    }
    assert_eq!(runs[0], runs[1]);
    assert_eq!(
        runs[0][0],
        (Instant::ZERO, Call::Start),
        "the clock starts again"
    );
}

#[test]
#[should_panic(expected = "a headless script must be in time order")]
fn a_script_out_of_time_order_is_refused() {
    let _ = HeadlessPlatform::new().script([
        (ms(9), PlatformEvent::FocusGained),
        (ms(4), PlatformEvent::FocusLost),
    ]);
}

// --- Properties -------------------------------------------------------

/// What a frame answers in the property test.
#[derive(Debug, Clone, Copy)]
enum Answer {
    /// `Wake::At(now + this many ms)`; 0 is "at once".
    In(u64),
    OnEvent,
    Exit,
}

fn any_answer() -> impl Strategy<Value = Answer> {
    prop_oneof![
        6 => (0u64..12).prop_map(Answer::In),
        2 => Just(Answer::OnEvent),
        1 => Just(Answer::Exit),
    ]
}

fn any_event() -> impl Strategy<Value = PlatformEvent> {
    prop_oneof![
        Just(PlatformEvent::KeyDown(Key::KeyW)),
        Just(PlatformEvent::KeyUp(Key::KeyW)),
        Just(PlatformEvent::FocusLost),
        Just(PlatformEvent::FocusGained),
        Just(PlatformEvent::CloseRequested),
        (-4i32..4).prop_map(|d| PlatformEvent::MouseMotion {
            dx: f64::from(d),
            dy: 1.0
        }),
    ]
}

/// A script in time order (gaps of 0 to 9 ms), answers for the frames
/// (ending with an exit), and maybe a time limit.
fn any_run() -> impl Strategy<Value = (Vec<(Instant, PlatformEvent)>, Vec<Answer>, Option<u64>)> {
    (
        prop::collection::vec((0u64..10, any_event()), 0..24),
        prop::collection::vec(any_answer(), 1..40),
        prop::option::of(0u64..120),
    )
        .prop_map(|(steps, mut answers, limit)| {
            // The app always exits in the end, so no run spins forever.
            answers.push(Answer::Exit);
            let mut at = 0;
            let script = steps
                .into_iter()
                .map(|(gap, event)| {
                    at += gap;
                    (ms(at), event)
                })
                .collect();
            (script, answers, limit)
        })
}

/// The rules written out: what a run must look like, from the script, the
/// answers (the last one repeats) and the limit. A close request exits.
fn model(
    script: &[(Instant, PlatformEvent)],
    answers: &[Answer],
    limit: Option<u64>,
) -> (Vec<(Instant, Call)>, Result<(), PlatformError>) {
    let mut calls = vec![(Instant::ZERO, Call::Start)];
    let mut now = Instant::ZERO;
    let mut next = 0;
    let mut frame = 0;
    let end = loop {
        while next < script.len() && script[next].0 <= now {
            calls.push((now, Call::Event(script[next].1)));
            next += 1;
            if script[next - 1].1 == PlatformEvent::CloseRequested {
                calls.push((now, Call::Stop));
                return (calls, Ok(()));
            }
        }
        calls.push((now, Call::Frame));
        let answer = answers[frame.min(answers.len() - 1)];
        frame += 1;
        let timer = match answer {
            Answer::Exit => break Ok(()),
            Answer::In(d) => Some(now + Duration::from_millis(d)),
            Answer::OnEvent => None,
        };
        let event = script.get(next).map(|&(at, _)| at);
        let wake = match (timer, event) {
            (Some(t), Some(e)) => t.min(e),
            (t, e) => match t.or(e) {
                Some(at) => at,
                None => break Err(PlatformError::Idle { at: now }),
            },
        };
        if let Some(limit) = limit.map(ms)
            && wake > limit
        {
            break Err(PlatformError::TimeLimit { limit });
        }
        now = wake;
    };
    calls.push((now, Call::Stop));
    (calls, end)
}

proptest! {
    #[test]
    fn runs_follow_the_virtual_clock((script, answers, limit) in any_run()) {
        let mut platform = HeadlessPlatform::new().script(script.iter().copied());
        if let Some(limit) = limit {
            platform = platform.time_limit(ms(limit));
        }
        let mut frame = 0;
        let mut app = Recorder::new(|now: Instant| {
            let answer = answers[frame.min(answers.len() - 1)];
            frame += 1;
            match answer {
                Answer::In(d) => Some(Wake::At(now + Duration::from_millis(d))),
                Answer::OnEvent => Some(Wake::OnEvent),
                Answer::Exit => None,
            }
        });
        app.exit_on_close = true;
        let result = platform.run(&mut app);
        let (calls, end) = model(&script, &answers, limit);
        prop_assert_eq!(&app.calls, &calls);
        prop_assert_eq!(result, end);
    }
}
