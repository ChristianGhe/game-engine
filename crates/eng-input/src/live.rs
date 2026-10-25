use eng_core::{PlayerCommand, Tick};

use crate::bindings::{Action, ActionSet, Bindings};
use crate::key::{Key, KeyEvent};

/// The stick value of a movement action: full speed.
pub const FULL_AXIS: i8 = 127;

/// One tick of live input: the command for the simulation, and the actions
/// that were active, including app actions the simulation never sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickInput {
    pub command: PlayerCommand,
    pub actions: ActionSet,
}

impl TickInput {
    /// Whether `quit` was active during the tick.
    pub const fn quit(&self) -> bool {
        self.actions.contains(Action::Quit)
    }
}

/// Live keyboard input, quantized into one [`PlayerCommand`] per tick
/// (spec 6.10).
///
/// The app passes every [`KeyEvent`] to [`handle`](Self::handle) as it
/// arrives, then calls [`sample`](Self::sample) once per tick with that
/// tick. There is no clock: which tick an event lands in is decided only by
/// when the app samples. The rules:
///
/// - A key counts for a tick if it is held when the tick is sampled, or if
///   it went down since the previous sample. So a tap shorter than a tick
///   counts for exactly one tick, and a key held across N samples for N.
///   A `Down` for a key already held (OS key repeat) changes nothing.
/// - [`KeyEvent::FocusLost`] releases every key, as if each got its `Up`.
/// - Opposite movement actions cancel. Movement is full speed (`127`).
/// - The view angles are held values set with [`set_view`](Self::set_view)
///   (0 until then); `buttons` and `select` are 0.
/// - `quit` is reported in [`TickInput::actions`] and never changes the
///   command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveInput {
    bindings: Bindings,
    held: [bool; Key::COUNT],
    /// Went from up to down since the last sample.
    pressed: [bool; Key::COUNT],
    view_yaw: u16,
    view_pitch: u16,
    /// The last tick sampled, to check that ticks increase (debug builds).
    last_sampled: Option<Tick>,
}

impl LiveInput {
    /// Nothing held, looking along +X.
    pub const fn new(bindings: Bindings) -> Self {
        Self {
            bindings,
            held: [false; Key::COUNT],
            pressed: [false; Key::COUNT],
            view_yaw: 0,
            view_pitch: 0,
            last_sampled: None,
        }
    }

    pub const fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// Rebinding takes effect at the next sample, keys held or not.
    pub const fn bindings_mut(&mut self) -> &mut Bindings {
        &mut self.bindings
    }

    /// The absolute view angles every following command carries (binary
    /// angles, 65536 per turn, as [`PlayerCommand`]). Movement is relative
    /// to the yaw: with yaw 0, `move_forward` goes along +X.
    pub const fn set_view(&mut self, yaw: u16, pitch: u16) {
        self.view_yaw = yaw;
        self.view_pitch = pitch;
    }

    /// Whether `key` is down now.
    pub const fn is_held(&self, key: Key) -> bool {
        self.held[key.index()]
    }

    /// Takes one event, in arrival order.
    pub const fn handle(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Down(key) => {
                // A repeat for a held key is not a new press: it must not
                // add a tick when the key is released before the sample.
                if !self.held[key.index()] {
                    self.pressed[key.index()] = true;
                }
                self.held[key.index()] = true;
            }
            KeyEvent::Up(key) => self.held[key.index()] = false,
            // A key that went down since the last sample still counts once,
            // as with any release.
            KeyEvent::FocusLost => self.held = [false; Key::COUNT],
        }
    }

    /// The input for `tick`, from the events handled since the previous
    /// sample. Call it once per tick, in increasing tick order: sampling a
    /// tick twice would lose its taps. Debug builds panic if a tick does
    /// not increase.
    pub fn sample(&mut self, tick: Tick) -> TickInput {
        debug_assert!(
            self.last_sampled.is_none_or(|last| tick > last),
            "[E-INPUT] LiveInput sampled {tick} after {}: ticks must increase",
            self.last_sampled.unwrap_or_default(),
        );
        self.last_sampled = Some(tick);
        let mut actions = ActionSet::EMPTY;
        for &key in Key::ALL {
            let i = key.index();
            if (self.held[i] || self.pressed[i])
                && let Some(action) = self.bindings.action(key)
            {
                actions = actions.with(action);
            }
        }
        self.pressed = [false; Key::COUNT];
        TickInput {
            command: command_for(actions, tick, self.view_yaw, self.view_pitch),
            actions,
        }
    }
}

/// The command a set of active actions gives. App actions do not take part.
pub const fn command_for(
    actions: ActionSet,
    tick: Tick,
    view_yaw: u16,
    view_pitch: u16,
) -> PlayerCommand {
    PlayerCommand {
        forward: axis(actions, Action::MoveForward, Action::MoveBack),
        side: axis(actions, Action::MoveRight, Action::MoveLeft),
        view_yaw,
        view_pitch,
        ..PlayerCommand::idle(tick)
    }
}

/// Full speed one way or the other; opposite actions cancel.
const fn axis(actions: ActionSet, plus: Action, minus: Action) -> i8 {
    match (actions.contains(plus), actions.contains(minus)) {
        (true, false) => FULL_AXIS,
        (false, true) => -FULL_AXIS,
        _ => 0,
    }
}

/// Quantizes a key timeline into the input for ticks `1..=ticks`, as
/// [`LiveInput`] would from its defaults (view angles 0).
///
/// Each event carries the tick whose sample first sees it: an event for
/// tick N arrived after tick N-1 was sampled and before tick N was. Events
/// for tick 0 land in tick 1, and events after `ticks` are dropped. The same
/// timeline always gives the same input: there is no clock and no float.
///
/// # Panics
/// If the ticks in `timeline` decrease.
pub fn quantize(bindings: &Bindings, timeline: &[(Tick, KeyEvent)], ticks: u64) -> Vec<TickInput> {
    assert!(
        timeline.windows(2).all(|w| w[0].0 <= w[1].0),
        "a key timeline must be in tick order"
    );
    let mut live = LiveInput::new(bindings.clone());
    let mut events = timeline.iter().peekable();
    (1..=ticks)
        .map(|t| {
            let tick = Tick(t);
            while let Some(&(_, event)) = events.next_if(|(at, _)| *at <= tick) {
                live.handle(event);
            }
            live.sample(tick)
        })
        .collect()
}
