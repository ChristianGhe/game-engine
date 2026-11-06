//! Rung 2 criterion 9, `mouse-look` (plan decision 9, queue Q3): the signs,
//! the exact sum (any split of the same motion into ticks gives the same
//! yaw, ties half away from zero), the pitch limit, yaw wrapping, focus
//! loss dropping unsampled motion, and the same timeline giving the same
//! commands.
//!
//! Expected values are worked by hand from the rule: N counts at s degrees
//! per count turn `round(N × s × 65536 / 360)` binary-angle units, ties
//! away from zero, after each delta is rounded to a 1/256 count. At the
//! default 0.066°, `N × 0.066 × 65536 / 360 = N × 22528 / 1875`, about
//! `N × 12.01493`. The properties compare with a model written from the
//! same rules in another form (`model`, `turn`); nothing here asks the code
//! under test for an expected value.

use std::cmp::Ordering;

use eng_core::{PlayerCommand, Tick};
use eng_input::KeyEvent::{Down, FocusLost, Up};
use eng_input::*;
use proptest::prelude::*;

/// The defaults, written out: 0.066° per count, not inverted, and 14563
/// units, the last binary angle not past 80° (80 × 65536 / 360 = 14563.56).
const DEFAULTS: LookSettings = LookSettings {
    sensitivity_microdeg: 66_000,
    invert_pitch: false,
    pitch_limit: 14_563,
};

const fn with(sensitivity_microdeg: u32, invert_pitch: bool, pitch_limit: u16) -> LookSettings {
    LookSettings {
        sensitivity_microdeg,
        invert_pitch,
        pitch_limit,
    }
}

/// `-units` as a binary angle.
const fn neg(units: u16) -> u16 {
    0u16.wrapping_sub(units)
}

/// Live input and the next tick to sample.
struct Rig {
    live: LiveInput,
    tick: u64,
}

impl Rig {
    /// Look on with `settings`, from `(yaw, pitch)`.
    fn new(settings: LookSettings, yaw: u16, pitch: u16) -> Self {
        let mut live = LiveInput::new(Bindings::default());
        live.set_view(yaw, pitch);
        live.set_look(Some(settings));
        Self { live, tick: 1 }
    }

    /// Handles `motion`, then samples the next tick: its command.
    fn command(&mut self, motion: &[(f64, f64)]) -> PlayerCommand {
        for &(dx, dy) in motion {
            self.live.handle_motion(dx, dy);
        }
        let command = self.live.sample(Tick(self.tick)).command;
        self.tick += 1;
        command
    }

    /// The same, giving the command's `(yaw, pitch)`.
    fn tick(&mut self, motion: &[(f64, f64)]) -> (u16, u16) {
        let c = self.command(motion);
        (c.view_yaw, c.view_pitch)
    }

    /// One tick per entry; the angles after the last.
    fn ticks(&mut self, per_tick: &[&[(f64, f64)]]) -> (u16, u16) {
        per_tick.iter().map(|m| self.tick(m)).last().unwrap()
    }
}

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
    assert_eq!(one(&[(0.3, 0.0)]), (neg(77), 0), "76.8 256ths -> 77");
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

// --- The model and the properties -------------------------------------

/// `round(num / den)`, ties away from zero, by Euclidean division (the
/// crate divides magnitudes instead).
fn round_half_away(num: i128, den: i128) -> i128 {
    let (q, r) = (num.div_euclid(den), num.rem_euclid(den));
    match (2 * r).cmp(&den) {
        Ordering::Less => q,
        Ordering::Greater => q + 1,
        Ordering::Equal if num > 0 => q + 1,
        Ordering::Equal => q,
    }
}

/// The turn for `n` 1/256 counts at `s` millionths of a degree per count,
/// unreduced: `(n / 256) × (s / 10^6) × 65536 / 360`.
fn turn(n: i128, s: u32) -> i128 {
    round_half_away(n * i128::from(s) * 65_536, 256 * 1_000_000 * 360)
}

/// An event: a key event, or motion in 1/512 counts (exact as `f64`),
/// both axes or one with the other not finite.
#[derive(Debug, Clone, Copy)]
enum Event {
    Key(KeyEvent),
    Motion(i64, i64),
    NanX(i64),
    InfY(i64),
}

impl Event {
    fn feed(self, live: &mut LiveInput) {
        let counts = |m: i64| m as f64 / 512.0;
        match self {
            Event::Key(key) => live.handle(key),
            Event::Motion(x, y) => live.handle_motion(counts(x), counts(y)),
            Event::NanX(y) => live.handle_motion(f64::NAN, counts(y)),
            Event::InfY(x) => live.handle_motion(counts(x), f64::NEG_INFINITY),
        }
    }
}

/// 1/512 counts to 1/256 counts, a half away from zero: `m % 2` has the
/// sign of `m`.
fn in_256ths(m: i64) -> i128 {
    (i128::from(m) + i128::from(m % 2)) / 2
}

/// The rules written out directly. Events for tick 0 belong to tick 1. A
/// tick's motion is its events after its last focus loss; yaw is the start
/// minus the turn for all motion so far; pitch is its anchor minus (plus,
/// inverted) the turn for the motion since, re-anchored at the limit.
fn model(
    s: LookSettings,
    start: (u16, u16),
    timeline: &[(Tick, Event)],
    ticks: u64,
) -> Vec<(u16, u16)> {
    let limit = i128::from(s.pitch_limit.min(16_384));
    let (mut x, mut y) = (0, 0);
    let mut anchor = i128::from(start.1 as i16);
    (1..=ticks)
        .map(|t| {
            let events: Vec<Event> = timeline
                .iter()
                .filter(|(at, _)| at.0.max(1) == t)
                .map(|&(_, e)| e)
                .collect();
            let after = events
                .iter()
                .rposition(|e| matches!(e, Event::Key(FocusLost)));
            for &event in &events[after.map_or(0, |i| i + 1)..] {
                match event {
                    Event::Motion(dx, dy) => (x, y) = (x + in_256ths(dx), y + in_256ths(dy)),
                    Event::NanX(dy) => y += in_256ths(dy),
                    Event::InfY(dx) => x += in_256ths(dx),
                    Event::Key(_) => {}
                }
            }
            let yaw = (i128::from(start.0) - turn(x, s.sensitivity_microdeg)).rem_euclid(65_536);
            let down = turn(y, s.sensitivity_microdeg);
            let mut pitch = anchor + if s.invert_pitch { down } else { -down };
            if pitch.abs() > limit {
                (anchor, y) = (limit * pitch.signum(), 0);
                pitch = anchor;
            }
            (yaw as u16, pitch.rem_euclid(65_536) as u16)
        })
        .collect()
}

fn any_settings() -> impl Strategy<Value = LookSettings> {
    let sensitivity = prop_oneof![
        Just(66_000),
        Just(703_125),
        Just(1_406_250),
        1u32..=4_000_000
    ];
    let limit = prop_oneof![Just(14_563), Just(0), Just(16_384), any::<u16>()];
    (sensitivity, any::<bool>(), limit).prop_map(|(s, i, l)| with(s, i, l))
}

/// Mostly a few counts; sometimes thousands, to pass the limit and wrap.
fn any_motion() -> impl Strategy<Value = i64> {
    prop_oneof![8 => -4096i64..=4096, 1 => -4_000_000i64..=4_000_000]
}

fn any_event() -> impl Strategy<Value = Event> {
    let key = prop::sample::select(vec![Key::KeyW, Key::KeyD, Key::Escape]);
    prop_oneof![
        8 => (any_motion(), any_motion()).prop_map(|(x, y)| Event::Motion(x, y)),
        1 => any_motion().prop_map(Event::NanX),
        1 => any_motion().prop_map(Event::InfY),
        1 => Just(Event::Key(FocusLost)),
        1 => key.clone().prop_map(|k| Event::Key(Down(k))),
        1 => key.prop_map(|k| Event::Key(Up(k))),
    ]
}

/// Events with non-decreasing ticks from 0, and a tick count past the last.
fn any_timeline() -> impl Strategy<Value = (Vec<(Tick, Event)>, u64)> {
    (
        prop::collection::vec((0u64..3, any_event()), 0..150),
        0u64..4,
    )
        .prop_map(|(steps, extra)| {
            let mut tick = 0;
            let timeline: Vec<_> = steps
                .into_iter()
                .map(|(step, event)| {
                    tick += step;
                    (Tick(tick), event)
                })
                .collect();
            (timeline, tick + extra)
        })
}

proptest! {
    #[test]
    fn the_same_timeline_gives_the_same_commands(
        s in any_settings(),
        start in (any::<u16>(), any::<u16>()),
        (timeline, ticks) in any_timeline(),
        split in 0u64..200,
    ) {
        // Twice from the start, and a copy taken part-way fed the rest.
        let mut lives = [0, 1].map(|_| {
            let mut live = LiveInput::new(Bindings::default());
            live.set_view(start.0, start.1);
            live.set_look(Some(s));
            live
        });
        let mut twin: Option<LiveInput> = None;
        let mut runs = [Vec::new(), Vec::new(), Vec::new()];
        let mut events = timeline.iter().peekable();
        for t in 1..=ticks {
            while let Some(&(_, event)) = events.next_if(|(at, _)| at.0 <= t) {
                for live in lives.iter_mut().chain(twin.as_mut()) {
                    event.feed(live);
                }
            }
            if t == split {
                twin = Some(lives[0].clone());
            }
            runs[0].push(lives[0].sample(Tick(t)));
            runs[1].push(lives[1].sample(Tick(t)));
            if let Some(twin) = twin.as_mut() {
                runs[2].push(twin.sample(Tick(t)));
            }
        }
        prop_assert_eq!(&runs[0], &runs[1]);
        if (1..=ticks).contains(&split) {
            prop_assert_eq!(&runs[2][..], &runs[0][split as usize - 1..]);
        }
        let angles: Vec<_> = runs[0].iter().map(|i| (i.command.view_yaw, i.command.view_pitch)).collect();
        prop_assert_eq!(angles, model(s, start, &timeline, ticks));
    }

    /// Sampling after any events at all, the yaw is the turn for the sum so
    /// far; within the limit, so is the pitch.
    #[test]
    fn any_split_gives_the_angles_of_the_sum(
        s in prop_oneof![1u32..=66_000, 1u32..=4_000_000],
        invert in any::<bool>(),
        start_yaw in any::<u16>(),
        motion in prop::collection::vec((any_motion(), -2048i64..=2048, any::<bool>()), 0..64),
    ) {
        // 64 events of at most 4 counts at most 0.066° are under 17° of
        // pitch, so it is checked only there.
        let s = with(s, invert, 16_384);
        let mut rig = Rig::new(s, start_yaw, 0);
        let (mut x, mut y) = (0, 0);
        for &(dx, dy, sample) in &motion {
            rig.live.handle_motion(dx as f64 / 512.0, dy as f64 / 512.0);
            (x, y) = (x + in_256ths(dx), y + in_256ths(dy));
            if sample {
                let (yaw, pitch) = rig.tick(&[]);
                let want = (i128::from(start_yaw) - turn(x, s.sensitivity_microdeg)).rem_euclid(65_536);
                prop_assert_eq!(i128::from(yaw), want);
                if s.sensitivity_microdeg <= 66_000 {
                    let down = turn(y, s.sensitivity_microdeg);
                    let want = if invert { down } else { -down };
                    prop_assert_eq!(i128::from(pitch as i16), want);
                }
            }
        }
    }
}
