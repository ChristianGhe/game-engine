//! The contract's plain data: the monotonic clock's `Instant`, sizes and
//! errors, and that the traits work as trait objects.

use std::time::Duration;

use eng_platform_api::{App, Control, Instant, PhysicalSize, Platform, PlatformError, Wake};

#[test]
fn an_instant_is_nanoseconds_since_the_run_started() {
    let t = Instant::from_nanos(1_500_000_000);
    assert_eq!(t.as_nanos(), 1_500_000_000);
    assert_eq!(Instant::ZERO.as_nanos(), 0);
    assert_eq!(Instant::default(), Instant::ZERO);
    assert_eq!(Instant::from_millis(1_500), t);
    assert!(Instant::ZERO < t);
    assert_eq!(t + Duration::from_millis(250), Instant::from_millis(1_750));
    assert_eq!(
        t.duration_since(Instant::from_millis(500)),
        Duration::from_secs(1)
    );
    assert_eq!(Instant::ZERO.duration_since(t), Duration::ZERO, "saturates");
    let last = Instant::from_nanos(u64::MAX);
    assert_eq!(last.checked_add(Duration::from_nanos(1)), None);
    assert_eq!(t.checked_add(Duration::ZERO), Some(t));
    assert_eq!(t.to_string(), "1.500000000 s");
    assert_eq!(Instant::from_nanos(7).to_string(), "0.000000007 s");
}

#[test]
#[should_panic(expected = "overflows the platform clock")]
fn adding_past_the_end_of_the_clock_panics() {
    let _ = Instant::from_nanos(u64::MAX) + Duration::from_nanos(1);
}

#[test]
fn sizes_and_wakes_are_plain_data() {
    let size = PhysicalSize::new(800, 600);
    assert_eq!((size.width, size.height), (800, 600));
    assert_eq!(PhysicalSize::default(), PhysicalSize::new(0, 0));
    assert_eq!(
        Wake::At(Instant::from_millis(5)),
        Wake::At(Instant::from_millis(5))
    );
    assert_ne!(Wake::At(Instant::ZERO), Wake::OnEvent);
}

#[test]
fn errors_say_what_happened() {
    let cases = [
        (
            PlatformError::Unavailable("no display".into()),
            "the platform cannot run here: no display",
        ),
        (
            PlatformError::Failed("the event loop died".into()),
            "the platform failed while running: the event loop died",
        ),
        (
            PlatformError::Idle {
                at: Instant::from_millis(13),
            },
            "the app waited for an event at 0.013000000 s, and none is left to come",
        ),
        (
            PlatformError::TimeLimit {
                limit: Instant::from_millis(5_000),
            },
            "the app had not exited by the time limit, 5.000000000 s",
        ),
        (
            PlatformError::Stalled {
                at: Instant::from_millis(40),
            },
            "the app kept asking to be woken at once at 0.040000000 s, so the clock never moved",
        ),
    ];
    for (error, shown) in cases {
        assert_eq!(error.to_string(), shown);
    }
}

/// A platform that runs nothing: enough to show that both traits work as
/// trait objects, so an app can be run by a platform chosen at run time.
struct Nothing;

impl Platform for Nothing {
    fn name(&self) -> &'static str {
        "nothing"
    }

    fn run(&mut self, _app: &mut dyn App) -> Result<(), PlatformError> {
        Err(PlatformError::Unavailable("it runs nothing".into()))
    }
}

struct Idle;

impl App for Idle {
    fn frame(&mut self, _ctx: &mut dyn Control) -> Wake {
        Wake::OnEvent
    }
}

#[test]
fn platforms_and_apps_are_object_safe() {
    let mut platforms: Vec<Box<dyn Platform>> = vec![Box::new(Nothing)];
    let app: &mut dyn App = &mut Idle;
    for platform in &mut platforms {
        assert_eq!(platform.name(), "nothing");
        assert!(matches!(
            platform.run(app),
            Err(PlatformError::Unavailable(_))
        ));
    }
}
