//! When a first-person frame is drawn (plan decision 10): at the window's
//! refresh rate when it reports one, else once per tick. Ticks still come
//! from the timer ([`crate::TickClock`]).
//!
//! The platform calls the app's `frame` after every batch of events, at
//! once (winit's `pace.rs`), and raw mouse motion comes in batches up to a
//! thousand times a second. So drawing on every `frame` would draw as fast
//! as the mouse moves wherever the present does not wait for the display
//! (Xvfb has no vsync: rung 1b's T11 saw it spin). Instead a frame is drawn
//! only when one is due; the other callbacks run the ticks due and return.
//!
//! - **A refresh rate known**: a frame is due a refresh period after the
//!   last one's due time, less a sixteenth of a period, so where the
//!   present waits for the display (Fifo), the display paces the frames
//!   and the timer never makes one miss its refresh; where it does not,
//!   the timer paces them, at most 16/15 of the rate.
//! - **None**: a tick period, with no slack, starting with the first tick
//!   clock's, so each frame comes in the callback that runs a tick, as in
//!   rung 1b.
//! - A frame more than a period late starts the cadence again rather than
//!   drawing several at once to catch up: from the next tick without a
//!   refresh rate, so frames stay on the ticks, else from now.

use eng_platform_api::Instant;

/// The cadence of drawn frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DrawPacer {
    period: u64,
    slack: u64,
    /// When the next frame is due; `None` before the first.
    next: Option<u64>,
}

impl DrawPacer {
    /// Frames for a window refreshing at `refresh_mhz` (millihertz), or
    /// once a tick at `tick_rate` without one.
    pub(crate) fn new(refresh_mhz: Option<u32>, tick_rate: u32) -> Self {
        let mut pacer = Self {
            period: 0,
            slack: 0,
            next: None,
        };
        pacer.set_rate(refresh_mhz, tick_rate);
        pacer
    }

    /// The window's refresh rate changed (another monitor), or became
    /// known. The next frame's due time stays.
    pub(crate) fn set_rate(&mut self, refresh_mhz: Option<u32>, tick_rate: u32) {
        (self.period, self.slack) = match refresh_mhz.filter(|&mhz| mhz > 0) {
            Some(mhz) => {
                let period = (1_000_000_000_000 / u64::from(mhz)).max(1);
                (period, period / 16)
            }
            // As `TickClock` counts a tick.
            None => (1_000_000_000 / u64::from(tick_rate.max(1)), 0),
        };
    }

    /// Nanoseconds between frames.
    pub(crate) const fn period(&self) -> u64 {
        self.period
    }

    /// Whether a frame is due at `now`.
    pub(crate) fn due(&self, now: Instant) -> bool {
        self.next
            .is_none_or(|next| now.as_nanos() >= next.saturating_sub(self.slack))
    }

    /// A frame was drawn, or tried (a hidden window draws nothing, and the
    /// next is still a period on), at `now`, with the next tick due at
    /// `next_tick`.
    pub(crate) fn drew(&mut self, now: Instant, next_tick: Instant) {
        let now = now.as_nanos();
        let next = self.next.unwrap_or(now).saturating_add(self.period);
        // Still due: the frame came more than a period late.
        let behind = now >= next.saturating_sub(self.slack);
        self.next = Some(match (behind, self.slack) {
            (false, _) => next,
            // Paced by ticks: back onto them.
            (true, 0) => next_tick.as_nanos().max(now.saturating_add(1)),
            (true, _) => now.saturating_add(self.period),
        });
    }

    /// When to wake for the next frame; `None` before the first.
    pub(crate) fn wake(&self) -> Option<Instant> {
        self.next
            .map(|next| Instant::from_nanos(next.saturating_sub(self.slack)))
    }
}

#[cfg(test)]
mod tests {
    use eng_platform_api::Instant;

    use super::DrawPacer;

    const TICK: u64 = 16_666_666;

    /// A next tick that never comes, where it does not matter.
    const NEVER: Instant = Instant::from_nanos(u64::MAX);

    fn at(nanos: u64) -> Instant {
        Instant::from_nanos(nanos)
    }

    #[test]
    fn without_a_refresh_rate_frames_come_once_a_tick_on_the_ticks() {
        let mut pacer = DrawPacer::new(None, 60);
        assert_eq!(pacer.period(), TICK);
        assert!(pacer.due(at(0)) && pacer.wake().is_none());
        pacer.drew(at(0), NEVER);
        assert_eq!(pacer.wake(), Some(at(TICK)));
        assert!(!pacer.due(at(TICK - 1)));
        assert!(pacer.due(at(TICK)));
        // A frame a little late keeps the cadence.
        pacer.drew(at(TICK + 500_000), NEVER);
        assert_eq!(pacer.wake(), Some(at(2 * TICK)));
    }

    #[test]
    fn with_one_they_come_at_it_less_a_sixteenth() {
        let mut pacer = DrawPacer::new(Some(120_000), 60);
        assert_eq!(pacer.period(), 8_333_333);
        pacer.drew(at(1_000), NEVER);
        // Due 8,333,333 - 520,833 ns after the first.
        let wake = 1_000 + 8_333_333 - 520_833;
        assert_eq!(pacer.wake(), Some(at(wake)));
        assert!(!pacer.due(at(wake - 1)) && pacer.due(at(wake)));
        // Drawn early within the slack, the cadence holds.
        pacer.drew(at(wake), NEVER);
        assert_eq!(pacer.wake(), Some(at(wake + 8_333_333)));
    }

    #[test]
    fn many_events_between_frames_draw_nothing_more() {
        let mut pacer = DrawPacer::new(Some(60_000), 60);
        pacer.drew(at(0), NEVER);
        let drawn = (1..1_000)
            .map(|ms| at(ms * 1_000_000))
            .filter(|&now| {
                let due = pacer.due(now);
                if due {
                    pacer.drew(now, NEVER);
                }
                due
            })
            .count();
        // A second of millisecond batches: 60 frames, give or take the edge.
        assert!((59..=61).contains(&drawn), "{drawn}");
    }

    #[test]
    fn a_frame_a_period_late_starts_again_on_the_next_tick_or_from_now() {
        let mut pacer = DrawPacer::new(None, 60);
        pacer.drew(at(0), at(TICK));
        // A wake 3 ms past tick 5: back onto the ticks, not mid-tick.
        pacer.drew(at(5 * TICK + 3_000_000), at(6 * TICK));
        assert_eq!(pacer.wake(), Some(at(6 * TICK)));
        let mut pacer = DrawPacer::new(Some(120_000), 60);
        pacer.drew(at(0), at(TICK));
        pacer.drew(at(50_000_000), at(4 * TICK));
        assert_eq!(pacer.wake(), Some(at(50_000_000 + 8_333_333 - 520_833)));
    }

    #[test]
    fn a_new_rate_takes_the_frame_after_next() {
        let mut pacer = DrawPacer::new(None, 60);
        pacer.drew(at(0), NEVER);
        pacer.set_rate(Some(120_000), 60);
        // The frame due stays due when it was.
        assert_eq!(pacer.wake(), Some(at(TICK - 520_833)));
        pacer.set_rate(Some(0), 60);
        assert_eq!(pacer.period(), TICK, "0 Hz is no rate");
    }
}
