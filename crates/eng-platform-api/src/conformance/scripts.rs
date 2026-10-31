//! The scripts the scripted runs deliver.

use crate::{
    GamepadAxis, GamepadButton, GamepadId, Instant, Key, MouseButton, PhysicalSize, PlatformEvent,
    WheelDelta,
};

const fn ms(millis: u64) -> Instant {
    Instant::from_millis(millis)
}

/// Every event kind, with the probe's timers falling between and on the
/// events' times, and two close requests: the app ignores the first and
/// exits on the second, in the middle of its batch.
pub(super) fn timers() -> Vec<(Instant, PlatformEvent)> {
    use PlatformEvent as E;
    let pad = GamepadId::from_raw(1);
    let south = GamepadButton::South;
    vec![
        (ms(0), E::FocusGained),
        (ms(0), E::Resized(PhysicalSize::new(800, 600))),
        (ms(4), E::KeyDown(Key::KeyW)),
        (ms(4), E::MouseMotion { dx: 3.0, dy: -1.5 }),
        (ms(4), E::MouseDown(MouseButton::Left)),
        (ms(6), E::Wheel(WheelDelta::Lines { x: 0.0, y: 1.0 })),
        (ms(6), E::KeyUp(Key::KeyW)),
        (ms(6), E::MouseUp(MouseButton::Left)),
        (ms(9), E::GamepadConnected(pad)),
        (ms(9), E::GamepadDown { pad, button: south }),
        (
            ms(9),
            E::GamepadAxis {
                pad,
                axis: GamepadAxis::LeftY,
                value: -0.5,
            },
        ),
        (ms(9), E::GamepadUp { pad, button: south }),
        (ms(9), E::GamepadDisconnected(pad)),
        (ms(10), E::Occluded(true)),
        (ms(12), E::Occluded(false)),
        (ms(12), E::Resized(PhysicalSize::new(1024, 768))),
        (ms(12), E::Wheel(WheelDelta::Pixels { x: -2.0, y: 7.5 })),
        (ms(15), E::FocusLost),
        (ms(20), E::CloseRequested),
        (ms(23), E::FocusGained),
        (ms(23), E::KeyDown(Key::Escape)),
        (ms(30), E::CloseRequested),
        (ms(30), E::KeyDown(Key::KeyA)),
        (ms(40), E::KeyUp(Key::Escape)),
    ]
}

/// For an app that only waits for events; it exits on the close request.
pub(super) fn events() -> Vec<(Instant, PlatformEvent)> {
    use PlatformEvent as E;
    vec![
        (ms(7), E::KeyDown(Key::KeyD)),
        (ms(13), E::KeyUp(Key::KeyD)),
        (ms(13), E::Resized(PhysicalSize::new(640, 360))),
        (ms(21), E::CloseRequested),
    ]
}
