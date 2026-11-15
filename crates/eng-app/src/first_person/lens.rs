//! What a first-person frame's camera takes from `play` rather than from
//! the game's scene (rung 2, T15; plan decision 10, T9's near plane):
//!
//! - **The live angles** between ticks, from live input
//!   (`LiveInput::view_now`), when the commands are live; a scripted run
//!   shows the script's, which the scene already has from the state.
//! - **`--fov`**: the vertical field of view.
//! - **The near plane**, brought in for the window's aspect at that field
//!   of view where eng-render's 0.1 m would cut walls open
//!   ([`eng_render::near_for`], which `shot` and `video` apply too). The
//!   field of view is never clamped: the player gets the one asked for.
//! - **`--look`**: the switches the file gives, over the scene's look.

use eng_cli::LookSwitches;
use eng_math::BAngle;
use eng_render::FirstPersonScene;

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
        camera.fit_near(aspect);
        scene.look = self.look.apply(scene.look);
    }
}

#[cfg(test)]
mod tests {
    use eng_cli::LookSwitches;
    use eng_math::{BAngle, Ev100, WorldPos};
    use eng_render::{
        FirstPersonCamera, FirstPersonScene, LinearRgb, Look, NEAR_PLANE, Sky, near_for,
    };

    use super::{Lens, to_command_angle};

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
        assert_eq!(set.camera.near, near_for(BAngle::QUARTER, 21.0 / 9.0));
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
