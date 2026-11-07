//! Reading transients back on a real adapter (rung 2, task T9):
//! `Targets::read_transient` gives the texels a graph's last frame left in
//! a depth, an integer and a float transient, as stored, and refuses a
//! transient it cannot give truly. Without an adapter each test skips,
//! unless `ENG_REQUIRE_GPU=1` (CI) makes it fail.

mod rig;

use eng_gpu::{
    Format, FrameInput, FullscreenDesc, FullscreenPipeline, Graph, LinearRgba, Pass, PassDecl,
    PassNode, PassRecorder, Resource, SceneDesc, Target, Targets, Transient,
};
use rig::{Script, Step, at, draw_quad, plain_uniform, quad, rgba, scene_of};

const MESH: &str = include_str!("wgsl/mesh.wgsl");
const VERIFY: &str = include_str!("wgsl/screen_verify.wgsl");
const BRIGHTEN: &str = include_str!("wgsl/brighten.wgsl");
const COPY_WGSL: &str = include_str!("wgsl/copy.wgsl");

const ALBEDO: Transient = Transient::new("albedo", Format::Rgba16Float);
const INFO: Transient = Transient::new("info", Format::Rg32Uint);
const DEPTH: Transient = Transient::new("depth", Format::Depth32Float);
const B: Transient = Transient::new("b", Format::Rgba8UnormSrgb);
const C: Transient = Transient::new("c", Format::Rgba16Float);

const WRITES: &[Resource] = &[
    Resource::Transient(ALBEDO),
    Resource::Transient(INFO),
    Resource::Transient(B),
    Resource::Transient(C),
    Resource::Transient(DEPTH),
];
const FILL: PassNode = PassNode {
    name: "fill",
    reads: &[],
    writes: WRITES,
    continues: None,
    bypassable: false,
};
const SHOW: PassNode = PassNode {
    name: "show",
    reads: WRITES,
    writes: &[Resource::Output],
    continues: None,
    bypassable: false,
};

/// 1.0 and 0.0 as half floats, little-endian.
const HALF_ONE: [u8; 2] = 0x3C00u16.to_le_bytes();
const HALF_ZERO: [u8; 2] = [0, 0];

/// A frame of `mesh.wgsl`'s `targets_fragment` over the middle half of a
/// 16x16 target, half deep, read by `screen_verify.wgsl` into the output.
fn fill(gpu: &eng_gpu::Gpu) -> (Graph, Targets) {
    let scene = scene_of(
        gpu,
        &[quad(-0.5, -0.5, 0.5, 0.5, 0.5, 1.0)],
        SceneDesc::default(),
    );
    let draw = rig::mesh(gpu, MESH, "targets_fragment", &[], WRITES);
    let show = rig::screen(gpu, VERIFY, "verify_fragment", WRITES, &[Resource::Output]);
    let graph = Graph::sorted(&[SHOW, FILL], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let targets = rig::targets(gpu, 16, 16);
    let uniform = plain_uniform();
    let instances = [at([0.0; 3], 0)];
    let input = FrameInput {
        uniform: &uniform,
        instances: &instances,
        scene: Some(&scene),
        ..FrameInput::default()
    };
    rig::render(
        gpu,
        &graph,
        &targets,
        &input,
        &mut [
            Script(SHOW.decl(), vec![Step::Screen(show)]),
            Script(
                FILL.decl(),
                vec![
                    Step::Clear(rgba(0.0, 0.0, 0.0, 0.0)),
                    Step::Mesh(draw, draw_quad(0, 0..1)),
                ],
            ),
        ],
    );
    (graph, targets)
}

#[test]
fn depth_integers_and_half_floats_read_back_as_drawn() {
    let Some(gpu) = rig::gpu() else { return };
    let (graph, targets) = fill(&gpu);
    let read = |transient| {
        targets
            .read_transient(&gpu, &graph, transient)
            .unwrap_or_else(|e| panic!("{e}"))
    };
    let (depth, info, albedo) = (read(DEPTH), read(INFO), read(ALBEDO));
    for texels in [&depth, &info, &albedo] {
        assert_eq!((texels.width, texels.height), (16, 16));
        let size = texels.format.texel_bytes() as usize;
        assert_eq!(texels.bytes.len(), 16 * 16 * size);
    }
    let red = [HALF_ONE, HALF_ZERO, HALF_ZERO, HALF_ONE].concat();
    for y in 0..16 {
        for x in 0..16 {
            // The quad covers columns and rows 4 to 11.
            let inside = (4..12).contains(&x) && (4..12).contains(&y);
            let (z, ids, colour) = if inside {
                (0.5, [7, 9], red.clone())
            } else {
                (0.0, [0, 0], vec![0; 8])
            };
            assert_eq!(depth.depth(x, y), z, "depth at ({x}, {y})");
            assert_eq!(info.uint2(x, y), ids, "info at ({x}, {y})");
            assert_eq!(albedo.texel(x, y), colour, "albedo at ({x}, {y})");
        }
    }
}

/// A pass that only clears its target.
struct Clear(PassDecl, LinearRgba);

impl Pass for Clear {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        rec.clear(self.1);
    }
}

/// A pass that draws one fullscreen pipeline.
struct Fullscreen(PassDecl, FullscreenPipeline);

impl Pass for Fullscreen {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        rec.fullscreen(&self.1);
    }
}

const PING: Transient = Transient::new("ping", Format::Rgba16Float);
const PONG: Transient = Transient::new("pong", Format::Rgba16Float);
const PING_AGAIN: Transient = Transient::new("ping-again", Format::Rgba16Float);

const fn node(
    name: &'static str,
    reads: &'static [Resource],
    writes: &'static [Resource],
) -> PassNode {
    PassNode {
        name,
        reads,
        writes,
        continues: None,
        bypassable: false,
    }
}

const START: PassNode = node("start", &[], &[Resource::Transient(PING)]);
const STEP_1: PassNode = node(
    "step-1",
    &[Resource::Transient(PING)],
    &[Resource::Transient(PONG)],
);
const STEP_2: PassNode = node(
    "step-2",
    &[Resource::Transient(PONG)],
    &[Resource::Transient(PING_AGAIN)],
);
const OUT: PassNode = node(
    "out",
    &[Resource::Transient(PING_AGAIN)],
    &[Resource::Output],
);

#[test]
fn a_transient_is_read_only_where_its_texture_still_holds_it() {
    let Some(gpu) = rig::gpu() else { return };
    let fullscreen = |wgsl: &str, entry: &str, target: Resource| {
        gpu.fullscreen_pipeline(&FullscreenDesc {
            label: entry,
            wgsl,
            entry,
            target,
        })
        .unwrap_or_else(|e| panic!("{e}"))
    };
    let graph = Graph::sorted(&[START, STEP_1, STEP_2, OUT], Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"));
    // Ping and ping-again live apart, so they share a texture.
    assert_eq!(graph.pool().len(), 2);
    assert_eq!(graph.pool()[0].transients, ["ping", "ping-again"]);
    let targets = rig::targets(&gpu, 8, 4);
    let refusal = |transient| {
        targets
            .read_transient(&gpu, &graph, transient)
            .expect_err("the read should be refused")
            .to_string()
    };
    assert_eq!(
        refusal(PONG),
        "[E-GPU] reading back `pong`: no frame has been rendered into these targets, or the \
         last one failed"
    );
    let mut start = Clear(START.decl(), rgba(0.5, 0.5, 0.5, 1.0));
    let mut step_1 = Fullscreen(
        STEP_1.decl(),
        fullscreen(BRIGHTEN, "brighten_fragment", Resource::Transient(PONG)),
    );
    let mut step_2 = Fullscreen(
        STEP_2.decl(),
        fullscreen(
            BRIGHTEN,
            "brighten_fragment",
            Resource::Transient(PING_AGAIN),
        ),
    );
    let mut out = Fullscreen(
        OUT.decl(),
        fullscreen(COPY_WGSL, "copy_fragment", Resource::Output),
    );
    graph
        .render(
            &gpu,
            &targets,
            &mut [&mut start, &mut step_1, &mut step_2, &mut out],
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let half = |value: u16| {
        let [lo, hi] = value.to_le_bytes();
        [lo, hi, lo, hi, lo, hi, HALF_ONE[0], HALF_ONE[1]]
    };
    let read = |transient| {
        targets
            .read_transient(&gpu, &graph, transient)
            .unwrap_or_else(|e| panic!("{e}"))
    };
    // 0.5 and a quarter, then a quarter more: 0.75 and 1.0, exact in f16.
    let pong = read(PONG);
    let again = read(PING_AGAIN);
    for (x, y) in [(0, 0), (7, 3), (3, 2)] {
        assert_eq!(pong.texel(x, y), half(0x3A00), "pong at ({x}, {y})");
        assert_eq!(again.texel(x, y), half(0x3C00), "ping-again at ({x}, {y})");
    }
    // Ping-again was drawn over ping, so ping is gone.
    assert_eq!(
        refusal(PING),
        "[E-GPU] reading back `ping`: it shares its texture with `ping-again`, which the frame \
         draws after it"
    );
    assert_eq!(
        refusal(Transient::new("nowhere", Format::Rgba16Float)),
        "[E-GPU] reading back `nowhere`: no pass of the graph that runs draws it"
    );
    assert_eq!(
        refusal(Transient::new("pong", Format::Rg32Uint)),
        "[E-GPU] reading back `pong`: the graph draws it as Rgba16Float at 1/1, not Rg32Uint \
         at 1/1"
    );

    // A frame of another graph, which pools otherwise, draws into the same
    // pool: the chain's transients can no longer be told apart.
    let short_out = PassNode {
        reads: &[Resource::Transient(PING)],
        ..OUT
    };
    let short =
        Graph::sorted(&[START, short_out], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let mut copy = Fullscreen(
        short_out.decl(),
        fullscreen(COPY_WGSL, "copy_fragment", Resource::Output),
    );
    short
        .render(&gpu, &targets, &mut [&mut start, &mut copy])
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        refusal(PONG),
        "[E-GPU] reading back `pong`: the last frame rendered into these targets ran a graph \
         that pools its transients otherwise"
    );
    let ping = targets
        .read_transient(&gpu, &short, PING)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        ping.texel(5, 1),
        half(0x3800),
        "0.5 through the short graph"
    );
    // A frame that fails leaves nothing to read.
    let wrong = short.render(&gpu, &targets, &mut [&mut start]);
    assert!(wrong.is_err());
    assert!(
        targets
            .read_transient(&gpu, &short, PING)
            .is_err_and(|e| e.to_string().contains("the last one failed"))
    );
}
