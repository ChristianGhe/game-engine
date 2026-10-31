//! The per-frame counters (spec 6.13; rung 1b plan, acceptance feature
//! `gpu-report-budgets`): [`Graph::render`] counts what the frame asks of the
//! GPU, its draws, pipeline and bind-group switches and uploaded bytes,
//! from the commands it encodes, so the counts are exact and the same on
//! every adapter. From task T9, `budgets.toml [gpu]` holds them.
//!
//! The passes here are made up for the test, so each expected count comes
//! from what the test records. Without an adapter each test skips, unless
//! `ENG_REQUIRE_GPU=1` (CI) makes it fail.

use eng_gpu::{
    Attribute, DRAW, FrameCounters, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, GpuOptions,
    Graph, InstancedDesc, InstancedPipeline, LinearRgba, Pass, PassDecl, PassRecorder, Resource,
    TONEMAP, Target, Targets,
};

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

/// A pipeline drawing `quads.wgsl`'s 32-byte instances into the scene.
/// Each call makes another pipeline.
fn quads(gpu: &Gpu) -> InstancedPipeline {
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
        target: Resource::Scene,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// `count` instances of `quads.wgsl`: small squares, all the same.
fn instances(count: usize) -> Vec<u8> {
    let one: Vec<u8> = [0.0f32, 0.0, 0.25, 0.25, 1.0, 1.0, 1.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .chain(7u32.to_le_bytes())
        .collect();
    assert_eq!(one.len(), 32);
    one.repeat(count)
}

/// What a test pass records, in order.
#[derive(Clone)]
enum Step {
    Clear,
    Fullscreen(FullscreenPipeline),
    Instanced(InstancedPipeline, Vec<u8>),
}

/// A pass that records its steps.
struct Script(PassDecl, Vec<Step>);

impl Pass for Script {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        for step in &self.1 {
            match step {
                Step::Clear => rec.clear(LinearRgba {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                }),
                Step::Fullscreen(pipeline) => rec.fullscreen(pipeline),
                Step::Instanced(pipeline, bytes) => rec.instanced(pipeline, bytes),
            }
        }
    }
}

fn render(gpu: &Gpu, graph: &Graph, passes: &mut [Script]) -> FrameCounters {
    let targets = Targets::new(gpu, 16, 8).unwrap_or_else(|e| panic!("{e}"));
    let mut passes: Vec<&mut dyn Pass> = passes.iter_mut().map(|p| p as &mut dyn Pass).collect();
    graph
        .render(gpu, &targets, &mut passes)
        .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn clears_count_nothing() {
    let Some(gpu) = gpu() else { return };
    let counters = render(
        &gpu,
        &Graph::fixed(Target::Offscreen),
        &mut [
            Script(DRAW, vec![Step::Clear]),
            Script(TONEMAP, vec![Step::Clear]),
        ],
    );
    assert_eq!(counters, FrameCounters::default());
}

#[test]
fn a_fullscreen_draw_binds_a_pipeline_and_the_pass_input() {
    let Some(gpu) = gpu() else { return };
    let map = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let counters = render(
        &gpu,
        &Graph::fixed(Target::Offscreen),
        &mut [
            Script(DRAW, vec![Step::Clear]),
            Script(TONEMAP, vec![Step::Fullscreen(map)]),
        ],
    );
    assert_eq!(
        counters,
        FrameCounters {
            draws: 1,
            pipeline_switches: 1,
            bind_group_switches: 1,
            upload_bytes: 0,
        }
    );
}

#[test]
fn a_pass_binds_its_input_once_and_a_pipeline_only_when_it_changes() {
    let Some(gpu) = gpu() else { return };
    let a = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let b = fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Output);
    let steps = [&a, &a, &b, &a, &a]
        .into_iter()
        .map(|p| Step::Fullscreen(p.clone()))
        .collect();
    let counters = render(
        &gpu,
        &Graph::fixed(Target::Offscreen),
        &mut [Script(DRAW, vec![Step::Clear]), Script(TONEMAP, steps)],
    );
    // a, b, a: three switches for five draws, and the one input.
    assert_eq!(
        counters,
        FrameCounters {
            draws: 5,
            pipeline_switches: 3,
            bind_group_switches: 1,
            upload_bytes: 0,
        }
    );
}

#[test]
fn each_pass_binds_its_own_pipeline_and_input() {
    let Some(gpu) = gpu() else { return };
    let map = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    // Two passes that draw the same pipeline: nothing stays bound from one
    // pass to the next.
    const AGAIN: PassDecl = PassDecl {
        name: "again",
        reads: &[Resource::Scene],
        writes: &[Resource::Output],
    };
    let graph =
        Graph::new(&[DRAW, TONEMAP, AGAIN], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let counters = render(
        &gpu,
        &graph,
        &mut [
            Script(DRAW, vec![Step::Clear]),
            Script(TONEMAP, vec![Step::Fullscreen(map.clone())]),
            Script(AGAIN, vec![Step::Fullscreen(map)]),
        ],
    );
    assert_eq!(
        counters,
        FrameCounters {
            draws: 2,
            pipeline_switches: 2,
            bind_group_switches: 2,
            upload_bytes: 0,
        }
    );
}

#[test]
fn counters_belong_to_one_frame() {
    let Some(gpu) = gpu() else { return };
    let map = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let targets = Targets::new(&gpu, 16, 8).unwrap_or_else(|e| panic!("{e}"));
    let graph = Graph::fixed(Target::Offscreen);
    let mut draw = Script(DRAW, vec![Step::Clear]);
    let mut tonemap = Script(TONEMAP, vec![Step::Fullscreen(map)]);
    let frames: Vec<FrameCounters> = (0..3)
        .map(|_| {
            graph
                .render(&gpu, &targets, &mut [&mut draw, &mut tonemap])
                .unwrap_or_else(|e| panic!("{e}"))
        })
        .collect();
    let one = FrameCounters {
        draws: 1,
        pipeline_switches: 1,
        bind_group_switches: 1,
        upload_bytes: 0,
    };
    assert_eq!(frames, [one, one, one]);
}

#[test]
fn uploads_count_every_instance_byte_and_switches_follow_the_pipelines() {
    let Some(gpu) = gpu() else { return };
    let (a, b) = (quads(&gpu), quads(&gpu));
    let map = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let draw = vec![
        Step::Clear,
        Step::Instanced(a.clone(), instances(3)),
        Step::Instanced(a.clone(), instances(1)),
        Step::Instanced(b, instances(2)),
        Step::Instanced(a, instances(1)),
    ];
    let counters = render(
        &gpu,
        &Graph::fixed(Target::Offscreen),
        &mut [
            Script(DRAW, draw),
            Script(TONEMAP, vec![Step::Fullscreen(map)]),
        ],
    );
    // Draw: four draws through a, b, a, and 7 instances of 32 bytes.
    // Tone map: one draw, its pipeline and its input.
    assert_eq!(
        counters,
        FrameCounters {
            draws: 5,
            pipeline_switches: 4,
            bind_group_switches: 1,
            upload_bytes: 224,
        }
    );
}

#[test]
fn an_empty_instance_list_draws_nothing() {
    let Some(gpu) = gpu() else { return };
    let (a, b) = (quads(&gpu), quads(&gpu));
    let map = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let draw = vec![
        Step::Clear,
        Step::Instanced(a, instances(1)),
        Step::Instanced(b, Vec::new()),
    ];
    let counters = render(
        &gpu,
        &Graph::fixed(Target::Offscreen),
        &mut [
            Script(DRAW, draw),
            Script(TONEMAP, vec![Step::Fullscreen(map)]),
        ],
    );
    // a's one instance and the tone map; b neither draws nor switches.
    assert_eq!(
        counters,
        FrameCounters {
            draws: 2,
            pipeline_switches: 2,
            bind_group_switches: 1,
            upload_bytes: 32,
        }
    );
}
