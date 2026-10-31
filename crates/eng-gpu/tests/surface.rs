//! The surface graph offscreen (rung 1b, task T8): an offscreen texture in
//! the surface's format stands in for a window's, so the whole fixed graph,
//! present pass included, renders and reads back without a window. Without
//! an adapter each GPU test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes it
//! fail.

use eng_gpu::{
    DRAW, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, GpuOptions, Graph, PRESENT, Pass,
    PassDecl, PassRecorder, Resource, TONEMAP, Target, Targets,
};

const LEVELS: &str = include_str!("wgsl/levels.wgsl");
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

fn pipeline(gpu: &Gpu, wgsl: &str, entry: &str, target: Resource) -> FullscreenPipeline {
    gpu.fullscreen_pipeline(&FullscreenDesc {
        label: entry,
        wgsl,
        entry,
        target,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// A pass that records nothing: its target starts transparent black.
struct Nothing(PassDecl);

impl Pass for Nothing {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, _: &mut PassRecorder) {}
}

struct Fullscreen(PassDecl, FullscreenPipeline);

impl Pass for Fullscreen {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        rec.fullscreen(&self.1);
    }
}

#[test]
fn the_present_pass_copies_every_level_into_the_surface() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::with_offscreen_surface(&gpu, 256, 4).unwrap_or_else(|e| panic!("{e}"));
    assert!(targets.has_surface());
    let mut draw = Nothing(DRAW);
    let mut levels = Fullscreen(
        TONEMAP,
        pipeline(&gpu, LEVELS, "levels_fragment", Resource::Output),
    );
    let mut present = Fullscreen(
        PRESENT,
        pipeline(&gpu, COPY_WGSL, "copy_fragment", Resource::Surface),
    );
    Graph::fixed(Target::Surface)
        .render(&gpu, &targets, &mut [&mut draw, &mut levels, &mut present])
        .unwrap_or_else(|e| panic!("{e}"));
    let output = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
    let surface = targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}"));
    // The output holds the pattern: red runs from 0 to 255 along a row,
    // green back down, and blue differs from red, so a swap of the two shows.
    assert_eq!(output.pixel(0, 0), [0, 255, 0, 255]);
    assert_eq!(output.pixel(255, 3)[0], 255);
    assert_ne!(output.pixel(100, 0)[0], output.pixel(100, 0)[2]);
    // The surface holds the same bytes in RGBA order, its B8G8R8A8 storage
    // read back as red, green, blue, alpha.
    assert_eq!((surface.width, surface.height), (256, 4));
    if let Some(x) = (0..256).find(|&x| surface.pixel(x, 0) != output.pixel(x, 0)) {
        panic!(
            "column {x}: the surface has {:?}, the output {:?}",
            surface.pixel(x, 0),
            output.pixel(x, 0)
        );
    }
    assert_eq!(surface, output);
}

#[test]
fn only_targets_with_a_surface_take_a_surface_graph() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::new(&gpu, 8, 8).unwrap_or_else(|e| panic!("{e}"));
    assert!(!targets.has_surface());
    let mut draw = Nothing(DRAW);
    let mut map = Nothing(TONEMAP);
    let mut present = Fullscreen(
        PRESENT,
        pipeline(&gpu, COPY_WGSL, "copy_fragment", Resource::Surface),
    );
    let rendered = Graph::fixed(Target::Surface).render(
        &gpu,
        &targets,
        &mut [&mut draw, &mut map, &mut present],
    );
    assert_eq!(rendered, Err(GpuError::NoSurface));
    assert_eq!(targets.read_surface(&gpu), Err(GpuError::NoSurface));
    assert_eq!(
        GpuError::NoSurface.to_string(),
        "[E-GPU] the targets have no surface to present into"
    );
    // An offscreen graph renders into targets with a surface, leaving the
    // surface alone.
    let targets = Targets::with_offscreen_surface(&gpu, 8, 8).unwrap_or_else(|e| panic!("{e}"));
    Graph::fixed(Target::Offscreen)
        .render(&gpu, &targets, &mut [&mut draw, &mut map])
        .unwrap_or_else(|e| panic!("{e}"));
}
