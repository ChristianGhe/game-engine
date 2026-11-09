//! The first-person frame loop (rung 2, T15; plan decisions 9 and 10):
//! `play` on a map whose view is first person, beside rung 1b's top-down
//! loop (`frame.rs`), which the dot map keeps as it was.
//!
//! - **Ticks** come from the timer, at most five a wake, as in 1b
//!   ([`TickClock`]); each samples live input once.
//! - **Mouse look.** Live input has mouse look on, with `--sensitivity`,
//!   `--pitch-limit` and `--invert-pitch` (eng-input's `MouseLook`), and
//!   starts from where the map's scene looks at tick 0 (the room's spawn,
//!   yaw 0), not [`eng_cli::GameDef::view_yaw`], which is the top-down
//!   view's. Raw motion (`PlatformEvent::MouseMotion`) turns the angles;
//!   each tick's sample puts them in that tick's `PlayerCommand`, the only
//!   way they reach the simulation. Movement goes along the yaw.
//! - **Between ticks** the camera's position is interpolated at the clock's
//!   alpha and its angles are the live ones, read without taking anything
//!   (`LiveInput::view_now`): presentation only. A scripted run
//!   (`--input`) shows the script's angles, from the state.
//! - **Frames** are drawn at the refresh rate when the window reports one,
//!   else once per tick (`pace.rs`), however often events wake the loop.
//! - **The window** opens fullscreen unless `--windowed`, and the cursor is
//!   grabbed while it has focus and freed when it loses it (`cursor.rs`).
//!   A scripted run never grabs. Not yet handled: under X11's `Confined`
//!   grab each raw motion arrives twice (T13, seen under Xvfb with XTEST),
//!   so mouse look there turns twice as far until a real X11 desktop is
//!   checked and the copies are paired or the core grab dropped.
//! - **The report** gets a `window` object: fullscreen, the grab, focus
//!   losses, the refresh rate, the frames' period, the last frame's lens,
//!   and the GCMouse shim's callback rate where it runs (`rate.rs`).

mod cursor;
mod lens;
mod pace;
mod rate;
mod screen;

use std::sync::Arc;

use eng_cli::{Exit, FirstPersonPlay, PlayRun, PlayStats, PlayWindow, Step};
use eng_input::LiveInput;
use eng_platform_api::window::Window;
use eng_platform_api::{App, Control, Instant, PlatformEvent, Wake};

use crate::clock::TickClock;
use cursor::Cursor;
use lens::{Lens, to_command_angle};
use pace::DrawPacer;
use rate::CallbackRate;
use screen::Screen;

/// A count of raw motion callbacks to read each frame (the GCMouse
/// shim's), and what the readings show.
struct Callbacks<'a> {
    count: &'a dyn Fn() -> u64,
    rate: CallbackRate,
}

/// `play` on a first-person map as a platform app.
pub(crate) struct FirstPersonFrames<'a> {
    run: Option<PlayRun<'a>>,
    settings: FirstPersonPlay,
    lens: Lens,
    /// The look's low resolution at the start, for the first targets.
    scale: u32,
    live: LiveInput,
    tick_rate: u32,
    platform: &'static str,
    clock: Option<TickClock>,
    pacer: DrawPacer,
    screen: Option<Screen>,
    cursor: Cursor,
    focused: bool,
    refresh_mhz: Option<u32>,
    callbacks: Option<Callbacks<'a>>,
    /// The window or GPU could not open: the message, printed in `stop`.
    refused: Option<String>,
    pub(crate) exit: Option<Exit>,
}

impl<'a> FirstPersonFrames<'a> {
    pub(crate) fn new(
        run: PlayRun<'a>,
        settings: FirstPersonPlay,
        platform: &'static str,
        count: Option<&'a dyn Fn() -> u64>,
    ) -> Self {
        let lens = Lens {
            fov_y: settings.fov_y,
            look: settings.look,
        };
        // Look from where the map's own camera looks at tick 0.
        let start = (settings.view.scene)(&run.frame(1.0, 1, 1));
        let mut live = LiveInput::new(run.bindings().clone());
        live.set_view(
            to_command_angle(start.camera.yaw),
            to_command_angle(start.camera.pitch),
        );
        live.set_look(Some(settings.mouse));
        let tick_rate = run.game().tick_rate;
        Self {
            scale: settings.look.apply(start.look).low_resolution,
            cursor: Cursor::new(!run.scripted()),
            run: Some(run),
            settings,
            lens,
            live,
            tick_rate,
            platform,
            clock: None,
            pacer: DrawPacer::new(None, tick_rate),
            screen: None,
            focused: false,
            refresh_mhz: None,
            callbacks: count.map(|count| Callbacks {
                count,
                rate: CallbackRate::default(),
            }),
            refused: None,
            exit: None,
        }
    }

    /// Runs the ticks due by now. `false`: the run is over.
    fn tick(&mut self, now: Instant) -> bool {
        let clock = self
            .clock
            .get_or_insert_with(|| TickClock::new(self.tick_rate, now));
        let due = clock.due(now);
        let run = self.run.as_mut().expect("the run lives until stop");
        if due.dropped > 0 {
            run.dropped(due.dropped);
        }
        for _ in 0..due.run {
            // One sample per tick, carrying the live angles: the only way
            // they reach the simulation. quit never does (decision 8).
            let input = self.live.sample(run.next_tick());
            if input.quit() || run.step(input.command) == Step::Done {
                return false;
            }
        }
        !run.is_done()
    }

    /// Reads the window's refresh rate again, and paces frames by it. One
    /// under 1 Hz is a misreport and counts as unknown, in the report too.
    fn read_refresh_rate(&mut self) {
        if let Some(screen) = &self.screen {
            self.refresh_mhz = screen.window().refresh_rate().filter(|&mhz| mhz >= 1_000);
            self.pacer.set_rate(self.refresh_mhz, self.tick_rate);
        }
    }

    /// The report's `window` object, as things stand.
    fn window_report(&self, frame: Option<eng_cli::FrameLens>) -> PlayWindow {
        PlayWindow {
            fullscreen_asked: self.cursor.fullscreen_asked,
            fullscreen: self.cursor.fullscreen,
            grab_asked: self.cursor.grab_asked,
            grab: self.cursor.granted,
            grab_at_end: self.cursor.now,
            focus_lost: self.cursor.focus_lost,
            refresh_rate_mhz: self.refresh_mhz,
            frame_period_ns: self.pacer.period(),
            frame,
            gcmouse: self.callbacks.as_ref().map(|c| c.rate.rate()),
        }
    }

    /// The run, if `stop` never came: a platform that could not start.
    pub(crate) fn into_unfinished(self) -> Option<PlayRun<'a>> {
        self.run
    }
}

impl App for FirstPersonFrames<'_> {
    fn window(&mut self, ctx: &mut dyn Control, window: Arc<dyn Window>) {
        let run = self.run.as_ref().expect("the run lives until stop");
        match Screen::open(window, self.settings.view, run, self.scale) {
            Ok(screen) => {
                self.screen = Some(screen);
                self.read_refresh_rate();
                // Only once the GPU is open, so a run refused for want of
                // an adapter never flashes fullscreen.
                self.cursor.window(ctx, self.settings.windowed);
            }
            Err(message) => {
                self.refused = Some(message);
                ctx.exit();
            }
        }
    }

    fn event(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        if let Some(key) = event.key_event() {
            // FocusLost releases every key and drops unsampled motion.
            self.live.handle(key);
        }
        match event {
            PlatformEvent::MouseMotion { dx, dy } => self.live.handle_motion(dx, dy),
            PlatformEvent::FocusGained => {
                self.focused = true;
                self.cursor.focus_gained(ctx);
            }
            PlatformEvent::FocusLost => {
                self.focused = false;
                self.cursor.focus_lost(ctx);
            }
            PlatformEvent::CloseRequested => ctx.exit(),
            PlatformEvent::Resized(size) => {
                if let Some(screen) = &mut self.screen {
                    screen.resize(size.width, size.height);
                }
                // Fullscreen, or another monitor, may change it.
                self.read_refresh_rate();
            }
            PlatformEvent::Occluded(occluded) => {
                if let Some(screen) = &mut self.screen {
                    screen.occluded = occluded;
                }
            }
            _ => {}
        }
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        let now = ctx.now();
        if !self.tick(now) {
            ctx.exit();
            return Wake::OnEvent;
        }
        if let Some(callbacks) = &mut self.callbacks {
            callbacks.rate.read(now, (callbacks.count)(), self.focused);
        }
        let clock = self.clock.expect("tick started the clock");
        if self.pacer.due(now) {
            if let Some(screen) = &mut self.screen {
                let run = self.run.as_mut().expect("the run lives until stop");
                // A script's angles are in the state; live ones are newer.
                let angles = (!run.scripted()).then(|| self.live.view_now());
                screen.draw(run, clock.alpha(now), angles, &self.lens);
            }
            self.pacer.drew(now, clock.next());
        }
        let next_frame = self.pacer.wake().unwrap_or(clock.next());
        Wake::At(clock.next().min(next_frame))
    }

    fn stop(&mut self, ctx: &dyn Control) {
        let _ = ctx;
        // The surface goes before the window it was made from.
        let (gpu, frame) = match self.screen.take().map(Screen::close) {
            Some((gpu, frame)) => (Some(gpu), frame),
            None => (None, None),
        };
        let window = self.window_report(frame);
        let mut run = self.run.take().expect("stop comes once");
        self.exit = Some(match self.refused.take() {
            Some(message) => run.refuse(&message),
            None => {
                run.observe_window(window);
                run.finish(PlayStats {
                    platform: self.platform,
                    gpu,
                })
            }
        });
    }
}
