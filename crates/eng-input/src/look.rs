//! Mouse look (rung 2 plan, decision 9; spec 6.10 and 6.14): raw mouse
//! counts into the view angles that [`PlayerCommand`](eng_core::PlayerCommand)s
//! carry, the same on every target.
//!
//! # Signs
//!
//! Positive yaw turns towards +Y (counter-clockwise seen from above), so
//! moving the mouse right (`dx > 0`) lowers yaw. `dy > 0` is towards the
//! user, down the screen (`eng_platform_api::PlatformEvent::MouseMotion`),
//! and pitch is positive up, so `dy > 0` lowers pitch, or raises it when
//! [`LookSettings::invert_pitch`] is set.
//!
//! # The arithmetic, and why it is exact
//!
//! - Each delta is rounded to the nearest 1/256 count, ties away from zero.
//!   Multiplying an `f64` by 256 is exact and `f64::round` is an exact IEEE
//!   operation, so every target gets the same integer. A delta that is not
//!   finite is dropped.
//! - The sub-counts (1/256 of a count) are summed in `i64`, exactly: the angle comes from the
//!   sum, never from a sum of rounded turns, so any split of the same
//!   events into ticks gives the same yaw.
//! - Sensitivity is an integer, millionths of a degree per count, so the
//!   setting a player writes (0.066) is exactly the value used. The turn
//!   for a sum of `n` sub-counts (1/256 of a count) at `s` millionths of a degree per count is
//!   `n/256 × s/10^6 × 65536/360 = n·s / 1_406_250` binary-angle units,
//!   rounded half away from zero by one integer division in `i128`. That is
//!   `round(N·s·65536/360)` for `N = n/256` counts and `s` in degrees.
//!   `|n·s| < 2^63 · 2^32 = 2^95`, so nothing overflows and nothing is a
//!   float. The one bound: a sum saturates at `±2^63` sub-counts (1/256 of a count) (3.6·10^16
//!   counts, years of a mouse moving flat out one way).
//! - Yaw wraps: it is the base yaw minus the turn, modulo 65536.
//!
//! # The pitch limit
//!
//! Pitch holds at `±pitch_limit`. When a sample would pass the limit, the
//! pitch is the limit and the vertical sum re-bases there, dropping the
//! motion past it (as Quake does). So within the limit any split of the
//! motion gives the same pitch; at the limit it can differ: up past the
//! limit in one tick and back down in the next ends lower than the same
//! motion in one tick, where the up and the down cancel before the limit
//! is checked.

/// A full turn in binary-angle units, as `PlayerCommand` carries angles.
const TURN: i128 = 65_536;

/// `256 × 10^6 × 360 / 65536`: the turn for `n` 1/256 counts at `s`
/// millionths of a degree per count is `n·s / DIVISOR` units.
const DIVISOR: i128 = 1_406_250;

/// How the mouse turns the view (spec 6.14: sensitivity and invert are
/// player settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LookSettings {
    /// Degrees per count, in millionths of a degree: `66_000` is 0.066°.
    /// The same for yaw and pitch.
    pub sensitivity_microdeg: u32,
    /// Moving the mouse towards the user (`dy > 0`) looks up instead of
    /// down.
    pub invert_pitch: bool,
    /// The largest pitch either way, in binary-angle units (65536 a turn).
    /// Values past [`MAX_PITCH_LIMIT`](Self::MAX_PITCH_LIMIT) count as it.
    pub pitch_limit: u16,
}

impl LookSettings {
    /// 0.066° per count: Quake's 0.022 × 3.
    pub const DEFAULT_SENSITIVITY: u32 = 66_000;
    /// The largest binary angle not past 80°, which is 14563.56 units.
    pub const DEFAULT_PITCH_LIMIT: u16 = 14_563;
    /// Straight up or down: a quarter turn.
    pub const MAX_PITCH_LIMIT: u16 = 16_384;
    /// 0.066° per count, not inverted, pitch within ±80°.
    pub const DEFAULT: Self = Self {
        sensitivity_microdeg: Self::DEFAULT_SENSITIVITY,
        invert_pitch: false,
        pitch_limit: Self::DEFAULT_PITCH_LIMIT,
    };
}

impl Default for LookSettings {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Motion in 1/256 counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Motion {
    x: i64,
    y: i64,
}

impl Motion {
    const ZERO: Self = Self { x: 0, y: 0 };
}

/// Mouse look's state: the angles it counts from and the motion since.
///
/// Feed it each motion event with [`motion`](Self::motion), and call
/// [`sample`](Self::sample) once per tick for that tick's angles. The rules
/// are the module's: an exact sum, the pitch limit, and
/// [`focus_lost`](Self::focus_lost) dropping motion not yet sampled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseLook {
    settings: LookSettings,
    base_yaw: u16,
    /// Signed binary-angle units; may start past the limit, which the first
    /// sample clamps.
    base_pitch: i32,
    /// Motion sampled since the base.
    sampled: Motion,
    /// Motion since the last sample.
    pending: Motion,
}

impl MouseLook {
    /// Look from `yaw` and `pitch` (binary angles, as `PlayerCommand`).
    pub const fn new(settings: LookSettings, yaw: u16, pitch: u16) -> Self {
        Self {
            settings,
            base_yaw: yaw,
            base_pitch: pitch.cast_signed() as i32,
            sampled: Motion::ZERO,
            pending: Motion::ZERO,
        }
    }

    /// The settings it looks with, as given.
    pub const fn settings(&self) -> LookSettings {
        self.settings
    }

    /// Takes one motion event: raw device counts, `dx` right and `dy`
    /// towards the user.
    pub fn motion(&mut self, dx: f64, dy: f64) {
        self.pending.x = self.pending.x.saturating_add(in_sub_counts(dx));
        self.pending.y = self.pending.y.saturating_add(in_sub_counts(dy));
    }

    /// The window lost focus: motion since the last sample is dropped.
    /// Motion already sampled stays, so the view does not jump back.
    pub const fn focus_lost(&mut self) {
        self.pending = Motion::ZERO;
    }

    /// Look from these angles from now on: the next sample gives exactly
    /// them (pitch clamped to the limit) plus the motion that follows this
    /// call. Motion before it is dropped.
    pub const fn set_view(&mut self, yaw: u16, pitch: u16) {
        *self = Self::new(self.settings, yaw, pitch);
    }

    /// The angles `(yaw, pitch)` for this tick, from all the motion since
    /// the base. Call it once per tick.
    pub fn sample(&mut self) -> (u16, u16) {
        self.sampled = self.with_pending();
        self.pending = Motion::ZERO;
        let (yaw, pitch) = self.angles(self.sampled);
        let limit = i128::from(self.limit());
        let pitch = if pitch > limit || pitch < -limit {
            // Re-base at the limit: the motion past it is gone.
            self.base_pitch = if pitch > 0 {
                self.limit()
            } else {
                -self.limit()
            };
            self.sampled.y = 0;
            i128::from(self.base_pitch)
        } else {
            pitch
        };
        (yaw, pitch.rem_euclid(TURN) as u16)
    }

    /// The angles `(yaw, pitch)` a [`sample`](Self::sample) would give
    /// now, without taking anything: the view between two ticks, for
    /// drawing (rung 2, T15). It changes nothing, so the samples are the
    /// same whether or not it is called, and the next sample gives exactly
    /// this when no motion comes in between. Pitch holds at the limit, as
    /// a sample's does, but nothing re-bases here.
    pub fn angles_now(&self) -> (u16, u16) {
        let (yaw, pitch) = self.angles(self.with_pending());
        let limit = i128::from(self.limit());
        (yaw, pitch.clamp(-limit, limit).rem_euclid(TURN) as u16)
    }

    /// The motion since the base with the pending motion added.
    const fn with_pending(&self) -> Motion {
        Motion {
            x: self.sampled.x.saturating_add(self.pending.x),
            y: self.sampled.y.saturating_add(self.pending.y),
        }
    }

    /// The yaw for `motion` since the base, and the pitch before the limit,
    /// in signed binary-angle units.
    fn angles(&self, motion: Motion) -> (u16, i128) {
        let s = self.settings.sensitivity_microdeg;
        // Right lowers yaw.
        let yaw = (i128::from(self.base_yaw) - turn(motion.x, s)).rem_euclid(TURN);
        // Towards the user lowers pitch, unless inverted.
        let down = turn(motion.y, s);
        let base = i128::from(self.base_pitch);
        let pitch = if self.settings.invert_pitch {
            base + down
        } else {
            base - down
        };
        (yaw as u16, pitch)
    }

    const fn limit(&self) -> i32 {
        let limit = self.settings.pitch_limit;
        if limit > LookSettings::MAX_PITCH_LIMIT {
            LookSettings::MAX_PITCH_LIMIT as i32
        } else {
            limit as i32
        }
    }
}

/// A delta in 1/256 counts, rounded to nearest, ties away from zero; 0 if
/// it is not finite. Exact: `× 256` only changes the exponent, `round` is
/// exact, and `as` saturates.
fn in_sub_counts(delta: f64) -> i64 {
    if delta.is_finite() {
        (delta * 256.0).round() as i64
    } else {
        0
    }
}

/// The turn in binary-angle units for `n` 1/256 counts at `s` millionths
/// of a degree per count: `round(n·s / DIVISOR)`, ties away from zero.
fn turn(n: i64, s: u32) -> i128 {
    let product = i128::from(n) * i128::from(s);
    let magnitude = (2 * product.abs() + DIVISOR) / (2 * DIVISOR);
    if product < 0 { -magnitude } else { magnitude }
}
