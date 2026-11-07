//! What the first-person look checks share (rung 2, task T10): a test
//! palette in GIMP's format, the room of `room/mod.rs` with its palette
//! table, or with one flat material, foggy regions, and a frame's output,
//! depth and `info`.
// Each test file uses some of these.
#![allow(dead_code)]

use std::fmt::Write as _;

use eng_gpu::{Gpu, GpuError, GpuOptions, Image, Targets};
use eng_render::{
    ColourTag, FirstPersonRenderer, FirstPersonScene, GplPalette, GpuWorld, LinearRgb, Look,
    Material, PaletteLut, RegionLook, SceneTexture, Texels, WorldMesh,
};

pub fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(gpu) => Some(gpu),
        Err(e @ GpuError::NoAdapter { .. }) if !eng_gpu::gpu_required() => {
            eprintln!("skipped: {e}");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

pub const W: u32 = 320;
pub const H: u32 = 240;

/// A test palette in GIMP's format: the 4x4x4 cube of codes 0, 85, 170
/// and 255, and greys every 8 codes between.
pub fn palette() -> GplPalette {
    let mut text = String::from("GIMP Palette\nName: look checks\nColumns: 8\n# cube, greys\n");
    for b in 0..4 {
        for g in 0..4 {
            for r in 0..4 {
                let _ = writeln!(text, "{:3} {:3} {:3}\tcube", r * 85, g * 85, b * 85);
            }
        }
    }
    for i in 1..32 {
        let v = i * 8;
        let _ = writeln!(text, "{v:3} {v:3} {v:3}\tgrey {i}");
    }
    GplPalette::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

pub fn lut() -> PaletteLut {
    PaletteLut::bake(&palette().colours).unwrap_or_else(|e| panic!("{e}"))
}

/// The room with its own textures, and the palette table.
pub fn room_world(gpu: &Gpu, lut: &PaletteLut) -> GpuWorld {
    let texels = crate::room::texels();
    let textures = crate::room::textures(&texels);
    let materials = crate::room::materials();
    let mesh = WorldMesh {
        label: "room",
        chunks: &crate::room::world_chunks([0.0; 3]),
        materials: &materials,
        textures: &textures,
    };
    GpuWorld::load_with_palette(gpu, &mesh, lut).unwrap_or_else(|e| panic!("{e}"))
}

/// The room with every surface one flat `rgb` (sRGB codes).
pub fn flat_world(gpu: &Gpu, rgb: [u8; 3], lut: &PaletteLut) -> GpuWorld {
    let texels = [rgb[0], rgb[1], rgb[2], 255].repeat(64 * 64);
    let textures = [SceneTexture {
        side: 64,
        tag: ColourTag::Srgb,
        rgba8: &texels,
    }];
    let materials = [Material {
        texture: 0,
        alpha_test: false,
    }];
    let chunks: Vec<_> = crate::room::chunks()
        .into_iter()
        .map(|(origin, faces)| {
            let faces = faces
                .into_iter()
                .map(|f| crate::room::Face { material: 0, ..f });
            (origin, faces.collect::<Vec<_>>())
        })
        .collect();
    let mesh = WorldMesh {
        label: "flat room",
        chunks: &crate::room::build([0.0; 3], &chunks),
        materials: &materials,
        textures: &textures,
    };
    GpuWorld::load_with_palette(gpu, &mesh, lut).unwrap_or_else(|e| panic!("{e}"))
}

pub const GREEN_FOG: LinearRgb = LinearRgb {
    r: 2.0,
    g: 30.0,
    b: 6.0,
};

/// The room's regions with light levels between whole steps, so banding
/// shows even without falloff, and fog in both.
pub fn foggy_regions() -> Vec<RegionLook> {
    vec![
        RegionLook {
            light: 203,
            fog: GREEN_FOG,
            fog_density: 0.15,
        },
        RegionLook {
            light: 165,
            fog: LinearRgb {
                r: 20.0,
                g: 4.0,
                b: 1.0,
            },
            fog_density: 0.4,
        },
    ]
}

/// The room's camera, with `look` and foggy regions.
pub fn scene(look: Look) -> FirstPersonScene {
    FirstPersonScene {
        look,
        regions: foggy_regions(),
        ..crate::room::scene([0.0; 3])
    }
}

/// What a frame left.
pub struct Frame {
    pub image: Image,
    pub depth: Texels,
    pub info: Texels,
}

pub fn render(
    gpu: &Gpu,
    renderer: &FirstPersonRenderer,
    targets: &Targets,
    world: &GpuWorld,
    scene: &FirstPersonScene,
    plain: bool,
) -> Frame {
    if plain {
        renderer.render_plain(gpu, targets, world, scene)
    } else {
        renderer.render(gpu, targets, world, scene)
    }
    .unwrap_or_else(|e| panic!("{e}"));
    Frame {
        image: targets.read_output(gpu).unwrap_or_else(|e| panic!("{e}")),
        depth: renderer
            .read_depth(gpu, targets)
            .unwrap_or_else(|e| panic!("{e}")),
        info: renderer
            .read_info(gpu, targets)
            .unwrap_or_else(|e| panic!("{e}")),
    }
}

/// An f16's value.
pub fn half(bits: u16) -> f64 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let fraction = f64::from(bits & 0x3ff);
    let scale = |e: i32| {
        let mut v = 1.0;
        for _ in 0..e.abs() {
            v *= 2.0;
        }
        if e < 0 { 1.0 / v } else { v }
    };
    sign * match exponent {
        0 => fraction * scale(-24),
        31 => panic!("not finite"),
        e => (1024.0 + fraction) * scale(e - 25),
    }
}

/// The RGB of an `Rgba16Float` texel.
pub fn rgb16(texels: &Texels, x: u32, y: u32) -> [f64; 3] {
    let t = texels.texel(x, y);
    [0, 1, 2].map(|c| half(u16::from_le_bytes([t[2 * c], t[2 * c + 1]])))
}
