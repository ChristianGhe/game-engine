//! One run: winit's callbacks (`ApplicationHandler`) turned into the
//! contract's.
//!
//! | winit | the app |
//! |---|---|
//! | `resumed` | the window is made (hidden); `start`; `window`; then it is shown |
//! | `window_event` | `event`, for the ones the contract has |
//! | `about_to_wait` | `frame`, when it is due; the app's answer becomes `ControlFlow` |
//! | `exiting` | `stop` |

use std::sync::Arc;

use eng_platform_api::{App, Control, Instant, PlatformEvent};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{WindowAttributes, WindowId};

use crate::map;
use crate::pace::{Clock, Pacer};
use crate::window::WinitWindow;

/// What the app sees during a callback: one reading of the clock.
struct Ctx {
    now: Instant,
    exit: bool,
}

impl Control for Ctx {
    fn now(&self) -> Instant {
        self.now
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

/// One run of an app on winit's event loop.
pub(crate) struct Run<'a> {
    app: &'a mut dyn App,
    attributes: WindowAttributes,
    /// Starts just before `start`.
    clock: Option<Clock>,
    pacer: Pacer,
    window: Option<Arc<WinitWindow>>,
    started: bool,
    exited: bool,
    stopped: bool,
    /// The window could not be made, so nothing was called.
    pub(crate) unavailable: Option<String>,
}

impl<'a> Run<'a> {
    pub(crate) fn new(app: &'a mut dyn App, attributes: WindowAttributes) -> Self {
        Self {
            app,
            attributes,
            clock: None,
            pacer: Pacer::default(),
            window: None,
            started: false,
            exited: false,
            stopped: false,
            unavailable: None,
        }
    }

    /// The app asked to exit.
    pub(crate) const fn exited(&self) -> bool {
        self.exited
    }

    /// The app has had `start` and has not exited: only then does it get
    /// events and frames. After an exit, winit may still deliver the rest
    /// of its batch, and `about_to_wait`; they are dropped.
    const fn live(&self) -> bool {
        self.started && !self.exited
    }

    fn now(&mut self) -> Instant {
        self.clock.as_mut().map_or(Instant::ZERO, Clock::now)
    }

    /// Calls the app at `now`, and ends the run if it exits.
    fn call<R>(
        &mut self,
        event_loop: &ActiveEventLoop,
        now: Instant,
        f: impl FnOnce(&mut dyn App, &mut Ctx) -> R,
    ) -> R {
        let mut ctx = Ctx { now, exit: false };
        let result = f(&mut *self.app, &mut ctx);
        if ctx.exit && !self.exited {
            self.exited = true;
            event_loop.exit();
        }
        result
    }

    fn deliver(&mut self, event_loop: &ActiveEventLoop, event: PlatformEvent) {
        self.pacer.event();
        let now = self.now();
        self.call(event_loop, now, |app, ctx| app.event(ctx, event));
    }

    /// `stop`, once, if the app had `start`; then the window goes. Called
    /// from `exiting`, and again after the loop returns in case winit
    /// ended it without `exiting`.
    pub(crate) fn finish(&mut self) {
        if self.started && !self.stopped {
            self.stopped = true;
            let now = self.now();
            self.app.stop(&Ctx { now, exit: false });
        }
        self.window = None;
    }
}

impl ApplicationHandler for Run<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Desktops resume once per run.
        if self.started || self.unavailable.is_some() {
            return;
        }
        // The window first, so that if it cannot be made nothing is called.
        let window = match event_loop.create_window(self.attributes.clone()) {
            Ok(window) => window,
            Err(e) => {
                self.unavailable = Some(format!("winit could not make the window: {e}"));
                event_loop.exit();
                return;
            }
        };
        // The run's clock starts now: `start` reads zero, as on headless.
        self.clock = Some(Clock::start());
        self.started = true;
        self.call(event_loop, Instant::ZERO, |app, ctx| app.start(ctx));
        if self.exited {
            // The window was never shown; it closes here.
            return;
        }
        let window = Arc::new(WinitWindow(window));
        self.window = Some(Arc::clone(&window));
        let now = self.now();
        self.call(event_loop, now, |app, ctx| app.window(ctx, window));
        if let Some(window) = self.window.as_ref().filter(|_| !self.exited) {
            window.0.set_visible(true);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let ours = self.window.as_ref().is_some_and(|w| w.0.id() == id);
        if !self.live() || !ours {
            return;
        }
        if let Some(event) = map::window_event(&event) {
            self.deliver(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.live() {
            return;
        }
        let now = self.now();
        if self.pacer.due(now) {
            let wake = self.call(event_loop, now, |app, ctx| app.frame(ctx));
            if self.exited {
                return;
            }
            self.pacer.framed(wake);
        }
        let flow = self
            .pacer
            .deadline(now)
            .and_then(|at| self.clock.as_ref()?.system_time(at))
            .map_or(ControlFlow::Wait, ControlFlow::WaitUntil);
        event_loop.set_control_flow(flow);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.finish();
    }
}
