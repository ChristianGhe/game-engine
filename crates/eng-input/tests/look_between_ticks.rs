//! The view between two ticks (rung 2, T15): `MouseLook::angles_now` and
//! `LiveInput::view_now` read the angles a sample would give now, for the
//! frames drawn between ticks, and take nothing. Reading them never
//! changes a command; the next sample gives what they read when no motion
//! comes in between; at the pitch limit they hold there without
//! re-basing. Expected values are worked by hand as in `mouse_look.rs`:
//! N counts at 0.066° turn `round(N × 22528 / 1875)` units.

use eng_core::Tick;
use eng_input::KeyEvent::FocusLost;
use eng_input::{Bindings, LiveInput, LookSettings, MouseLook};
use proptest::prelude::*;

const DEFAULTS: LookSettings = LookSettings::DEFAULT;

fn live(settings: Option<LookSettings>, yaw: u16, pitch: u16) -> LiveInput {
    let mut live = LiveInput::new(Bindings::default());
    live.set_view(yaw, pitch);
    live.set_look(settings);
    live
}

#[test]
fn the_angles_now_turn_with_motion_not_yet_sampled() {
    let mut look = MouseLook::new(DEFAULTS, 0x4000, 0);
    assert_eq!(look.angles_now(), (0x4000, 0));
    // 10 counts right: 120.15, so 120 units off the yaw.
    look.motion(10.0, 0.0);
    assert_eq!(look.angles_now(), (0x4000 - 120, 0));
    // 10 more, still unsampled: 240.3, so 240.
    look.motion(10.0, 0.0);
    assert_eq!(look.angles_now(), (0x4000 - 240, 0));
    // The sample gives exactly what was read.
    assert_eq!(look.sample(), (0x4000 - 240, 0));
    assert_eq!(look.angles_now(), (0x4000 - 240, 0));
    // The sum since the base is turned, as a sample turns it: 0.04 counts
    // (10 sub-counts) turn 0.47 units, so nothing, and twice that 0.94, so
    // one unit.
    let mut look = MouseLook::new(DEFAULTS, 0x4000, 0);
    look.motion(0.04, 0.0);
    assert_eq!(look.angles_now(), (0x4000, 0));
    look.motion(0.04, 0.0);
    assert_eq!(look.angles_now(), (0x4000 - 1, 0));
}

#[test]
fn reading_takes_nothing() {
    let mut read = MouseLook::new(DEFAULTS, 100, 200);
    let mut unread = read.clone();
    for (dx, dy) in [(3.3, -1.25), (0.004, 7.0), (-250.0, 12.5)] {
        read.motion(dx, dy);
        unread.motion(dx, dy);
        let _ = read.angles_now();
        let _ = read.angles_now();
        assert_eq!(read, unread);
        assert_eq!(read.sample(), unread.sample());
    }
}

#[test]
fn at_the_pitch_limit_the_view_holds_and_nothing_re_bases() {
    // 2000 counts up: 24029.9 units, past the 14563 limit.
    let mut look = MouseLook::new(DEFAULTS, 0, 0);
    look.motion(0.0, -2000.0);
    let before = look.clone();
    assert_eq!(look.angles_now(), (0, 14_563));
    assert_eq!(look, before, "reading re-based nothing");
    // Back down before the tick: the up and the down cancel, as they do in
    // one sample, and the view reads level again.
    look.motion(0.0, 2000.0);
    assert_eq!(look.angles_now(), (0, 0));
    assert_eq!(look.sample(), (0, 0));
    // Down past the limit the other way, read, then sampled.
    look.motion(0.0, 3000.0);
    assert_eq!(look.angles_now(), (0, 0u16.wrapping_sub(14_563)));
    assert_eq!(look.sample(), (0, 0u16.wrapping_sub(14_563)));
}

#[test]
fn live_input_reads_the_view_with_look_on_and_the_held_angles_off() {
    let mut off = live(None, 0x4000, 0x0100);
    off.handle_motion(500.0, -500.0);
    assert_eq!(off.view_now(), (0x4000, 0x0100));
    let mut on = live(Some(DEFAULTS), 0, 0);
    on.handle_motion(-10.0, -10.0);
    assert_eq!(on.view_now(), (120, 120));
    let command = on.sample(Tick(1)).command;
    assert_eq!((command.view_yaw, command.view_pitch), (120, 120));
    // Focus loss drops motion not yet sampled, from the view too.
    on.handle_motion(-10.0, 0.0);
    assert_eq!(on.view_now(), (240, 120));
    on.handle(FocusLost);
    assert_eq!(on.view_now(), (120, 120));
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// Any motion, read between ticks or not, gives the same commands;
    /// each tick's command is what the last read before it saw.
    #[test]
    fn reads_never_change_the_commands(
        sensitivity in 1u32..2_000_000,
        invert in any::<bool>(),
        limit in 0u16..=20_000,
        start in any::<(u16, u16)>(),
        ticks in prop::collection::vec(
            prop::collection::vec((-4_000i32..4_000, -4_000i32..4_000, any::<bool>()), 0..6),
            1..20,
        ),
    ) {
        let settings = LookSettings {
            sensitivity_microdeg: sensitivity,
            invert_pitch: invert,
            pitch_limit: limit,
        };
        let mut read = live(Some(settings), start.0, start.1);
        let mut unread = read.clone();
        for (t, events) in ticks.iter().enumerate() {
            let mut last = None;
            for &(dx, dy, peek) in events {
                // Quarter counts, exact as f64.
                let (dx, dy) = (f64::from(dx) / 4.0, f64::from(dy) / 4.0);
                read.handle_motion(dx, dy);
                unread.handle_motion(dx, dy);
                last = peek.then(|| read.view_now());
            }
            let now = read.view_now();
            if let Some(seen) = last {
                prop_assert_eq!(seen, now, "no motion came after the last event's read");
            }
            let tick = Tick(t as u64 + 1);
            let a = read.sample(tick).command;
            let b = unread.sample(tick).command;
            prop_assert_eq!(a, b);
            prop_assert_eq!((a.view_yaw, a.view_pitch), now);
        }
    }
}
