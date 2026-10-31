//! winit's events as the contract's plain data: physical keys by W3C
//! code, focus, size, occlusion and close requests. Everything else winit
//! reports is left out, the mouse included: rung 1b maps only keys (plan,
//! eng-platform-api's row), and the mouse comes with rung 2's raw-input
//! shim (`docs/rungs/1b/PROGRESS.md`, T10, has winit's conventions).

use eng_platform_api::{Key, PhysicalSize, PlatformEvent};
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// winit's code for each of the contract's keys, in [`Key::ALL`] order.
/// Both name keys by W3C `code`, except that winit calls `MetaLeft` and
/// `MetaRight` `SuperLeft` and `SuperRight`.
pub(crate) const KEYS: &[(KeyCode, Key)] = &[
    (KeyCode::Backquote, Key::Backquote),
    (KeyCode::Backslash, Key::Backslash),
    (KeyCode::BracketLeft, Key::BracketLeft),
    (KeyCode::BracketRight, Key::BracketRight),
    (KeyCode::Comma, Key::Comma),
    (KeyCode::Digit0, Key::Digit0),
    (KeyCode::Digit1, Key::Digit1),
    (KeyCode::Digit2, Key::Digit2),
    (KeyCode::Digit3, Key::Digit3),
    (KeyCode::Digit4, Key::Digit4),
    (KeyCode::Digit5, Key::Digit5),
    (KeyCode::Digit6, Key::Digit6),
    (KeyCode::Digit7, Key::Digit7),
    (KeyCode::Digit8, Key::Digit8),
    (KeyCode::Digit9, Key::Digit9),
    (KeyCode::Equal, Key::Equal),
    (KeyCode::IntlBackslash, Key::IntlBackslash),
    (KeyCode::IntlRo, Key::IntlRo),
    (KeyCode::IntlYen, Key::IntlYen),
    (KeyCode::KeyA, Key::KeyA),
    (KeyCode::KeyB, Key::KeyB),
    (KeyCode::KeyC, Key::KeyC),
    (KeyCode::KeyD, Key::KeyD),
    (KeyCode::KeyE, Key::KeyE),
    (KeyCode::KeyF, Key::KeyF),
    (KeyCode::KeyG, Key::KeyG),
    (KeyCode::KeyH, Key::KeyH),
    (KeyCode::KeyI, Key::KeyI),
    (KeyCode::KeyJ, Key::KeyJ),
    (KeyCode::KeyK, Key::KeyK),
    (KeyCode::KeyL, Key::KeyL),
    (KeyCode::KeyM, Key::KeyM),
    (KeyCode::KeyN, Key::KeyN),
    (KeyCode::KeyO, Key::KeyO),
    (KeyCode::KeyP, Key::KeyP),
    (KeyCode::KeyQ, Key::KeyQ),
    (KeyCode::KeyR, Key::KeyR),
    (KeyCode::KeyS, Key::KeyS),
    (KeyCode::KeyT, Key::KeyT),
    (KeyCode::KeyU, Key::KeyU),
    (KeyCode::KeyV, Key::KeyV),
    (KeyCode::KeyW, Key::KeyW),
    (KeyCode::KeyX, Key::KeyX),
    (KeyCode::KeyY, Key::KeyY),
    (KeyCode::KeyZ, Key::KeyZ),
    (KeyCode::Minus, Key::Minus),
    (KeyCode::Period, Key::Period),
    (KeyCode::Quote, Key::Quote),
    (KeyCode::Semicolon, Key::Semicolon),
    (KeyCode::Slash, Key::Slash),
    (KeyCode::AltLeft, Key::AltLeft),
    (KeyCode::AltRight, Key::AltRight),
    (KeyCode::Backspace, Key::Backspace),
    (KeyCode::CapsLock, Key::CapsLock),
    (KeyCode::ContextMenu, Key::ContextMenu),
    (KeyCode::ControlLeft, Key::ControlLeft),
    (KeyCode::ControlRight, Key::ControlRight),
    (KeyCode::Enter, Key::Enter),
    (KeyCode::SuperLeft, Key::MetaLeft),
    (KeyCode::SuperRight, Key::MetaRight),
    (KeyCode::ShiftLeft, Key::ShiftLeft),
    (KeyCode::ShiftRight, Key::ShiftRight),
    (KeyCode::Space, Key::Space),
    (KeyCode::Tab, Key::Tab),
    (KeyCode::Delete, Key::Delete),
    (KeyCode::End, Key::End),
    (KeyCode::Help, Key::Help),
    (KeyCode::Home, Key::Home),
    (KeyCode::Insert, Key::Insert),
    (KeyCode::PageDown, Key::PageDown),
    (KeyCode::PageUp, Key::PageUp),
    (KeyCode::ArrowDown, Key::ArrowDown),
    (KeyCode::ArrowLeft, Key::ArrowLeft),
    (KeyCode::ArrowRight, Key::ArrowRight),
    (KeyCode::ArrowUp, Key::ArrowUp),
    (KeyCode::NumLock, Key::NumLock),
    (KeyCode::Numpad0, Key::Numpad0),
    (KeyCode::Numpad1, Key::Numpad1),
    (KeyCode::Numpad2, Key::Numpad2),
    (KeyCode::Numpad3, Key::Numpad3),
    (KeyCode::Numpad4, Key::Numpad4),
    (KeyCode::Numpad5, Key::Numpad5),
    (KeyCode::Numpad6, Key::Numpad6),
    (KeyCode::Numpad7, Key::Numpad7),
    (KeyCode::Numpad8, Key::Numpad8),
    (KeyCode::Numpad9, Key::Numpad9),
    (KeyCode::NumpadAdd, Key::NumpadAdd),
    (KeyCode::NumpadComma, Key::NumpadComma),
    (KeyCode::NumpadDecimal, Key::NumpadDecimal),
    (KeyCode::NumpadDivide, Key::NumpadDivide),
    (KeyCode::NumpadEnter, Key::NumpadEnter),
    (KeyCode::NumpadEqual, Key::NumpadEqual),
    (KeyCode::NumpadMultiply, Key::NumpadMultiply),
    (KeyCode::NumpadSubtract, Key::NumpadSubtract),
    (KeyCode::Escape, Key::Escape),
    (KeyCode::F1, Key::F1),
    (KeyCode::F2, Key::F2),
    (KeyCode::F3, Key::F3),
    (KeyCode::F4, Key::F4),
    (KeyCode::F5, Key::F5),
    (KeyCode::F6, Key::F6),
    (KeyCode::F7, Key::F7),
    (KeyCode::F8, Key::F8),
    (KeyCode::F9, Key::F9),
    (KeyCode::F10, Key::F10),
    (KeyCode::F11, Key::F11),
    (KeyCode::F12, Key::F12),
    (KeyCode::PrintScreen, Key::PrintScreen),
    (KeyCode::ScrollLock, Key::ScrollLock),
    (KeyCode::Pause, Key::Pause),
];

/// The contract's name for a physical key; `None` for one it cannot name,
/// such as F13 or a media key, which the platform drops.
pub(crate) fn key(key: PhysicalKey) -> Option<Key> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    KEYS.iter().find(|&&(c, _)| c == code).map(|&(_, key)| key)
}

/// A key going down or up, if the contract has it. OS key repeat is
/// dropped: the key did not go down again. So are winit's synthetic
/// events, presses for keys already held when focus comes back and
/// releases when it goes: the contract reports keys held at focus only
/// when they change, and `FocusLost` releases them all.
pub(crate) fn keyboard(
    key: PhysicalKey,
    state: ElementState,
    repeat: bool,
    synthetic: bool,
) -> Option<PlatformEvent> {
    if repeat || synthetic {
        return None;
    }
    let key = self::key(key)?;
    Some(match state {
        ElementState::Pressed => PlatformEvent::KeyDown(key),
        ElementState::Released => PlatformEvent::KeyUp(key),
    })
}

/// A window event, if the contract has it.
pub(crate) fn window_event(event: &WindowEvent) -> Option<PlatformEvent> {
    Some(match event {
        WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } => return keyboard(event.physical_key, event.state, event.repeat, *is_synthetic),
        WindowEvent::Focused(true) => PlatformEvent::FocusGained,
        WindowEvent::Focused(false) => PlatformEvent::FocusLost,
        WindowEvent::Resized(size) => {
            PlatformEvent::Resized(PhysicalSize::new(size.width, size.height))
        }
        WindowEvent::Occluded(occluded) => PlatformEvent::Occluded(*occluded),
        WindowEvent::CloseRequested => PlatformEvent::CloseRequested,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use winit::dpi::{PhysicalPosition, PhysicalSize as WinitSize};
    use winit::keyboard::NativeKeyCode;

    use super::*;

    #[test]
    fn every_key_has_the_winit_code_of_the_same_w3c_name() {
        assert_eq!(KEYS.len(), Key::COUNT, "one code per key");
        for (i, &(code, key)) in KEYS.iter().enumerate() {
            assert_eq!(key, Key::ALL[i], "entry {i} is out of Key::ALL order");
            // winit calls the W3C `MetaLeft` and `MetaRight` keys Super.
            let want = match key {
                Key::MetaLeft => "SuperLeft",
                Key::MetaRight => "SuperRight",
                _ => key.name(),
            };
            assert_eq!(format!("{code:?}"), want, "entry {i}");
            assert_eq!(super::key(PhysicalKey::Code(code)), Some(key));
        }
    }

    #[test]
    fn keys_the_contract_cannot_name_are_dropped() {
        let unnamed = [
            KeyCode::F13,
            KeyCode::Fn,
            KeyCode::Lang1,
            KeyCode::NumpadBackspace,
            KeyCode::MediaPlayPause,
            KeyCode::AudioVolumeUp,
            KeyCode::Meta,
        ];
        for code in unnamed {
            assert_eq!(key(PhysicalKey::Code(code)), None, "{code:?}");
        }
        let native = PhysicalKey::Unidentified(NativeKeyCode::Xkb(0x1234));
        assert_eq!(key(native), None);
        assert_eq!(keyboard(native, ElementState::Pressed, false, false), None);
    }

    #[test]
    fn keys_go_down_and_up_by_physical_code() {
        let w = PhysicalKey::Code(KeyCode::KeyW);
        assert_eq!(
            keyboard(w, ElementState::Pressed, false, false),
            Some(PlatformEvent::KeyDown(Key::KeyW))
        );
        assert_eq!(
            keyboard(w, ElementState::Released, false, false),
            Some(PlatformEvent::KeyUp(Key::KeyW))
        );
        let meta = PhysicalKey::Code(KeyCode::SuperLeft);
        assert_eq!(
            keyboard(meta, ElementState::Pressed, false, false),
            Some(PlatformEvent::KeyDown(Key::MetaLeft))
        );
    }

    #[test]
    fn key_repeats_and_synthetic_keys_are_dropped() {
        let esc = PhysicalKey::Code(KeyCode::Escape);
        // OS key repeat: the key did not go down again.
        assert_eq!(keyboard(esc, ElementState::Pressed, true, false), None);
        // winit's presses for keys already held when focus comes back, and
        // its releases when focus goes: the contract reports neither.
        assert_eq!(keyboard(esc, ElementState::Pressed, false, true), None);
        assert_eq!(keyboard(esc, ElementState::Released, false, true), None);
    }

    #[test]
    fn window_events_the_contract_has() {
        let pairs = [
            (WindowEvent::Focused(true), PlatformEvent::FocusGained),
            (WindowEvent::Focused(false), PlatformEvent::FocusLost),
            (
                WindowEvent::Resized(WinitSize::new(1024, 768)),
                PlatformEvent::Resized(PhysicalSize::new(1024, 768)),
            ),
            (
                WindowEvent::Resized(WinitSize::new(0, 0)),
                PlatformEvent::Resized(PhysicalSize::new(0, 0)),
            ),
            (WindowEvent::Occluded(true), PlatformEvent::Occluded(true)),
            (WindowEvent::Occluded(false), PlatformEvent::Occluded(false)),
            (WindowEvent::CloseRequested, PlatformEvent::CloseRequested),
        ];
        for (winit, ours) in pairs {
            assert_eq!(window_event(&winit), Some(ours), "{winit:?}");
        }
    }

    #[test]
    fn window_events_the_contract_leaves_out() {
        let others = [
            WindowEvent::RedrawRequested,
            WindowEvent::Destroyed,
            WindowEvent::Moved(PhysicalPosition::new(10, 20)),
            WindowEvent::HoveredFileCancelled,
        ];
        for event in others {
            assert_eq!(window_event(&event), None, "{event:?}");
        }
    }
}
