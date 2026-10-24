use core::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

use eng_schema_core::Schema;

use crate::{Real, sqrt, sqrt_f64};

/// A 2D vector in meters (X forward at yaw 0, Y left).
#[derive(Debug, Clone, Copy, PartialEq, Default, Schema)]
#[schema(name = "eng.math.Vec2", version = 1)]
pub struct Vec2 {
    pub x: Real,
    pub y: Real,
}

/// A 3D vector: right-handed, Z up, meters.
#[derive(Debug, Clone, Copy, PartialEq, Default, Schema)]
#[schema(name = "eng.math.Vec3", version = 1)]
pub struct Vec3 {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

/// An absolute world position in f64 meters. Renderers subtract the camera
/// position in f64 before converting to f32 ([`WorldPos::relative_to`]).
#[derive(Debug, Clone, Copy, PartialEq, Default, Schema)]
#[schema(name = "eng.math.WorldPos", version = 1)]
pub struct WorldPos {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: Real, y: Real) -> Self {
        Self { x, y }
    }

    pub fn dot(self, o: Self) -> Real {
        self.x * o.x + self.y * o.y
    }

    pub fn length(self) -> Real {
        sqrt(self.dot(self))
    }
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    pub const fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, o: Self) -> Real {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length_squared(self) -> Real {
        self.dot(self)
    }

    pub fn length(self) -> Real {
        sqrt(self.length_squared())
    }

    /// The unit vector in this direction, or zero for a zero-length vector.
    pub fn normalize_or_zero(self) -> Self {
        let len = self.length();
        if len > 0.0 {
            self * (1.0 / len)
        } else {
            Self::ZERO
        }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl WorldPos {
    pub const ORIGIN: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// `self + d * t`, computed in f64.
    pub fn add_scaled(self, d: Vec3, t: f64) -> Self {
        Self::new(
            self.x + f64::from(d.x) * t,
            self.y + f64::from(d.y) * t,
            self.z + f64::from(d.z) * t,
        )
    }

    /// `self - origin`, subtracted in f64 and then rounded to f32.
    pub fn relative_to(self, origin: Self) -> Vec3 {
        Vec3::new(
            (self.x - origin.x) as f32,
            (self.y - origin.y) as f32,
            (self.z - origin.z) as f32,
        )
    }

    pub fn distance(self, o: Self) -> f64 {
        let (dx, dy, dz) = (self.x - o.x, self.y - o.y, self.z - o.z);
        sqrt_f64(dx * dx + dy * dy + dz * dz)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

macro_rules! vec_ops {
    ($ty:ident { $($f:ident),* }) => {
        impl Add for $ty {
            type Output = Self;
            fn add(self, o: Self) -> Self { Self { $($f: self.$f + o.$f),* } }
        }
        impl Sub for $ty {
            type Output = Self;
            fn sub(self, o: Self) -> Self { Self { $($f: self.$f - o.$f),* } }
        }
        impl Mul<Real> for $ty {
            type Output = Self;
            fn mul(self, s: Real) -> Self { Self { $($f: self.$f * s),* } }
        }
        impl Neg for $ty {
            type Output = Self;
            fn neg(self) -> Self { Self { $($f: -self.$f),* } }
        }
        impl AddAssign for $ty {
            fn add_assign(&mut self, o: Self) { *self = *self + o; }
        }
        impl SubAssign for $ty {
            fn sub_assign(&mut self, o: Self) { *self = *self - o; }
        }
    };
}

vec_ops!(Vec2 { x, y });
vec_ops!(Vec3 { x, y, z });

impl Add<Vec3> for WorldPos {
    type Output = Self;
    fn add(self, d: Vec3) -> Self {
        self.add_scaled(d, 1.0)
    }
}
