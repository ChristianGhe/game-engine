//! The frame loop's tick clock (plan, "One frame"): ticks come from the
//! timer, at most five per wake, and the rest are dropped and counted.

use eng_app::{Due, MAX_TICKS_PER_WAKE, TickClock};
use eng_platform_api::Instant;

/// A tick at 60 Hz, in nanoseconds: 1/60 s rounded down, as the headless
/// ticker paces `run`.
const TICK: u64 = 16_666_666;

fn at(nanos: u64) -> Instant {
    Instant::from_nanos(nanos)
}

#[test]
fn the_first_tick_is_due_a_tick_after_the_start() {
    let mut clock = TickClock::new(60, at(1_000));
    assert_eq!(clock.next(), at(1_000 + TICK));
    assert_eq!(clock.due(at(1_000)), Due { run: 0, dropped: 0 });
    assert_eq!(clock.due(at(1_000 + TICK - 1)), Due { run: 0, dropped: 0 });
    assert_eq!(clock.due(at(1_000 + TICK)), Due { run: 1, dropped: 0 });
    assert_eq!(clock.next(), at(1_000 + 2 * TICK));
}

#[test]
fn each_due_tick_runs_once() {
    let mut clock = TickClock::new(60, at(0));
    // Three ticks are due a hair after the third: all three run, once.
    assert_eq!(clock.due(at(3 * TICK + 5)), Due { run: 3, dropped: 0 });
    assert_eq!(clock.due(at(3 * TICK + 5)), Due { run: 0, dropped: 0 });
    assert_eq!(clock.next(), at(4 * TICK));
    assert_eq!(clock.due(at(4 * TICK)), Due { run: 1, dropped: 0 });
}

#[test]
fn at_most_five_ticks_run_per_wake_and_the_rest_are_dropped() {
    assert_eq!(MAX_TICKS_PER_WAKE, 5);
    let mut clock = TickClock::new(60, at(0));
    assert_eq!(clock.due(at(5 * TICK)), Due { run: 5, dropped: 0 });
    let mut clock = TickClock::new(60, at(0));
    assert_eq!(clock.due(at(6 * TICK)), Due { run: 5, dropped: 1 });
    // A second-long stall: 60 ticks due, 5 run, 55 dropped, and the clock
    // goes on from the next tick after the stall rather than catching up.
    let mut clock = TickClock::new(60, at(0));
    assert_eq!(
        clock.due(at(60 * TICK + 7)),
        Due {
            run: 5,
            dropped: 55
        }
    );
    assert_eq!(clock.next(), at(61 * TICK));
    assert_eq!(clock.due(at(61 * TICK)), Due { run: 1, dropped: 0 });
}

#[test]
fn alpha_is_how_far_the_clock_is_from_the_latest_tick_to_the_next() {
    // 50 Hz: 20 ms a tick, so quarters are whole nanoseconds.
    const T: u64 = 20_000_000;
    let mut clock = TickClock::new(50, at(0));
    assert_eq!(clock.alpha(at(0)), 0.0);
    assert_eq!(clock.alpha(at(T / 2)), 0.5);
    clock.due(at(T));
    assert_eq!(clock.alpha(at(T)), 0.0);
    assert_eq!(clock.alpha(at(T + T / 4)), 0.25);
    // Never outside [0, 1], even for a frame that comes before its tick or
    // long after the next one.
    let fresh = TickClock::new(50, at(T));
    assert_eq!(fresh.alpha(at(5)), 0.0);
    assert_eq!(clock.alpha(at(10 * T)), 1.0);
}

#[test]
fn other_tick_rates_pace_alike() {
    let mut clock = TickClock::new(30, at(0));
    assert_eq!(clock.next(), at(33_333_333));
    assert_eq!(clock.due(at(100_000_000)), Due { run: 3, dropped: 0 });
}

#[test]
#[should_panic(expected = "tick rate must be positive")]
fn a_zero_tick_rate_is_refused() {
    let _ = TickClock::new(0, at(0));
}
