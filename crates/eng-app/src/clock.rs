//! The frame loop's clock (plan, "One frame"): a fixed-timestep accumulator
//! on the platform's monotonic clock. Ticks come from the timer, not from
//! presenting (spec 6.6).

use eng_platform_api::Instant;

/// The most ticks one wake runs. A frame later than that drops the rest:
/// the simulation falls behind the clock instead of spending ever longer
/// catching up (plan, "One frame").
pub const MAX_TICKS_PER_WAKE: u32 = 5;

/// What one wake of the frame loop does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Due {
    /// Ticks to run now, at most [`MAX_TICKS_PER_WAKE`].
    pub run: u32,
    /// Ticks of time dropped: due, but past the limit.
    pub dropped: u64,
}

/// When each tick is due: a tick period apart from a start, in whole
/// nanoseconds (1/60 s rounded down at 60 Hz, as the headless ticker
/// paces `run`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickClock {
    period: u64,
    /// When the next tick is due, in nanoseconds of the platform's clock.
    next: u64,
}

impl TickClock {
    /// The clock for a run whose latest tick was at `start`: the next one is
    /// due a period later.
    ///
    /// # Panics
    /// If `tick_rate` is zero.
    pub fn new(tick_rate: u32, start: Instant) -> Self {
        assert!(tick_rate > 0, "tick rate must be positive");
        let period = 1_000_000_000 / u64::from(tick_rate);
        Self {
            period,
            next: start.as_nanos().saturating_add(period),
        }
    }

    /// When the next tick is due: the time to ask the platform to wake.
    pub fn next(&self) -> Instant {
        Instant::from_nanos(self.next)
    }

    /// The ticks due by `now`, at most [`MAX_TICKS_PER_WAKE`]; the clock
    /// moves past every one of them, dropped ones included.
    pub fn due(&mut self, now: Instant) -> Due {
        let now = now.as_nanos();
        if now < self.next {
            return Due { run: 0, dropped: 0 };
        }
        let due = (now - self.next) / self.period + 1;
        self.next = self.next.saturating_add(due.saturating_mul(self.period));
        let run = due.min(u64::from(MAX_TICKS_PER_WAKE));
        Due {
            // At most MAX_TICKS_PER_WAKE, so it fits.
            run: run as u32,
            dropped: due - run,
        }
    }

    /// How far `now` is from the latest tick (0) to the next one (1), for
    /// interpolating between their states.
    pub fn alpha(&self, now: Instant) -> f64 {
        let latest = self.next.saturating_sub(self.period);
        let since = now.as_nanos().saturating_sub(latest).min(self.period);
        since as f64 / self.period as f64
    }
}
