//! What a first-person frame's camera takes from `play` rather than from
//! the game's scene (rung 2, T15; plan decision 10, T9's near plane):
//!
//! - **The live angles** between ticks, from live input
//!   (`LiveInput::view_now`), when the commands are live; a scripted run
//!   shows the script's, which the scene already has from the state.
//! - **`--fov`**: the vertical field of view.
//! - **The near plane.** eng-render's 0.1 m keeps the near rectangle clear
//!   of a wall the hull touches only while `tan²(fov_y/2)·(1 + aspect²)`
//!   is at most 5.25 (90° up to an aspect of 2.06; `camera.rs`). A wider
//!   window or field of view, such as fullscreen on a 21:9 monitor, would
//!   cut walls open, so the near plane comes in instead, to keep the
//!   rectangle's farthest corner [`NEAR_REACH`] from the eye. The field of
//!   view is never clamped: the player gets the one asked for, and reversed
//!   Z loses nothing worth having at a nearer plane.
//! - **`--look`**: the switches the file gives, over the scene's look.

use eng_cli::LookSwitches;
use eng_math::{BAngle, min_f64, sin_cos_f64, sqrt_f64};
use eng_render::{FirstPersonScene, NEAR_PLANE};

/// The farthest the near rectangle's corners may be from the eye, in
/// meters: inside the hull's 0.25 m half width, with a centimetre to
/// spare.
pub(crate) const NEAR_REACH: f64 = 0.24;

/// The near plane for a vertical field of view at an aspect (width over
/// height): [`NEAR_PLANE`], or nearer where that one's rectangle would
/// reach past [`NEAR_REACH`]: `min(0.1, 0.24 / sqrt(1 + tan²(fov_y/2)·(1 +
/// aspect²)))`.
pub(crate) fn near_for(fov_y: BAngle, aspect: f64) -> f32 {
    let (sin, cos) = sin_cos_f64(fov_y.radians_f64() * 0.5);
    let tan = sin / cos;
    let corner = sqrt_f64(1.0 + tan * tan * (1.0 + aspect * aspect));
    let near = min_f64(f64::from(NEAR_PLANE), NEAR_REACH / corner);
    // At most 0.1, and above 0 for any field of view below a half turn.
    near as f32
}

/// A binary angle to the nearest `PlayerCommand` angle (65,536 a turn),
/// halves up, wrapping.
pub(crate) const fn to_command_angle(angle: BAngle) -> u16 {
    BAngle(angle.0.wrapping_add(0x8000)).to_u16()
}

/// What `play` sets on each frame's scene.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lens {
    /// `--fov`; `None` keeps the scene's.
    pub fov_y: Option<BAngle>,
    /// `--look`.
    pub look: LookSwitches,
}

impl Lens {
    /// Sets `scene`'s camera and look for an image `aspect` times as wide
    /// as high: the live `angles` (yaw, pitch) if given, the field of view,
    /// a near plane that holds at it, and the look switches.
    pub(crate) fn apply(
        &self,
        scene: &mut FirstPersonScene,
        angles: Option<(u16, u16)>,
        aspect: f64,
    ) {
        let camera = &mut scene.camera;
        if let Some((yaw, pitch)) = angles {
            camera.yaw = BAngle::from_u16(yaw);
            camera.pitch = BAngle::from_u16(pitch);
        }
        if let Some(fov_y) = self.fov_y {
            camera.fov_y = fov_y;
        }
        let near = near_for(camera.fov_y, aspect);
        if near < camera.near {
            camera.near = near;
        }
        scene.look = self.look.apply(scene.look);
    }
}

#[cfg(test)]
mod tests {
    use eng_cli::LookSwitches;
    use eng_math::{BAngle, Ev100, WorldPos};
    use eng_render::{FirstPersonCamera, FirstPersonScene, LinearRgb, Look, NEAR_PLANE, Sky};

    use super::{Lens, NEAR_REACH, near_for, to_command_angle};

    /// The near rectangle's farthest corner from the eye.
    fn reach(near: f32, fov_y: BAngle, aspect: f64) -> f64 {
        let half = fov_y.radians_f64() * 0.5;
        let tan = eng_math::sin_f64(half) / eng_math::cos_f64(half);
        f64::from(near) * eng_math::sqrt_f64(1.0 + tan * tan * (1.0 + aspect * aspect))
    }

    #[test]
    fn the_near_plane_holds_at_0_1_m_while_its_rectangle_fits() {
        // The room's field of view, 2 atan(3/4): safe at 4:3, 16:9, and
        // up to an aspect of about 2.73 at 0.24 m (T12's 2.9 is at 0.25 m).
        let room = BAngle(879_750_026);
        for aspect in [4.0 / 3.0, 16.0 / 9.0, 2.5, 2.7] {
            assert_eq!(near_for(room, aspect), NEAR_PLANE, "{aspect}");
        }
        // 90° up to 1.9 (0.24 m, a centimetre inside T9's 2.06 at 0.25 m).
        assert_eq!(near_for(BAngle::QUARTER, 16.0 / 9.0), NEAR_PLANE);
        assert_eq!(near_for(BAngle::QUARTER, 1.9), NEAR_PLANE);
    }

    #[test]
    fn past_it_the_near_plane_comes_in_and_the_corner_stays_inside() {
        let room = BAngle(879_750_026);
        // 21:9 at 90°, 32:9 at the room's, and 170° at 16:9.
        let wide = BAngle((170u64 * (1 << 32) / 360) as u32);
        for (fov_y, aspect) in [
            (BAngle::QUARTER, 21.0 / 9.0),
            (room, 32.0 / 9.0),
            (wide, 16.0 / 9.0),
        ] {
            let near = near_for(fov_y, aspect);
            assert!(near < NEAR_PLANE && near > 0.0, "{near}");
            let corner = reach(near, fov_y, aspect);
            assert!(
                (corner - NEAR_REACH).abs() < 1e-6,
                "{fov_y:?} {aspect}: {corner}"
            );
        }
    }

    #[test]
    fn command_angles_round_to_nearest() {
        assert_eq!(to_command_angle(BAngle::ZERO), 0);
        assert_eq!(to_command_angle(BAngle::from_u16(0x4000)), 0x4000);
        assert_eq!(to_command_angle(BAngle(0x4000_7fff)), 0x4000);
        assert_eq!(to_command_angle(BAngle(0x4000_8000)), 0x4001);
        assert_eq!(to_command_angle(BAngle(u32::MAX)), 0);
    }

    fn scene() -> FirstPersonScene {
        FirstPersonScene {
            camera: FirstPersonCamera {
                eye: WorldPos::new(1.0, 2.0, 1.6),
                yaw: BAngle::from_u16(100),
                pitch: BAngle::from_u16(200),
                fov_y: BAngle(879_750_026),
                near: NEAR_PLANE,
            },
            sky: Sky {
                horizon: LinearRgb {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                },
                zenith: LinearRgb {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                },
            },
            ev100: Ev100(5.0),
            look: Look::ON,
            regions: Vec::new(),
            sprites: Vec::new(),
        }
    }

    #[test]
    fn the_lens_sets_the_live_angles_the_field_of_view_and_the_look() {
        let mut kept = scene();
        let lens = Lens {
            fov_y: None,
            look: LookSwitches::default(),
        };
        lens.apply(&mut kept, None, 4.0 / 3.0);
        assert_eq!(kept, scene(), "nothing to set");

        let mut set = scene();
        let lens = Lens {
            fov_y: Some(BAngle::QUARTER),
            look: LookSwitches {
                fog: Some(false),
                ..LookSwitches::default()
            },
        };
        lens.apply(&mut set, Some((0x4000, 0xff00)), 21.0 / 9.0);
        assert_eq!(set.camera.yaw, BAngle::from_u16(0x4000));
        assert_eq!(set.camera.pitch, BAngle::from_u16(0xff00));
        assert_eq!(set.camera.fov_y, BAngle::QUARTER);
        assert!(set.camera.near < NEAR_PLANE);
        assert_eq!(set.camera.eye, scene().camera.eye);
        assert_eq!(
            set.look,
            Look {
                fog: false,
                ..Look::ON
            }
        );
    }
}
