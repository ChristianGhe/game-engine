//! GPU time per pass on a real adapter (rung 2, task T8; plan O11): the
//! device asks for timestamp queries only when the adapter has them, and
//! then [`Targets::frame_stats`] gives one time for each pass that ran, in
//! running order; without them, `None`. The tier stays 0. Beside the times,
//! the pool's texture count. Without an adapter each test skips, unless
//! `ENG_REQUIRE_GPU=1` (CI) makes it fail.

mod rig;

use eng_gpu::{
    DRAW, Format, FrameStats, FullscreenDesc, FullscreenPipeline, Gpu, Graph, PassNode, Resource,
    TONEMAP, Target, Targets, Transient,
};
use rig::{Script, Step, rgba};

const CLAMP: &str = include_str!("wgsl/clamp.wgsl");
const COPY_WGSL: &str = include_str!("wgsl/copy.wgsl");

fn fullscreen(gpu: &Gpu, wgsl: &str, entry: &str, target: Resource) -> FullscreenPipeline {
    gpu.fullscreen_pipeline(&FullscreenDesc {
        label: entry,
        wgsl,
        entry,
        target,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// The names of the passes `stats` has times for, or `None`.
fn timed(stats: &FrameStats) -> Option<Vec<&'static str>> {
    stats
        .pass_times
        .as_ref()
        .map(|times| times.iter().map(|t| t.pass).collect())
}

fn stats(gpu: &Gpu, targets: &Targets) -> FrameStats {
    targets.frame_stats(gpu).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn each_pass_that_runs_is_timed_when_the_adapter_has_timestamps() {
    let Some(gpu) = rig::gpu() else { return };
    let report = gpu.report();
    // Asking for the feature changes neither the tier nor the report.
    assert_eq!(report.tier, 0);
    let timestamps = report.capabilities.timestamp_queries;
    eprintln!(
        "{} ({}): timestamp queries {}",
        report.adapter,
        report.backend.as_str(),
        if timestamps { "on" } else { "unsupported" }
    );
    const GLOW: Resource = Resource::Transient(Transient::new("glow", Format::Rgba16Float));
    // `debug` writes a transient nothing reads: culled, so not timed.
    let nodes = [
        TONEMAP.node(),
        PassNode {
            name: "debug",
            reads: &[Resource::Scene],
            writes: &[GLOW],
            continues: None,
            bypassable: false,
        },
        DRAW.node(),
    ];
    let graph = Graph::sorted(&nodes, Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(graph.culled(), ["debug"]);
    let targets = rig::targets(&gpu, 16, 8);
    // Before any frame, nothing has been timed.
    assert_eq!(
        stats(&gpu, &targets),
        FrameStats {
            pass_times: None,
            transient_textures: 0
        }
    );
    let clamp = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let copy = fullscreen(&gpu, COPY_WGSL, "copy_fragment", GLOW);
    let mut passes = [
        Script(TONEMAP, vec![Step::Fullscreen(clamp)]),
        Script(nodes[1].decl(), vec![Step::Fullscreen(copy)]),
        Script(DRAW, vec![Step::Clear(rgba(0.5, 0.5, 0.5, 1.0))]),
    ];
    let mut dyn_passes: Vec<&mut dyn eng_gpu::Pass> = passes
        .iter_mut()
        .map(|p| p as &mut dyn eng_gpu::Pass)
        .collect();
    graph
        .render(&gpu, &targets, &mut dyn_passes)
        .unwrap_or_else(|e| panic!("{e}"));
    let first = stats(&gpu, &targets);
    let expected = timestamps.then(|| vec!["draw", "tonemap"]);
    assert_eq!(timed(&first), expected);
    // `debug`'s transient is culled with it, so the pool holds none.
    assert_eq!(first.transient_textures, 0);
    // Asked again, the same frame's times.
    assert_eq!(stats(&gpu, &targets), first);
    // The times are measurements: real, so not all 0, and never more than
    // the frame could take.
    eprintln!("pass times: {:?}", first.pass_times);
    if let Some(times) = &first.pass_times {
        assert!(times.iter().any(|t| t.nanoseconds > 0), "{times:?}");
        assert!(
            times.iter().all(|t| t.nanoseconds < 60_000_000_000),
            "{times:?}"
        );
    }
}

#[test]
fn a_later_frame_replaces_the_times() {
    let Some(gpu) = rig::gpu() else { return };
    let timestamps = gpu.report().capabilities.timestamp_queries;
    let targets = Targets::with_offscreen_surface(&gpu, 16, 8).unwrap_or_else(|e| panic!("{e}"));
    let clamp = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let present = fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Surface);
    let mut draw = Script(DRAW, vec![Step::Clear(rgba(1.0, 0.0, 0.0, 1.0))]);
    let mut tonemap = Script(TONEMAP, vec![Step::Fullscreen(clamp)]);
    let mut blit = Script(eng_gpu::PRESENT, vec![Step::Fullscreen(present)]);
    // Two offscreen frames, then a surface frame nobody waits for between.
    for _ in 0..2 {
        Graph::fixed(Target::Offscreen)
            .render(&gpu, &targets, &mut [&mut draw, &mut tonemap])
            .unwrap_or_else(|e| panic!("{e}"));
    }
    Graph::fixed(Target::Surface)
        .render(&gpu, &targets, &mut [&mut draw, &mut tonemap, &mut blit])
        .unwrap_or_else(|e| panic!("{e}"));
    let expected = timestamps.then(|| vec!["draw", "tonemap", "present"]);
    assert_eq!(timed(&stats(&gpu, &targets)), expected);
    let surface = targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(surface.pixel(0, 0), [255, 0, 0, 255]);
}
