//! Rung 2 criterion 9, `mouse-look`: the same timeline gives the same
//! commands (twice, and from a copy taken part-way), and any split gives
//! the angles of the sum, against a model written from the rules in
//! another form: Euclidean division over the unreduced denominator, where
//! the crate divides magnitudes by a reduced one. Neither side asks the
//! code under test for an expected value.

mod look_rig;

use std::cmp::Ordering;

use eng_core::Tick;
use eng_input::KeyEvent::{Down, FocusLost, Up};
use eng_input::*;
use look_rig::{Rig, with};
use proptest::prelude::*;

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
fn in_sub_counts(m: i64) -> i128 {
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
                    Event::Motion(dx, dy) => {
                        (x, y) = (x + in_sub_counts(dx), y + in_sub_counts(dy))
                    }
                    Event::NanX(dy) => y += in_sub_counts(dy),
                    Event::InfY(dx) => x += in_sub_counts(dx),
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
            (x, y) = (x + in_sub_counts(dx), y + in_sub_counts(dy));
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
