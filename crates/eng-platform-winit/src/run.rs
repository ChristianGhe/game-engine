//! One run: winit's callbacks (`ApplicationHandler`) turned into the
//! contract's.
//!
//! | winit | the app |
//! |---|---|
//! | `resumed` | the window is made (hidden); `start`; `window`; then it is shown |
//! | `window_event` | `event`, for the ones the contract has |
//! | `device_event` | `event`: raw mouse motion, while the window has focus |
//! | `about_to_wait` | `frame`, when it is due; the app's answer becomes `ControlFlow` |
//! | `exiting` | `stop` |

use std::sync::Arc;

use eng_platform_api::{App, Control, CursorGrab, Instant, PlatformEvent};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{WindowAttributes, WindowId};

use crate::map;
use crate::pace::{Clock, Pacer};
use crate::request::Requests;
use crate::window::WinitWindow;

/// What the app sees during a callback: one reading of the clock, and the
/// window its requests act on once it has been handed over.
struct Ctx<'w> {
    now: Instant,
    exit: bool,
    window: Option<&'w winit::window::Window>,
    requests: &'w mut Requests,
}

impl Control for Ctx<'_> {
    fn now(&self) -> Instant {
        self.now
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn grab_cursor(&mut self, grab: bool) -> CursorGrab {
        match self.window {
            Some(window) => self.requests.grab(window, grab),
            None => CursorGrab::None,
        }
    }

    fn set_fullscreen(&mut self, on: bool) -> bool {
        self.window
            .is_some_and(|window| self.requests.fullscreen(window, on))
    }
}

/// One run of an app on winit's event loop.
pub(crate) struct Run<'a> {
    app: &'a mut dyn App,
    attributes: WindowAttributes,
    /// Starts just before `start`.
    clock: Option<Clock>,
    pacer: Pacer,
    /// The window, from just before `App::window`: requests act on it.
    window: Option<Arc<WinitWindow>>,
    requests: Requests,
    /// macOS's natural scrolling is on (the wheel's signs).
    natural: bool,
    started: bool,
    exited: bool,
    stopped: bool,
    /// The window could not be made, so nothing was called.
    pub(crate) unavailable: Option<String>,
}

impl<'a> Run<'a> {
    pub(crate) fn new(app: &'a mut dyn App, attributes: WindowAttributes, natural: bool) -> Self {
        Self {
            app,
            attributes,
            clock: None,
            pacer: Pacer::default(),
            window: None,
            requests: Requests::default(),
            natural,
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
        let mut ctx = Ctx {
            now,
            exit: false,
            window: self.window.as_deref().map(|w| &w.0),
            requests: &mut self.requests,
        };
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
            self.app.stop(&Ctx {
                now,
                exit: false,
                window: None,
                requests: &mut self.requests,
            });
        }
        if let Some(window) = self.window.take() {
            self.requests.finish(&window.0);
        }
    }

    /// Focus changed: the grab follows before the app hears of it.
    fn focus(&mut self, focused: bool) {
        if let Some(window) = &self.window {
            if focused {
                self.requests.focus_gained(&window.0);
            } else {
                self.requests.focus_lost(&window.0);
            }
        }
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
            self.requests.shown(&window.0);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let ours = self.window.as_ref().is_some_and(|w| w.0.id() == id);
        if !self.live() || !ours {
            return;
        }
        if let WindowEvent::Focused(focused) = event {
            self.focus(focused);
        }
        if let Some(event) = map::window_event(&event, self.natural) {
            self.deliver(event_loop, event);
        }
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        // Raw motion only with focus, as the app was last told of it.
        if !self.live() || !self.requests.focused() {
            return;
        }
        if let DeviceEvent::MouseMotion { delta } = event
            && map::RAW_MOTION
        {
            self.deliver(event_loop, map::motion(delta));
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
