//! Physical keys, and the keyboard's view of platform events. `Key` and
//! `KeyEvent` came here from `eng-input` in rung 1b (T3); `eng-input`
//! re-exports both, so bindings files and their tests did not change.

use eng_platform_api::{
    GamepadAxis, GamepadButton, GamepadId, Key, KeyEvent, MouseButton, PhysicalSize, PlatformEvent,
    WheelDelta,
};

#[test]
fn key_names_are_w3c_codes_and_round_trip() {
    assert_eq!(Key::COUNT, Key::ALL.len());
    for (i, &key) in Key::ALL.iter().enumerate() {
        assert_eq!(key.index(), i);
        assert_eq!(Key::from_name(key.name()), Some(key));
        assert_eq!(key.to_string(), key.name());
    }
    let mut names: Vec<_> = Key::ALL.iter().map(|k| k.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Key::COUNT, "names are unique");
    // Spot checks against the W3C `code` table.
    for name in [
        "KeyA",
        "KeyZ",
        "Digit0",
        "Digit9",
        "ArrowUp",
        "ArrowLeft",
        "Escape",
        "Space",
        "Enter",
        "Tab",
        "Backspace",
        "ShiftLeft",
        "ControlRight",
        "AltLeft",
        "MetaLeft",
        "Backquote",
        "Quote",
        "IntlBackslash",
        "Numpad0",
        "NumpadEnter",
        "F1",
        "F12",
        "PageDown",
    ] {
        assert!(Key::from_name(name).is_some(), "{name}");
    }
    // Names are exact; there are no aliases.
    for not_a_key in ["keyw", "W", "Esc", "Up", "SuperLeft", "OSLeft", "F13", ""] {
        assert_eq!(Key::from_name(not_a_key), None, "{not_a_key}");
    }
}

/// The key set is part of the contract: a key added or removed changes
/// what platforms report and what bindings files may name.
#[test]
fn the_key_set_is_closed() {
    assert_eq!(Key::COUNT, 110);
    assert_eq!(Key::ALL.first(), Some(&Key::Backquote));
    assert_eq!(Key::ALL.last(), Some(&Key::Pause));
}

#[test]
fn only_keyboard_events_have_a_key_event() {
    use PlatformEvent as E;
    assert_eq!(
        E::KeyDown(Key::KeyW).key_event(),
        Some(KeyEvent::Down(Key::KeyW))
    );
    assert_eq!(
        E::KeyUp(Key::Escape).key_event(),
        Some(KeyEvent::Up(Key::Escape))
    );
    // Losing focus releases every key, so the keyboard sees it too.
    assert_eq!(E::FocusLost.key_event(), Some(KeyEvent::FocusLost));

    let pad = GamepadId::from_raw(3);
    let others = [
        E::FocusGained,
        E::MouseMotion { dx: 1.5, dy: -2.0 },
        E::MouseDown(MouseButton::Left),
        E::MouseUp(MouseButton::Other(7)),
        E::Wheel(WheelDelta::Lines { x: 0.0, y: 1.0 }),
        E::Wheel(WheelDelta::Pixels { x: -4.0, y: 12.5 }),
        E::GamepadConnected(pad),
        E::GamepadDown {
            pad,
            button: GamepadButton::South,
        },
        E::GamepadUp {
            pad,
            button: GamepadButton::DPadLeft,
        },
        E::GamepadAxis {
            pad,
            axis: GamepadAxis::LeftTrigger,
            value: 0.25,
        },
        E::GamepadDisconnected(pad),
        E::Resized(PhysicalSize::new(640, 480)),
        E::Occluded(true),
        E::CloseRequested,
    ];
    for event in others {
        assert_eq!(event.key_event(), None, "{event:?}");
    }
    assert_eq!(pad.raw(), 3);
}
