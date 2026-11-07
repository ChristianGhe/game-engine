//! A scene's textures on a real adapter (rung 2, task T8; ADR-0020):
//! `texture_2d_array`s bucketed by size (64, 128 and 256 px), one per bucket
//! and colour tag, each texture a layer, chosen through the material table;
//! nearest sampling that repeats, no mipmaps; and the alpha test flag.
//! Without an adapter each test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes
//! it fail.

mod rig;

use eng_gpu::{
    ColourTag, FrameInput, Graph, Material, PassDecl, Resource, SceneDesc, SceneTexture, Target,
    TextureSlot,
};
use rig::{Script, Step, at, at_clip, draw_quad, plain_uniform, quad, rgba, scene_of};

const MESH: &str = include_str!("wgsl/mesh.wgsl");

const DRAW_OUTPUT: PassDecl = PassDecl {
    name: "draw",
    reads: &[],
    writes: &[Resource::Output],
};

/// A `side` px texture whose left half is `left` and right half `right`.
fn halves(side: u32, left: [u8; 4], right: [u8; 4]) -> Vec<u8> {
    let mut texels = Vec::with_capacity((side * side * 4) as usize);
    for _ in 0..side {
        for x in 0..side {
            texels.extend_from_slice(if x < side / 2 { &left } else { &right });
        }
    }
    texels
}

/// A texture of one colour.
fn solid(side: u32, colour: [u8; 4]) -> Vec<u8> {
    halves(side, colour, colour)
}

#[test]
fn each_material_samples_its_layer_of_its_bucket_and_tag() {
    let Some(gpu) = rig::gpu() else { return };
    // sRGB bytes come back as they were. Linear ones are encoded to sRGB by
    // the output: 0 and 255 stay, and 128 (0.502) becomes 188, which an
    // array of the wrong tag would leave at 128.
    let colours: [(u32, ColourTag, [u8; 4]); 6] = [
        (64, ColourTag::Srgb, [200, 40, 90, 255]),
        (128, ColourTag::Linear, [128, 0, 255, 255]),
        (256, ColourTag::Srgb, [10, 120, 230, 255]),
        (64, ColourTag::Srgb, [90, 200, 40, 255]),
        (64, ColourTag::Linear, [0, 255, 255, 255]),
        (256, ColourTag::Linear, [255, 255, 0, 255]),
    ];
    let texels: Vec<Vec<u8>> = colours.iter().map(|&(side, _, c)| solid(side, c)).collect();
    let textures: Vec<SceneTexture<'_>> = colours
        .iter()
        .zip(&texels)
        .map(|(&(side, tag, _), rgba8)| SceneTexture { side, tag, rgba8 })
        .collect();
    // Material i shows texture 5 - i, so that a material's index is not its
    // texture's.
    let materials: Vec<Material> = (0..6)
        .map(|i| Material {
            texture: 5 - i,
            alpha_test: false,
        })
        .collect();
    // Six cells across the target, one quad each.
    let quads: Vec<Vec<u8>> = (0..6)
        .map(|i| {
            let x = -1.0 + i as f32 / 3.0;
            quad(x, -1.0, x + 1.0 / 3.0, 1.0, 0.5, 1.0)
        })
        .collect();
    let scene = scene_of(
        &gpu,
        &quads,
        SceneDesc {
            label: "six",
            materials: &materials,
            textures: &textures,
            ..SceneDesc::default()
        },
    );
    assert_eq!(
        scene.slot(3),
        Some(TextureSlot {
            side: 64,
            tag: ColourTag::Srgb,
            layer: 1
        })
    );
    assert_eq!(scene.slot(5).map(TextureSlot::bucket), Some(2));
    assert_eq!(scene.slot(6), None);
    let texel_bytes: u64 = texels.iter().map(|t| t.len() as u64).sum();
    // Vertices of 20 bytes, four a quad; six indices of 4 bytes a quad;
    // material records of 16 bytes.
    assert_eq!(scene.bytes(), 6 * 4 * 20 + 6 * 6 * 4 + 6 * 16 + texel_bytes);

    let pipeline = rig::mesh(&gpu, MESH, "textured_fragment", &[], &[Resource::Output]);
    let instances: Vec<_> = (0..6).map(|i| at([0.0; 3], i)).collect();
    let steps = (0..6)
        .map(|i| Step::Mesh(pipeline.clone(), draw_quad(i, i..i + 1)))
        .collect();
    let graph = Graph::new(&[DRAW_OUTPUT], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 48, 8);
    let uniform = plain_uniform();
    let input = FrameInput {
        uniform: &uniform,
        instances: &instances,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    let (counters, image) = rig::render(
        &gpu,
        &graph,
        &targets,
        &input,
        &mut [Script(DRAW_OUTPUT, steps)],
    );
    for cell in 0..6 {
        let x = -1.0 + (cell as f32 + 0.5) / 3.0;
        let (_, tag, mut expected) = colours[5 - cell];
        if tag == ColourTag::Linear && expected[0] == 128 {
            expected[0] = 188;
        }
        rig::assert_close(at_clip(&image, x, 0.0), expected, &format!("cell {cell}"));
    }
    // The uniform's 32 bytes and six records of 16; nothing of the scene.
    assert_eq!(counters.upload_bytes, 32 + 6 * 16);
    assert_eq!(
        (
            counters.draws,
            counters.pipeline_switches,
            counters.bind_group_switches
        ),
        (6, 1, 2)
    );
}

#[test]
fn sampling_takes_the_nearest_texel_repeats_and_alpha_tests() {
    let Some(gpu) = rig::gpu() else { return };
    let (red, blue, clear) = ([255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 0, 0]);
    let opaque = halves(128, red, blue);
    let holed = halves(64, red, clear);
    let textures = [
        SceneTexture {
            side: 128,
            tag: ColourTag::Srgb,
            rgba8: &opaque,
        },
        SceneTexture {
            side: 64,
            tag: ColourTag::Srgb,
            rgba8: &holed,
        },
    ];
    let materials = [
        Material {
            texture: 0,
            alpha_test: false,
        },
        Material {
            texture: 1,
            alpha_test: true,
        },
        Material {
            texture: 1,
            alpha_test: false,
        },
    ];
    // The top half: texture coordinates from 0 to 2, so the texture shows
    // twice across. The bottom half: two quads of the holed texture, the
    // left one alpha-tested.
    let scene = scene_of(
        &gpu,
        &[
            quad(-1.0, 0.0, 1.0, 1.0, 0.5, 2.0),
            quad(-1.0, -1.0, 0.0, 0.0, 0.5, 1.0),
            quad(0.0, -1.0, 1.0, 0.0, 0.5, 1.0),
        ],
        SceneDesc {
            label: "sampling",
            materials: &materials,
            textures: &textures,
            ..SceneDesc::default()
        },
    );
    let pipeline = rig::mesh(&gpu, MESH, "textured_fragment", &[], &[Resource::Output]);
    let instances = [at([0.0; 3], 0), at([0.0; 3], 1), at([0.0; 3], 2)];
    let graph = Graph::new(&[DRAW_OUTPUT], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 64, 16);
    let uniform = plain_uniform();
    let input = FrameInput {
        uniform: &uniform,
        instances: &instances,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    let steps = vec![
        Step::Clear(rgba(0.0, 1.0, 0.0, 1.0)),
        Step::Mesh(pipeline.clone(), draw_quad(0, 0..1)),
        Step::Mesh(pipeline.clone(), draw_quad(1, 1..2)),
        Step::Mesh(pipeline, draw_quad(2, 2..3)),
    ];
    let (_, image) = rig::render(
        &gpu,
        &graph,
        &targets,
        &input,
        &mut [Script(DRAW_OUTPUT, steps)],
    );
    // Across the top row: red, blue, red, blue in quarters, and only those
    // two colours, as nearest sampling blends nothing.
    let row: Vec<[u8; 4]> = (0..64).map(|x| image.pixel(x, 2)).collect();
    for (x, pixel) in row.iter().enumerate() {
        let expected = if (x / 16) % 2 == 0 { red } else { blue };
        assert_eq!(*pixel, expected, "top row, pixel {x}");
    }
    // The alpha-tested quad keeps the green clear where its texture is
    // transparent; the other draws the texel, alpha 0 and all.
    let green = [0, 255, 0, 255];
    assert_eq!(
        at_clip(&image, -0.9, -0.5),
        red,
        "alpha-tested, opaque half"
    );
    assert_eq!(
        at_clip(&image, -0.1, -0.5),
        green,
        "alpha-tested, clear half"
    );
    assert_eq!(at_clip(&image, 0.1, -0.5), red, "untested, opaque half");
    assert_eq!(
        at_clip(&image, 0.9, -0.5),
        [0, 0, 0, 0],
        "untested, clear half"
    );
}

#[test]
fn a_scene_its_arrays_cannot_hold_is_an_error() {
    let Some(gpu) = rig::gpu() else { return };
    let texels = solid(100, [0; 4]);
    let e = gpu
        .scene(&SceneDesc {
            label: "odd",
            textures: &[SceneTexture {
                side: 100,
                tag: ColourTag::Linear,
                rgba8: &texels,
            }],
            ..SceneDesc::default()
        })
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "[E-GPU] scene `odd`: texture 0 is 100x100 px; a scene texture is 64, 128 or 256 px square"
    );
    let e = gpu
        .scene(&SceneDesc {
            label: "ragged",
            vertices: &[0; 6],
            ..SceneDesc::default()
        })
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "[E-GPU] scene `ragged`: 6 bytes of vertices are not whole 4-byte words"
    );
    // An empty scene is a scene: every array has one blank layer.
    let empty = gpu
        .scene(&SceneDesc::default())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((empty.bytes(), empty.index_count()), (0, 0));
}
