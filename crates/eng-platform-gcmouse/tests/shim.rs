//! The shim's logic on a fake source: rounding, signs, sums, focus, the
//! callback count and when it looks for mice. CI's macOS runner has no
//! mouse (criterion 11), so nothing here needs one; the same tests run on
//! every target.

use std::cell::Cell;
use std::rc::Rc;

use eng_platform_gcmouse::{Feed, Motion, Shim, Source};
use proptest::prelude::*;

/// A source that counts how often the shim asks it to look, keeps the feed
/// it was given, and finds one more mouse each time it looks after the
/// first, as GCMouse lists mice only once the framework has seen them.
struct Fake {
    refreshes: Rc<Cell<u32>>,
    feed: Rc<Cell<Option<Feed>>>,
}

struct Rig {
    shim: Shim,
    feed: Feed,
    refreshes: Rc<Cell<u32>>,
}

impl Source for Fake {
    fn refresh(&mut self, feed: &Feed) {
        self.refreshes.set(self.refreshes.get() + 1);
        self.feed.set(Some(feed.clone()));
    }

    fn mice(&self) -> usize {
        self.refreshes.get() as usize - 1
    }
}

/// A shim on a fake source, and the feed the source was handed, which
/// stands in for a mouse's handler.
fn rig() -> Rig {
    let refreshes = Rc::new(Cell::new(0));
    let given = Rc::new(Cell::new(None));
    let shim = Shim::with_source(Box::new(Fake {
        refreshes: Rc::clone(&refreshes),
        feed: Rc::clone(&given),
    }));
    let feed = given
        .take()
        .expect("the shim asks its source to look at once");
    Rig {
        shim,
        feed,
        refreshes,
    }
}

/// A focused rig.
fn focused() -> Rig {
    let mut rig = rig();
    rig.shim.set_focused(true);
    rig
}

const fn motion(dx: i64, dy: i64, callbacks: u64) -> Motion {
    Motion { dx, dy, callbacks }
}

/// Mouse look's rounding of a delta in counts (`eng_input`'s `look.rs`):
/// what the drained motion meets next.
fn look_sub_counts(delta: f64) -> i64 {
    if delta.is_finite() {
        (delta * 256.0).round() as i64
    } else {
        0
    }
}

#[test]
fn deltas_round_to_the_nearest_256th_of_a_count_ties_away_from_zero() {
    // 2^-9 is half a sub-count; 3·2^-9 one and a half.
    let half = 1.0 / 512.0;
    let cases = [
        (1.0, 256),
        (0.25, 64),
        (half, 1),
        (-half, -1),
        (3.0 * half, 2),
        (-3.0 * half, -2),
        (half / 2.0, 0),
        (0.0, 0),
        (-0.0, 0),
        (2.5, 640),
    ];
    for (delta, want) in cases {
        let mut rig = focused();
        rig.feed.moved(delta, 0.0);
        assert_eq!(rig.shim.drain(), motion(want, 0, 1), "x {delta}");
        // `y` rounds the same way before its sign turns.
        rig.feed.moved(0.0, -delta);
        assert_eq!(rig.shim.drain(), motion(0, want, 1), "y {}", -delta);
    }
}

#[test]
fn x_keeps_its_sign_and_y_turns_to_towards_the_user() {
    let mut rig = focused();
    // GCMouse: right and up.
    rig.feed.moved(2.0, 3.0);
    assert_eq!(rig.shim.drain(), motion(512, -768, 1));
    assert_eq!(
        rig.shim.drain(),
        Motion::default(),
        "a drain starts a new sum"
    );
    // Left and down (towards the user).
    rig.feed.moved(-1.0, -0.5);
    let moved = rig.shim.drain();
    assert_eq!(moved, motion(-256, 128, 1));
    assert_eq!(moved.counts(), (-1.0, 0.5), "the contract's counts");
}

#[test]
fn a_drain_sums_its_callbacks_exactly() {
    let mut rig = focused();
    let deltas = [(0.1, 0.0), (0.1, -0.3), (0.1, 0.0), (-0.05, 0.7)];
    for (x, y) in deltas {
        rig.feed.moved(x, y);
    }
    // Each rounded on its own, then summed: 26 + 26 + 26 - 13 and
    // -(0 - 77 + 0 + 179).
    assert_eq!(rig.shim.drain(), motion(65, -102, 4));
}

#[test]
fn deltas_that_are_not_finite_add_nothing_but_count() {
    let mut rig = focused();
    rig.feed.moved(f32::NAN, 1.0);
    rig.feed.moved(2.0, f32::INFINITY);
    rig.feed.moved(f32::NEG_INFINITY, f32::NAN);
    assert_eq!(rig.shim.drain(), motion(512, -256, 3));
    assert_eq!(rig.shim.callbacks().get(), 3);
}

#[test]
fn sums_saturate_and_never_panic() {
    let mut rig = focused();
    for _ in 0..3 {
        rig.feed.moved(f32::MAX, f32::MAX);
    }
    assert_eq!(rig.shim.drain(), motion(i64::MAX, i64::MIN, 3));
    for _ in 0..3 {
        rig.feed.moved(f32::MIN, f32::MIN);
    }
    assert_eq!(rig.shim.drain(), motion(i64::MIN, i64::MAX, 3));
}

#[test]
fn nothing_is_summed_without_focus_but_every_callback_counts() {
    let mut rig = rig();
    assert!(!rig.shim.focused(), "the shim starts without focus");
    rig.feed.moved(5.0, 5.0);
    assert_eq!(rig.shim.drain(), Motion::default());
    rig.shim.set_focused(true);
    rig.feed.moved(1.0, 0.0);
    rig.shim.set_focused(false);
    rig.feed.moved(1.0, 0.0);
    rig.shim.set_focused(true);
    rig.feed.moved(0.0, 1.0);
    assert_eq!(
        rig.shim.drain(),
        motion(0, -256, 1),
        "only motion since focus came back"
    );
    assert_eq!(rig.shim.callbacks().get(), 4, "focused or not");
}

#[test]
fn a_focus_change_drops_what_was_not_drained() {
    let mut rig = focused();
    rig.feed.moved(1.0, 1.0);
    rig.shim.set_focused(false);
    rig.shim.set_focused(true);
    assert_eq!(rig.shim.drain(), Motion::default(), "lost with the focus");
    // Told again of what it knows: no change, nothing dropped.
    rig.feed.moved(1.0, 1.0);
    rig.shim.set_focused(true);
    assert_eq!(rig.shim.drain(), motion(256, -256, 1));
    rig.shim.set_focused(false);
    rig.shim.set_focused(false);
    assert!(!rig.shim.focused());
}

#[test]
fn the_callback_count_is_read_at_any_time_and_outlives_the_shim() {
    let rig = focused();
    let callbacks = rig.shim.callbacks();
    let again = callbacks.clone();
    assert_eq!(callbacks.get(), 0);
    rig.feed.moved(1.0, 0.0);
    rig.feed.moved(0.0, 0.0);
    assert_eq!(callbacks.get(), 2, "before any drain");
    drop(rig.shim);
    // A handler the framework still holds may call after the shim goes.
    rig.feed.moved(1.0, 0.0);
    assert_eq!(again.get(), 3);
}

#[test]
fn handlers_may_call_from_any_thread() {
    let mut rig = focused();
    let feeds: Vec<_> = (0..4).map(|_| rig.feed.clone()).collect();
    std::thread::scope(|scope| {
        for feed in &feeds {
            scope.spawn(|| {
                for _ in 0..100 {
                    feed.moved(0.5, -0.25);
                }
            });
        }
    });
    assert_eq!(rig.shim.drain(), motion(400 * 128, 400 * 64, 400));
}

#[test]
fn it_looks_for_mice_at_start_and_when_focus_comes() {
    let mut rig = rig();
    let callbacks = rig.shim.callbacks();
    assert_eq!(rig.refreshes.get(), 1, "at start");
    assert_eq!((rig.shim.mice(), callbacks.mice()), (0, 0), "none yet");
    let _ = rig.shim.drain();
    assert_eq!(rig.refreshes.get(), 1, "not without focus");
    rig.shim.set_focused(true);
    assert_eq!(rig.refreshes.get(), 2, "focus came");
    assert_eq!((rig.shim.mice(), callbacks.mice()), (1, 1), "found then");
    let _ = rig.shim.drain();
    assert_eq!(rig.refreshes.get(), 2, "not again within a second");
    rig.shim.set_focused(false);
    assert_eq!(rig.refreshes.get(), 2, "not when focus goes");
}

#[test]
fn a_focused_drain_looks_again_after_a_second() {
    let mut rig = focused();
    assert_eq!(rig.refreshes.get(), 2);
    std::thread::sleep(std::time::Duration::from_millis(1_050));
    let _ = rig.shim.drain();
    assert_eq!(rig.refreshes.get(), 3, "a second passed");
    assert_eq!(rig.shim.mice(), 2);
    let _ = rig.shim.drain();
    assert_eq!(rig.refreshes.get(), 3);
}

#[test]
fn the_shim_feeds_a_source_its_own_feed() {
    let mut rig = focused();
    rig.shim.feed().moved(1.0, 0.0);
    rig.feed.moved(1.0, 0.0);
    assert_eq!(rig.shim.drain(), motion(512, 0, 2), "one sum");
}

#[test]
fn it_does_not_start_off_the_main_thread_or_off_macos() {
    // Test threads are never the main thread: on macOS GCMouse's handlers
    // run on the main queue, so the shim starts only there
    // (`tests/start.rs` checks that).
    assert!(Shim::start().is_none());
}

proptest! {
    /// A drained sum reaches mouse look as the yaw the separate deltas
    /// would give (ADR-0021): mouse look's rounding of the drained counts
    /// gives back the shim's sum of each delta rounded on its own, however
    /// the callbacks are split between drains.
    #[test]
    fn drained_sums_lose_nothing(
        deltas in prop::collection::vec((-2000.0f32..2000.0, -2000.0f32..2000.0), 0..60),
        cuts in prop::collection::vec(any::<bool>(), 60),
    ) {
        let mut rig = focused();
        let (mut x, mut y) = (0_i64, 0_i64);
        let (mut want_x, mut want_y) = (0_i64, 0_i64);
        let mut drained = |shim: &mut Shim| {
            let (dx, dy) = shim.drain().counts();
            x += look_sub_counts(dx);
            y += look_sub_counts(dy);
        };
        for (&(dx, dy), &cut) in deltas.iter().zip(&cuts) {
            rig.feed.moved(dx, dy);
            want_x += look_sub_counts(f64::from(dx));
            want_y -= look_sub_counts(f64::from(dy));
            if cut {
                drained(&mut rig.shim);
            }
        }
        drained(&mut rig.shim);
        prop_assert_eq!((x, y), (want_x, want_y));
    }

    /// `counts` is exact below 2^53 sub-counts.
    #[test]
    fn counts_round_trip(dx in -(1_i64 << 53)..(1_i64 << 53), dy in -(1_i64 << 53)..(1_i64 << 53)) {
        let (x, y) = motion(dx, dy, 1).counts();
        prop_assert_eq!((look_sub_counts(x), look_sub_counts(y)), (dx, dy));
    }
}
