//! The first-person passes' per-frame uniform (ADR-0020), the sky it
//! carries, and the checks a scene passes before any GPU work.

use eng_math::Vec3;

use super::look::{
    CONTRAST_STEPS, DITHER_CODES, LIGHT_FALLOFF, LIGHT_STEP_LUX, Look, MAX_REGIONS, RegionLook,
    exposure,
};
use crate::{FirstPersonScene, LinearRgb, RenderError, ViewProj};

/// The sky, drawn by the `sky` pass wherever nothing else was: linear and
/// scene-referred, a luminance in nits, as `lit` holds it. It runs from
/// `horizon`, at eye level and below, to `zenith` straight up, linearly in
/// the sine of the view ray's elevation. With both the same, or the look's
/// sky gradient off, it is flat: `horizon` everywhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    pub horizon: LinearRgb,
    pub zenith: LinearRgb,
}

fn finite(c: LinearRgb) -> bool {
    c.r.is_finite() && c.g.is_finite() && c.b.is_finite()
}

fn negative(c: LinearRgb) -> bool {
    c.r < 0.0 || c.g < 0.0 || c.b < 0.0
}

/// An `[E-SCENE]` error saying `problem`.
fn scene_error(problem: String) -> RenderError {
    RenderError::Scene {
        shape: None,
        problem,
    }
}

/// Whether `scene` can be drawn: an `[E-SCENE]` error unless the sky's and
/// the fogs' colours are finite and not negative (the tone map takes
/// non-negative light), each fog's density is finite and not negative, it
/// lists at most [`MAX_REGIONS`] regions, and its EV100 gives a finite
/// exposure above 0.
pub(crate) fn check(scene: &FirstPersonScene) -> Result<(), RenderError> {
    let sky = [scene.sky.horizon, scene.sky.zenith];
    if !sky.iter().all(|&c| finite(c)) {
        return Err(scene_error("the sky's colours must be finite".into()));
    }
    if sky.iter().any(|&c| negative(c)) {
        return Err(scene_error("the sky's colours must not be negative".into()));
    }
    if scene.regions.len() > MAX_REGIONS {
        return Err(scene_error(format!(
            "{} region looks; a frame carries at most {MAX_REGIONS}",
            scene.regions.len()
        )));
    }
    for (index, region) in scene.regions.iter().enumerate() {
        let density = region.fog_density;
        if !finite(region.fog) || negative(region.fog) || !density.is_finite() || density < 0.0 {
            return Err(scene_error(format!(
                "region {index}'s fog colour and density must be finite and not negative"
            )));
        }
    }
    let scale = exposure(scene.ev100);
    if !(scale.is_finite() && scale > 0.0) {
        return Err(scene_error(format!(
            "EV100 {} gives no exposure a frame can use",
            scene.ev100.0
        )));
    }
    Ok(())
}

/// The bytes of the uniform's 39 16-byte rows.
pub(crate) const UNIFORM_BYTES: usize = 39 * 16;

/// Bits of the uniform's `switches.x`: light comes in whole steps.
pub(crate) const BANDING: u32 = 1;
/// The sky runs from horizon to zenith.
pub(crate) const SKY_GRADIENT: u32 = 2;

/// The per-frame uniform, `struct Frame` in every first-person shader,
/// which each declares alike (`tests/first_person_layout.rs` checks it):
///
/// | Rows | Field | Holds |
/// |---|---|---|
/// | 0-3 | `clip_from_eye: mat4x4<f32>` | [`ViewProj::matrix`], column by column |
/// | 4 | `forward: vec4<f32>` | the forward axis; `w` the near plane |
/// | 5 | `ray_right: vec4<f32>` | what one unit of clip-space x adds to a ray ([`ViewProj::ray`]); `w` 0 |
/// | 6 | `ray_up: vec4<f32>` | the same for y; `w` 0 |
/// | 7 | `sky_horizon: vec4<f32>` | [`Sky::horizon`]; `a` 0 |
/// | 8 | `sky_zenith: vec4<f32>` | [`Sky::zenith`]; `a` 0 |
/// | 9 | `look: vec4<f32>` | the exposure ([`exposure`]); light steps lost per meter ([`LIGHT_FALLOFF`], 0 with falloff off); the dither in codes ([`DITHER_CODES`], 0 off); the contrast in steps ([`CONTRAST_STEPS`], 0 off) |
/// | 10 | `switches: vec4<u32>` | bits: 1 banding, 2 the sky's gradient; the palette table's first layer in the 64 px sRGB array; 0; 0 |
/// | 11-18 | `light_steps: array<vec4<f32>, 8>` | [`LIGHT_STEP_LUX`], step `s` at `[s / 4][s % 4]` |
/// | 19-22 | `region_light: array<vec4<f32>, 4>` | region `r`'s light level over 8 at `[r / 4][r % 4]` |
/// | 23-38 | `region_fog: array<vec4<f32>, 16>` | region `r`'s fog: its colour, then its density per meter |
///
/// The eye is the origin: chunks come as offsets from it. Regions the
/// scene does not list get [`RegionLook::DEFAULT`].
pub(crate) fn uniform(view: &ViewProj, scene: &FirstPersonScene, palette_layer: u32) -> Vec<u8> {
    let mut words: Vec<u32> = view
        .matrix()
        .cols
        .iter()
        .flatten()
        .map(|f| f.to_bits())
        .collect();
    let mut floats = |values: [f32; 4]| words.extend(values.map(f32::to_bits));
    let row = |v: Vec3, w: f32| [v.x, v.y, v.z, w];
    let colour = |c: LinearRgb, a: f32| [c.r, c.g, c.b, a];
    let [right, up] = view.ray_steps();
    let look: &Look = &scene.look;
    let on = |switch: bool, value: f32| if switch { value } else { 0.0 };
    floats(row(view.forward(), view.near()));
    floats(row(right, 0.0));
    floats(row(up, 0.0));
    floats(colour(scene.sky.horizon, 0.0));
    floats(colour(scene.sky.zenith, 0.0));
    floats([
        exposure(scene.ev100),
        on(look.falloff, LIGHT_FALLOFF),
        on(look.palette && look.dither, DITHER_CODES),
        on(look.contrast, CONTRAST_STEPS),
    ]);
    let bits =
        if look.banding { BANDING } else { 0 } | if look.sky_gradient { SKY_GRADIENT } else { 0 };
    words.extend([bits, palette_layer, 0, 0]);
    let mut floats = |values: [f32; 4]| words.extend(values.map(f32::to_bits));
    for steps in LIGHT_STEP_LUX.as_chunks::<4>().0 {
        floats(*steps);
    }
    let regions: Vec<RegionLook> = (0..MAX_REGIONS)
        .map(|r| scene.regions.get(r).copied().unwrap_or(RegionLook::DEFAULT))
        .collect();
    for four in regions.as_chunks::<4>().0 {
        floats(four.map(|r| f32::from(r.light) / 8.0));
    }
    for region in &regions {
        floats(colour(region.fog, region.fog_density));
    }
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    debug_assert_eq!(bytes.len(), UNIFORM_BYTES);
    bytes
}

#[cfg(test)]
mod tests {
    use eng_math::{BAngle, Ev100, WorldPos};

    use super::{BANDING, SKY_GRADIENT, Sky, UNIFORM_BYTES, uniform};
    use crate::{
        DITHER_CODES, FirstPersonCamera, FirstPersonScene, LIGHT_FALLOFF, LIGHT_STEP_LUX,
        LinearRgb, Look, NEAR_PLANE, RegionLook, exposure,
    };

    fn scene(look: Look) -> FirstPersonScene {
        FirstPersonScene {
            camera: FirstPersonCamera {
                eye: WorldPos::new(5.0, 6.0, 7.0),
                yaw: BAngle(0x1234_5678),
                pitch: BAngle(0x0400_0000),
                fov_y: BAngle::QUARTER,
                near: NEAR_PLANE,
            },
            sky: Sky {
                horizon: LinearRgb {
                    r: 0.5,
                    g: 0.25,
                    b: 0.125,
                },
                zenith: LinearRgb {
                    r: 1.0,
                    g: 2.0,
                    b: 3.0,
                },
            },
            ev100: Ev100(7.0),
            look,
            regions: vec![
                RegionLook {
                    light: 200,
                    ..RegionLook::DEFAULT
                },
                RegionLook {
                    light: 9,
                    fog: LinearRgb {
                        r: 0.1,
                        g: 0.6,
                        b: 0.2,
                    },
                    fog_density: 0.25,
                },
            ],
        }
    }

    #[test]
    fn the_uniform_holds_its_rows_in_order() {
        let scene = scene(Look::ON);
        let view = scene.camera.view_proj(1.5).expect("a valid camera");
        let bytes = uniform(&view, &scene, 37);
        assert_eq!(bytes.len(), UNIFORM_BYTES);
        let words: Vec<u32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w))
            .collect();
        let floats: Vec<f32> = words.iter().map(|&w| f32::from_bits(w)).collect();
        let matrix: Vec<f32> = view.matrix().cols.iter().flatten().copied().collect();
        assert_eq!(floats[..16], matrix[..]);
        let row = |r: usize| [0, 1, 2, 3].map(|c| floats[r * 4 + c]);
        let f = view.forward();
        assert_eq!(row(4), [f.x, f.y, f.z, NEAR_PLANE]);
        // Forward plus the two steps is the ray through the top right corner.
        let corner = view.ray(1.0, 1.0);
        let (right, up) = (row(5), row(6));
        for (axis, want) in [corner.x, corner.y, corner.z].into_iter().enumerate() {
            assert_eq!(row(4)[axis] + right[axis] + up[axis], want);
        }
        assert_eq!((right[3], up[3]), (0.0, 0.0));
        assert_eq!(row(7), [0.5, 0.25, 0.125, 0.0]);
        assert_eq!(row(8), [1.0, 2.0, 3.0, 0.0]);
        assert_eq!(
            row(9),
            [exposure(Ev100(7.0)), LIGHT_FALLOFF, DITHER_CODES, 2.0]
        );
        assert_eq!(words[40..44], [BANDING | SKY_GRADIENT, 37, 0, 0]);
        assert_eq!(floats[44..76], LIGHT_STEP_LUX[..]);
        // Region 0 at 200 / 8, region 1 at 9 / 8, the rest at 255 / 8.
        assert_eq!(row(19), [25.0, 1.125, 31.875, 31.875]);
        assert_eq!(row(22), [31.875; 4]);
        assert_eq!(row(23), [0.0; 4]);
        assert_eq!(row(24), [0.1, 0.6, 0.2, 0.25]);
        assert_eq!(row(38), [0.0; 4]);

        let mut plain = scene.clone();
        plain.look = Look::OFF;
        let off = uniform(&view, &plain, 0);
        let off: Vec<u32> = off
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w))
            .collect();
        assert_eq!(
            off[36..44],
            [exposure(Ev100(7.0)).to_bits(), 0, 0, 0, 0, 0, 0, 0]
        );
    }
}
