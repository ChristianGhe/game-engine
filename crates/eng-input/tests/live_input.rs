//! Rung 1b criterion 18, `live-input`: default bindings from strict TOML,
//! `[E-BINDINGS]` with the line, one `PlayerCommand` per tick from a key
//! timeline (the same on every target), taps, focus loss, and `quit`
//! staying out of the commands.

use eng_core::{PlayerCommand, Tick};
use eng_input::Key::{
    ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Escape, KeyA, KeyD, KeyI, KeyQ, KeyS, KeyW, Space,
};
use eng_input::KeyEvent::{Down, FocusLost, Up};
use eng_input::*;
use proptest::prelude::*;

fn at(tick: u64, event: KeyEvent) -> (Tick, KeyEvent) {
    (Tick(tick), event)
}

/// `(forward, side, quit)` for each tick.
fn summary(inputs: &[TickInput]) -> Vec<(i8, i8, bool)> {
    inputs
        .iter()
        .map(|i| (i.command.forward, i.command.side, i.quit()))
        .collect()
}

fn run(timeline: &[(Tick, KeyEvent)], ticks: u64) -> Vec<(i8, i8, bool)> {
    summary(&quantize(&Bindings::default(), timeline, ticks))
}

// --- Bindings ---------------------------------------------------------

#[test]
fn default_bindings_load_from_toml() {
    let defaults = Bindings::default();
    assert_eq!(
        defaults,
        Bindings::parse(DEFAULT_BINDINGS, "default").unwrap()
    );
    let want = [
        (Action::MoveForward, vec![KeyW, ArrowUp]),
        (Action::MoveBack, vec![KeyS, ArrowDown]),
        (Action::MoveLeft, vec![KeyA, ArrowLeft]),
        (Action::MoveRight, vec![KeyD, ArrowRight]),
        (Action::Quit, vec![Escape]),
    ];
    for (action, mut keys) in want {
        keys.sort();
        let mut got: Vec<_> = defaults.keys(action).collect();
        got.sort();
        assert_eq!(got, keys, "{action}");
    }
    let bound = Key::ALL
        .iter()
        .filter(|&&k| defaults.action(k).is_some())
        .count();
    assert_eq!(bound, 9, "nothing else is bound");
}

#[test]
fn a_bindings_file_replaces_the_defaults() {
    let text =
        "version = 1\nx_note = \"ignored\"\n\n[actions]\nmove_forward = [\"KeyI\"]\nquit = []\n";
    let b = Bindings::parse(text, "mine.toml").unwrap();
    assert_eq!(b.action(KeyI), Some(Action::MoveForward));
    assert_eq!(b.action(KeyW), None);
    assert_eq!(b.action(Escape), None, "`quit = []` unbinds Escape");
    assert_eq!(b.keys(Action::MoveBack).count(), 0, "left out: unbound");
}

#[test]
fn unknown_names_fail_with_e_bindings_and_the_line() {
    let cases = [
        (
            "version = 1\n[actions]\nmove_forward = [\"KeyW\"]\njmup = [\"Space\"]\n",
            4,
            "unknown action `jmup`",
        ),
        (
            "version = 1\n[actions]\nmove_forward = [\n  \"KeyW\",\n  \"KeyWW\",\n]\n",
            5,
            "unknown key `KeyWW`",
        ),
        (
            "version = 1\n[actions]\nMove_Forward = [\"KeyW\"]\n",
            3,
            "unknown action `Move_Forward`",
        ),
        // Inline tables and dotted keys name the right line too.
        (
            "version = 1\n\nactions = { quit = [\"Escape\"], fly = [] }\n",
            3,
            "unknown action `fly`",
        ),
        (
            "version = 1\nactions.quit = [\"Escape\"]\nactions.move_back = [\"Escap\"]\n",
            3,
            "unknown key `Escap`",
        ),
        // The first problem in file order, not in name order, and an
        // unknown action whatever its value.
        (
            "version = 1\n[actions]\nzoom = []\naim = []\n",
            3,
            "unknown action `zoom`",
        ),
        (
            "version = 1\n[actions]\nfly = []\nquit = \"Escape\"\n",
            3,
            "unknown action `fly`",
        ),
        (
            "version = 1\n[actions]\nfly = \"KeyW\"\n",
            3,
            "unknown action `fly`",
        ),
        (
            "version = 1\n\n[actions.fly]\nkey = \"KeyW\"\n",
            3,
            "unknown action `fly`",
        ),
        // A key triggers one action.
        (
            "version = 1\n[actions]\nmove_forward = [\"KeyW\"]\nquit = [\"Escape\", \"KeyW\"]\n",
            4,
            "key `KeyW` is already bound to `move_forward` on line 3",
        ),
    ];
    for (text, line, needle) in cases {
        let err = Bindings::parse(text, "b.toml").unwrap_err();
        assert_eq!(err.line, Some(line), "{err}");
        let shown = err.to_string();
        let prefix = format!("[E-BINDINGS] b.toml:{line}: ");
        assert!(
            shown.starts_with(&prefix),
            "{shown:?} should start {prefix:?}"
        );
        assert!(
            shown.contains(needle),
            "{shown:?} should mention {needle:?}"
        );
    }
}

// --- Quantization -----------------------------------------------------

#[test]
fn a_held_key_counts_until_its_release() {
    let t = [at(2, Down(KeyW)), at(5, Up(KeyW))];
    let forward: Vec<_> = run(&t, 6).iter().map(|s| s.0).collect();
    assert_eq!(forward, [0, 127, 127, 127, 0, 0]);

    // OS key repeat sends more `Down`s while the key is held. A held key
    // is not a tap, so a repeat just before the release adds no tick.
    let repeats = [
        at(2, Down(KeyW)),
        at(3, Down(KeyW)),
        at(5, Down(KeyW)),
        at(5, Up(KeyW)),
    ];
    assert_eq!(run(&repeats, 6), run(&t, 6));
}

#[test]
fn a_tap_shorter_than_a_tick_counts_for_one_tick() {
    let tap = [at(3, Down(KeyW)), at(3, Up(KeyW))];
    let forward: Vec<_> = run(&tap, 5).iter().map(|s| s.0).collect();
    assert_eq!(forward, [0, 0, 127, 0, 0]);

    // Several taps between two samples still count once.
    let taps = [
        at(2, Down(KeyD)),
        at(2, Up(KeyD)),
        at(2, Down(KeyD)),
        at(2, Up(KeyD)),
    ];
    let side: Vec<_> = run(&taps, 3).iter().map(|s| s.1).collect();
    assert_eq!(side, [0, 127, 0]);

    // Streaming: a tap, then a wake that runs three ticks at once.
    let mut live = LiveInput::new(Bindings::default());
    live.handle(Down(Escape));
    live.handle(Up(Escape));
    let quits: Vec<_> = (1..=3).map(|t| live.sample(Tick(t)).quit()).collect();
    assert_eq!(quits, [true, false, false]);
}

#[test]
fn losing_focus_releases_every_key() {
    let t = [
        at(1, Down(KeyW)),
        at(1, Down(KeyD)),
        at(1, Down(Escape)),
        at(4, FocusLost),
        at(5, Up(KeyW)), // stale: changes nothing
        at(6, Down(KeyS)),
    ];
    let want = [
        (127, 127, true),
        (127, 127, true),
        (127, 127, true),
        (0, 0, false),
        (0, 0, false),
        (-127, 0, false),
    ];
    assert_eq!(run(&t, 6), want);

    let mut live = LiveInput::new(Bindings::default());
    live.handle(Down(ArrowLeft));
    live.handle(FocusLost);
    assert!(!live.is_held(ArrowLeft));
    // Focus loss is a release: the press since the last sample counts once.
    assert_eq!(live.sample(Tick(1)).command.side, -127);
    assert_eq!(live.sample(Tick(2)).command.side, 0);
}

#[test]
fn quit_never_becomes_a_player_command() {
    let t = [at(1, Down(Escape)), at(3, Up(Escape))];
    let inputs = quantize(&Bindings::default(), &t, 4);
    for (input, quit) in inputs.iter().zip([true, true, false, false]) {
        assert_eq!(input.command, PlayerCommand::idle(input.command.tick));
        assert_eq!(input.quit(), quit);
    }

    // Rebound to W, W quits and no longer moves.
    let mut b = Bindings::default();
    assert_eq!(b.bind(KeyW, Action::Quit), Some(Action::MoveForward));
    let inputs = quantize(&b, &[at(1, Down(KeyW))], 2);
    assert!(inputs.iter().all(|i| i.quit()));
    assert!(
        inputs
            .iter()
            .all(|i| i.command == PlayerCommand::idle(i.command.tick))
    );
}

/// A fixed pseudo-random timeline (a 64-bit LCG, no floats).
fn fixed_timeline() -> Vec<(Tick, KeyEvent)> {
    const POOL: [Key; 11] = [
        KeyW, KeyA, KeyS, KeyD, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Escape, Space, KeyQ,
    ];
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize
    };
    let mut tick = 0;
    (0..2000)
        .map(|_| {
            tick += (next() % 4) as u64 / 2; // 0, 0, 1, 1: about two events a tick
            let key = POOL[next() % POOL.len()];
            let event = match next() % 16 {
                0 => FocusLost,
                1..=8 => Down(key),
                _ => Up(key),
            };
            (Tick(tick), event)
        })
        .collect()
}

/// FNV-1a 64 over every field of every tick's input.
fn fingerprint(inputs: &[TickInput]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for input in inputs {
        let c = &input.command;
        let mut bytes = c.tick.0.to_le_bytes().to_vec();
        bytes.extend(c.buttons.to_le_bytes());
        bytes.extend([c.forward as u8, c.side as u8]);
        bytes.extend(c.view_yaw.to_le_bytes());
        bytes.extend(c.view_pitch.to_le_bytes());
        bytes.push(c.select);
        bytes.extend(Action::ALL.map(|a| u8::from(input.actions.contains(a))));
        for b in bytes {
            hash = (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// The same timeline gives the same commands on every target: a known
/// answer, computed once and pinned. A change here changes what players'
/// keys do, so it is never a baseline to update casually.
#[test]
fn a_fixed_timeline_gives_known_commands() {
    let timeline = fixed_timeline();
    let ticks = timeline.last().unwrap().0.0 + 2;
    let inputs = quantize(&Bindings::default(), &timeline, ticks);
    let count = |f: &dyn Fn(&TickInput) -> bool| inputs.iter().filter(|i| f(i)).count();
    let counts = [
        count(&|i| i.command.forward == 127),
        count(&|i| i.command.forward == -127),
        count(&|i| i.command.side == 127),
        count(&|i| i.command.side == -127),
        count(&|i| i.quit()),
        count(&|i| i.actions.is_empty()),
    ];
    assert_eq!((ticks, counts), (1014, [202, 196, 239, 199, 388, 116]));
    assert_eq!(fingerprint(&inputs), KNOWN_FINGERPRINT);
}

const KNOWN_FINGERPRINT: u64 = 0x4d12_793a_10d0_b98f;

// --- Properties -------------------------------------------------------

fn any_key() -> impl Strategy<Value = Key> {
    let pool = vec![KeyW, KeyA, KeyS, KeyD, ArrowUp, ArrowDown, Escape, Space];
    prop_oneof![
        4 => prop::sample::select(pool),
        1 => prop::sample::select(Key::ALL),
    ]
}

fn any_event() -> impl Strategy<Value = KeyEvent> {
    prop_oneof![
        4 => any_key().prop_map(Down),
        4 => any_key().prop_map(Up),
        1 => Just(FocusLost),
    ]
}

/// Events with non-decreasing ticks from 0, and a tick count past the last.
fn any_timeline() -> impl Strategy<Value = (Vec<(Tick, KeyEvent)>, u64)> {
    (
        prop::collection::vec((0u64..3, any_event()), 0..120),
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

fn any_bindings() -> impl Strategy<Value = Bindings> {
    let action = prop::option::of(prop::sample::select(Action::ALL.to_vec()));
    prop::collection::vec((any_key(), action), 0..12).prop_map(|changes| {
        let mut b = Bindings::default();
        for (key, action) in changes {
            match action {
                Some(a) => b.bind(key, a),
                None => b.unbind(key),
            };
        }
        b
    })
}

/// The rules written out directly: a key counts for tick t if its last
/// `Down`, `Up` or `FocusLost` up to t was a `Down`, or if it went from up
/// to down during t. Events for tick 0 belong to tick 1.
fn model(b: &Bindings, timeline: &[(Tick, KeyEvent)], ticks: u64) -> Vec<(i8, i8, bool)> {
    let tick_of = |at: Tick| at.0.max(1);
    (1..=ticks)
        .map(|t| {
            let active = |key: Key| {
                let mut held = false;
                let mut went_down = false;
                for &(at, event) in timeline.iter().filter(|(at, _)| tick_of(*at) <= t) {
                    match event {
                        Down(k) if k == key => {
                            went_down |= !held && tick_of(at) == t;
                            held = true;
                        }
                        Up(k) if k == key => held = false,
                        FocusLost => held = false,
                        _ => {}
                    }
                }
                held || went_down
            };
            let on = |action| {
                Key::ALL
                    .iter()
                    .any(|&k| b.action(k) == Some(action) && active(k))
            };
            let axis = |plus, minus| 127 * (i8::from(on(plus)) - i8::from(on(minus)));
            (
                axis(Action::MoveForward, Action::MoveBack),
                axis(Action::MoveRight, Action::MoveLeft),
                on(Action::Quit),
            )
        })
        .collect()
}

proptest! {
    #[test]
    fn quantize_follows_the_rules(b in any_bindings(), (timeline, ticks) in any_timeline()) {
        let inputs = quantize(&b, &timeline, ticks);
        prop_assert_eq!(summary(&inputs), model(&b, &timeline, ticks));
        for (input, t) in inputs.iter().zip(1..) {
            let c = input.command;
            prop_assert_eq!(c.tick, Tick(t));
            prop_assert_eq!((c.buttons, c.select, c.view_yaw, c.view_pitch), (0, 0, 0, 0));
        }
    }

    #[test]
    fn the_same_timeline_gives_the_same_commands(
        b in any_bindings(),
        (timeline, ticks) in any_timeline(),
        split in 0u64..200,
    ) {
        let once = quantize(&b, &timeline, ticks);
        prop_assert_eq!(&once, &quantize(&b, &timeline, ticks));
        // Streaming gives the same, and so does a copy of the state taken
        // part-way and fed the rest of the timeline.
        let mut live = LiveInput::new(b.clone());
        let mut twin: Option<LiveInput> = None;
        let mut events = timeline.iter().peekable();
        let (mut streamed, mut from_twin) = (Vec::new(), Vec::new());
        for t in 1..=ticks {
            while let Some(&(_, event)) = events.next_if(|(at, _)| at.0 <= t) {
                live.handle(event);
                if let Some(twin) = twin.as_mut() {
                    twin.handle(event);
                }
            }
            if t == split {
                twin = Some(live.clone());
            }
            streamed.push(live.sample(Tick(t)));
            if let Some(twin) = twin.as_mut() {
                from_twin.push(twin.sample(Tick(t)));
            }
        }
        prop_assert_eq!(&streamed, &once);
        if (1..=ticks).contains(&split) {
            prop_assert_eq!(&from_twin[..], &once[split as usize - 1..]);
        }
    }

    #[test]
    fn quit_keys_never_change_commands(b in any_bindings(), (timeline, ticks) in any_timeline()) {
        let quits = |k: Key| b.action(k) == Some(Action::Quit);
        let without_quit: Vec<_> = timeline
            .iter()
            .copied()
            .filter(|(_, e)| !matches!(*e, Down(k) | Up(k) if quits(k)))
            .collect();
        let commands = |t: &[(Tick, KeyEvent)]| -> Vec<PlayerCommand> {
            quantize(&b, t, ticks).iter().map(|i| i.command).collect()
        };
        prop_assert_eq!(commands(&timeline), commands(&without_quit));
    }

    #[test]
    fn after_focus_loss_everything_is_released(
        b in any_bindings(),
        (mut timeline, ticks) in any_timeline(),
    ) {
        timeline.push((Tick(ticks + 1), FocusLost));
        let inputs = quantize(&b, &timeline, ticks + 2);
        let last = inputs.last().unwrap();
        prop_assert!(last.actions.is_empty());
        prop_assert_eq!(last.command, PlayerCommand::idle(Tick(ticks + 2)));
    }
}
