//! The platform contract (spec 6.1, 6.10): what every platform reports and
//! how it runs an app. Implementations: `eng-platform-headless` (no window,
//! a virtual clock and scripted events, for tests and headless runs) and
//! `eng-platform-winit` (a desktop window, rung 1b T10). Both must pass
//! [`conformance`].
//!
//! - **Events** ([`PlatformEvent`]) are plain data: physical keys ([`Key`],
//!   by W3C `code`), raw mouse motion, mouse buttons, the wheel, gamepad
//!   buttons and axes, focus, resize, occlusion and close requests.
//!   [`PlatformEvent::key_event`] gives the keyboard's part as a
//!   [`KeyEvent`], which live input in `eng-input` reads.
//! - **The clock** is [`Control::now`]: an [`Instant`], nanoseconds since
//!   the run started. It never goes back.
//! - **The callbacks** ([`App`]): the platform owns the event loop, as
//!   winit does, and calls the app back. Ticks come from the timer, not
//!   from presenting (spec 6.6): the app answers each frame with when it
//!   wants the next one ([`Wake`]).
//! - **The window seam** ([`window::Window`], ADR-0016) is the only part
//!   that names `raw-window-handle`'s traits. A windowed platform hands its
//!   window to [`App::window`]; nothing a platform must implement mentions
//!   it, so the headless platform, outside the seam, never does.
//!
//! # The rules
//!
//! A run calls, in this order:
//!
//! 1. [`App::start`], once, first.
//! 2. [`App::window`], once, on a platform with a window.
//! 3. Batches: the events due, in the order they happened, then one
//!    [`App::frame`]. The first frame comes at once, after any events
//!    already due. After [`Wake::At`]`(t)` the next frame comes at `t`, or
//!    at once if `t` is not after now, or sooner with an event's batch;
//!    without an event, never before `t`. After [`Wake::OnEvent`] it comes
//!    only with an event's batch.
//! 4. [`App::stop`], once, last, whatever `run` returns.
//!
//! [`Control::exit`] ends the run: the app then gets only `stop`, and the
//! rest of the batch is dropped. [`Platform::run`] returns `Ok(())` after
//! an exit. A close request is an event like any other: the platform keeps
//! running until the app exits. The clock never goes back. `run` can be
//! called again once it has returned, and each run's clock starts at zero.
//!
//! [`conformance::check`] checks these rules on any platform, and
//! [`conformance::check_scripted`] checks event delivery, exact times and
//! close requests on a platform that can be scripted.
#![forbid(unsafe_code)]

mod app;
mod clock;
pub mod conformance;
mod event;
mod key;
pub mod window;

pub use app::{App, Control, Platform, PlatformError, Wake};
pub use clock::Instant;
pub use event::{
    GamepadAxis, GamepadButton, GamepadId, MouseButton, PhysicalSize, PlatformEvent, WheelDelta,
};
pub use key::{Key, KeyEvent};
