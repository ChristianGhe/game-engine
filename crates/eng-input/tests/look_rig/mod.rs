//! What the mouse look tests share (`mouse_look.rs`, `mouse_look_exact.rs`,
//! `mouse_look_model.rs`): the defaults written out, settings, negative
//! angles, and a rig that feeds motion and samples tick after tick.
#![allow(dead_code)]

use eng_core::{PlayerCommand, Tick};
use eng_input::{Bindings, LiveInput, LookSettings};

/// The defaults, written out: 0.066° per count, not inverted, and 14563
/// units, the last binary angle not past 80° (80 × 65536 / 360 = 14563.56).
pub const DEFAULTS: LookSettings = LookSettings {
    sensitivity_microdeg: 66_000,
    invert_pitch: false,
    pitch_limit: 14_563,
};

pub const fn with(sensitivity_microdeg: u32, invert_pitch: bool, pitch_limit: u16) -> LookSettings {
    LookSettings {
        sensitivity_microdeg,
        invert_pitch,
        pitch_limit,
    }
}

/// `-units` as a binary angle.
pub const fn neg(units: u16) -> u16 {
    0u16.wrapping_sub(units)
}

/// Live input and the next tick to sample.
pub struct Rig {
    pub live: LiveInput,
    pub tick: u64,
}

impl Rig {
    /// Look on with `settings`, from `(yaw, pitch)`.
    pub fn new(settings: LookSettings, yaw: u16, pitch: u16) -> Self {
        let mut live = LiveInput::new(Bindings::default());
        live.set_view(yaw, pitch);
        live.set_look(Some(settings));
        Self { live, tick: 1 }
    }

    /// Handles `motion`, then samples the next tick: its command.
    pub fn command(&mut self, motion: &[(f64, f64)]) -> PlayerCommand {
        for &(dx, dy) in motion {
            self.live.handle_motion(dx, dy);
        }
        let command = self.live.sample(Tick(self.tick)).command;
        self.tick += 1;
        command
    }

    /// The same, giving the command's `(yaw, pitch)`.
    pub fn tick(&mut self, motion: &[(f64, f64)]) -> (u16, u16) {
        let c = self.command(motion);
        (c.view_yaw, c.view_pitch)
    }

    /// One tick per entry; the angles after the last.
    pub fn ticks(&mut self, per_tick: &[&[(f64, f64)]]) -> (u16, u16) {
        per_tick.iter().map(|m| self.tick(m)).last().unwrap()
    }
}
