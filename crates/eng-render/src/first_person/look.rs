//! The first-person look (rung 2, T10; spec 6.7; ADR-0020): the switches
//! that turn each look feature off alone, each region's light and fog, and
//! the light model's numbers.
//!
//! **The light model** (plan, "The first-person graph"): each region has a
//! Doom light level, 0 to 255. A pixel's *light step* starts at its
//! region's level over 8, 0 to 31.875, and then:
//! - **fake contrast** (Doom's wall-direction term) adds
//!   [`CONTRAST_STEPS`] on a wall facing ±X ([`crate::SurfaceClass::WallFacingX`],
//!   one that runs along Y) and takes as many from a wall facing ±Y;
//! - **falloff** takes [`LIGHT_FALLOFF`] steps per meter of the pixel's
//!   distance from the eye (radial, along the view ray);
//! - the step is clamped to 0 to 31, and **banding** rounds it down to a
//!   whole step, so a region shows at most 32 levels of one material.
//!
//! The step's illuminance in lux is [`LIGHT_STEP_LUX`] at a whole step,
//! interpolated linearly between two steps without banding. A surface's
//! luminance in nits is then Lambertian: albedo times lux over π. The sky
//! and fog colours are luminances in nits too. The tone map exposes it all
//! by the scene's EV100 ([`exposure`]).

use eng_math::{Ev100, pow};

use crate::LinearRgb;

/// The region looks a frame carries: regions 0 to 15. A pixel of a region
/// at or past [`crate::FirstPersonScene::regions`]' length takes
/// [`RegionLook::DEFAULT`]; one of region 16 or more takes region 15's.
pub const MAX_REGIONS: usize = 16;

/// Light steps a region's light comes in (Doom's 32 light levels).
pub const LIGHT_STEPS: usize = 32;

/// The illuminance of each light step, in lux: 320 lux at step 31, half as
/// much every 4 steps below it (a quarter stop a step), so step 0 is
/// 320 / 2^7.75, about 1.5 lux.
pub const LIGHT_STEP_LUX: [f32; LIGHT_STEPS] = light_step_lux();

/// `2^(-q/4)` for `q` from 0 to 3.
const QUARTER_STOPS: [f32; 4] = [1.0, 0.840_896_4, 0.707_106_77, 0.594_603_54];

const fn light_step_lux() -> [f32; LIGHT_STEPS] {
    let mut lux = [0.0; LIGHT_STEPS];
    let mut step = 0;
    while step < LIGHT_STEPS {
        let below = LIGHT_STEPS - 1 - step;
        let halvings = (below / 4) as i32;
        lux[step] = 320.0 * QUARTER_STOPS[below % 4] / (1u32 << halvings) as f32;
        step += 1;
    }
    lux
}

/// Light steps lost per meter from the eye, with falloff on: 6 across the
/// room's 12 m hall, a stop and a half.
pub const LIGHT_FALLOFF: f32 = 0.5;

/// Light steps that fake contrast adds to a wall facing ±X and takes from
/// one facing ±Y: Doom's one level of 16, two of 32.
pub const CONTRAST_STEPS: f32 = 2.0;

/// The palette's dither, in 8-bit codes: the 4x4 Bayer offset
/// ([`crate::bayer4_offset`]) times this, one table cell, is added to each
/// channel's code before the lookup.
pub const DITHER_CODES: f32 = 8.0;

/// The low resolution [`Look::ON`] asks for: a window shows each texel as
/// 4x4 pixels (1920x1080 draws 480x270).
pub const LOW_RESOLUTION: u32 = 4;

/// The exposure that scales luminance in nits before the tone map, for an
/// exposure value at ISO 100: `1 / (1.2 * 2^ev100)`, the luminance that
/// saturates a sensor by the saturation-based sensitivity rule
/// (Lagarde and de Rousiers, "Moving Frostbite to PBR", 2014), so a
/// surface of that luminance maps to 1.
pub fn exposure(ev100: Ev100) -> f32 {
    1.0 / (1.2 * pow(2.0, ev100.0))
}

/// Which look features are on (`--look`, task T15). Each switches off
/// alone; with every one off the first-person view draws what
/// [`crate::FirstPersonRenderer::render_plain`] draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    /// Light falls off with distance from the eye ([`LIGHT_FALLOFF`]).
    pub falloff: bool,
    /// Light comes in 32 whole steps; off, steps are interpolated.
    pub banding: bool,
    /// Fake contrast: walls facing ±X lighter, ±Y darker
    /// ([`CONTRAST_STEPS`]).
    pub contrast: bool,
    /// The sky runs from its horizon to its zenith colour; off, it is the
    /// horizon colour everywhere.
    pub sky_gradient: bool,
    /// The fog pass; off, it is bypassed and the tone map reads `lit`.
    pub fog: bool,
    /// The palette pass; off, it is bypassed and the tone map draws into
    /// the output.
    pub palette: bool,
    /// The palette's ordered dither ([`DITHER_CODES`]); with the palette
    /// only.
    pub dither: bool,
    /// Low internal resolution: a window shows each texel as this many
    /// pixels across and down, 1 for off. It is the window targets'
    /// business ([`eng_gpu::Targets::upscaled`]); a shot is the output at
    /// its own size whatever this says.
    pub low_resolution: u32,
}

impl Look {
    /// Every feature on: the default.
    pub const ON: Self = Self {
        falloff: true,
        banding: true,
        contrast: true,
        sky_gradient: true,
        fog: true,
        palette: true,
        dither: true,
        low_resolution: LOW_RESOLUTION,
    };

    /// Every feature off: the plain pipeline's image.
    pub const OFF: Self = Self {
        falloff: false,
        banding: false,
        contrast: false,
        sky_gradient: false,
        fog: false,
        palette: false,
        dither: false,
        low_resolution: 1,
    };
}

impl Default for Look {
    fn default() -> Self {
        Self::ON
    }
}

/// How one region looks: its light and its fog.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionLook {
    /// Doom's light level, 0 (darkest) to 255 (brightest); the light step
    /// is it over 8.
    pub light: u8,
    /// The fog's colour, linear, as a luminance in nits.
    pub fog: LinearRgb,
    /// How thick the fog is, per meter: a surface `d` meters away shows
    /// `1 - e^(-fog_density * d)` of the fog's colour. 0 for none; at least
    /// 0 and finite.
    pub fog_density: f32,
}

impl RegionLook {
    /// A region the scene does not list: full light, no fog.
    pub const DEFAULT: Self = Self {
        light: 255,
        fog: LinearRgb {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        },
        fog_density: 0.0,
    };
}

impl Default for RegionLook {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use eng_math::{Ev100, pow};

    use super::{LIGHT_STEP_LUX, exposure};

    #[test]
    fn the_light_steps_are_quarter_stops_down_from_320_lux() {
        assert_eq!(LIGHT_STEP_LUX[31], 320.0);
        assert_eq!(LIGHT_STEP_LUX[27], 160.0);
        assert_eq!(LIGHT_STEP_LUX[3], 2.5);
        for step in 0..31 {
            let ratio = LIGHT_STEP_LUX[step + 1] / LIGHT_STEP_LUX[step];
            assert!(
                (ratio - pow(2.0, 0.25)).abs() < 1e-6,
                "step {step}: {ratio}"
            );
        }
    }

    #[test]
    fn ev100_0_saturates_at_1_2_nits() {
        assert_eq!(exposure(Ev100(0.0)), 1.0 / 1.2);
        assert_eq!(exposure(Ev100(3.0)), 1.0 / 9.6);
    }
}
