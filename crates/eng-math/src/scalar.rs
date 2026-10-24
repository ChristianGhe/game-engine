//! Scalar functions. Each wraps the pinned `libm`, or is defined here so its
//! result never depends on the platform.

pub const PI: f32 = core::f32::consts::PI;
pub const TAU: f32 = core::f32::consts::TAU;
pub const PI_F64: f64 = core::f64::consts::PI;
pub const TAU_F64: f64 = core::f64::consts::TAU;

/// The smaller of `a` and `b`; `b` when they compare equal (so `min(0.0, -0.0)`
/// is `-0.0`) or when either is NaN.
#[inline]
pub fn min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// The larger of `a` and `b`; `b` when they compare equal or either is NaN.
#[inline]
pub fn max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `x` limited to `[lo, hi]`. Returns `x` unchanged when it is NaN.
#[inline]
pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

#[inline]
pub fn min_f64(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

#[inline]
pub fn max_f64(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

#[inline]
pub fn clamp_f64(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

macro_rules! wrap1 {
    ($($name:ident => $libm:ident : $ty:ty),* $(,)?) => {$(
        #[inline]
        pub fn $name(x: $ty) -> $ty {
            libm::$libm(x)
        }
    )*};
}

macro_rules! wrap2 {
    ($($name:ident => $libm:ident : $ty:ty),* $(,)?) => {$(
        #[inline]
        pub fn $name(a: $ty, b: $ty) -> $ty {
            libm::$libm(a, b)
        }
    )*};
}

wrap1! {
    sqrt => sqrtf: f32,
    cbrt => cbrtf: f32,
    sin => sinf: f32,
    cos => cosf: f32,
    tan => tanf: f32,
    asin => asinf: f32,
    acos => acosf: f32,
    atan => atanf: f32,
    exp => expf: f32,
    exp2 => exp2f: f32,
    ln => logf: f32,
    log2 => log2f: f32,
    log10 => log10f: f32,
    floor => floorf: f32,
    ceil => ceilf: f32,
    round => roundf: f32,
    trunc => truncf: f32,
    sqrt_f64 => sqrt: f64,
    sin_f64 => sin: f64,
    cos_f64 => cos: f64,
    floor_f64 => floor: f64,
    ceil_f64 => ceil: f64,
    round_f64 => round: f64,
    trunc_f64 => trunc: f64,
}

wrap2! {
    atan2 => atan2f: f32,
    pow => powf: f32,
    hypot => hypotf: f32,
    atan2_f64 => atan2: f64,
}

#[inline]
pub fn sin_cos(x: f32) -> (f32, f32) {
    libm::sincosf(x)
}

#[inline]
pub fn sin_cos_f64(x: f64) -> (f64, f64) {
    libm::sincos(x)
}
