//! The first-person passes' per-frame uniform (ADR-0020), and the sky it
//! carries.

use eng_math::Vec3;

use crate::{LinearRgb, RenderError, ViewProj};

/// The sky, drawn by the `sky` pass wherever nothing else was: linear and
/// scene-referred, as `lit` holds it. It runs from `horizon`, at eye level
/// and below, to `zenith` straight up, linearly in the sine of the view
/// ray's elevation. With both the same it is flat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    pub horizon: LinearRgb,
    pub zenith: LinearRgb,
}

impl Sky {
    /// An `[E-SCENE]` error unless every channel is finite.
    pub(crate) fn check(&self) -> Result<(), RenderError> {
        let channels = [self.horizon, self.zenith];
        if channels
            .iter()
            .all(|c| c.r.is_finite() && c.g.is_finite() && c.b.is_finite())
        {
            Ok(())
        } else {
            Err(RenderError::Scene {
                shape: None,
                problem: "the sky's colours must be finite".into(),
            })
        }
    }
}

/// The bytes of the uniform's nine 16-byte rows.
pub(crate) const UNIFORM_BYTES: usize = 144;

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
///
/// The eye is the origin: chunks come as offsets from it. Rung 2's light,
/// fog and tone map (T10) add rows after these for exposure and the look
/// switches.
pub(crate) fn uniform(view: &ViewProj, sky: &Sky) -> Vec<u8> {
    let mut floats: Vec<f32> = view.matrix().cols.iter().flatten().copied().collect();
    let row = |v: Vec3, w: f32| [v.x, v.y, v.z, w];
    let colour = |c: LinearRgb| [c.r, c.g, c.b, 0.0];
    let [right, up] = view.ray_steps();
    floats.extend(row(view.forward(), view.near()));
    floats.extend(row(right, 0.0));
    floats.extend(row(up, 0.0));
    floats.extend(colour(sky.horizon));
    floats.extend(colour(sky.zenith));
    let bytes: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    debug_assert_eq!(bytes.len(), UNIFORM_BYTES);
    bytes
}

#[cfg(test)]
mod tests {
    use eng_math::{BAngle, WorldPos};

    use super::{Sky, UNIFORM_BYTES, uniform};
    use crate::{FirstPersonCamera, LinearRgb, NEAR_PLANE};

    #[test]
    fn the_uniform_holds_its_rows_in_order() {
        let view = FirstPersonCamera {
            eye: WorldPos::new(5.0, 6.0, 7.0),
            yaw: BAngle(0x1234_5678),
            pitch: BAngle(0x0400_0000),
            fov_y: BAngle::QUARTER,
            near: NEAR_PLANE,
        }
        .view_proj(1.5)
        .expect("a valid camera");
        let sky = Sky {
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
        };
        let bytes = uniform(&view, &sky);
        assert_eq!(bytes.len(), UNIFORM_BYTES);
        let floats: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| f32::from_le_bytes(*w))
            .collect();
        let matrix: Vec<f32> = view.matrix().cols.iter().flatten().copied().collect();
        assert_eq!(floats[..16], matrix[..]);
        let row = |r: usize| {
            [
                floats[r * 4],
                floats[r * 4 + 1],
                floats[r * 4 + 2],
                floats[r * 4 + 3],
            ]
        };
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
    }
}
