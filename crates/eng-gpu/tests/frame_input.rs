//! What a frame brings beside its passes, on a real adapter (rung 2, task
//! T8; ADR-0020): the per-frame uniform, which screen passes bind too; the
//! instance records, chunk origins minus the eye taken in f64, that place
//! chunk-relative meshes from one megabuffer; and what the frame uploads
//! and counts, which is nothing of it unless a pass binds it. Without an
//! adapter each test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes it fail.

mod rig;

use eng_gpu::{
    DRAW, FrameInput, FullscreenDesc, Graph, InstanceRecord, MeshDraw, PassDecl, Resource,
    SceneDesc, ScreenDesc, TONEMAP, Target,
};
use rig::{Script, Step, at, at_clip, draw_quad, plain_uniform, quad, rgba, scene_of, uniform};

const MESH: &str = include_str!("wgsl/mesh.wgsl");
const UNIFORM_WGSL: &str = include_str!("wgsl/screen_uniform.wgsl");
const CLAMP: &str = include_str!("wgsl/clamp.wgsl");

const DRAW_OUTPUT: PassDecl = PassDecl {
    name: "draw",
    reads: &[],
    writes: &[Resource::Output],
};

#[test]
fn a_screen_pass_shows_the_uniform_and_counts_it() {
    let Some(gpu) = rig::gpu() else { return };
    let show = rig::screen(
        &gpu,
        UNIFORM_WGSL,
        "uniform_fragment",
        &[],
        &[Resource::Output],
    );
    let graph = Graph::new(&[DRAW_OUTPUT], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 8, 8);
    let bytes = uniform([1.0; 4], [1.0, 0.0, 1.0, 1.0]);
    let records = [at([0.0; 3], 0), at([1.0; 3], 1)];
    let input = FrameInput {
        uniform: &bytes,
        instances: &records,
        ..FrameInput::default()
    };
    let (counters, image) = rig::render(
        &gpu,
        &graph,
        &targets,
        &input,
        &mut [Script(DRAW_OUTPUT, vec![Step::Screen(show)])],
    );
    assert_eq!(image.pixel(3, 3), [255, 0, 255, 255]);
    // The uniform's 32 bytes and two records of 16: the frame group, bound
    // once with the pipeline.
    assert_eq!(
        counters,
        eng_gpu::FrameCounters {
            draws: 1,
            pipeline_switches: 1,
            bind_group_switches: 1,
            upload_bytes: 32 + 2 * InstanceRecord::BYTES,
        }
    );
}

#[test]
fn a_frame_of_rung_1b_pipelines_uploads_nothing_of_its_input() {
    let Some(gpu) = rig::gpu() else { return };
    let clamp = gpu
        .fullscreen_pipeline(&FullscreenDesc {
            label: "clamp",
            wgsl: CLAMP,
            entry: "clamp_fragment",
            target: Resource::Output,
        })
        .unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 8, 8);
    let bytes = plain_uniform();
    let records = [at([0.0; 3], 0)];
    let input = FrameInput {
        uniform: &bytes,
        instances: &records,
        ..FrameInput::default()
    };
    let (counters, image) = rig::render(
        &gpu,
        &Graph::fixed(Target::Offscreen),
        &targets,
        &input,
        &mut [
            Script(DRAW, vec![Step::Clear(rgba(0.0, 1.0, 0.0, 1.0))]),
            Script(TONEMAP, vec![Step::Fullscreen(clamp)]),
        ],
    );
    assert_eq!(image.pixel(0, 0), [0, 255, 0, 255]);
    // As 1b counts it: no pass binds the uniform, so it is not written.
    assert_eq!(
        counters,
        eng_gpu::FrameCounters {
            draws: 1,
            pipeline_switches: 1,
            bind_group_switches: 1,
            upload_bytes: 0,
        }
    );
}

#[test]
fn chunk_relative_meshes_land_where_their_f64_origins_say() {
    let Some(gpu) = rig::gpu() else { return };
    // One mesh, a 0.5 m square at its chunk's origin, drawn for two chunks
    // 2^30 m from the world's origin. In f32 those origins are 128 m
    // apart steps; their offsets from the eye, taken in f64, are exact.
    let far = 1_073_741_824.0_f64;
    let eye = [far, far, 0.0];
    let records = [
        InstanceRecord::relative([far - 0.75, far + 0.25, 0.5], eye, 0),
        InstanceRecord::relative([far + 0.25, far - 0.75, 0.5], eye, 1),
    ];
    assert_eq!(records[0].offset, [-0.75, 0.25, 0.5]);
    assert_eq!(records[1].offset, [0.25, -0.75, 0.5]);
    // Two meshes in one vertex buffer: the second a smaller square, its
    // indices counted from its own first vertex.
    let scene = scene_of(
        &gpu,
        &[
            quad(0.0, 0.0, 0.5, 0.5, 0.0, 1.0),
            quad(0.0, 0.0, 0.25, 0.25, 0.0, 1.0),
        ],
        SceneDesc::default(),
    );
    assert_eq!(scene.index_count(), 12);
    let pipeline = rig::mesh(&gpu, MESH, "colour_fragment", &[], &[Resource::Output]);
    let graph = Graph::new(&[DRAW_OUTPUT], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 32, 32);
    let bytes = plain_uniform();
    let mut all = records.to_vec();
    // A third record puts the small square at the centre, blue.
    all.push(InstanceRecord::relative([far, far, 0.5], eye, 2));
    let input = FrameInput {
        uniform: &bytes,
        instances: &all,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    let steps = vec![
        Step::Clear(rgba(0.0, 0.0, 0.0, 1.0)),
        Step::Mesh(pipeline.clone(), draw_quad(0, 0..2)),
        Step::Mesh(pipeline, draw_quad(1, 2..3)),
    ];
    let (counters, image) = rig::render(
        &gpu,
        &graph,
        &targets,
        &input,
        &mut [Script(DRAW_OUTPUT, steps)],
    );
    let (red, green, blue, black) = (
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [0, 0, 0, 255],
    );
    // Chunk 0 covers x -0.75..-0.25, y 0.25..0.75; chunk 1 x 0.25..0.75,
    // y -0.75..-0.25; the small square x 0..0.25, y 0..0.25.
    assert_eq!(at_clip(&image, -0.5, 0.5), red);
    assert_eq!(at_clip(&image, 0.5, -0.5), green);
    assert_eq!(at_clip(&image, 0.125, 0.125), blue);
    assert_eq!(at_clip(&image, 0.5, 0.5), black);
    assert_eq!(at_clip(&image, -0.5, -0.5), black);
    assert_eq!(
        at_clip(&image, 0.375, 0.125),
        black,
        "beyond the small square"
    );
    // Two draws, one pipeline, the frame and the scene bound once; the
    // uniform and three records uploaded.
    assert_eq!(
        counters,
        eng_gpu::FrameCounters {
            draws: 2,
            pipeline_switches: 1,
            bind_group_switches: 2,
            upload_bytes: 32 + 3 * InstanceRecord::BYTES,
        }
    );
}

#[test]
fn a_frame_without_what_its_pipelines_bind_is_an_error() {
    let Some(gpu) = rig::gpu() else { return };
    let scene = scene_of(
        &gpu,
        &[quad(0.0, 0.0, 1.0, 1.0, 0.5, 1.0)],
        SceneDesc::default(),
    );
    let mesh = rig::mesh(&gpu, MESH, "colour_fragment", &[], &[Resource::Output]);
    let graph = Graph::new(&[DRAW_OUTPUT], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 4, 4);
    let bytes = plain_uniform();
    let records = [at([0.0; 3], 0)];
    let full = FrameInput {
        uniform: &bytes,
        instances: &records,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    let fails = |input: &FrameInput<'_>, draw: MeshDraw| {
        rig::render_err(
            &gpu,
            &graph,
            &targets,
            input,
            &mut [Script(DRAW_OUTPUT, vec![Step::Mesh(mesh.clone(), draw)])],
        )
    };
    assert_eq!(
        fails(
            &FrameInput {
                scene: None,
                ..full
            },
            draw_quad(0, 0..1)
        ),
        "[E-GPU] pass `draw`: the pipeline binds the scene, but the frame has none"
    );
    assert_eq!(
        fails(
            &FrameInput {
                uniform: &[],
                ..full
            },
            draw_quad(0, 0..1)
        ),
        "[E-GPU] pass `draw`: the pipeline binds the frame's uniform, but the frame has none"
    );
    assert_eq!(
        fails(
            &FrameInput {
                uniform: &bytes[..20],
                ..full
            },
            draw_quad(0, 0..1)
        ),
        "[E-GPU] pass `draw`: the frame's uniform has 20 bytes, not a whole number of 16-byte \
         rows"
    );
    assert_eq!(
        fails(&full, draw_quad(0, 0..2)),
        "[E-GPU] pass `draw`: the draw takes instance records 0..2, but the frame has 1"
    );
    assert_eq!(
        fails(&full, draw_quad(1, 0..1)),
        "[E-GPU] pass `draw`: the draw takes indices 6..12, but the scene has 6"
    );
    // A pipeline made for another pass's reads and writes.
    let other = rig::mesh(&gpu, MESH, "colour_fragment", &[], &[Resource::Scene]);
    let e = rig::render_err(
        &gpu,
        &graph,
        &targets,
        &full,
        &mut [Script(
            DRAW_OUTPUT,
            vec![Step::Mesh(other, draw_quad(0, 0..1))],
        )],
    );
    assert_eq!(
        e,
        "[E-GPU] pass `draw`: the pipeline was made for a pass that reads [] and writes [scene], \
         but this pass reads [] and writes [output]"
    );
    // A screen pipeline that binds the scene needs one too.
    let scenic = gpu
        .screen_pipeline(&ScreenDesc {
            label: "scenic",
            wgsl: UNIFORM_WGSL,
            entry: "uniform_fragment",
            reads: &[],
            writes: &[Resource::Output],
            scene: true,
        })
        .unwrap_or_else(|e| panic!("{e}"));
    let e = rig::render_err(
        &gpu,
        &graph,
        &targets,
        &FrameInput {
            scene: None,
            ..full
        },
        &mut [Script(DRAW_OUTPUT, vec![Step::Screen(scenic)])],
    );
    assert_eq!(
        e,
        "[E-GPU] pass `draw`: the pipeline binds the scene, but the frame has none"
    );
}
