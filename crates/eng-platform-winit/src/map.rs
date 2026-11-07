//! winit's events as the contract's plain data: physical keys by W3C
//! code, mouse buttons, the wheel, raw mouse motion, focus, size,
//! occlusion and close requests. Everything else winit reports is left
//! out. Rung 1b's T10 found winit's conventions
//! (`docs/rungs/1b/PROGRESS.md`, "For rung 2"); rung 2's T13 maps them:
//!
//! - **The wheel.** winit's deltas say where content moves, right and down
//!   positive. The contract's say where the wheel turned, up (away from the
//!   user) and right positive: `y` keeps winit's sign and `x` is negated,
//!   as SDL does on macOS. Natural scrolling reverses both; on macOS, where
//!   it is on by default, the platform reads the setting and turns them
//!   back ([`wheel`]'s `natural`).
//! - **Raw motion** is winit's device motion (`DeviceEvent::MouseMotion`),
//!   which winit sends only while the window has focus; the platform also
//!   drops any that comes without focus. On macOS winit's deltas are
//!   accelerated and coalesced, not the raw counts the contract promises,
//!   so the platform leaves them out there ([`RAW_MOTION`]): macOS motion
//!   comes from the GCMouse shim (`eng-platform-gcmouse`, `motion.rs`).
//!   Under X11's `Confined` grab (an X pointer grab) the X server sends
//!   winit each raw motion twice, from the same device (seen under Xvfb
//!   with XTEST input; once each without the grab). SDL drops a raw motion
//!   with the same X time and deltas as the one before; winit gives neither
//!   the time nor anything else that tells the copies apart, so both pass.

use eng_platform_api::{Key, MouseButton, PhysicalSize, PlatformEvent, WheelDelta};
use winit::event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Whether winit's device motion is the contract's raw motion here: not on
/// macOS, where winit reports NSEvent's accelerated deltas, coalesced.
pub(crate) const RAW_MOTION: bool = !cfg!(target_os = "macos");

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

/// A mouse button going down or up. Both name the same buttons.
pub(crate) const fn button(
    button: winit::event::MouseButton,
    state: ElementState,
) -> PlatformEvent {
    use winit::event::MouseButton as W;
    let button = match button {
        W::Left => MouseButton::Left,
        W::Right => MouseButton::Right,
        W::Middle => MouseButton::Middle,
        W::Back => MouseButton::Back,
        W::Forward => MouseButton::Forward,
        W::Other(n) => MouseButton::Other(n),
    };
    match state {
        ElementState::Pressed => PlatformEvent::MouseDown(button),
        ElementState::Released => PlatformEvent::MouseUp(button),
    }
}

/// The wheel: winit says where content moves (right and down positive), the
/// contract where the wheel turned (right and up, away from the user).
/// With `natural` scrolling the system has reversed both already.
///
/// X11 reports a wheel that only has buttons 4 to 7 (XTEST's, as `xdotool
/// click 4` sends under Xvfb) on both the press and the release, so such a
/// notch comes twice; a wheel with XInput2 scroll valuators (evdev,
/// libinput) comes once. winit's event does not say which it was.
pub(crate) fn wheel(delta: MouseScrollDelta, natural: bool) -> PlatformEvent {
    // `0.0 - v`, not `-v`: an axis that did not move stays +0.
    let flip = |v: f64, flip: bool| if flip { 0.0 - v } else { v };
    PlatformEvent::Wheel(match delta {
        MouseScrollDelta::LineDelta(x, y) => WheelDelta::Lines {
            x: flip(f64::from(x), !natural),
            y: flip(f64::from(y), natural),
        },
        MouseScrollDelta::PixelDelta(p) => WheelDelta::Pixels {
            x: flip(p.x, !natural),
            y: flip(p.y, natural),
        },
    })
}

/// Raw device motion: winit's `DeviceEvent::MouseMotion`, `dx` right and
/// `dy` down, as the contract has it. Only where [`RAW_MOTION`].
pub(crate) const fn motion((dx, dy): (f64, f64)) -> PlatformEvent {
    PlatformEvent::MouseMotion { dx, dy }
}

/// The contract's event for one of winit's device events, given whether
/// the window has focus as the app was last told: raw motion, only with
/// focus and only where [`RAW_MOTION`]. The rest are left out.
pub(crate) const fn device_event(event: &DeviceEvent, focused: bool) -> Option<PlatformEvent> {
    match event {
        DeviceEvent::MouseMotion { delta } if focused && RAW_MOTION => Some(motion(*delta)),
        _ => None,
    }
}

/// Whether macOS's natural scrolling is on, from the output of `defaults
/// read -g com.apple.swipescrolldirection`: `None` (no such key, or no
/// output) is the system's default, on.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn natural_scrolling(output: Option<&str>) -> bool {
    output.is_none_or(|out| out.trim() != "0")
}

/// A window event, if the contract has it. `natural`: the system's natural
/// scrolling is on, as far as the platform can tell.
pub(crate) fn window_event(event: &WindowEvent, natural: bool) -> Option<PlatformEvent> {
    Some(match event {
        WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } => return keyboard(event.physical_key, event.state, event.repeat, *is_synthetic),
        WindowEvent::MouseInput { state, button, .. } => self::button(*button, *state),
        WindowEvent::MouseWheel { delta, .. } => wheel(*delta, natural),
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
            assert_eq!(window_event(&winit, false), Some(ours), "{winit:?}");
            assert_eq!(window_event(&winit, true), Some(ours), "{winit:?}");
        }
    }

    #[test]
    fn mouse_buttons_go_down_and_up_by_name() {
        use winit::event::MouseButton as W;
        let pairs = [
            (W::Left, MouseButton::Left),
            (W::Right, MouseButton::Right),
            (W::Middle, MouseButton::Middle),
            (W::Back, MouseButton::Back),
            (W::Forward, MouseButton::Forward),
            (W::Other(12), MouseButton::Other(12)),
        ];
        for (winit, ours) in pairs {
            assert_eq!(
                button(winit, ElementState::Pressed),
                PlatformEvent::MouseDown(ours)
            );
            assert_eq!(
                button(winit, ElementState::Released),
                PlatformEvent::MouseUp(ours)
            );
        }
    }

    #[test]
    fn the_wheel_says_where_it_turned_not_where_content_moves() {
        // A notch away from the user: winit's content moves down, +y.
        assert_eq!(
            wheel(MouseScrollDelta::LineDelta(0.0, 1.0), false),
            PlatformEvent::Wheel(WheelDelta::Lines { x: 0.0, y: 1.0 })
        );
        // Tilted right: content moves left, winit's -x; X11's button 7.
        assert_eq!(
            wheel(MouseScrollDelta::LineDelta(-1.0, 0.0), false),
            PlatformEvent::Wheel(WheelDelta::Lines { x: 1.0, y: 0.0 })
        );
        let pixels = MouseScrollDelta::PixelDelta(PhysicalPosition::new(3.5, -7.25));
        assert_eq!(
            wheel(pixels, false),
            PlatformEvent::Wheel(WheelDelta::Pixels { x: -3.5, y: -7.25 })
        );
        // Natural scrolling has reversed both: turned back.
        assert_eq!(
            wheel(MouseScrollDelta::LineDelta(0.5, -1.0), true),
            PlatformEvent::Wheel(WheelDelta::Lines { x: 0.5, y: 1.0 })
        );
        assert_eq!(
            wheel(pixels, true),
            PlatformEvent::Wheel(WheelDelta::Pixels { x: 3.5, y: 7.25 })
        );
    }

    #[test]
    fn natural_scrolling_is_on_unless_the_setting_says_0() {
        assert!(natural_scrolling(None), "never set: macOS's default");
        assert!(natural_scrolling(Some("1\n")));
        assert!(!natural_scrolling(Some("0\n")));
        assert!(!natural_scrolling(Some("0")));
    }

    #[test]
    fn raw_motion_keeps_its_counts_and_signs() {
        assert_eq!(
            motion((3.0, -1.5)),
            PlatformEvent::MouseMotion { dx: 3.0, dy: -1.5 }
        );
    }

    #[test]
    fn raw_motion_comes_only_with_focus_and_not_on_macos() {
        let moved = DeviceEvent::MouseMotion { delta: (3.0, -1.5) };
        assert_eq!(device_event(&moved, false), None, "without focus");
        let with_focus = device_event(&moved, true);
        if cfg!(target_os = "macos") {
            assert_eq!(with_focus, None, "macOS's comes from the GCMouse shim");
        } else {
            assert_eq!(
                with_focus,
                Some(PlatformEvent::MouseMotion { dx: 3.0, dy: -1.5 })
            );
        }
        let others = [
            DeviceEvent::Added,
            DeviceEvent::Removed,
            DeviceEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(0.0, 1.0),
            },
            DeviceEvent::Motion {
                axis: 0,
                value: 2.0,
            },
            DeviceEvent::Button {
                button: 1,
                state: ElementState::Pressed,
            },
        ];
        for event in &others {
            for focused in [false, true] {
                assert_eq!(device_event(event, focused), None, "{event:?}");
            }
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
            assert_eq!(window_event(&event, false), None, "{event:?}");
        }
    }
}
