//! Rung 2 criterion 9, `mouse-look`: the exact arithmetic. Each delta
//! rounds to a 1/256 count, ties away from zero; the turn comes from the
//! exact sum, so any split into ticks gives the same angles; yaw wraps;
//! products past `i64` stay exact, and sums saturate at 2^63 sub-counts.
//! Values are worked by hand from the rule in `mouse_look.rs`.

mod look_rig;

use look_rig::{DEFAULTS, Rig, neg, with};

#[test]
fn ties_round_half_away_from_zero() {
    // At 0.703125° per count a 1/256 count is 0.703125 / 256 × 65536 / 360
    // = 0.5 units exactly.
    let s = with(703_125, false, 14_563);
    let one = |dx: f64, dy: f64| Rig::new(s, 0, 0).tick(&[(dx, dy)]);
    let n = |k: f64| k / 256.0;
    assert_eq!(one(n(1.0), 0.0), (neg(1), 0), "0.5 -> 1");
    assert_eq!(one(n(-1.0), 0.0), (1, 0), "-0.5 -> -1");
    assert_eq!(one(n(5.0), 0.0), (neg(3), 0), "2.5 -> 3, not 2");
    assert_eq!(one(n(-5.0), 0.0), (3, 0), "-2.5 -> -3");
    assert_eq!(one(0.0, n(1.0)), (0, neg(1)));
    assert_eq!(one(0.0, n(-3.0)), (0, 2), "pitch +1.5 -> 2");
    // Two ticks of one half each: the sum is 1, not 1 + 1.
    let mut rig = Rig::new(s, 0, 0);
    assert_eq!(
        rig.ticks(&[&[(n(1.0), n(1.0))], &[(n(1.0), n(1.0))]]),
        (neg(1), neg(1))
    );
}

#[test]
fn each_delta_rounds_to_a_256th_of_a_count_first() {
    // At 1.40625° per count a 1/256 count is 1 unit exactly.
    let s = with(1_406_250, false, 16_384);
    let one = |motion: &[(f64, f64)]| Rig::new(s, 0, 0).tick(motion);
    assert_eq!(
        one(&[(1.0 / 512.0, -1.0 / 512.0)]),
        (neg(1), 1),
        "a half 256th rounds away from zero: 1 right, 1 up"
    );
    assert_eq!(one(&[(0.3, 0.0)]), (neg(77), 0), "76.8 sub-counts -> 77");
    // Below half a 256th, each delta is 0 on its own, however many come.
    assert_eq!(one(&[(0.0019, 0.0019); 1000]), (0, 0));
    // A delta that is not finite is dropped, on its own axis.
    let bad = [
        (f64::NAN, 1.0),
        (f64::INFINITY, f64::NEG_INFINITY),
        (1.0, f64::NAN),
    ];
    assert_eq!(one(&bad), (neg(256), neg(256)));
}

#[test]
fn any_split_of_the_same_motion_gives_the_same_angles() {
    // 100 counts: 2252800 / 1875 = 1201.49, so 1201. Rounding each tick's
    // turn instead would give 601 + 601 = 1202 (50 counts are 600.75), or
    // 100 × 12 = 1200 one count at a time.
    let want = (neg(1201), neg(1201));
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    assert_eq!(rig.tick(&[(100.0, 100.0)]), want);
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    assert_eq!(rig.tick(&[(50.0, 50.0)]), (neg(601), neg(601)));
    assert_eq!(rig.tick(&[(50.0, 50.0)]), want);
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    assert_eq!(rig.tick(&[(1.0, 1.0); 100]), want);
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    let mut per_tick = Vec::new();
    for _ in 0..100 {
        per_tick.push(rig.tick(&[(1.0, 1.0)]));
    }
    // 1 count is 12.01, 2 are 24.03.
    assert_eq!(per_tick[..2], [(neg(12), neg(12)), (neg(24), neg(24))]);
    assert_eq!(per_tick[99], want);
}

#[test]
fn yaw_wraps() {
    // 3000 counts: 36044.8, so 36045; -36045 is 29491 mod 65536.
    assert_eq!(Rig::new(DEFAULTS, 0, 0).tick(&[(3000.0, 0.0)]), (29_491, 0));
    // -6000 counts: 72089.6, so 72090, which is 6554 mod 65536.
    assert_eq!(Rig::new(DEFAULTS, 0, 0).tick(&[(-6000.0, 0.0)]), (6_554, 0));
    // Across 0 both ways: 100 - 120 = -20, and 65530 + 12 = 65542.
    assert_eq!(
        Rig::new(DEFAULTS, 100, 0).tick(&[(10.0, 0.0)]),
        (neg(20), 0)
    );
    assert_eq!(Rig::new(DEFAULTS, 65_530, 0).tick(&[(-1.0, 0.0)]), (6, 0));
    // 10^9 counts over ten ticks: 10^9 × 22528 / 1875 = 12014933333.33,
    // so 12014933333 = 183333 × 65536 + 21845, and yaw is -21845.
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    let tick: &[(f64, f64)] = &[(1_000_000.0, 0.0); 100];
    assert_eq!(rig.ticks(&[tick; 10]), (neg(21_845), 0));
}

/// `n · s` passes `i64` long before a sum saturates: these products only
/// an `i128` (or wider) holds exactly.
#[test]
fn products_past_i64_are_exact() {
    // 4218.75° per count is 703125 × 6000 millionths, and 703125 millionths
    // turn 128 units a count, so a count turns 768000 units exactly.
    // 10^15 + 1 counts (exact in f64) are 256 × (10^15 + 1) sub-counts, and
    // that × 4218750000 is about 1.08 × 10^27, far past i64's 9.2 × 10^18.
    let s = with(4_218_750_000, false, 14_563);
    // 768000 × 10^15 is a multiple of 2^16 (768000 = 2^11 × 375, 10^15 =
    // 2^15 × 5^15), so only 768000 = 11 × 65536 + 47104 is left: yaw is
    // -47104, which is 18432.
    let far = 1e15 + 1.0;
    assert_eq!(Rig::new(s, 0, 0).tick(&[(far, 0.0)]), (18_432, 0));
    assert_eq!(Rig::new(s, 0, 0).tick(&[(-far, 0.0)]), (47_104, 0));
    // One millionth more per count adds 10^15 × 128 / 703125 units for
    // 10^15 counts: 2^15 × 5^8 × 128 / 9 = 1638400000000 / 9 =
    // 182044444444.44, rounded down. 182044444444 = 2777777 × 65536 +
    // 50972, so yaw is -50972, which is 14564.
    let s = with(4_218_750_001, false, 14_563);
    assert_eq!(Rig::new(s, 0, 0).tick(&[(1e15, 0.0)]), (14_564, 0));
}

#[test]
fn sums_saturate_at_2_to_the_63_sub_counts() {
    // At 1.40625° per count a 1/256 count is 1 unit, so yaw is minus the
    // sum in sub-counts. f64::MAX × 256 is infinite, which saturates to
    // 2^63 - 1, and -(2^63 - 1) is 1 mod 65536. (2^63 - 1) × 1406250 is
    // about 1.3 × 10^25: i128 again.
    let s = with(1_406_250, false, 16_384);
    assert_eq!(Rig::new(s, 0, 0).tick(&[(f64::MAX, 0.0)]), (1, 0));
    assert_eq!(Rig::new(s, 0, 0).tick(&[(f64::MAX, 0.0); 2]), (1, 0));
    // 2^54 counts are 2^62 sub-counts, 0 mod 65536; two ticks of them sum to
    // 2^63, one past i64, which saturates to 2^63 - 1.
    let big = (1u64 << 54) as f64;
    let mut rig = Rig::new(s, 0, 0);
    assert_eq!(rig.tick(&[(big, 0.0)]), (0, 0));
    assert_eq!(rig.tick(&[(big, 0.0)]), (1, 0));
    // Saturated, more motion that way is lost, and motion back counts
    // from 2^63 - 1: one count back is 2^63 - 257, so yaw is 257.
    assert_eq!(rig.tick(&[(1.0, 0.0)]), (1, 0));
    assert_eq!(rig.tick(&[(-1.0, 0.0)]), (257, 0));
    // The other way: f64::MIN is -2^63 sub-counts; one count right makes
    // -2^63 + 256, so yaw is 2^63 - 256, which is -256 mod 65536.
    let back = [(f64::MIN, 0.0), (1.0, 0.0)];
    assert_eq!(Rig::new(s, 0, 0).tick(&back), (neg(256), 0));
    // At 0.066°: (2^63 - 1) × 66000 / 1406250 = (2^63 - 1) × 88 / 1875 =
    // 432883594263050811 + 391/1875, rounded down, and 432883594263050811
    // = 6605279453476 × 65536 + 47675, so yaw is -47675, which is 17861.
    assert_eq!(
        Rig::new(DEFAULTS, 0, 0).tick(&[(f64::MAX, 0.0)]),
        (17_861, 0)
    );
    // Pitch: the limit holds and re-bases, so 10 counts up from there are
    // 120 above -14563.
    let mut rig = Rig::new(DEFAULTS, 0, 0);
    assert_eq!(rig.tick(&[(0.0, f64::MAX)]).1, neg(14_563));
    assert_eq!(rig.tick(&[(0.0, -10.0)]).1, neg(14_443));
}
