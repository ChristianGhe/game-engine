//! Bypassed and several-write passes on a real adapter (rung 2, queue item
//! Q1; ADR-0019): a bypassed pass's readers read its input's
//! texture, a write bypassed into the output draws there, and a pass with
//! several writes, which only the plan takes so far, does not render.
//! Without an adapter each test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes
//! it fail.

use eng_gpu::{
    DRAW, Format, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, GpuOptions, Graph, Image,
    LinearRgba, Pass, PassDecl, PassNode, PassRecorder, Resource, TONEMAP, Target, Targets,
    Transient,
};

const BRIGHTEN: &str = include_str!("wgsl/brighten.wgsl");
const CLAMP: &str = include_str!("wgsl/clamp.wgsl");
const COPY_WGSL: &str = include_str!("wgsl/copy.wgsl");

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

fn assert_all(image: &Image, expected: [u8; 4]) {
    let (pixels, _) = image.rgba8.as_chunks::<4>();
    assert!(
        pixels.iter().all(|p| *p == expected),
        "expected {expected:?} everywhere; first pixel {:?}",
        pixels.first()
    );
}

const PING: Transient = Transient::new("ping", Format::Rgba16Float);
const PONG: Transient = Transient::new("pong", Format::Rgba16Float);
const LAST: Transient = Transient::new("last", Format::Rgba16Float);

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
    bypassable: true,
};
const STEP_2: PassNode = PassNode {
    name: "step-2",
    reads: &[Resource::Transient(PONG)],
    writes: &[Resource::Transient(LAST)],
    continues: None,
    bypassable: false,
};
const OUT: PassNode = PassNode {
    name: "out",
    reads: &[Resource::Transient(LAST)],
    writes: &[Resource::Output],
    continues: None,
    bypassable: false,
};

#[test]
fn a_bypassed_step_hands_its_input_to_the_next() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::new(&gpu, 16, 8).unwrap_or_else(|e| panic!("{e}"));
    let brighten = |into: Transient| {
        fullscreen(
            &gpu,
            BRIGHTEN,
            "brighten_fragment",
            Resource::Transient(into),
        )
    };
    let mut start = Clear(START.decl(), rgba(0.5, 0.5, -0.25));
    let mut step_1 = Fullscreen(STEP_1.decl(), brighten(PONG));
    let mut step_2 = Fullscreen(STEP_2.decl(), brighten(LAST));
    let mut out = Fullscreen(
        OUT.decl(),
        fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Output),
    );
    let nodes = [OUT, STEP_2, STEP_1, START];
    let mut frame = |graph: &Graph| {
        graph
            .render(
                &gpu,
                &targets,
                &mut [&mut out, &mut step_2, &mut step_1, &mut start],
            )
            .unwrap_or_else(|e| panic!("{e}"));
        targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"))
    };
    // Both steps: 0.5 + 0.5 = 1, and -0.25 + 0.5 = 0.25 (sRGB 137).
    let on = Graph::sorted(&nodes, Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    assert_all(&frame(&on), [255, 255, 137, 255]);
    // One step: 0.5 + 0.25 is 0.75 (sRGB 225), and -0.25 + 0.25 is 0.
    let off = Graph::sorted_without(&nodes, &["step-1"], Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(off.bypassed(), ["step-1"]);
    assert_all(&frame(&off), [225, 225, 0, 255]);
}

#[test]
fn a_write_bypassed_into_the_output_is_drawn_there() {
    let Some(gpu) = gpu() else { return };
    const TONED: Resource = Resource::Transient(Transient::new("toned", Format::Rgba8UnormSrgb));
    const TONE: PassNode = PassNode {
        name: "tone",
        reads: &[Resource::Scene],
        writes: &[TONED],
        continues: None,
        bypassable: false,
    };
    const PALETTE: PassNode = PassNode {
        name: "palette",
        reads: &[TONED],
        writes: &[Resource::Output],
        continues: None,
        bypassable: true,
    };
    let graph = Graph::sorted_without(
        &[PALETTE, TONE, DRAW.node()],
        &["palette"],
        Target::Offscreen,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(graph.pool().is_empty());
    let targets = Targets::new(&gpu, 16, 8).unwrap_or_else(|e| panic!("{e}"));
    let mut palette = Fullscreen(
        PALETTE.decl(),
        fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Output),
    );
    let mut tone = Fullscreen(
        TONE.decl(),
        fullscreen(&gpu, CLAMP, "clamp_fragment", TONED),
    );
    let mut draw = Clear(DRAW, rgba(2.0, 0.0, 1.0));
    graph
        .render(&gpu, &targets, &mut [&mut palette, &mut tone, &mut draw])
        .unwrap_or_else(|e| panic!("{e}"));
    assert_all(
        &targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}")),
        [255, 0, 255, 255],
    );
    assert_eq!(targets.transient_textures(), 0);
}

#[test]
fn a_pass_with_several_writes_is_planned_but_not_drawn() {
    let Some(gpu) = gpu() else { return };
    const EXTRA: Resource = Resource::Transient(Transient::new("extra", Format::Rgba16Float));
    const BOTH: PassDecl = PassDecl {
        name: "both",
        reads: &[],
        writes: &[Resource::Scene, EXTRA],
    };
    const LOOK: PassDecl = PassDecl {
        name: "look",
        reads: &[EXTRA],
        writes: &[Resource::Output],
    };
    let graph = Graph::sorted(
        &[BOTH.node(), TONEMAP.node(), LOOK.node()],
        Target::Offscreen,
    );
    // Two writers of the output: the plan refuses it before any GPU work.
    assert!(graph.is_err());
    let graph = Graph::sorted(&[BOTH.node(), TONEMAP.node()], Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"));
    let targets = Targets::new(&gpu, 16, 8).unwrap_or_else(|e| panic!("{e}"));
    let mut both = Clear(BOTH, rgba(1.0, 0.0, 0.0));
    let mut tonemap = Fullscreen(
        TONEMAP,
        fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output),
    );
    let e = graph
        .render(&gpu, &targets, &mut [&mut both, &mut tonemap])
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "[E-GPU] pass `both`: it writes 2 resources, and drawing into more than one target is \
         not built yet (rung 2, T8)"
    );
}
