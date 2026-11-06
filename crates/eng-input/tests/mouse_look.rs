//! Rung 2 criterion 9, `mouse-look` (plan decision 9, queue Q3), in three
//! files: here the signs, the pitch limit, focus loss, and turning look on
//! and off; `mouse_look_exact.rs` the rounding, splits, wrapping and wide
//! arithmetic; `mouse_look_model.rs` the same timeline giving the same
//! commands, against a model. `look_rig/mod.rs` holds what they share.
//!
//! Expected values are worked by hand from the rule: N counts at s degrees
//! per count turn `round(N × s × 65536 / 360)` binary-angle units, ties
//! away from zero, after each delta is rounded to a 1/256 count. At the
//! default 0.066°, `N × 0.066 × 65536 / 360 = N × 22528 / 1875`, about
//! `N × 12.01493`. Nothing here asks the code under test for an expected
//! value.

mod look_rig;

use eng_core::Tick;
use eng_input::KeyEvent::{Down, FocusLost, Up};
use eng_input::*;
use look_rig::{DEFAULTS, Rig, neg, with};

#[test]
fn look_is_off_by_default_and_then_ignores_motion() {
    assert_eq!(LookSettings::default(), DEFAULTS);
    let mut live = LiveInput::new(Bindings::default());
    assert_eq!(live.look_settings(), None);
    live.set_view(0x4000, 0x0100);
    live.handle_motion(500.0, -500.0);
    let c = live.sample(Tick(1)).command;
    assert_eq!((c.view_yaw, c.view_pitch), (0x4000, 0x0100));
    // Motion while off is not saved up for later.
    live.handle_motion(500.0, -500.0);
    live.set_look(Some(DEFAULTS));
    assert_eq!(live.look_settings(), Some(DEFAULTS));
    let c = live.sample(Tick(2)).command;
    assert_eq!((c.view_yaw, c.view_pitch), (0x4000, 0x0100));
}

#[test]
fn right_lowers_yaw_and_towards_the_user_lowers_pitch() {
    // 10 counts: 225280 / 1875 = 120.15, so 120 units.
    let one = |s: LookSettings, dx: f64, dy: f64| Rig::new(s, 0, 0).tick(&[(dx, dy)]);
    assert_eq!(one(DEFAULTS, 10.0, 0.0), (neg(120), 0));
    assert_eq!(one(DEFAULTS, -10.0, 0.0), (120, 0));
    assert_eq!(one(DEFAULTS, 0.0, 10.0), (0, neg(120)));
    assert_eq!(one(DEFAULTS, 0.0, -10.0), (0, 120));
    let inverted = with(66_000, true, 14_563);
    assert_eq!(one(inverted, 0.0, 10.0), (0, 120));
    assert_eq!(one(inverted, 0.0, -10.0), (0, neg(120)));
    assert_eq!(one(inverted, 10.0, 0.0), (neg(120), 0), "yaw never inverts");
}

#[test]
fn pitch_holds_at_its_limit() {
    // 1212 counts up: 14562.10, so 14562, inside 14563. 1213: 14574.11,
    // past it.
    let one = |s: LookSettings, dy: f64| Rig::new(s, 0, 0).tick(&[(0.0, dy)]).1;
    assert_eq!(one(DEFAULTS, -1212.0), 14_562);
    assert_eq!(one(DEFAULTS, -1213.0), 14_563);
    assert_eq!(one(DEFAULTS, 1213.0), neg(14_563));
    assert_eq!(one(with(66_000, true, 14_563), 1213.0), 14_563);
    // Limit 0 holds the pitch level; a limit past a quarter turn is one.
    assert_eq!(one(with(66_000, false, 0), -100.0), 0);
    assert_eq!(one(with(66_000, false, 0), 100.0), 0);
    assert_eq!(one(with(66_000, false, 20_000), -2000.0), 16_384);
    assert_eq!(one(with(66_000, false, 20_000), 2000.0), neg(16_384));
    // A view that starts past the limit is clamped at the first sample.
    assert_eq!(Rig::new(DEFAULTS, 7, 0x4000).tick(&[]), (7, 14_563));
}

/// Inside the limit the split does not matter; past it, the motion past the
/// limit is dropped at each sample, so it does (plan decision 9).
#[test]
fn only_the_pitch_limit_depends_on_tick_boundaries() {
    // Inside: 1000 up then 100 down is 900 up either way:
    // 20275200 / 1875 = 10813.44, so 10813.
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    // 1000 alone: 22528000 / 1875 = 12014.93, so 12015.
    assert_eq!(rig.tick(&[(0.0, -1000.0)]).1, 12_015);
    assert_eq!(rig.tick(&[(0.0, 100.0)]).1, 10_813);
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    assert_eq!(rig.tick(&[(0.0, -1000.0), (0.0, 100.0)]).1, 10_813);

    // Past it: 1213 up holds at 14563 and re-bases there, more up changes
    // nothing, and 100 down then gives 14563 - 1201 = 13362.
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    assert_eq!(rig.tick(&[(10.0, -1213.0)]), (neg(120), 14_563));
    assert_eq!(rig.tick(&[(0.0, -500.0)]).1, 14_563);
    // Yaw keeps its sum: 20 counts are 240.30.
    assert_eq!(rig.tick(&[(10.0, 100.0)]), (neg(240), 13_362));
    // The same 1213 up and 100 down in one tick is 1113 up:
    // 25073664 / 1875 = 13372.62, so 13373.
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    assert_eq!(
        rig.tick(&[(10.0, -1213.0), (10.0, 100.0)]),
        (neg(240), 13_373)
    );
}

#[test]
fn losing_focus_drops_the_motion_not_yet_sampled() {
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    rig.live.handle_motion(10.0, 10.0);
    rig.live.handle(FocusLost);
    assert_eq!(rig.tick(&[]), (0, 0));
    // Motion after the focus loss counts: 20 counts are 240.30.
    rig.live.handle_motion(10.0, 10.0);
    rig.live.handle(FocusLost);
    assert_eq!(rig.tick(&[(20.0, 20.0)]), (neg(240), neg(240)));
    // Motion already sampled stays: the view does not jump back.
    rig.live.handle_motion(5.0, 5.0);
    rig.live.handle(FocusLost);
    assert_eq!(rig.tick(&[]), (neg(240), neg(240)));
}

#[test]
fn set_view_and_set_look_start_look_again() {
    // From +Y, 10 counts right: 16384 - 120 = 16264.
    let mut rig = Rig::new(DEFAULTS, 0x4000, 0);
    rig.live.handle(Down(Key::KeyW));
    let c = rig.command(&[(10.0, 0.0)]);
    assert_eq!(
        (c.forward, c.view_yaw, c.view_pitch),
        (127, 16_264, 0),
        "keys still move"
    );
    rig.live.handle(Up(Key::KeyW));
    // set_view drops the motion before it and counts from its angles.
    rig.live.handle_motion(1000.0, 1000.0);
    rig.live.set_view(0x2000, 0x0100);
    assert_eq!(rig.tick(&[]), (0x2000, 0x0100));
    assert_eq!(rig.tick(&[(10.0, 0.0)]), (0x2000 - 120, 0x0100));
    // Off: the last angles hold, motion is ignored, set_view works again.
    rig.live.set_look(None);
    assert_eq!(rig.live.look_settings(), None);
    assert_eq!(rig.tick(&[(100.0, 100.0)]), (0x2000 - 120, 0x0100));
    // On again, from those angles: 10 counts down lower the pitch by 120.
    rig.live.set_look(Some(DEFAULTS));
    assert_eq!(rig.tick(&[(0.0, 10.0)]), (0x2000 - 120, 0x0100 - 120));
    rig.live.set_look(None);
    rig.live.set_view(3, 4);
    assert_eq!(rig.tick(&[(9.0, 9.0)]), (3, 4));
}

/// Turning look on while it is on starts it over from the angles the last
/// command carried: the motion not yet sampled is dropped, the new
/// settings apply, and the exact sum starts again.
#[test]
fn turning_look_on_again_starts_it_over() {
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    // 50 counts: 1126400 / 1875 = 600.75, so 601.
    assert_eq!(rig.tick(&[(50.0, 50.0)]), (neg(601), neg(601)));
    rig.live.handle_motion(1000.0, 1000.0);
    rig.live.set_look(Some(DEFAULTS));
    assert_eq!(
        rig.tick(&[]),
        (neg(601), neg(601)),
        "the 1000 counts are gone"
    );
    // The sum starts again at 601: 50 more counts are another 601, 1202 in
    // all, where one look would turn 1201 for the 100 counts.
    assert_eq!(rig.tick(&[(50.0, 50.0)]), (neg(1202), neg(1202)));
    // At 0.132° and inverted, N counts are N × 45056 / 1875: 10 are 240.30,
    // so 240, and towards the user now raises the pitch.
    let faster = with(132_000, true, 14_563);
    rig.live.set_look(Some(faster));
    assert_eq!(rig.live.look_settings(), Some(faster));
    assert_eq!(rig.tick(&[(10.0, 10.0)]), (neg(1442), neg(962)));
}
