//! Low internal resolution on a real adapter (rung 2, task T10;
//! ADR-0020): upscaled targets render every pass at the window's size over
//! k, and the present pass draws each texel of the output as k×k pixels of
//! the window's image, centred, the remainder left as the pass cleared it.
//! Without an adapter each test skips, unless `ENG_REQUIRE_GPU=1` (CI)
//! makes it fail.

mod rig;

use eng_gpu::{
    DRAW, FullscreenDesc, FullscreenPipeline, Graph, LinearRgba, PRESENT, Pass, PassDecl,
    PassRecorder, Resource, TONEMAP, Target, Targets,
};

const QUADRANTS: &str = include_str!("wgsl/quadrants.wgsl");
const COPY_WGSL: &str = include_str!("wgsl/copy.wgsl");

/// A pass that clears its target to `self.1`, then draws `self.2` if any.
struct Fill(PassDecl, LinearRgba, Option<FullscreenPipeline>);

impl Pass for Fill {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        rec.clear(self.1);
        if let Some(pipeline) = &self.2 {
            rec.fullscreen(pipeline);
        }
    }
}

fn pipeline(gpu: &eng_gpu::Gpu, wgsl: &str, entry: &str, target: Resource) -> FullscreenPipeline {
    gpu.fullscreen_pipeline(&FullscreenDesc {
        label: entry,
        wgsl,
        entry,
        target,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn each_texel_presents_as_k_by_k_pixels_centred_in_the_window() {
    let Some(gpu) = rig::gpu() else { return };
    // (window, scale, targets, where the image starts)
    let cases = [
        ((36, 20), 4, (9, 5), (0, 0)),
        ((39, 23), 4, (9, 5), (1, 1)),
        ((17, 9), 2, (8, 4), (0, 0)),
        ((16, 8), 1, (16, 8), (0, 0)),
    ];
    let black = rig::rgba(0.0, 0.0, 0.0, 1.0);
    for ((ww, wh), k, (tw, th), (ox, oy)) in cases {
        let targets = Targets::upscaled_with_offscreen_surface(&gpu, ww, wh, k)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(targets.size(), (tw, th));
        assert_eq!(targets.surface_size(), (ww, wh));
        assert_eq!(targets.upscale(), k);
        let mut draw = Fill(DRAW, black, None);
        let mut pattern = Fill(
            TONEMAP,
            black,
            Some(pipeline(
                &gpu,
                QUADRANTS,
                "quadrants_fragment",
                Resource::Output,
            )),
        );
        let mut present = Fill(
            PRESENT,
            black,
            Some(pipeline(
                &gpu,
                COPY_WGSL,
                "copy_fragment",
                Resource::Surface,
            )),
        );
        Graph::fixed(Target::Surface)
            .render(&gpu, &targets, &mut [&mut draw, &mut pattern, &mut present])
            .unwrap_or_else(|e| panic!("{e}"));
        let output = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
        let surface = targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((output.width, output.height), (tw, th));
        assert_eq!((surface.width, surface.height), (ww, wh));
        for y in 0..wh {
            for x in 0..ww {
                let inside = (ox..ox + tw * k).contains(&x) && (oy..oy + th * k).contains(&y);
                let want = if inside {
                    output.pixel((x - ox) / k, (y - oy) / k)
                } else {
                    [0, 0, 0, 255]
                };
                assert_eq!(
                    surface.pixel(x, y),
                    want,
                    "window {ww}x{wh} at {k}x: pixel ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn upscaled_targets_refuse_a_scale_of_0_and_sides_beyond_tier_0() {
    let Some(gpu) = rig::gpu() else { return };
    let message = |result: Result<Targets, eng_gpu::GpuError>| match result {
        Ok(_) => "made".to_string(),
        Err(e) => e.to_string(),
    };
    assert_eq!(
        message(Targets::upscaled(&gpu, 64, 64, 0)),
        "[E-GPU] an upscale of 0: each texel presents as at least 1x1 pixels"
    );
    assert_eq!(
        message(Targets::upscaled(&gpu, 9000, 64, 2)),
        "[E-GPU] target size 9000x64: each side must be 1 to 8192 (Tier 0)"
    );
    // Narrower than the scale: the largest scale that fits.
    let narrow = Targets::upscaled(&gpu, 3, 40, 4).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        (narrow.size(), narrow.surface_size(), narrow.upscale()),
        ((1, 13), (3, 40), 3)
    );
    // 1:1 targets present into their own size.
    let plain = Targets::new(&gpu, 5, 7).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((plain.surface_size(), plain.upscale()), ((5, 7), 1));
}
