//! Depth and several colour targets on a real adapter (rung 2, task T8;
//! ADR-0020): mesh passes test and write a reversed depth target, a pass
//! that continues another loads its depth, a pass draws into four colour
//! targets of three formats and a depth target, and a screen pass reads
//! each kind back. Without an adapter each test skips, unless
//! `ENG_REQUIRE_GPU=1` (CI) makes it fail.

mod rig;

use eng_gpu::{
    Format, FrameInput, FullscreenDesc, Graph, MAX_COLOUR_TARGETS, MeshDesc, PassDecl, PassNode,
    Resource, SceneDesc, ScreenDesc, Target, Transient,
};
use rig::{Script, Step, at, at_clip, draw_quad, plain_uniform, quad, rgba, scene_of};

const MESH: &str = include_str!("wgsl/mesh.wgsl");
const DEPTH_WGSL: &str = include_str!("wgsl/screen_depth.wgsl");
const VERIFY: &str = include_str!("wgsl/screen_verify.wgsl");
const COPY_WGSL: &str = include_str!("wgsl/copy.wgsl");

const ALBEDO: Resource = Resource::Transient(Transient::new("albedo", Format::Rgba16Float));
const INFO: Resource = Resource::Transient(Transient::new("info", Format::Rg32Uint));
const DEPTH: Resource = Resource::Transient(Transient::new("depth", Format::Depth32Float));
const B: Resource = Resource::Transient(Transient::new("b", Format::Rgba8UnormSrgb));
const C: Resource = Resource::Transient(Transient::new("c", Format::Rgba16Float));

const OPAQUE: PassNode = PassNode {
    name: "opaque",
    reads: &[],
    writes: &[ALBEDO, DEPTH],
    continues: None,
    bypassable: false,
};
const SPRITES: PassNode = PassNode {
    name: "sprites",
    continues: Some("opaque"),
    ..OPAQUE
};
const SHOW: PassNode = PassNode {
    name: "show",
    reads: &[ALBEDO, DEPTH],
    writes: &[Resource::Output],
    continues: None,
    bypassable: false,
};

/// Two quads that overlap in the middle column: the left one 0.25 deep
/// (far, as depth is reversed), the right one 0.75 (near).
fn overlapping() -> Vec<Vec<u8>> {
    vec![
        quad(-1.0, -1.0, 0.5, 1.0, 0.25, 1.0),
        quad(-0.5, -1.0, 1.0, 1.0, 0.75, 1.0),
    ]
}

#[test]
fn the_nearer_fragment_wins_in_either_order_and_depth_reads_back() {
    let Some(gpu) = rig::gpu() else { return };
    let scene = scene_of(&gpu, &overlapping(), SceneDesc::default());
    let draw = rig::mesh(&gpu, MESH, "colour_fragment", &[], &[ALBEDO, DEPTH]);
    let show = rig::screen(
        &gpu,
        DEPTH_WGSL,
        "albedo_depth_fragment",
        &[ALBEDO, DEPTH],
        &[Resource::Output],
    );
    let graph = Graph::sorted(&[SHOW, OPAQUE], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 32, 16);
    let uniform = plain_uniform();
    // Record 0 draws the far quad red, record 1 the near one green.
    let instances = [at([0.0; 3], 0), at([0.0; 3], 1)];
    let input = FrameInput {
        uniform: &uniform,
        instances: &instances,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    for (first, second) in [((0, 0), (1, 1)), ((1, 1), (0, 0))] {
        let steps = vec![
            Step::Clear(rgba(0.0, 0.0, 1.0, 1.0)),
            Step::Mesh(draw.clone(), draw_quad(first.0, first.1..first.1 + 1)),
            Step::Mesh(draw.clone(), draw_quad(second.0, second.1..second.1 + 1)),
        ];
        let (counters, image) = rig::render(
            &gpu,
            &graph,
            &targets,
            &input,
            &mut [
                Script(SHOW.decl(), vec![Step::Screen(show.clone())]),
                Script(OPAQUE.decl(), steps),
            ],
        );
        // Alpha is the depth, stored linear: 0.25 is 64, 0.75 is 191, the
        // cleared far plane 0.
        rig::assert_close(
            at_clip(&image, -0.8, 0.0),
            [255, 0, 0, 64],
            "far quad alone",
        );
        rig::assert_close(at_clip(&image, 0.0, 0.0), [0, 255, 0, 191], "the overlap");
        rig::assert_close(
            at_clip(&image, 0.8, 0.0),
            [0, 255, 0, 191],
            "near quad alone",
        );
        // Three draws through two pipelines; opaque binds the frame and the
        // scene, show the frame and its reads.
        assert_eq!(
            (
                counters.draws,
                counters.pipeline_switches,
                counters.bind_group_switches
            ),
            (3, 2, 4)
        );
    }
}

#[test]
fn a_continuing_pass_tests_against_the_depth_it_loads() {
    let Some(gpu) = rig::gpu() else { return };
    let scene = scene_of(&gpu, &overlapping(), SceneDesc::default());
    let draw = rig::mesh(&gpu, MESH, "colour_fragment", &[], &[ALBEDO, DEPTH]);
    let show = rig::screen(
        &gpu,
        DEPTH_WGSL,
        "albedo_depth_fragment",
        &[ALBEDO, DEPTH],
        &[Resource::Output],
    );
    let graph = Graph::sorted(&[SPRITES, SHOW, OPAQUE], Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 32, 16);
    let uniform = plain_uniform();
    let instances = [at([0.0; 3], 1), at([0.0; 3], 0)];
    let input = FrameInput {
        uniform: &uniform,
        instances: &instances,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    // Opaque draws the near quad green; sprites then draws the far one red,
    // which shows only where opaque drew nothing.
    let (_, image) = rig::render(
        &gpu,
        &graph,
        &targets,
        &input,
        &mut [
            Script(
                SPRITES.decl(),
                vec![Step::Mesh(draw.clone(), draw_quad(0, 1..2))],
            ),
            Script(SHOW.decl(), vec![Step::Screen(show)]),
            Script(
                OPAQUE.decl(),
                vec![
                    Step::Clear(rgba(0.0, 0.0, 1.0, 1.0)),
                    Step::Mesh(draw, draw_quad(1, 0..1)),
                ],
            ),
        ],
    );
    rig::assert_close(
        at_clip(&image, -0.8, 0.0),
        [255, 0, 0, 64],
        "far quad alone",
    );
    rig::assert_close(at_clip(&image, 0.0, 0.0), [0, 255, 0, 191], "the overlap");
    rig::assert_close(
        at_clip(&image, 0.8, 0.0),
        [0, 255, 0, 191],
        "near quad alone",
    );
}

#[test]
fn a_pass_draws_four_colour_targets_and_a_depth_target_and_a_pass_reads_them() {
    let Some(gpu) = rig::gpu() else { return };
    const WRITES: &[Resource] = &[ALBEDO, INFO, B, C, DEPTH];
    const READS: &[Resource] = &[ALBEDO, INFO, B, C, DEPTH];
    const FILL: PassNode = PassNode {
        name: "fill",
        reads: &[],
        writes: WRITES,
        continues: None,
        bypassable: false,
    };
    const VERIFY_PASS: PassNode = PassNode {
        name: "verify",
        reads: READS,
        writes: &[Resource::Output],
        continues: None,
        bypassable: false,
    };
    assert_eq!(MAX_COLOUR_TARGETS, 4);
    // The middle half of the target, half deep.
    let scene = scene_of(
        &gpu,
        &[quad(-0.5, -0.5, 0.5, 0.5, 0.5, 1.0)],
        SceneDesc::default(),
    );
    let fill = rig::mesh(&gpu, MESH, "targets_fragment", &[], WRITES);
    let verify = rig::screen(&gpu, VERIFY, "verify_fragment", READS, &[Resource::Output]);
    let graph =
        Graph::sorted(&[VERIFY_PASS, FILL], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 16, 16);
    let uniform = plain_uniform();
    let instances = [at([0.0; 3], 0)];
    let input = FrameInput {
        uniform: &uniform,
        instances: &instances,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    let (_, image) = rig::render(
        &gpu,
        &graph,
        &targets,
        &input,
        &mut [
            Script(VERIFY_PASS.decl(), vec![Step::Screen(verify)]),
            Script(
                FILL.decl(),
                vec![
                    Step::Clear(rgba(0.0, 0.0, 0.0, 0.0)),
                    Step::Mesh(fill, draw_quad(0, 0..1)),
                ],
            ),
        ],
    );
    for (x, y) in [(-0.4, -0.4), (0.0, 0.0), (0.4, 0.4)] {
        assert_eq!(
            at_clip(&image, x, y),
            [255, 255, 255, 255],
            "on the quad at ({x}, {y})"
        );
    }
    for (x, y) in [(-0.9, 0.0), (0.9, 0.9), (0.0, -0.9)] {
        assert_eq!(
            at_clip(&image, x, y),
            [0, 0, 0, 255],
            "around it at ({x}, {y})"
        );
    }
    // Five textures of four kinds: two of Rgba16Float, read.
    assert_eq!(targets.transient_textures(), 5);
}

#[test]
fn a_pass_refuses_five_colour_targets_or_two_depth_targets() {
    let Some(gpu) = rig::gpu() else { return };
    const D: Resource = Resource::Transient(Transient::new("d", Format::Rgba16Float));
    const DEPTH_2: Resource = Resource::Transient(Transient::new("depth-2", Format::Depth32Float));
    let refused = |writes: &[Resource]| {
        gpu.mesh_pipeline(&MeshDesc {
            label: "wide",
            wgsl: MESH,
            vertex_entry: "mesh_vertex",
            fragment_entry: "colour_fragment",
            vertex: rig::VERTEX,
            reads: &[],
            writes,
            depth_write: true,
            cull_back: false,
        })
        .unwrap_err()
        .to_string()
    };
    assert_eq!(
        refused(&[ALBEDO, B, C, D, Resource::Output]),
        "[E-SHADER] wide: its pass cannot draw: it writes 5 colour targets; a pass draws into \
         at most 4"
    );
    assert_eq!(
        refused(&[ALBEDO, DEPTH, DEPTH_2]),
        "[E-SHADER] wide: its pass cannot draw: it writes 2 depth targets; a pass draws into \
         at most one"
    );
    assert_eq!(
        refused(&[Resource::Surface]),
        "[E-SHADER] wide: its pass cannot draw: only the present blit draws into `surface`"
    );
    let screen = gpu
        .screen_pipeline(&ScreenDesc {
            label: "deep",
            wgsl: VERIFY,
            entry: "verify_fragment",
            reads: &[],
            writes: &[ALBEDO, DEPTH],
            scene: false,
        })
        .unwrap_err();
    assert_eq!(
        screen.to_string(),
        "[E-SHADER] deep: a screen pipeline draws colour only; its pass cannot write a depth \
         target"
    );
    // A stage samples 16 textures at Tier 0, the scene's arrays among them.
    let reading = gpu
        .mesh_pipeline(&MeshDesc {
            label: "reading",
            wgsl: MESH,
            vertex_entry: "mesh_vertex",
            fragment_entry: "colour_fragment",
            vertex: rig::VERTEX,
            reads: &[ALBEDO; 11],
            writes: &[Resource::Output],
            depth_write: false,
            cull_back: false,
        })
        .unwrap_err();
    assert_eq!(
        reading.to_string(),
        "[E-SHADER] reading: its pass reads 11 textures; a stage samples at most 10 beside the \
         scene's six arrays (Tier 0: 16)"
    );
    let reading = gpu
        .screen_pipeline(&ScreenDesc {
            label: "reading",
            wgsl: VERIFY,
            entry: "verify_fragment",
            reads: &[ALBEDO; 17],
            writes: &[Resource::Output],
            scene: false,
        })
        .unwrap_err();
    assert_eq!(
        reading.to_string(),
        "[E-SHADER] reading: its pass reads 17 textures; a stage samples at most 16 (Tier 0: 16)"
    );
    // The graph plans such a pass, and the render refuses it before any GPU
    // work.
    const WIDE: PassDecl = PassDecl {
        name: "wide",
        reads: &[],
        writes: &[ALBEDO, B, C, D, Resource::Output],
    };
    let graph = Graph::sorted(&[WIDE.node()], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 4, 4);
    let e = rig::render_err(
        &gpu,
        &graph,
        &targets,
        &FrameInput::default(),
        &mut [Script(WIDE, vec![])],
    );
    assert_eq!(
        e,
        "[E-GPU] pass `wide`: it writes 5 colour targets; a pass draws into at most 4"
    );
    // Its targets share one size: a half-size transient beside the output
    // is refused, naming both.
    const HALF: Resource =
        Resource::Transient(Transient::new("half", Format::Rgba16Float).divided(2));
    const MIXED: PassDecl = PassDecl {
        name: "mixed",
        reads: &[],
        writes: &[Resource::Output, HALF],
    };
    let graph = Graph::sorted(&[MIXED.node()], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let e = rig::render_err(
        &gpu,
        &graph,
        &targets,
        &FrameInput::default(),
        &mut [Script(MIXED, vec![])],
    );
    assert_eq!(
        e,
        "[E-GPU] pass `mixed`: it writes `output` at 1/1 of the targets' size and `half` at 1/2; \
         a pass draws into targets of one size"
    );
}

#[test]
fn rung_1b_pipelines_refuse_what_they_cannot_draw_or_read() {
    let Some(gpu) = rig::gpu() else { return };
    let depth_target = gpu
        .fullscreen_pipeline(&FullscreenDesc {
            label: "copy",
            wgsl: COPY_WGSL,
            entry: "copy_fragment",
            target: DEPTH,
        })
        .unwrap_err();
    assert_eq!(
        depth_target.to_string(),
        "[E-SHADER] copy: a fullscreen pipeline draws colour, and `depth` is a depth target"
    );
    // A 1b fullscreen pass filters its input, which an integer target cannot
    // be: the screen pipelines read those.
    const MAKE: PassDecl = PassDecl {
        name: "make",
        reads: &[],
        writes: &[INFO],
    };
    const LOOK: PassDecl = PassDecl {
        name: "look",
        reads: &[INFO],
        writes: &[Resource::Output],
    };
    let copy = gpu
        .fullscreen_pipeline(&FullscreenDesc {
            label: "copy",
            wgsl: COPY_WGSL,
            entry: "copy_fragment",
            target: Resource::Output,
        })
        .unwrap_or_else(|e| panic!("{e}"));
    let graph = Graph::new(&[MAKE, LOOK], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(&gpu, 4, 4);
    let e = rig::render_err(
        &gpu,
        &graph,
        &targets,
        &FrameInput::default(),
        &mut [
            Script(MAKE, vec![Step::Clear(rgba(0.0, 0.0, 0.0, 0.0))]),
            Script(LOOK, vec![Step::Fullscreen(copy)]),
        ],
    );
    assert_eq!(
        e,
        "[E-GPU] pass `look`: a fullscreen draw filters its input, and `info` is not colour; a \
         screen pipeline reads it"
    );
}
