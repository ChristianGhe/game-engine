//! Sorted graphs on a real adapter (rung 2, queue item Q1; ADR-0019,
//! proposed): transients drawn into the targets' pool and its textures
//! reused from frame to frame, a continuing pass drawing on top of the
//! target it continues, and culled passes left out. Without an adapter each
//! test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes it fail.

use eng_gpu::{
    Attribute, DRAW, Format, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, GpuOptions, Graph,
    GraphError, Image, InstancedDesc, InstancedPipeline, LinearRgba, PRESENT, Pass, PassDecl,
    PassNode, PassRecorder, Resource, TONEMAP, Target, Targets, Transient,
};

const BRIGHTEN: &str = include_str!("wgsl/brighten.wgsl");
const CLAMP: &str = include_str!("wgsl/clamp.wgsl");
const COPY_WGSL: &str = include_str!("wgsl/copy.wgsl");
const QUADS: &str = include_str!("wgsl/quads.wgsl");

fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(gpu) => Some(gpu),
        Err(e @ GpuError::NoAdapter { .. }) if !eng_gpu::gpu_required() => {
            eprintln!("skipped: {e}");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

fn fullscreen(gpu: &Gpu, wgsl: &str, entry: &str, target: Resource) -> FullscreenPipeline {
    gpu.fullscreen_pipeline(&FullscreenDesc {
        label: entry,
        wgsl,
        entry,
        target,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

fn sorted(nodes: &[PassNode], target: Target) -> Graph {
    Graph::sorted(nodes, target).unwrap_or_else(|e| panic!("{e}"))
}

fn targets(gpu: &Gpu, width: u32, height: u32) -> Targets {
    Targets::new(gpu, width, height).unwrap_or_else(|e| panic!("{e}"))
}

const fn rgba(r: f32, g: f32, b: f32) -> LinearRgba {
    LinearRgba { r, g, b, a: 1.0 }
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

/// A pass that draws `quads.wgsl`'s instances, and clears first if asked.
struct Quads(PassDecl, InstancedPipeline, Vec<u8>, Option<LinearRgba>);

impl Pass for Quads {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        if let Some(colour) = self.3 {
            rec.clear(colour);
        }
        rec.instanced(&self.1, &self.2);
    }
}

/// A pass that counts the frames it recorded and records nothing.
struct Counted(PassDecl, usize);

impl Pass for Counted {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, _: &mut PassRecorder) {
        self.1 += 1;
    }
}

/// Whether every pixel of `image` is `expected`.
fn assert_all(image: &Image, expected: [u8; 4]) {
    let (pixels, _) = image.rgba8.as_chunks::<4>();
    assert_eq!(pixels.len(), (image.width * image.height) as usize);
    assert!(
        pixels.iter().all(|p| *p == expected),
        "expected {expected:?} everywhere; first pixel {:?}, last {:?}",
        pixels.first(),
        pixels.last()
    );
}

const PING: Transient = Transient::new("ping", Format::Rgba16Float);
const PONG: Transient = Transient::new("pong", Format::Rgba16Float);
const PING_AGAIN: Transient = Transient::new("ping-again", Format::Rgba16Float);

/// A chain of four passes over ping-pong transients: a clear into ping, two
/// steps that each add a quarter, then a copy into the output.
const START: PassNode = PassNode {
    name: "start",
    reads: &[],
    writes: &[Resource::Transient(PING)],
    continues: None,
    bypassable: false,
};
const STEP_1: PassNode = PassNode {
    name: "step-1",
    reads: &[Resource::Transient(PING)],
    writes: &[Resource::Transient(PONG)],
    continues: None,
    bypassable: false,
};
const STEP_2: PassNode = PassNode {
    name: "step-2",
    reads: &[Resource::Transient(PONG)],
    writes: &[Resource::Transient(PING_AGAIN)],
    continues: None,
    bypassable: false,
};
const OUT: PassNode = PassNode {
    name: "out",
    reads: &[Resource::Transient(PING_AGAIN)],
    writes: &[Resource::Output],
    continues: None,
    bypassable: false,
};

/// The chain's passes, declared out of order.
struct Chain {
    graph: Graph,
    step_1: FullscreenPipeline,
    step_2: FullscreenPipeline,
    out: FullscreenPipeline,
}

impl Chain {
    fn new(gpu: &Gpu) -> Self {
        let brighten = |into: Transient| {
            fullscreen(
                gpu,
                BRIGHTEN,
                "brighten_fragment",
                Resource::Transient(into),
            )
        };
        Self {
            graph: sorted(&[OUT, STEP_2, START, STEP_1], Target::Offscreen),
            step_1: brighten(PONG),
            step_2: brighten(PING_AGAIN),
            out: fullscreen(gpu, COPY_WGSL, "copy_fragment", Resource::Output),
        }
    }

    /// One frame that starts from `colour`, read back.
    fn frame(&self, gpu: &Gpu, targets: &Targets, colour: LinearRgba) -> Image {
        let mut out = Fullscreen(OUT.decl(), self.out.clone());
        let mut step_2 = Fullscreen(STEP_2.decl(), self.step_2.clone());
        let mut start = Clear(START.decl(), colour);
        let mut step_1 = Fullscreen(STEP_1.decl(), self.step_1.clone());
        self.graph
            .render(
                gpu,
                targets,
                &mut [&mut out, &mut step_2, &mut start, &mut step_1],
            )
            .unwrap_or_else(|e| panic!("{e}"));
        targets.read_output(gpu).unwrap_or_else(|e| panic!("{e}"))
    }
}

#[test]
fn four_passes_over_ping_pong_transients_use_two_textures_kept_across_frames() {
    let Some(gpu) = gpu() else { return };
    let chain = Chain::new(&gpu);
    let names: Vec<&str> = chain.graph.passes().iter().map(|p| p.name).collect();
    assert_eq!(names, ["start", "step-1", "step-2", "out"]);
    assert_eq!(chain.graph.pool().len(), 2);
    let targets = targets(&gpu, 16, 8);
    assert_eq!(targets.transient_textures(), 0);
    // A half plus two quarters is exactly 1 in Rgba16Float, and -0.5 ends
    // at exactly 0: the output's sRGB bytes are 255 and 0.
    let first = chain.frame(&gpu, &targets, rgba(0.5, 0.5, 0.5));
    assert_all(&first, [255, 255, 255, 255]);
    assert_eq!(targets.transient_textures(), 2);
    // The second frame draws into the same two textures, and nothing of
    // the first frame is left in them.
    let second = chain.frame(&gpu, &targets, rgba(0.5, -0.5, 0.5));
    assert_all(&second, [255, 0, 255, 255]);
    assert_eq!(targets.transient_textures(), 2);
}

#[test]
fn another_graph_shares_the_pool_and_a_new_size_makes_new_textures() {
    let Some(gpu) = gpu() else { return };
    let chain = Chain::new(&gpu);
    let small = targets(&gpu, 16, 8);
    assert_all(
        &chain.frame(&gpu, &small, rgba(0.5, 0.5, 0.5)),
        [255, 255, 255, 255],
    );
    assert_eq!(small.transient_textures(), 2);
    // A graph with one transient of the same kind takes the first texture.
    let copy = sorted(
        &[
            START,
            PassNode {
                reads: &[Resource::Transient(PING)],
                ..OUT
            },
        ],
        Target::Offscreen,
    );
    assert_eq!(copy.pool().len(), 1);
    let mut start = Clear(START.decl(), rgba(0.0, 1.0, 0.0));
    let mut out = Fullscreen(
        copy.passes()[1],
        fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Output),
    );
    copy.render(&gpu, &small, &mut [&mut start, &mut out])
        .unwrap_or_else(|e| panic!("{e}"));
    assert_all(
        &small.read_output(&gpu).unwrap_or_else(|e| panic!("{e}")),
        [0, 255, 0, 255],
    );
    assert_eq!(small.transient_textures(), 2);
    // A resize makes new targets, whose pool starts empty.
    let large = targets(&gpu, 32, 16);
    assert_eq!(large.transient_textures(), 0);
    let image = chain.frame(&gpu, &large, rgba(0.5, -0.5, 0.5));
    assert_eq!((image.width, image.height), (32, 16));
    assert_all(&image, [255, 0, 255, 255]);
    assert_eq!(large.transient_textures(), 2);
    assert_eq!(small.transient_textures(), 2);
}

const LAYER: Transient = Transient::new("layer", Format::Rgba16Float);
const BASE: PassNode = PassNode {
    name: "base",
    reads: &[],
    writes: &[Resource::Transient(LAYER)],
    continues: None,
    bypassable: false,
};
// "a-over" sorts before "base" by name, but runs after it.
const OVER: PassNode = PassNode {
    name: "a-over",
    reads: &[],
    writes: &[Resource::Transient(LAYER)],
    continues: Some("base"),
    bypassable: false,
};
const SHOW: PassNode = PassNode {
    name: "show",
    reads: &[Resource::Transient(LAYER)],
    writes: &[Resource::Output],
    continues: None,
    bypassable: false,
};

/// A quad over the left half, in green.
fn left_half_green() -> Vec<u8> {
    [-0.5f32, 0.0, 0.5, 1.0, 0.0, 1.0, 0.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .chain(7u32.to_le_bytes())
        .collect()
}

fn quads_into_layer(gpu: &Gpu) -> InstancedPipeline {
    gpu.instanced_pipeline(&InstancedDesc {
        label: "quads",
        wgsl: QUADS,
        vertex_entry: "quad_vertex",
        fragment_entry: "quad_fragment",
        instance: &[
            Attribute::F32x2,
            Attribute::F32x2,
            Attribute::F32x3,
            Attribute::U32,
        ],
        vertices: 4,
        target: Resource::Transient(LAYER),
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_continuing_pass_draws_on_top_of_the_target_it_continues() {
    let Some(gpu) = gpu() else { return };
    let graph = sorted(&[SHOW, OVER, BASE], Target::Offscreen);
    let names: Vec<&str> = graph.passes().iter().map(|p| p.name).collect();
    assert_eq!(names, ["base", "a-over", "show"]);
    let targets = targets(&gpu, 16, 8);
    let mut show = Fullscreen(
        SHOW.decl(),
        fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Output),
    );
    let mut over = Quads(OVER.decl(), quads_into_layer(&gpu), left_half_green(), None);
    let mut base = Clear(BASE.decl(), rgba(1.0, 0.0, 0.0));
    graph
        .render(&gpu, &targets, &mut [&mut show, &mut over, &mut base])
        .unwrap_or_else(|e| panic!("{e}"));
    let image = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
    // Had the quads' pass cleared, the right half would be transparent.
    for y in 0..8 {
        for x in 0..16 {
            let expected = if x < 8 {
                [0, 255, 0, 255]
            } else {
                [255, 0, 0, 255]
            };
            assert_eq!(image.pixel(x, y), expected, "pixel ({x}, {y})");
        }
    }
    assert_eq!(targets.transient_textures(), 1);
}

#[test]
fn a_continuing_pass_cannot_clear() {
    let Some(gpu) = gpu() else { return };
    let graph = sorted(&[SHOW, OVER, BASE], Target::Offscreen);
    let targets = targets(&gpu, 16, 8);
    let mut show = Fullscreen(
        SHOW.decl(),
        fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Output),
    );
    let mut over = Quads(
        OVER.decl(),
        quads_into_layer(&gpu),
        left_half_green(),
        Some(rgba(0.0, 0.0, 1.0)),
    );
    let mut base = Clear(BASE.decl(), rgba(1.0, 0.0, 0.0));
    let e = graph
        .render(&gpu, &targets, &mut [&mut show, &mut over, &mut base])
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "[E-GPU] pass `a-over`: the pass continues an earlier pass's target, so it draws on top \
         of it and cannot clear it"
    );
}

#[test]
fn a_culled_pass_is_checked_but_never_records() {
    let Some(gpu) = gpu() else { return };
    const DEBUG: Transient = Transient::new("debug", Format::Rgba16Float);
    const UNUSED: PassDecl = PassDecl {
        name: "unused",
        reads: &[Resource::Scene],
        writes: &[Resource::Transient(DEBUG)],
    };
    let graph = sorted(
        &[UNUSED.node(), TONEMAP.node(), DRAW.node()],
        Target::Offscreen,
    );
    assert_eq!(graph.culled(), ["unused"]);
    let targets = targets(&gpu, 16, 8);
    let clamp = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let mut unused = Counted(UNUSED, 0);
    let mut tonemap = Fullscreen(TONEMAP, clamp.clone());
    let mut draw = Clear(DRAW, rgba(0.0, 0.0, 1.0));
    graph
        .render(&gpu, &targets, &mut [&mut unused, &mut tonemap, &mut draw])
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(unused.1, 0);
    assert_eq!(targets.transient_textures(), 0);
    assert_all(
        &targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}")),
        [0, 0, 255, 255],
    );
    // Its place is still checked, and counted.
    let mut impostor = Counted(
        PassDecl {
            name: "x",
            ..UNUSED
        },
        0,
    );
    let e = graph
        .render(
            &gpu,
            &targets,
            &mut [&mut impostor, &mut tonemap, &mut draw],
        )
        .unwrap_err();
    assert_eq!(
        e,
        GpuError::Graph(GraphError::PassMismatch {
            index: 0,
            expected: UNUSED,
            found: impostor.0,
        })
    );
    let e = graph
        .render(&gpu, &targets, &mut [&mut tonemap, &mut draw])
        .unwrap_err();
    assert_eq!(
        e,
        GpuError::Graph(GraphError::PassCount {
            expected: 3,
            found: 2,
        })
    );
}

#[test]
fn rung_1b_passes_sorted_render_what_the_fixed_graph_renders() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::with_offscreen_surface(&gpu, 16, 8).unwrap_or_else(|e| panic!("{e}"));
    let clamp = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let copy = fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Surface);
    let mut draw = Clear(DRAW, rgba(2.0, 0.215_861, -1.0));
    let mut tonemap = Fullscreen(TONEMAP, clamp);
    let mut present = Fullscreen(PRESENT, copy);

    let fixed = Graph::fixed(Target::Surface)
        .render(&gpu, &targets, &mut [&mut draw, &mut tonemap, &mut present])
        .unwrap_or_else(|e| panic!("{e}"));
    let fixed_image = targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}"));

    let graph = sorted(
        &[PRESENT.node(), DRAW.node(), TONEMAP.node()],
        Target::Surface,
    );
    let shuffled = graph
        .render(&gpu, &targets, &mut [&mut present, &mut draw, &mut tonemap])
        .unwrap_or_else(|e| panic!("{e}"));
    let shuffled_image = targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_all(&fixed_image, [255, 128, 0, 255]);
    assert_eq!(shuffled_image, fixed_image);
    assert_eq!(shuffled, fixed);
}
