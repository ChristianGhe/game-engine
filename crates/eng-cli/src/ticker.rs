//! `run`'s pace: one tick per frame, a tick apart on the headless
//! platform's virtual clock, so the probe runs on the same platform
//! contract as every other mode, as fast as the machine allows.

use std::time::Duration;

use eng_platform_api::{App, Control, Platform, Wake};
use eng_platform_headless::HeadlessPlatform;

use crate::Exit;

/// What one tick decided.
pub(crate) enum Flow {
    /// Go on to the next tick, if there is one.
    Next,
    /// Stop here; the run's outputs are still written.
    Stop,
    /// Stop here and leave with this exit, writing nothing more.
    Exit(Exit),
}

/// An app that calls `tick` once per frame, `ticks` times, unless a tick
/// stops early.
pub(crate) struct Ticker<F> {
    period: Duration,
    left: u64,
    tick: F,
    exit: Option<Exit>,
}

impl<F: FnMut() -> Flow> Ticker<F> {
    /// # Panics
    /// If `tick_rate` is zero.
    pub(crate) fn new(tick_rate: u32, ticks: u64, tick: F) -> Self {
        assert!(tick_rate > 0, "tick rate must be positive");
        Self {
            period: Duration::from_nanos(1_000_000_000 / u64::from(tick_rate)),
            left: ticks,
            tick,
            exit: None,
        }
    }

    /// Runs the ticks on the headless platform. Returns the exit a tick
    /// left with, if one did.
    pub(crate) fn run(mut self) -> Option<Exit> {
        // Nothing is scripted and every frame asks for a time, so the run
        // ends only when the ticker exits.
        HeadlessPlatform::new()
            .run(&mut self)
            .unwrap_or_else(|e| panic!("[E-PLATFORM] the headless run failed: {e}"));
        self.exit
    }
}

impl<F: FnMut() -> Flow> App for Ticker<F> {
    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        if self.left > 0 {
            self.left -= 1;
            match (self.tick)() {
                Flow::Next if self.left > 0 => return Wake::At(ctx.now() + self.period),
                Flow::Next | Flow::Stop => {}
                Flow::Exit(exit) => self.exit = Some(exit),
            }
        }
        ctx.exit();
        Wake::OnEvent
    }
}

#[cfg(test)]
mod tests {
    use super::{Flow, Ticker};
    use crate::Exit;

    /// Runs `ticks` ticks whose answers come from `flow(n)`, n from 1.
    fn ticks(ticks: u64, flow: impl Fn(u32) -> Flow) -> (u32, Option<Exit>) {
        let mut count = 0;
        let exit = Ticker::new(60, ticks, || {
            count += 1;
            flow(count)
        })
        .run();
        (count, exit)
    }

    #[test]
    fn every_tick_runs_once() {
        assert_eq!(ticks(5, |_| Flow::Next), (5, None));
        assert_eq!(ticks(0, |_| Flow::Next), (0, None));
    }

    #[test]
    fn a_tick_can_stop_the_run() {
        let stop_at_3 = |n| if n == 3 { Flow::Stop } else { Flow::Next };
        assert_eq!(ticks(10, stop_at_3), (3, None));
    }

    #[test]
    fn a_tick_can_leave_with_an_exit() {
        let leave_at_4 = |n| {
            if n == 4 {
                Flow::Exit(Exit::Desync)
            } else {
                Flow::Next
            }
        };
        assert_eq!(ticks(10, leave_at_4), (4, Some(Exit::Desync)));
        assert_eq!(
            ticks(1, |_| Flow::Exit(Exit::Usage)),
            (1, Some(Exit::Usage))
        );
    }
}
