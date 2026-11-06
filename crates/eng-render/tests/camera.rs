//! The first-person camera and its projection (rung 2 plan, queue item Q4,
//! done-when 1). No GPU is needed.
//!
//! Expected values are worked out by hand from AGENTS.md's conventions:
//! right-handed, Z up; yaw 0 looks along +X and positive yaw turns toward
//! +Y; positive pitch looks up. With a 90° vertical field of view the
//! focal length is 1, so a point `d` meters ahead lands at
//! `(right / d / aspect, up / d)` in normalized device coordinates, with
//! depth `near / d`.

use eng_math::{BAngle, Vec3, WorldPos, sin_cos_f64};
use eng_render::{FirstPersonCamera, RenderError, ViewProj};
use proptest::prelude::*;

const NEAR: f32 = 0.05;
/// An eighth of a turn, 45°.
const EIGHTH: BAngle = BAngle(1 << 29);

fn camera(eye: WorldPos, yaw: BAngle, pitch: BAngle) -> FirstPersonCamera {
    FirstPersonCamera {
        eye,
        yaw,
        pitch,
        fov_y: BAngle::QUARTER,
        near: NEAR,
    }
}

fn ready(yaw: BAngle, pitch: BAngle) -> ViewProj {
    camera(WorldPos::ORIGIN, yaw, pitch)
        .view_proj(1.0)
        .expect("a valid camera")
}

/// The normalized device coordinates of `p`, which must be in front.
fn ndc(view: &ViewProj, p: WorldPos) -> Vec3 {
    view.project(p).ndc().expect("the point is in front")
}

fn close(got: Vec3, want: [f32; 3], tolerance: f32) {
    let diffs = [got.x - want[0], got.y - want[1], got.z - want[2]];
    assert!(
        diffs.iter().all(|d| d.abs() <= tolerance),
        "got {got:?}, want {want:?}"
    );
}

#[test]
fn the_near_plane_maps_to_depth_one() {
    let view = ready(BAngle::ZERO, BAngle::ZERO);
    // Anywhere on the plane 0.05 m ahead of the eye, not only its centre.
    for (y, z) in [(0.0, 0.0), (0.03, -0.02), (-0.05, 0.05)] {
        let clip = view.project(WorldPos::new(f64::from(NEAR), y, z));
        assert_eq!(clip.z / clip.w, 1.0, "at ({y}, {z})");
    }
}

#[test]
fn the_near_plane_maps_to_depth_one_at_any_heading() {
    for (yaw, pitch) in [(0x1234_5678, 0x0800_0000), (0xC000_0001, 0xF000_0000)] {
        let (yaw, pitch) = (BAngle(yaw), BAngle(pitch));
        // Forward, from the angles by hand: the heading raised by the pitch.
        let (sy, cy) = sin_cos_f64(yaw.radians_f64());
        let (sp, cp) = sin_cos_f64(pitch.radians_f64());
        let near = f64::from(NEAR);
        let ahead = WorldPos::new(cp * cy * near, cp * sy * near, sp * near);
        let depth = ndc(&ready(yaw, pitch), ahead).z;
        assert!((depth - 1.0).abs() < 1e-6, "depth {depth}");
    }
}

#[test]
fn a_million_meters_ahead_is_nearly_depth_zero() {
    let view = ready(BAngle::ZERO, BAngle::ZERO);
    let depth = ndc(&view, WorldPos::new(1e6, 0.0, 0.0)).z;
    // near / d = 0.05 / 10^6 = 5e-8: under 1e-6 and still above 0.
    assert!(depth > 0.0 && depth < 1e-6, "depth {depth}");
    // Reversed-Z: depth falls as distance grows, and never reaches 0.
    let mut last = f32::INFINITY;
    for d in [0.05, 1.0, 10.0, 1e3, 1e6, 1e9, 1e30] {
        let depth = ndc(&view, WorldPos::new(d, 0.0, 0.0)).z;
        assert!(depth > 0.0 && depth < last, "at {d} m, depth {depth}");
        last = depth;
    }
}

#[test]
fn points_behind_or_beside_the_eye_have_no_positive_w() {
    let view = ready(BAngle::ZERO, BAngle::ZERO);
    for p in [
        (-0.01, 0.0, 0.0),
        (-1.0, 0.0, 0.0),
        (-1e6, 5.0, 3.0),
        (-3.0, -400.0, 900.0),
        (0.0, 3.0, 4.0),
        (0.0, 0.0, 0.0),
    ] {
        let clip = view.project(WorldPos::new(p.0, p.1, p.2));
        assert!(clip.w <= 0.0, "{p:?} gives w = {}", clip.w);
        assert_eq!(clip.ndc(), None);
    }
}

proptest! {
    /// Over any heading, w is the distance ahead of the eye, so its sign
    /// says whether a point is in front, away from the eye's plane.
    #[test]
    fn w_is_positive_exactly_in_front(
        yaw in any::<u32>(),
        pitch in -(1i32 << 30)..(1i32 << 30),
        offset in (-50.0f64..50.0, -50.0f64..50.0, -50.0f64..50.0),
    ) {
        let (yaw, pitch) = (BAngle(yaw), BAngle(pitch as u32));
        let (sy, cy) = sin_cos_f64(yaw.radians_f64());
        let (sp, cp) = sin_cos_f64(pitch.radians_f64());
        let ahead = offset.0 * cp * cy + offset.1 * cp * sy + offset.2 * sp;
        prop_assume!(ahead.abs() > 1e-3);
        let eye = WorldPos::new(12.5, -3.25, 1.6);
        let view = camera(eye, yaw, pitch).view_proj(1.5).unwrap();
        let point = WorldPos::new(eye.x + offset.0, eye.y + offset.1, eye.z + offset.2);
        let w = view.project(point).w;
        prop_assert_eq!(w > 0.0, ahead > 0.0, "w {} ahead {}", w, ahead);
    }
}

#[test]
fn a_far_camera_projects_like_one_at_the_origin() {
    // Offsets with few binary digits, so eye + offset - eye is exact in f64.
    let offsets = [
        (3.5, -1.25, 0.5),
        (10.0, 2.0, -1.5),
        (0.25, 0.125, 0.0625),
        (-7.0, 40.0, 3.0),
    ];
    // Each eye is 10^9 m from the origin.
    for far_eye in [
        WorldPos::new(1e9, 0.0, 0.0),
        WorldPos::new(6e8, -8e8, 0.0),
        WorldPos::new(0.0, 0.0, -1e9),
    ] {
        for (yaw, pitch) in [
            (0, 0),
            (0x2345_6789, 0x0600_0000),
            (0x9000_0000, 0xFA00_0000),
        ] {
            let (yaw, pitch) = (BAngle(yaw), BAngle(pitch));
            let far = camera(far_eye, yaw, pitch).view_proj(16.0 / 9.0).unwrap();
            let home = camera(WorldPos::ORIGIN, yaw, pitch)
                .view_proj(16.0 / 9.0)
                .unwrap();
            for (x, y, z) in offsets {
                let point = WorldPos::new(far_eye.x + x, far_eye.y + y, far_eye.z + z);
                let (a, b) = (far.project(point), home.project(WorldPos::new(x, y, z)));
                let bits = |c: eng_render::Clip| [c.x, c.y, c.z, c.w].map(f32::to_bits);
                assert_eq!(bits(a), bits(b), "eye {far_eye:?}, offset {:?}", (x, y, z));
            }
        }
    }
    // The f64 subtraction is what keeps it: rounded to f32 first, 3.5 m
    // from a point 10^9 m out is lost (f32 steps by 64 m there).
    let lost = (1e9 + 3.5) as f32 - 1e9f64 as f32;
    assert_eq!(lost, 0.0);
}

#[test]
fn yaw_zero_looks_along_x_with_y_on_the_left_and_z_up() {
    let view = ready(BAngle::ZERO, BAngle::ZERO);
    // Straight ahead: the image's centre, depth near / 10.
    close(
        ndc(&view, WorldPos::new(10.0, 0.0, 0.0)),
        [0.0, 0.0, 0.005],
        1e-7,
    );
    // +Y is on the left: 2 m left at 10 m is x = -0.2.
    close(
        ndc(&view, WorldPos::new(10.0, 2.0, 0.0)),
        [-0.2, 0.0, 0.005],
        1e-6,
    );
    // -Y on the right, +Z up the image.
    close(
        ndc(&view, WorldPos::new(10.0, -2.0, 3.0)),
        [0.2, 0.3, 0.005],
        1e-6,
    );
    // 10 m ahead and 10 m up is the top edge of a 90° view.
    close(
        ndc(&view, WorldPos::new(10.0, 0.0, 10.0)),
        [0.0, 1.0, 0.005],
        1e-6,
    );
}

#[test]
fn positive_yaw_turns_toward_y() {
    // A quarter turn: looking along +Y, +X is on the right.
    let quarter = ready(BAngle::QUARTER, BAngle::ZERO);
    close(
        ndc(&quarter, WorldPos::new(0.0, 10.0, 0.0)),
        [0.0, 0.0, 0.005],
        1e-6,
    );
    close(
        ndc(&quarter, WorldPos::new(2.0, 10.0, 0.0)),
        [0.2, 0.0, 0.005],
        1e-6,
    );
    // An eighth: the diagonal (1, 1) is ahead, 10√2 m away.
    let eighth = ready(EIGHTH, BAngle::ZERO);
    let depth = 0.05 / (10.0 * core::f32::consts::SQRT_2);
    close(
        ndc(&eighth, WorldPos::new(10.0, 10.0, 0.0)),
        [0.0, 0.0, depth],
        1e-6,
    );
    // A half turn looks along -X, with +Y now on the right.
    let half = ready(BAngle::HALF, BAngle::ZERO);
    close(
        ndc(&half, WorldPos::new(-10.0, 2.0, 0.0)),
        [0.2, 0.0, 0.005],
        1e-6,
    );
    // Three quarters (yaw -90°) looks along -Y.
    let back = ready(BAngle(3 << 30), BAngle::ZERO);
    close(
        ndc(&back, WorldPos::new(0.0, -10.0, 0.0)),
        [0.0, 0.0, 0.005],
        1e-6,
    );
    assert!(back.project(WorldPos::new(0.0, 10.0, 0.0)).w < 0.0);
}

#[test]
fn positive_pitch_looks_up() {
    // Pitched up 45°: the point 10 m ahead and 10 m up is at the centre.
    let up = ready(BAngle::ZERO, EIGHTH);
    let depth = 0.05 / (10.0 * core::f32::consts::SQRT_2);
    close(
        ndc(&up, WorldPos::new(10.0, 0.0, 10.0)),
        [0.0, 0.0, depth],
        1e-6,
    );
    // A level point is 45° below the view axis: the bottom edge, 10/√2 m
    // along it.
    let level = ndc(&up, WorldPos::new(10.0, 0.0, 0.0));
    close(level, [0.0, -1.0, 2.0 * depth], 1e-6);
    // Pitched down 45°, the point 10 m below is at the centre.
    let down = ready(BAngle::ZERO, BAngle(EIGHTH.0.wrapping_neg()));
    close(
        ndc(&down, WorldPos::new(10.0, 0.0, -10.0)),
        [0.0, 0.0, depth],
        1e-6,
    );
    // Straight up: +Z ahead, and the heading's left (+Y) still on the left.
    let zenith = ready(BAngle::ZERO, BAngle::QUARTER);
    close(
        ndc(&zenith, WorldPos::new(0.0, 0.0, 10.0)),
        [0.0, 0.0, 0.005],
        1e-6,
    );
    close(
        ndc(&zenith, WorldPos::new(0.0, 2.0, 10.0)),
        [-0.2, 0.0, 0.005],
        1e-6,
    );
}

#[test]
fn the_basis_matches_the_conventions() {
    let basis = camera(WorldPos::ORIGIN, BAngle::QUARTER, BAngle::ZERO).basis();
    let round = |v: Vec3| [v.x, v.y, v.z].map(|c| eng_math::round(c * 1e6) / 1e6);
    assert_eq!(round(basis.forward), [0.0, 1.0, 0.0]);
    assert_eq!(round(basis.right), [1.0, 0.0, 0.0]);
    assert_eq!(round(basis.up), [0.0, 0.0, 1.0]);
}

#[test]
fn aspect_and_field_of_view_scale_the_image() {
    let wide = camera(WorldPos::ORIGIN, BAngle::ZERO, BAngle::ZERO)
        .view_proj(2.0)
        .unwrap();
    close(
        ndc(&wide, WorldPos::new(10.0, -2.0, 3.0)),
        [0.1, 0.3, 0.005],
        1e-6,
    );
    // 60° vertically: the focal length is cot 30° = √3.
    let narrow = FirstPersonCamera {
        fov_y: BAngle(((1u64 << 32) / 6) as u32),
        ..camera(WorldPos::ORIGIN, BAngle::ZERO, BAngle::ZERO)
    };
    let narrow = narrow.view_proj(1.0).unwrap();
    let sqrt3 = 1.732_050_8;
    close(
        ndc(&narrow, WorldPos::new(10.0, 0.0, 1.0)),
        [0.0, 0.1 * sqrt3, 0.005],
        1e-5,
    );
}

#[test]
fn the_matrix_is_stored_by_column() {
    let matrix = ready(BAngle::ZERO, BAngle::ZERO).matrix();
    // Rows: right (0, -1, 0), up (0, 0, 1), (0, 0, 0, near), forward (1, 0, 0).
    assert_eq!(matrix.row(2), [0.0, 0.0, 0.0, NEAR]);
    assert_eq!(matrix.row(3), [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(matrix.cols[3], [0.0, 0.0, NEAR, 0.0]);
    assert_eq!(matrix.cols[0], [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn cameras_that_cannot_make_an_image_are_scene_errors() {
    let good = camera(WorldPos::ORIGIN, BAngle::ZERO, BAngle::ZERO);
    let bad = [
        FirstPersonCamera {
            eye: WorldPos::new(f64::NAN, 0.0, 0.0),
            ..good
        },
        FirstPersonCamera {
            fov_y: BAngle::ZERO,
            ..good
        },
        FirstPersonCamera {
            fov_y: BAngle::HALF,
            ..good
        },
        FirstPersonCamera { near: 0.0, ..good },
        FirstPersonCamera {
            near: f32::INFINITY,
            ..good
        },
    ];
    for camera in bad {
        let error = camera.view_proj(1.0).unwrap_err();
        assert!(matches!(error, RenderError::Scene { shape: None, .. }));
        assert!(error.to_string().starts_with("[E-SCENE] "), "{error}");
    }
    for aspect in [0.0, -1.0, f32::NAN] {
        assert!(good.view_proj(aspect).is_err(), "aspect {aspect}");
    }
}
