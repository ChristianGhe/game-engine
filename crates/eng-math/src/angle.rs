use eng_schema_core::Schema;

use crate::{Real, TAU_F64, Vec2, sin_cos_f64};

/// A binary angle: a full turn is 2^32, and arithmetic wraps. 0 points along
/// +X and angles grow counter-clockwise seen from +Z.
///
/// `PlayerCommand` carries 16-bit angles; [`BAngle::from_u16`] widens them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Schema)]
#[schema(name = "eng.math.BAngle", version = 1)]
pub struct BAngle(pub u32);

impl BAngle {
    pub const ZERO: Self = Self(0);
    pub const QUARTER: Self = Self(1 << 30);
    pub const HALF: Self = Self(1 << 31);

    /// Widens a 16-bit command angle (`<< 16`).
    pub const fn from_u16(a: u16) -> Self {
        Self((a as u32) << 16)
    }

    /// The top 16 bits, as carried by `PlayerCommand`.
    pub const fn to_u16(self) -> u16 {
        (self.0 >> 16) as u16
    }

    pub const fn wrapping_add(self, o: Self) -> Self {
        Self(self.0.wrapping_add(o.0))
    }

    pub const fn wrapping_sub(self, o: Self) -> Self {
        Self(self.0.wrapping_sub(o.0))
    }

    /// The signed difference `self - o`, in `[-HALF, HALF)` as an `i32`.
    pub const fn delta(self, o: Self) -> i32 {
        self.0.wrapping_sub(o.0) as i32
    }

    /// The angle in radians, in `[0, TAU)`, computed in f64.
    pub fn radians_f64(self) -> f64 {
        f64::from(self.0) * (TAU_F64 / 4_294_967_296.0)
    }

    /// `(sin, cos)` of the angle, computed in f64 and rounded to f32.
    pub fn sin_cos(self) -> (Real, Real) {
        let (s, c) = sin_cos_f64(self.radians_f64());
        (s as Real, c as Real)
    }

    /// The unit vector `(cos, sin)` this angle points along.
    pub fn direction(self) -> Vec2 {
        let (s, c) = self.sin_cos();
        Vec2::new(c, s)
    }
}
