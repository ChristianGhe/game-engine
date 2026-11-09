//! The mouse's own rate (criterion 25, `gcmouse-rate`): on macOS the
//! GCMouse shim counts its callbacks (`WinitPlatform::raw_motion_callbacks`),
//! and the frame loop reads that count once per `frame`.
//!
//! The count includes callbacks while the window lacks focus, and runs on
//! across the platform's runs, so only its growth while the window has
//! focus counts: a span starts at the first reading with focus and ends at
//! a reading without it. The peak rate is the most callbacks per second
//! over any stretch of at least a second inside one span, ending at a
//! reading: from the latest reading at least a second before it. A mouse
//! at rest sends no callbacks, so moving it fast is what shows the peak,
//! which criterion 25 compares with `Window::refresh_rate()`.

use std::collections::VecDeque;

use eng_cli::MouseRate;
use eng_platform_api::Instant;

const SECOND: u64 = 1_000_000_000;

/// Readings of a callback count and what they show.
#[derive(Debug, Default)]
pub(crate) struct CallbackRate {
    /// The current span's readings `(time, count)`, from the latest at
    /// least a second before the newest.
    span: VecDeque<(u64, u64)>,
    rate: MouseRate,
    /// Time focused while read, in nanoseconds, summed before dividing.
    focused_ns: u64,
}

impl CallbackRate {
    /// One reading of the count at `now`, with the window `focused` or not.
    pub(crate) fn read(&mut self, now: Instant, count: u64, focused: bool) {
        let now = now.as_nanos();
        if !focused {
            self.span.clear();
            return;
        }
        if let Some(&(then, before)) = self.span.back() {
            self.rate.callbacks += count.saturating_sub(before);
            self.focused_ns = self.focused_ns.saturating_add(now.saturating_sub(then));
        }
        self.span.push_back((now, count));
        // Keep the latest reading at least a second old, and nothing older.
        while self.span.len() > 2 && self.span[1].0.saturating_add(SECOND) <= now {
            self.span.pop_front();
        }
        let (start, first) = self.span[0];
        let over = now - start;
        if over >= SECOND {
            let per_second =
                u128::from(count.saturating_sub(first)) * u128::from(SECOND) / u128::from(over);
            let per_second = u64::try_from(per_second).unwrap_or(u64::MAX);
            self.rate.peak_per_second = self.rate.peak_per_second.max(per_second);
        }
    }

    /// What the readings showed.
    pub(crate) const fn rate(&self) -> MouseRate {
        MouseRate {
            focused_ms: self.focused_ns / 1_000_000,
            ..self.rate
        }
    }
}

#[cfg(test)]
mod tests {
    use eng_platform_api::Instant;

    use super::{CallbackRate, SECOND};

    fn ms(millis: u64) -> Instant {
        Instant::from_millis(millis)
    }

    #[test]
    fn a_steady_mouse_shows_its_rate_after_a_second() {
        let mut rate = CallbackRate::default();
        // Readings every 8 ms (a 120 Hz display), the mouse at 1000 Hz.
        for step in 0..=250u64 {
            rate.read(ms(step * 8), 5_000 + step * 8, true);
            if step * 8 < 1_000 {
                assert_eq!(rate.rate().peak_per_second, 0, "no full second yet");
            }
        }
        let shown = rate.rate();
        assert_eq!(shown.peak_per_second, 1_000);
        assert_eq!(shown.callbacks, 2_000);
        assert_eq!(shown.focused_ms, 2_000);
    }

    #[test]
    fn focused_time_is_summed_before_rounding() {
        let mut rate = CallbackRate::default();
        // 1,000 readings 1.5 ms apart: 1.5 s, not 1,000 ms rounded down.
        for step in 0..=1_000u64 {
            rate.read(Instant::from_nanos(step * 1_500_000), step, true);
        }
        assert_eq!(rate.rate().focused_ms, 1_500);
    }

    #[test]
    fn the_peak_is_the_fastest_second() {
        let mut rate = CallbackRate::default();
        let mut count = 0;
        for step in 0..=300u64 {
            // 10 ms readings: 2 callbacks each, then a burst of 20 each
            // between 1.0 and 1.5 s.
            count += if (100..150).contains(&step) { 20 } else { 2 };
            rate.read(ms(step * 10), count, true);
        }
        // The best second holds the half second of burst: 50 x 20 + 50 x 2.
        assert_eq!(rate.rate().peak_per_second, 1_100);
    }

    #[test]
    fn only_growth_with_focus_counts() {
        let mut rate = CallbackRate::default();
        rate.read(ms(0), 100, true);
        rate.read(ms(500), 600, true);
        // Unfocused: 10,000 callbacks the window never saw.
        rate.read(ms(600), 700, false);
        rate.read(ms(900), 10_700, false);
        // Focus again: a new span, which needs its own second.
        rate.read(ms(1_000), 10_800, true);
        assert_eq!(rate.rate().peak_per_second, 0);
        rate.read(ms(2_000), 11_300, true);
        let shown = rate.rate();
        assert_eq!(shown.peak_per_second, 500);
        assert_eq!(shown.callbacks, 500 + 500);
        assert_eq!(shown.focused_ms, 1_500);
        assert_eq!(SECOND, 1_000_000_000);
    }
}
