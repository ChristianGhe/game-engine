//! The platform's monotonic clock.

use core::fmt;
use core::ops::Add;
use core::time::Duration;

/// A reading of the platform's monotonic clock: nanoseconds since the run
/// started ([`Instant::ZERO`]). The headless platform's clock is virtual:
/// it jumps to each wake, so a run takes no real time and repeats exactly.
///
/// It is not wall-clock time and never goes back. Only the platform makes
/// readings, through [`Control::now`](crate::Control::now).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Instant(u64);

impl Instant {
    /// When the run started.
    pub const ZERO: Instant = Instant(0);

    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// # Panics
    /// If the time does not fit the clock (about 584 years).
    pub const fn from_millis(millis: u64) -> Self {
        match millis.checked_mul(1_000_000) {
            Some(nanos) => Self(nanos),
            None => panic!("the time overflows the platform clock"),
        }
    }

    pub const fn as_nanos(self) -> u64 {
        self.0
    }

    /// `None` if the result does not fit the clock.
    pub fn checked_add(self, duration: Duration) -> Option<Instant> {
        let nanos = u64::try_from(duration.as_nanos()).ok()?;
        self.0.checked_add(nanos).map(Instant)
    }

    /// The time from `earlier` to `self`, or zero if `earlier` is later.
    pub const fn duration_since(self, earlier: Instant) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(earlier.0))
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    /// # Panics
    /// If the result does not fit the clock (about 584 years).
    fn add(self, duration: Duration) -> Instant {
        self.checked_add(duration)
            .unwrap_or_else(|| panic!("{self} + {duration:?} overflows the platform clock"))
    }
}

impl fmt::Display for Instant {
    /// Seconds, to the nanosecond: `1.500000000 s`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{:09} s",
            self.0 / 1_000_000_000,
            self.0 % 1_000_000_000
        )
    }
}
