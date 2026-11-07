//! The depth test's rules on a real adapter (rung 2, task T8; ADR-0020):
//! depth is reversed and a fragment passes at a depth greater than or
//! equal to what is there, so a draw over the same triangles at the same
//! depth wins; a pipeline that does not write depth still tests it and
//! leaves it as it was. Without an adapter each test skips, unless
//! `ENG_REQUIRE_GPU=1` (CI) makes it fail.

mod rig;

use eng_gpu::{
    Format, FrameInput, Graph, MeshDesc, PassNode, Resource, SceneDesc, Target, Transient,
};
use rig::{Script, Step, at, at_clip, draw_quad, plain_uniform, quad, rgba, scene_of};

const MESH: &str = include_str!("wgsl/mesh.wgsl");
const DEPTH_WGSL: &str = include_str!("wgsl/screen_depth.wgsl");

const ALBEDO: Resource = Resource::Transient(Transient::new("albedo", Format::Rgba16Float));
const DEPTH: Resource = Resource::Transient(Transient::new("depth", Format::Depth32Float));

const OPAQUE: PassNode = PassNode {
    name: "opaque",
    reads: &[],
    writes: &[ALBEDO, DEPTH],
    continues: None,
    bypassable: false,
};
const SHOW: PassNode = PassNode {
    name: "show",
    reads: &[ALBEDO, DEPTH],
    writes: &[Resource::Output],
    continues: None,
    bypassable: false,
};

#[test]
fn an_equal_depth_passes_and_a_draw_that_writes_no_depth_leaves_it() {
    let Some(gpu) = rig::gpu() else { return };
    // The left half at 0.5; the right half twice, near (0.75) and far
    // (0.25).
    let scene = scene_of(
        &gpu,
        &[
            quad(-1.0, -1.0, 0.0, 1.0, 0.5, 1.0),
            quad(0.0, -1.0, 1.0, 1.0, 0.75, 1.0),
            quad(0.0, -1.0, 1.0, 1.0, 0.25, 1.0),
        ],
        SceneDesc::default(),
    );
    let writing = rig::mesh(&gpu, MESH, "colour_fragment", &[], &[ALBEDO, DEPTH]);
    let testing = gpu
        .mesh_pipeline(&MeshDesc {
            label: "testing",
            wgsl: MESH,
            vertex_entry: "mesh_vertex",
            fragment_entry: "colour_fragment",
            vertex: rig::VERTEX,
            reads: &[],
            writes: &[ALBEDO, DEPTH],
            depth_write: false,
            cull_back: true,
        })
        .unwrap_or_else(|e| panic!("{e}"));
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
    // Red, green, blue, white.
    let instances = [
        at([0.0; 3], 0),
        at([0.0; 3], 1),
        at([0.0; 3], 2),
        at([0.0; 3], 3),
    ];
    let input = FrameInput {
        uniform: &uniform,
        instances: &instances,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    let steps = vec![
        Step::Clear(rgba(0.0, 0.0, 0.0, 1.0)),
        // The left half red, then green at the same depth: green, as the
        // test passes on equal depths (it would stay red if only greater
        // ones passed).
        Step::Mesh(writing.clone(), draw_quad(0, 0..1)),
        Step::Mesh(writing.clone(), draw_quad(0, 1..2)),
        // The right half: blue near without writing depth, then white far,
        // which passes against the depth blue left at the far plane.
        Step::Mesh(testing, draw_quad(1, 2..3)),
        Step::Mesh(writing, draw_quad(2, 3..4)),
    ];
    let (_, image) = rig::render(
        &gpu,
        &graph,
        &targets,
        &input,
        &mut [
            Script(SHOW.decl(), vec![Step::Screen(show)]),
            Script(OPAQUE.decl(), steps),
        ],
    );
    // Alpha is the depth, stored linear: 0.5 is 128 (127.5), 0.25 is 64.
    rig::assert_close(at_clip(&image, -0.5, 0.0), [0, 255, 0, 128], "equal depth");
    rig::assert_close(
        at_clip(&image, 0.5, 0.0),
        [255, 255, 255, 64],
        "after no depth write",
    );
}
