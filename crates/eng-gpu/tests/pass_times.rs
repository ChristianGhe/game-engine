//! GPU time per pass on a real adapter (rung 2, task T8; plan O11): the
//! device asks for timestamp queries only when the adapter has them, and
//! timing is opt-in per targets ([`Targets::time_passes`]). Then
//! [`Targets::frame_stats`] gives one time for each pass that ran, in
//! running order; with timing off, without timestamp queries or after a
//! failed frame, `None`. The tier stays 0. Beside the times, the pool's
//! texture count. Without an adapter each test skips, unless
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

/// The names of the passes `graph` runs, in running order.
fn running(graph: &Graph) -> Vec<&'static str> {
    graph.passes().iter().map(|p| p.name).collect()
}

fn render(gpu: &Gpu, graph: &Graph, targets: &Targets, passes: &mut [&mut dyn eng_gpu::Pass]) {
    graph
        .render(gpu, targets, passes)
        .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn each_pass_that_runs_is_timed_once_timing_is_on() {
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
    // Declared tonemap first; run draw first.
    assert_eq!(running(&graph), ["draw", "tonemap"]);
    let targets = rig::targets(&gpu, 16, 8);
    let none = FrameStats {
        pass_times: None,
        transient_textures: 0,
    };
    // Before any frame, nothing has been timed.
    assert_eq!(stats(&gpu, &targets), none);
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
    // Timing starts off: a frame has no times, whatever the adapter.
    render(&gpu, &graph, &targets, &mut dyn_passes);
    assert_eq!(stats(&gpu, &targets), none);
    targets.time_passes(true);
    render(&gpu, &graph, &targets, &mut dyn_passes);
    let first = stats(&gpu, &targets);
    // One time for each pass that ran, named by the pass that ran at that
    // place, in running order.
    assert_eq!(timed(&first), timestamps.then(|| running(&graph)));
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
fn a_later_frame_replaces_the_times_and_a_failed_one_leaves_none() {
    let Some(gpu) = rig::gpu() else { return };
    let timestamps = gpu.report().capabilities.timestamp_queries;
    let targets = Targets::with_offscreen_surface(&gpu, 16, 8).unwrap_or_else(|e| panic!("{e}"));
    targets.time_passes(true);
    let clamp = fullscreen(&gpu, CLAMP, "clamp_fragment", Resource::Output);
    let present = fullscreen(&gpu, COPY_WGSL, "copy_fragment", Resource::Surface);
    let mut draw = Script(DRAW, vec![Step::Clear(rgba(1.0, 0.0, 0.0, 1.0))]);
    let mut tonemap = Script(TONEMAP, vec![Step::Fullscreen(clamp)]);
    let mut blit = Script(eng_gpu::PRESENT, vec![Step::Fullscreen(present)]);
    let (offscreen, surface) = (
        Graph::fixed(Target::Offscreen),
        Graph::fixed(Target::Surface),
    );
    let expect = |graph: &Graph| timestamps.then(|| running(graph));
    // Two offscreen frames nobody reads the times of, then a surface frame
    // of more passes than the targets' query set holds.
    for _ in 0..2 {
        render(&gpu, &offscreen, &targets, &mut [&mut draw, &mut tonemap]);
    }
    render(
        &gpu,
        &surface,
        &targets,
        &mut [&mut draw, &mut tonemap, &mut blit],
    );
    assert_eq!(timed(&stats(&gpu, &targets)), expect(&surface));
    let image = targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(image.pixel(0, 0), [255, 0, 0, 255]);
    // Fewer passes again, each frame read, through the same queries.
    for _ in 0..2 {
        render(&gpu, &offscreen, &targets, &mut [&mut draw, &mut tonemap]);
        assert_eq!(timed(&stats(&gpu, &targets)), expect(&offscreen));
    }
    // A frame that fails leaves none of the last frame's times.
    assert!(offscreen.render(&gpu, &targets, &mut [&mut draw]).is_err());
    assert_eq!(stats(&gpu, &targets).pass_times, None);
    // Off again: frames are not timed, until timing is back on.
    targets.time_passes(false);
    render(&gpu, &offscreen, &targets, &mut [&mut draw, &mut tonemap]);
    assert_eq!(stats(&gpu, &targets).pass_times, None);
    targets.time_passes(true);
    render(&gpu, &offscreen, &targets, &mut [&mut draw, &mut tonemap]);
    assert_eq!(timed(&stats(&gpu, &targets)), expect(&offscreen));
}
