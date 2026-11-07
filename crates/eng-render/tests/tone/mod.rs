//! A CPU reference for the first-person tail (rung 2, T10), in f64 and
//! written from the definitions, not from eng-render's shaders: EV100
//! exposure, Khronos PBR Neutral, and the sRGB curve both ways. The tests
//! compare the GPU's 8-bit output with it, to one level.
// Each test file uses some of these.
#![allow(dead_code)]

use eng_math::{clamp_f64, max_f64, min_f64};

/// The luminance in nits that exposure at `ev100` maps to 1: a sensor's
/// saturation-based sensitivity rule, `1.2 * 2^ev100` (ISO 100, K 12.5,
/// q 0.65: 78 / (100 * 0.65) = 1.2). The power of two goes through
/// eng-math's `exp2` for the fraction and exact doubling for the rest.
pub fn saturating_luminance(ev100: f64) -> f64 {
    let whole = eng_math::floor_f64(ev100);
    let mut value = f64::from(eng_math::exp2((ev100 - whole) as f32));
    let mut n = whole as i32;
    while n > 0 {
        value *= 2.0;
        n -= 1;
    }
    while n < 0 {
        value /= 2.0;
        n += 1;
    }
    1.2 * value
}

/// Khronos PBR Neutral (github.com/KhronosGroup/ToneMapping, PBR_Neutral),
/// from its definition: `F90 = 0.04`, `Ks = 0.8 - F90`, `Kd = 0.15`; `x`
/// the least channel; `f = x - x^2 / (4 F90)` if `x <= 2 F90`, else `F90`;
/// `p` the greatest channel of `c - f`; if `p <= Ks` the result is
/// `c - f`; else `pn = 1 - (1 - Ks)^2 / (p + 1 - 2 Ks)`,
/// `g = 1 / (Kd (p - pn) + 1)`, and the result is
/// `(c - f) (pn / p) g + pn (1 - g)` in every channel.
pub fn pbr_neutral(c: [f64; 3]) -> [f64; 3] {
    const F90: f64 = 0.04;
    const KS: f64 = 0.8 - F90;
    const KD: f64 = 0.15;
    let x = min_f64(c[0], min_f64(c[1], c[2]));
    let f = if x <= 2.0 * F90 {
        x - x * x / (4.0 * F90)
    } else {
        F90
    };
    let shifted = c.map(|v| v - f);
    let p = max_f64(shifted[0], max_f64(shifted[1], shifted[2]));
    if p <= KS {
        return shifted;
    }
    let pn = 1.0 - (1.0 - KS) * (1.0 - KS) / (p + 1.0 - 2.0 * KS);
    let g = 1.0 / (KD * (p - pn) + 1.0);
    shifted.map(|v| v * (pn / p) * g + pn * (1.0 - g))
}

/// An sRGB code's linear value (IEC 61966-2-1).
pub fn decode(code: u8) -> f64 {
    let v = f64::from(code) / 255.0;
    if v <= 0.040_45 {
        v / 12.92
    } else {
        f64::from(eng_math::pow(((v + 0.055) / 1.055) as f32, 2.4))
    }
}

/// A linear value in [0, 1] as an 8-bit sRGB code, unrounded.
pub fn encode(linear: f64) -> f64 {
    let c = clamp_f64(linear, 0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * f64::from(eng_math::pow(c as f32, 1.0 / 2.4)) - 0.055
    };
    encoded * 255.0
}

/// The codes the tail gives a luminance of `nits`, at `ev100`, unrounded.
pub fn toned_codes(nits: [f64; 3], ev100: f64) -> [f64; 3] {
    let exposure = 1.0 / saturating_luminance(ev100);
    pbr_neutral(nits.map(|n| n * exposure)).map(encode)
}

/// The luminance of a Lambertian surface of albedo `srgb` (8-bit sRGB
/// codes) under `lux`.
pub fn lambert(srgb: [u8; 3], lux: f64) -> [f64; 3] {
    srgb.map(|code| decode(code) * lux / std::f64::consts::PI)
}

/// Whether `got` is within one level of `want` in each channel, opaque.
pub fn within_one(got: [u8; 4], want: [f64; 3]) -> bool {
    got[3] == 255 && (0..3).all(|c| (f64::from(got[c]) - want[c]).abs() <= 1.0)
}

#[cfg(test)]
mod tests {
    use super::pbr_neutral;

    /// The curve's own landmarks: below the compression it only offsets,
    /// it is continuous where compression starts, it keeps grey grey, and
    /// it never reaches 1.
    #[test]
    fn the_reference_curve_has_its_landmarks() {
        assert_eq!(pbr_neutral([0.0; 3]), [0.0; 3]);
        let low = pbr_neutral([0.5, 0.3, 0.2]);
        for (got, want) in low.iter().zip([0.46, 0.26, 0.16]) {
            assert!((got - want).abs() < 1e-12, "{low:?}");
        }
        let at = pbr_neutral([0.8; 3])[0];
        assert!((at - 0.76).abs() < 1e-12, "{at}");
        let above = pbr_neutral([0.800_001; 3])[0];
        assert!((above - 0.76).abs() < 1e-5, "{above}");
        let bright = pbr_neutral([1000.0, 2.0, 1.0]);
        assert!(bright.iter().all(|&v| v < 1.0 && v > 0.9), "{bright:?}");
    }
}
