//! The first-person view's CPU side (rung 2, tasks T9 and T10), no GPU
//! needed: `info`'s packing, the world vertex and chunk layouts, the
//! per-frame uniform's declaration in every shader, the constants the
//! shaders repeat, the camera's rays, and the near plane's bounds.

use eng_math::{BAngle, WorldPos, sin_cos_f64, sqrt_f64};
use eng_render::{
    BAYER_4X4, Chunk, FirstPersonCamera, Info, LIGHT_STEPS, MAX_REGIONS, NEAR_PLANE, SurfaceClass,
    WorldVertex,
};
use proptest::prelude::*;

const OPAQUE_WGSL: &str = include_str!("../src/opaque.wgsl");
const SKY_WGSL: &str = include_str!("../src/sky.wgsl");
const LIGHT_WGSL: &str = include_str!("../src/light.wgsl");
const FOG_WGSL: &str = include_str!("../src/fog.wgsl");
const TONEMAP_WGSL: &str = include_str!("../src/pbr_neutral.wgsl");
const PALETTE_WGSL: &str = include_str!("../src/palette.wgsl");

fn class() -> impl Strategy<Value = SurfaceClass> {
    (0..SurfaceClass::ALL.len()).prop_map(|i| SurfaceClass::ALL[i])
}

proptest! {
    #[test]
    fn info_unpacks_to_what_was_packed(region: u16, class in class(), id: u32) {
        let info = Info { region, class, id };
        let [r, g] = info.pack();
        prop_assert_eq!(r & 0xffff, u32::from(region));
        prop_assert_eq!((r >> 16) & 0xff, u32::from(class.code()));
        prop_assert_eq!(r & Info::RESERVED, 0);
        prop_assert_eq!(g, id);
        prop_assert_eq!(Info::unpack([r, g]), Some(info));
    }

    #[test]
    fn words_with_a_reserved_bit_or_an_unknown_class_unpack_to_nothing(r: u32, g: u32) {
        let class_known = usize::from((r >> 16) as u8) < SurfaceClass::ALL.len();
        let fits = r & Info::RESERVED == 0 && class_known;
        prop_assert_eq!(Info::unpack([r, g]).is_some(), fits);
    }
}

#[test]
fn the_packing_and_the_class_codes_are_fixed() {
    // The shaders read these codes, so they never move.
    let codes: Vec<u8> = SurfaceClass::ALL.iter().map(|c| c.code()).collect();
    assert_eq!(codes, [0, 1, 2, 3, 4, 5, 6]);
    for class in SurfaceClass::ALL {
        assert_eq!(SurfaceClass::from_code(class.code()), Some(class));
    }
    assert_eq!(SurfaceClass::from_code(7), None);
    let info = Info {
        region: 0x1234,
        class: SurfaceClass::WallFacingY,
        id: 0xDEAD_BEEF,
    };
    assert_eq!(info.pack(), [0x0005_1234, 0xDEAD_BEEF]);
    // `opaque` clears `info` to 0: nothing drawn.
    assert_eq!(Info::NONE.pack(), [0, 0]);
    assert_eq!(Info::unpack([0, 0]), Some(Info::NONE));
    assert_eq!(Info::unpack([0x0100_0000, 0]), None);
}

#[test]
fn a_world_vertex_is_32_bytes_in_the_shaders_order() {
    let vertex = WorldVertex {
        position: [1.5, -2.0, 0.25],
        uv: [3.0, 0.5],
        material: 7,
        info: Info {
            region: 2,
            class: SurfaceClass::Floor,
            id: 99,
        },
    };
    let bytes = vertex.to_bytes();
    assert_eq!(bytes.len(), WorldVertex::BYTES);
    let words: Vec<u32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect();
    let float = |f: f32| f.to_bits();
    assert_eq!(
        words,
        [
            float(1.5),
            float(-2.0),
            float(0.25),
            float(3.0),
            float(0.5),
            7,
            0x0001_0002,
            99
        ]
    );
}

#[test]
fn a_chunks_quads_are_taken_from_its_origin_in_f64() {
    // 10^9 m out, f32 steps by 64 m; the corners' offsets are exact.
    let origin = WorldPos::new(1.0e9, -2.0e9, 250.0);
    let at = |x: f64, y: f64, z: f64| WorldPos::new(origin.x + x, origin.y + y, origin.z + z);
    let info = Info {
        region: 0,
        class: SurfaceClass::Floor,
        id: 1,
    };
    let mut chunk = Chunk::new(origin);
    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let floor = [
        at(0.125, 0.0, 0.0),
        at(2.0, 0.0, 0.0),
        at(2.0, 3.5, 0.0),
        at(0.125, 3.5, 0.0),
    ];
    chunk.quad(floor, uv, 0, info);
    chunk.quad(
        floor.map(|p| WorldPos::new(p.x, p.y, p.z - 1.0)),
        uv,
        1,
        info,
    );
    let positions: Vec<[f32; 3]> = chunk.vertices.iter().map(|v| v.position).collect();
    assert_eq!(
        positions[..4],
        [
            [0.125, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 3.5, 0.0],
            [0.125, 3.5, 0.0]
        ]
    );
    assert_eq!(positions[4], [0.125, 0.0, -1.0]);
    assert_eq!(chunk.indices, [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]);
    assert_eq!(chunk.vertices[5].material, 1);
}

/// The text of `struct Frame { ... }` in `wgsl`, if it declares one.
fn frame_struct(wgsl: &str) -> Option<&str> {
    let start = wgsl.find("struct Frame {")?;
    let end = start + wgsl[start..].find('}')?;
    Some(&wgsl[start..=end])
}

#[test]
fn every_first_person_shader_declares_the_uniform_alike() {
    let opaque = frame_struct(OPAQUE_WGSL).expect("opaque.wgsl declares the uniform");
    for wgsl in [SKY_WGSL, LIGHT_WGSL, FOG_WGSL, TONEMAP_WGSL, PALETTE_WGSL] {
        assert_eq!(frame_struct(wgsl), Some(opaque));
    }
    // 39 16-byte rows, as `first_person/frame.rs` writes them.
    let fields: Vec<&str> = opaque
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| l.ends_with(','))
        .collect();
    assert_eq!(
        fields,
        [
            "clip_from_eye: mat4x4<f32>,",
            "forward: vec4<f32>,",
            "ray_right: vec4<f32>,",
            "ray_up: vec4<f32>,",
            "sky_horizon: vec4<f32>,",
            "sky_zenith: vec4<f32>,",
            "look: vec4<f32>,",
            "switches: vec4<u32>,",
            "light_steps: array<vec4<f32>, 8>,",
            "region_light: array<vec4<f32>, 4>,",
            "region_fog: array<vec4<f32>, 16>,",
        ]
    );
    // The arrays hold what eng-render sizes them for.
    assert_eq!(LIGHT_STEPS, 8 * 4);
    assert_eq!(MAX_REGIONS, 4 * 4);
    for wgsl in [LIGHT_WGSL, FOG_WGSL] {
        assert!(wgsl.contains("const LAST_REGION: u32 = 15u;"));
    }
}

#[test]
fn the_shaders_repeat_the_crates_constants() {
    // The palette pass's Bayer matrix, row by row.
    let start = PALETTE_WGSL
        .find("const BAYER_4X4: array<u32, 16> = array<u32, 16>(")
        .expect("palette.wgsl declares the matrix");
    let end = start + PALETTE_WGSL[start..].find(");").expect("the matrix ends");
    let values: Vec<u8> = PALETTE_WGSL[start..end]
        .split_once('(')
        .map(|(_, list)| list)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.trim_end_matches('u').parse().expect("a u32 literal"))
        .collect();
    assert_eq!(values, BAYER_4X4.concat());
    // The class codes the light pass reads.
    assert!(LIGHT_WGSL.contains(&format!(
        "const WALL_FACING_X: u32 = {}u;",
        SurfaceClass::WallFacingX.code()
    )));
    assert!(LIGHT_WGSL.contains(&format!(
        "const WALL_FACING_Y: u32 = {}u;",
        SurfaceClass::WallFacingY.code()
    )));
}

fn camera(yaw: u32, pitch: u32, fov: u32) -> FirstPersonCamera {
    FirstPersonCamera {
        eye: WorldPos::new(3.0, -4.0, 1.6),
        yaw: BAngle(yaw),
        pitch: BAngle(pitch),
        fov_y: BAngle(fov),
        near: NEAR_PLANE,
    }
}

proptest! {
    /// The ray through a point of the image goes back to that point, and
    /// lies one meter ahead per unit, as the sky pass relies on.
    #[test]
    fn a_ray_projects_back_to_where_it_was_cast(
        yaw: u32,
        pitch in -954_437_176i32..=954_437_176,
        fov in 0x0800_0000u32..0x6000_0000,
        x in -1.0f32..=1.0,
        y in -1.0f32..=1.0,
        aspect in 0.5f32..=2.5,
    ) {
        let view = camera(yaw, pitch as u32, fov)
            .view_proj(aspect)
            .expect("a valid camera");
        let ray = view.ray(x, y);
        let forward = view.forward();
        let ahead = ray.x * forward.x + ray.y * forward.y + ray.z * forward.z;
        prop_assert!((ahead - 1.0).abs() < 1e-5, "{}", ahead);
        let ndc = view.project_offset(ray).ndc().expect("in front");
        prop_assert!((ndc.x - x).abs() < 1e-4 && (ndc.y - y).abs() < 1e-4, "{:?}", ndc);
        prop_assert!((ndc.z - NEAR_PLANE).abs() < 1e-5 * NEAR_PLANE, "{}", ndc.z);
        prop_assert_eq!(view.near(), NEAR_PLANE);
    }
}

/// How far the near rectangle's farthest corner is from the eye, through
/// a vertical field of view whose half has tangent `ty`, at `aspect`.
fn near_corner_at(ty: f64, aspect: f64) -> f64 {
    f64::from(NEAR_PLANE) * sqrt_f64(1.0 + ty * ty * (1.0 + aspect * aspect))
}

/// The same at a horizontal field of view of `degrees` on a 16:9 image.
fn near_corner(degrees: f64) -> f64 {
    let (s, c) = sin_cos_f64(degrees / 2.0 * std::f64::consts::PI / 180.0);
    near_corner_at(s / c * 9.0 / 16.0, 16.0 / 9.0)
}

#[test]
fn the_near_plane_keeps_inside_the_hull_as_its_documentation_says() {
    assert_eq!(NEAR_PLANE, 0.1);
    // Walls: the eye stays 0.25 m off them. At a 90° vertical field of
    // view, up to an aspect of 2.06: 16:9 holds, 21:9 does not.
    assert!(near_corner_at(1.0, 2.06) < 0.25);
    assert!(near_corner_at(1.0, 2.07) > 0.25);
    assert!(near_corner_at(1.0, 16.0 / 9.0) < 0.25);
    assert!(near_corner_at(1.0, 21.0 / 9.0) > 0.25);
    // At 16:9, up to 126° across.
    assert!(near_corner(126.0) < 0.25);
    assert!(near_corner(127.0) > 0.25);
    // A ceiling at the hull's top, 0.15 m above the eye: up to 88°.
    assert!(near_corner(88.0) < 0.15);
    assert!(near_corner(89.0) > 0.15);
}
