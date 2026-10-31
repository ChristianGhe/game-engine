//! The only math the simulation may use (spec 6.4).
//!
//! - Only IEEE basic operations plus the functions here, which wrap the
//!   pure-Rust `libm` pinned to `=0.2.16`, whose `arch` code stays off
//!   (`force-soft-floats`, even where another crate turns the feature on),
//!   so every target runs the same code.
//! - [`min`], [`max`] and [`clamp`] replace std's, which may return either
//!   signed zero.
//! - Never `mul_add`, `algebraic_*` or approximate reciprocals (clippy bans).
//! - Units: meters, right-handed, Z-up. Absolute positions are f64
//!   ([`WorldPos`]); local math is [`Real`] = f32. Angles are [`BAngle`]s.
//!
//! [`Fnv1a64`] (the state hash) and [`Pcg32`] (the gameplay RNG) are frozen:
//! the known-answer tests fail if either changes.
#![no_std]
#![forbid(unsafe_code)]

mod angle;
mod hash;
mod rng;
mod scalar;
mod units;
mod vec;

pub use angle::BAngle;
pub use hash::Fnv1a64;
pub use rng::Pcg32;
pub use scalar::*;
pub use units::{Ev100, Lux, Meters, Nits};
pub use vec::{Vec2, Vec3, WorldPos};

/// The scalar type for local simulation math.
pub type Real = f32;
