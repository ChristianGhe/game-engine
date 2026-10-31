//! The frame loop: the platform's callbacks, answered (plan, "One frame").
//!
//! Each frame runs the ticks the clock says are due, at most five, each with
//! one sample of live input, then draws the newest states interpolated at
//! the clock's alpha, and asks to be woken when the next tick is due.
//! `quit` (Esc by default) and a close request end the run; the files are
//! written in `stop`.

use std::sync::Arc;

use eng_cli::{Exit, PlayRun, PlayStats, Step};
use eng_input::LiveInput;
use eng_platform_api::window::Window;
use eng_platform_api::{App, Control, PlatformEvent, Wake};

use crate::clock::TickClock;
use crate::view::View;

/// `play` as a platform app: the run, live input, the clock and, with a
/// window, the view.
pub(crate) struct Frames<'g> {
    run: Option<PlayRun<'g>>,
    live: LiveInput,
    tick_rate: u32,
    platform: &'static str,
    clock: Option<TickClock>,
    view: Option<View>,
    /// The window or GPU could not open: the message, printed in `stop`.
    refused: Option<String>,
    pub(crate) exit: Option<Exit>,
}

impl<'g> Frames<'g> {
    pub(crate) fn new(run: PlayRun<'g>, platform: &'static str) -> Self {
        Self {
            live: LiveInput::new(run.bindings().clone()),
            tick_rate: run.game().tick_rate,
            run: Some(run),
            platform,
            clock: None,
            view: None,
            refused: None,
            exit: None,
        }
    }

    /// Runs the ticks due by now. `false`: the run is over.
    fn tick(&mut self, ctx: &mut dyn Control) -> bool {
        let now = ctx.now();
        let clock = self
            .clock
            .get_or_insert_with(|| TickClock::new(self.tick_rate, now));
        let due = clock.due(now);
        let run = self.run.as_mut().expect("the run lives until stop");
        if due.dropped > 0 {
            run.dropped(due.dropped);
        }
        for _ in 0..due.run {
            // One sample per tick: quit is an app action and never reaches
            // the simulation (plan decision 8).
            let input = self.live.sample(run.next_tick());
            if input.quit() || run.step(input.command) == Step::Done {
                return false;
            }
        }
        !run.is_done()
    }
}

impl App for Frames<'_> {
    fn window(&mut self, ctx: &mut dyn Control, window: Arc<dyn Window>) {
        match View::open(window) {
            Ok(view) => self.view = Some(view),
            Err(message) => {
                self.refused = Some(message);
                ctx.exit();
            }
        }
    }

    fn event(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        if let Some(key) = event.key_event() {
            self.live.handle(key);
        }
        match event {
            PlatformEvent::CloseRequested => ctx.exit(),
            PlatformEvent::Resized(size) => {
                if let Some(view) = &mut self.view {
                    view.resize(size.width, size.height);
                }
            }
            PlatformEvent::Occluded(occluded) => {
                if let Some(view) = &mut self.view {
                    view.occluded = occluded;
                }
            }
            _ => {}
        }
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        if !self.tick(ctx) {
            ctx.exit();
            return Wake::OnEvent;
        }
        let clock = self.clock.expect("tick started the clock");
        if let Some(view) = &mut self.view {
            let run = self.run.as_mut().expect("the run lives until stop");
            view.draw(run, clock.alpha(ctx.now()));
        }
        Wake::At(clock.next())
    }

    fn stop(&mut self, ctx: &dyn Control) {
        let _ = ctx;
        // The surface goes before the window it was made from.
        let gpu = self.view.take().map(View::close);
        let run = self.run.take().expect("stop comes once");
        self.exit = Some(match self.refused.take() {
            Some(message) => run.refuse(&message),
            None => run.finish(PlayStats {
                platform: self.platform,
                gpu,
            }),
        });
    }
}

impl<'g> Frames<'g> {
    /// The run, if `stop` never came: a platform that could not start.
    pub(crate) fn into_unfinished(self) -> Option<PlayRun<'g>> {
        self.run
    }
}
