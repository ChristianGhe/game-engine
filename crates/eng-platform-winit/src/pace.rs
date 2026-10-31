//! The run's clock and the contract's wake rules, kept apart from winit
//! so they can be tested without a display.

use std::time::Duration;

use eng_platform_api::{Instant, Wake};

/// The run's clock: time since the run started, from the system's
/// monotonic clock.
pub(crate) struct Clock {
    origin: std::time::Instant,
    /// The latest reading, which the next may not go below.
    last: Instant,
}

impl Clock {
    /// A clock that reads zero now.
    pub(crate) fn start() -> Self {
        Self {
            origin: std::time::Instant::now(),
            last: Instant::ZERO,
        }
    }

    /// The time now.
    pub(crate) fn now(&mut self) -> Instant {
        let elapsed = self.origin.elapsed();
        self.read(elapsed)
    }

    /// The reading for `elapsed` since the start. std's clock is monotonic
    /// on every desktop, but std documents hardware where it is not; a
    /// reading never goes below the one before, so the contract's clock
    /// never goes back. Past its range (about 584 years) it stays at the
    /// end.
    pub(crate) fn read(&mut self, elapsed: Duration) -> Instant {
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.last = self.last.max(Instant::from_nanos(nanos));
        self.last
    }

    /// When `at` comes on the system's clock; `None` past its range.
    pub(crate) fn system_time(&self, at: Instant) -> Option<std::time::Instant> {
        self.origin.checked_add(Duration::from_nanos(at.as_nanos()))
    }
}

/// The longest the loop sleeps without looking at the time again, so a
/// far-off wake never reaches the system's timer.
pub(crate) const MAX_WAIT: Duration = Duration::from_secs(60);

/// When the next frame is due: the contract's rules, given what the app
/// last asked for and whether an event has come since.
#[derive(Debug, Default)]
pub(crate) struct Pacer {
    /// The app's answer to its last frame; `None` before the first.
    asked: Option<Wake>,
    /// An event has reached the app since its last frame.
    event: bool,
}

impl Pacer {
    /// An event reached the app.
    pub(crate) fn event(&mut self) {
        self.event = true;
    }

    /// The app answered a frame with `wake`.
    pub(crate) fn framed(&mut self, wake: Wake) {
        self.asked = Some(wake);
        self.event = false;
    }

    /// Whether the frame is due at `now`: the first at once; after an
    /// event, at once; after `At(t)`, from `t`; after `OnEvent`, only
    /// after an event. winit may wake the loop sooner (an OS event the
    /// contract leaves out, a timer firing early), and then it is not.
    pub(crate) fn due(&self, now: Instant) -> bool {
        self.event
            || match self.asked {
                None => true,
                Some(Wake::At(at)) => now >= at,
                Some(Wake::OnEvent) => false,
            }
    }

    /// When to wake the loop for the next frame, at `now`: `None` waits
    /// for an event. A wake further off than [`MAX_WAIT`] wakes the loop
    /// after `MAX_WAIT` instead, to look again.
    pub(crate) fn deadline(&self, now: Instant) -> Option<Instant> {
        match self.asked? {
            Wake::At(at) => Some(match now.checked_add(MAX_WAIT) {
                Some(look) => at.min(look),
                None => at,
            }),
            Wake::OnEvent => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Instant {
        Instant::from_millis(millis)
    }

    #[test]
    fn the_first_frame_comes_at_once() {
        let pacer = Pacer::default();
        assert!(pacer.due(Instant::ZERO));
    }

    #[test]
    fn a_timed_wake_is_due_at_its_time_and_not_before() {
        let mut pacer = Pacer::default();
        pacer.framed(Wake::At(ms(5)));
        assert_eq!(pacer.deadline(Instant::ZERO), Some(ms(5)));
        assert!(!pacer.due(Instant::ZERO));
        assert!(!pacer.due(Instant::from_nanos(4_999_999)));
        assert!(pacer.due(ms(5)));
        assert!(pacer.due(ms(6)));
    }

    #[test]
    fn a_wake_in_the_past_is_due_at_once() {
        let mut pacer = Pacer::default();
        pacer.framed(Wake::At(Instant::ZERO));
        assert!(pacer.due(ms(3)));
        assert_eq!(pacer.deadline(ms(3)), Some(Instant::ZERO));
    }

    #[test]
    fn an_event_makes_the_frame_due_at_once() {
        let mut pacer = Pacer::default();
        pacer.framed(Wake::At(ms(100)));
        pacer.event();
        assert!(pacer.due(ms(1)));
        // The frame after it answers again, and only its answer counts.
        pacer.framed(Wake::At(ms(100)));
        assert!(!pacer.due(ms(2)));
    }

    #[test]
    fn a_wait_for_events_ends_only_with_an_event() {
        let mut pacer = Pacer::default();
        pacer.framed(Wake::OnEvent);
        assert_eq!(pacer.deadline(ms(1)), None);
        assert!(!pacer.due(ms(1_000_000)));
        pacer.event();
        assert!(pacer.due(ms(1_000_000)));
        pacer.framed(Wake::OnEvent);
        assert!(!pacer.due(ms(2_000_000)));
    }

    #[test]
    fn the_clock_starts_at_zero_and_counts_nanoseconds() {
        let mut clock = Clock::start();
        assert_eq!(clock.read(Duration::ZERO), Instant::ZERO);
        assert_eq!(
            clock.read(Duration::from_nanos(1_500_000_123)),
            Instant::from_nanos(1_500_000_123)
        );
    }

    #[test]
    fn the_clock_never_goes_back() {
        let mut clock = Clock::start();
        assert_eq!(clock.read(Duration::from_millis(5)), ms(5));
        // A system clock that steps back: the reading stands still.
        assert_eq!(clock.read(Duration::from_millis(3)), ms(5));
        assert_eq!(clock.read(Duration::from_millis(7)), ms(7));
        // Past the clock's range (about 584 years) it stays at the end.
        let end = Instant::from_nanos(u64::MAX);
        assert_eq!(clock.read(Duration::MAX), end);
    }

    #[test]
    fn a_far_wake_looks_again_after_max_wait_and_is_still_not_due() {
        let mut pacer = Pacer::default();
        let never = Instant::from_nanos(u64::MAX);
        pacer.framed(Wake::At(never));
        let look = ms(1) + MAX_WAIT;
        assert_eq!(pacer.deadline(ms(1)), Some(look));
        assert!(!pacer.due(look));
        assert_eq!(pacer.deadline(look), Some(look + MAX_WAIT));
    }

    #[test]
    fn wakes_map_to_the_system_clock() {
        let mut clock = Clock::start();
        let origin = clock.system_time(Instant::ZERO).expect("now is in range");
        let later = clock.system_time(ms(250)).expect("in range");
        assert_eq!(later.duration_since(origin), Duration::from_millis(250));
        assert!(clock.now() < ms(250), "a test step takes far less");
    }
}
