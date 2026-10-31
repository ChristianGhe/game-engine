//! Physical keys, and the keyboard's view of platform events: plain data
//! with no platform types.
//!
//! Both came from `eng-input` in rung 1b (T3), unchanged; `eng-input`
//! re-exports them, so bindings files and live input did not change.

use core::fmt;

macro_rules! keys {
    ($($(#[$meta:meta])* $key:ident,)+) => {
        /// A physical key, named by its W3C UI Events `code` value
        /// (<https://www.w3.org/TR/uievents-code/>), the same name a
        /// bindings file uses.
        ///
        /// A code names a position on the keyboard, not the character it
        /// types: `KeyQ` is the key where QWERTY has Q, which types A on
        /// AZERTY. So bindings do not depend on the keyboard layout
        /// (spec 6.10).
        ///
        /// The set is closed: the keys of standard 104-key (ANSI), 105-key
        /// (ISO) and Japanese layouts, minus the IME and media keys. A
        /// platform drops any key it cannot name here. Discriminants follow
        /// declaration order and are never written anywhere; files and
        /// messages use [`Key::name`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Key {
            $($(#[$meta])* $key,)+
        }

        impl Key {
            /// Every key, in declaration order (`ALL[k.index()] == k`).
            pub const ALL: &'static [Key] = &[$(Key::$key,)+];

            /// The W3C `code` name, for example `"KeyW"` or `"ArrowUp"`.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Key::$key => stringify!($key),)+
                }
            }
        }
    };
}

keys! {
    // Writing system keys (W3C UI Events code, section 3.1.1).
    /// `` ` `` and `~` on a US keyboard.
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Equal,
    /// The extra key left of Z on ISO keyboards.
    IntlBackslash,
    /// The Japanese `ro` key.
    IntlRo,
    /// The Japanese yen key.
    IntlYen,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Minus,
    Period,
    /// `'` and `"` on a US keyboard.
    Quote,
    Semicolon,
    Slash,
    // Functional keys (3.1.2).
    AltLeft,
    /// AltGr on many layouts.
    AltRight,
    Backspace,
    CapsLock,
    ContextMenu,
    ControlLeft,
    ControlRight,
    Enter,
    /// Command on macOS, the Windows key on Windows.
    MetaLeft,
    MetaRight,
    ShiftLeft,
    ShiftRight,
    Space,
    Tab,
    // Control pad (3.2).
    Delete,
    End,
    Help,
    Home,
    Insert,
    PageDown,
    PageUp,
    // Arrow pad (3.3).
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    // Numpad (3.4).
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadComma,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadEqual,
    NumpadMultiply,
    NumpadSubtract,
    // Function section (3.5).
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    PrintScreen,
    ScrollLock,
    Pause,
}

impl Key {
    /// How many keys there are.
    pub const COUNT: usize = Key::ALL.len();

    /// The key's position in [`Key::ALL`], for tables indexed by key.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The key with this W3C `code` name. Exact: `"keyw"` and `"W"` are
    /// not keys.
    pub fn from_name(name: &str) -> Option<Key> {
        Key::ALL.iter().copied().find(|key| key.name() == name)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What the keyboard did, in the order the platform reports it: the
/// keyboard's part of the platform's events
/// ([`PlatformEvent::key_event`](crate::PlatformEvent::key_event)), which
/// is what live input (`eng-input`) reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyEvent {
    /// A key went down. OS key repeat may send it again while the key is
    /// held; a `Down` for a key already down changes nothing.
    Down(Key),
    /// A key came up. An `Up` for a key that is not down changes nothing.
    Up(Key),
    /// The window lost keyboard focus. No `Up` follows for the keys still
    /// down, so this releases all of them.
    FocusLost,
}
