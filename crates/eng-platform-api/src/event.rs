//! What a platform reports: plain data, the same from every platform
//! (spec 6.10). Mouse and gamepad events are defined from the start, so
//! the rungs that use them (2 and 6) need no contract change.

use crate::key::{Key, KeyEvent};

/// One thing that happened, in the order the platform saw it.
///
/// Input arrives as device events: physical keys, raw mouse motion, mouse
/// buttons, the wheel and gamepads. The rest is about the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlatformEvent {
    /// A key went down. OS key repeat may send it again while the key is
    /// held.
    KeyDown(Key),
    /// A key came up.
    KeyUp(Key),
    /// The mouse moved: raw, unaccelerated device counts, not pixels. `dx`
    /// is positive to the right and `dy` towards the user (down the
    /// screen), as operating systems report it.
    MouseMotion {
        dx: f64,
        dy: f64,
    },
    MouseDown(MouseButton),
    MouseUp(MouseButton),
    /// The wheel turned, or a touchpad scrolled.
    Wheel(WheelDelta),
    /// A gamepad was plugged in. Its id stays the same until it goes.
    GamepadConnected(GamepadId),
    /// A gamepad went away. Its buttons count as released and its axes as
    /// centred; nothing more comes from that id.
    GamepadDisconnected(GamepadId),
    GamepadDown {
        pad: GamepadId,
        button: GamepadButton,
    },
    GamepadUp {
        pad: GamepadId,
        button: GamepadButton,
    },
    /// An axis moved to `value`: sticks from -1 to 1, triggers from 0
    /// (released) to 1 (pressed fully).
    GamepadAxis {
        pad: GamepadId,
        axis: GamepadAxis,
        value: f32,
    },
    /// The window has keyboard focus again. Keys already down are not
    /// reported until they change.
    FocusGained,
    /// The window lost keyboard focus. Treat it as releasing every key and
    /// mouse button that is down: no `KeyUp` or `MouseUp` is promised for
    /// them, and one that comes later is for something already released.
    FocusLost,
    /// The window's drawable area changed, in physical pixels. It may be
    /// 0 x 0 while the window is minimised.
    Resized(PhysicalSize),
    /// `true`: the window cannot be seen (minimised or covered), so the
    /// app should not draw; ticks go on (spec 6.6). `false`: seen again.
    Occluded(bool),
    /// Someone asked to close the window, with its close button or the
    /// system's shortcut. The platform keeps running: the app decides, and
    /// calls [`Control::exit`](crate::Control::exit) to quit.
    CloseRequested,
}

impl PlatformEvent {
    /// The keyboard's part of this event, as live input (`eng-input`)
    /// reads it: a key down or up, or focus lost, which releases every
    /// key. `None` for every other event.
    pub const fn key_event(&self) -> Option<KeyEvent> {
        match *self {
            PlatformEvent::KeyDown(key) => Some(KeyEvent::Down(key)),
            PlatformEvent::KeyUp(key) => Some(KeyEvent::Up(key)),
            PlatformEvent::FocusLost => Some(KeyEvent::FocusLost),
            _ => None,
        }
    }
}

/// A mouse button. `Other` numbers the extra buttons of a gaming mouse as
/// the platform does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    /// The thumb button that means "back" in a browser.
    Back,
    /// The thumb button that means "forward" in a browser.
    Forward,
    Other(u16),
}

/// How far the wheel turned. `y` is positive away from the user (up) and
/// `x` to the right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelDelta {
    /// Notches of a wheel, possibly fractional.
    Lines { x: f64, y: f64 },
    /// Pixels, from a touchpad or a smooth-scrolling wheel.
    Pixels { x: f64, y: f64 },
}

/// A connected gamepad, numbered by the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GamepadId(u32);

impl GamepadId {
    /// For platform implementations only.
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// For platform implementations only.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// A button of the standard gamepad layout, named by position: `South` is
/// A on an Xbox pad and Cross on a PlayStation pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    LeftShoulder,
    RightShoulder,
    /// Back, Select, View or Share.
    Back,
    /// Start, Menu or Options.
    Start,
    /// The logo button in the middle.
    Guide,
    /// Pressing the left stick in.
    LeftStick,
    /// Pressing the right stick in.
    RightStick,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

/// An axis of the standard gamepad layout. Sticks give -1 to 1, `x`
/// positive to the right and `y` positive up (pushed away from the
/// player); triggers give 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GamepadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}

/// A size in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

impl PhysicalSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}
