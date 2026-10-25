//! Quantization details beyond criterion 18 (which is `live_input.rs`):
//! cancelling, shared actions, the view, rebinding and misuse.

use eng_core::{PlayerCommand, Tick};
use eng_input::Key::{ArrowDown, ArrowUp, KeyA, KeyS, KeyW};
use eng_input::KeyEvent::{Down, FocusLost, Up};
use eng_input::*;

fn at(tick: u64, event: KeyEvent) -> (Tick, KeyEvent) {
    (Tick(tick), event)
}

/// `(forward, side, quit)` for each tick, with the default bindings.
fn run(timeline: &[(Tick, KeyEvent)], ticks: u64) -> Vec<(i8, i8, bool)> {
    quantize(&Bindings::default(), timeline, ticks)
        .iter()
        .map(|i| (i.command.forward, i.command.side, i.quit()))
        .collect()
}

#[test]
fn opposite_actions_cancel_and_keys_share_actions() {
    let t = [
        at(1, Down(KeyW)),
        at(1, Down(KeyA)),
        at(2, Down(KeyS)),
        at(3, Down(ArrowDown)),
        at(3, Up(KeyW)),
        at(4, Up(KeyS)), // ArrowDown still holds move_back
    ];
    let want = [
        (127, -127, false),
        (0, -127, false),
        (-127, -127, false),
        (-127, -127, false),
    ];
    assert_eq!(run(&t, 4), want);
}

#[test]
fn commands_carry_the_tick_and_the_held_view() {
    let mut live = LiveInput::new(Bindings::default());
    live.set_view(0x4000, 0x0100);
    live.handle(Down(ArrowUp));
    let input = live.sample(Tick(7));
    let want = PlayerCommand {
        forward: 127,
        view_yaw: 0x4000,
        view_pitch: 0x0100,
        ..PlayerCommand::idle(Tick(7))
    };
    assert_eq!(input.command, want);
    // Rebinding applies at the next sample, to keys already held.
    live.bindings_mut().bind(ArrowUp, Action::MoveRight);
    let next = live.sample(Tick(8)).command;
    assert_eq!((next.forward, next.side), (0, 127));
}

#[test]
#[should_panic(expected = "tick order")]
fn a_timeline_out_of_tick_order_panics() {
    quantize(
        &Bindings::default(),
        &[at(3, FocusLost), at(2, FocusLost)],
        3,
    );
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "[E-INPUT] LiveInput sampled tick 4 after tick 4")]
fn sampling_a_tick_twice_panics_in_debug_builds() {
    let mut live = LiveInput::new(Bindings::default());
    live.sample(Tick(4));
    live.sample(Tick(4));
}
