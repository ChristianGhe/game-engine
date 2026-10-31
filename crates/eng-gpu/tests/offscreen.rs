//! Offscreen rendering on a real adapter (rung 1b, task T7): the fixed graph
//! without a surface, a clear, a fullscreen pass and readback. Without an
//! adapter each GPU test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes it fail
//! (plan decision 4).

use eng_gpu::{
    Backend, DRAW, DeviceType, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, GpuOptions,
    Graph, GraphError, Image, LinearRgba, Pass, PassDecl, PassRecorder, Resource, TONEMAP, Target,
    Targets,
};

const CLAMP: &str = include_str!("wgsl/clamp.wgsl");
const QUADRANTS: &str = include_str!("wgsl/quadrants.wgsl");
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

/// A draw pass that only clears the scene.
struct Clear(LinearRgba);

impl Pass for Clear {
    fn decl(&self) -> PassDecl {
        DRAW
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        rec.clear(self.0);
    }
}

/// Any pass that draws one fullscreen pipeline.
struct Fullscreen(PassDecl, FullscreenPipeline);

impl Pass for Fullscreen {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        rec.fullscreen(&self.1);
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

fn tonemap(gpu: &Gpu, wgsl: &str, entry: &str) -> Fullscreen {
    Fullscreen(TONEMAP, pipeline(gpu, wgsl, entry, Resource::Output))
}

const BLACK: LinearRgba = LinearRgba {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

#[test]
fn a_cleared_scene_reads_back_tone_mapped_and_srgb_encoded() {
    let Some(gpu) = gpu() else { return };
    // The shot size of plan decision 3.
    let targets = Targets::new(&gpu, 320, 240).unwrap_or_else(|e| panic!("{e}"));
    // Linear and scene-referred. Red is above 1 and blue below 0, so both
    // clamp; green 0.215861 is sRGB 128/255 decoded, half a step from either
    // neighbour.
    let mut draw = Clear(LinearRgba {
        r: 4.0,
        g: 0.215_861,
        b: -1.0,
        a: 1.0,
    });
    let mut map = tonemap(&gpu, CLAMP, "clamp_fragment");
    Graph::fixed(Target::Offscreen)
        .render(&gpu, &targets, &mut [&mut draw, &mut map])
        .unwrap_or_else(|e| panic!("{e}"));
    let image = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((image.width, image.height), (320, 240));
    assert_eq!(image.rgba8.len(), 320 * 240 * 4);
    let (pixels, rest) = image.rgba8.as_chunks::<4>();
    assert!(rest.is_empty());
    assert!(
        pixels.iter().all(|p| *p == [255, 128, 0, 255]),
        "first pixel {:?}, last {:?}",
        pixels.first(),
        pixels.last()
    );
    assert_eq!(image.pixel(319, 239), [255, 128, 0, 255]);
}

#[test]
fn readback_gives_rows_top_first_without_row_padding() {
    let Some(gpu) = gpu() else { return };
    // 100 pixels make a 400-byte row, which a copy pads to 512.
    let targets = Targets::new(&gpu, 100, 60).unwrap_or_else(|e| panic!("{e}"));
    let mut draw = Clear(BLACK);
    let mut pattern = tonemap(&gpu, QUADRANTS, "quadrants_fragment");
    Graph::fixed(Target::Offscreen)
        .render(&gpu, &targets, &mut [&mut draw, &mut pattern])
        .unwrap_or_else(|e| panic!("{e}"));
    let image = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(image.rgba8.len(), 100 * 60 * 4);
    assert_pattern(&image);
}

/// What `quadrants.wgsl` draws: red on the left half, green on the top
/// half, blue on alternate pixels.
fn assert_pattern(image: &Image) {
    let (width, height) = (image.width, image.height);
    for y in 0..height {
        for x in 0..width {
            let on = |yes: bool| if yes { 255 } else { 0 };
            let expected = [
                on(x < width / 2),
                on(y < height / 2),
                on((x + y) % 2 == 1),
                255,
            ];
            assert_eq!(image.pixel(x, y), expected, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn a_fullscreen_pass_samples_its_input_texel_for_texel() {
    let Some(gpu) = gpu() else { return };
    // An even size, so that no pixel centre lies on the pattern's halves.
    let targets = Targets::new(&gpu, 90, 50).unwrap_or_else(|e| panic!("{e}"));
    // The pattern goes into the output, is copied back into the scene by
    // sampling, then tone mapped into the output again.
    const PATTERN: PassDecl = PassDecl {
        name: "pattern",
        reads: &[Resource::Scene],
        writes: &[Resource::Output],
    };
    const COPY: PassDecl = PassDecl {
        name: "copy",
        reads: &[Resource::Output],
        writes: &[Resource::Scene],
    };
    let graph = Graph::new(&[DRAW, PATTERN, COPY, TONEMAP], Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"));
    let mut draw = Clear(BLACK);
    let mut pattern = Fullscreen(
        PATTERN,
        pipeline(&gpu, QUADRANTS, "quadrants_fragment", Resource::Output),
    );
    let mut copy = Fullscreen(
        COPY,
        pipeline(&gpu, COPY_WGSL, "copy_fragment", Resource::Scene),
    );
    let mut map = tonemap(&gpu, CLAMP, "clamp_fragment");
    graph
        .render(
            &gpu,
            &targets,
            &mut [&mut draw, &mut pattern, &mut copy, &mut map],
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let image = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_pattern(&image);
}

#[test]
fn the_report_describes_the_adapter_at_tier_0() {
    let Some(gpu) = gpu() else { return };
    let report = gpu.report();
    eprintln!("{report:?}");
    assert_eq!(report.tier, 0);
    assert!(!report.adapter.is_empty());
    let native = if cfg!(target_os = "macos") {
        Backend::Metal
    } else if cfg!(windows) {
        Backend::Dx12
    } else {
        Backend::Vulkan
    };
    assert_eq!(report.backend, native);
    if report.adapter.contains("llvmpipe") {
        assert_eq!(report.device_type, DeviceType::Cpu);
    }
    if let Some(wanted) = GpuOptions::from_env().adapter {
        let name = report.adapter.to_lowercase();
        assert!(name.contains(&wanted.to_lowercase()), "{name} for {wanted}");
    }
    assert_eq!(Backend::Metal.as_str(), "metal");
    assert_eq!(DeviceType::DiscreteGpu.as_str(), "discrete-gpu");
}

#[test]
fn an_adapter_name_that_matches_nothing_fails_with_e_no_adapter() {
    let options = GpuOptions {
        adapter: Some("no adapter is called this".into()),
    };
    match Gpu::new(&options) {
        Err(e @ GpuError::NoAdapter { .. }) => {
            let text = e.to_string();
            assert!(text.starts_with("[E-NO-ADAPTER] "), "{text}");
            assert!(text.contains("\"no adapter is called this\""), "{text}");
        }
        Err(e) => panic!("{e}"),
        Ok(gpu) => panic!("picked {}", gpu.report().adapter),
    }
}

#[test]
fn a_broken_shader_is_an_error_not_a_panic() {
    let Some(gpu) = gpu() else { return };
    let broken = gpu.fullscreen_pipeline(&FullscreenDesc {
        label: "broken",
        wgsl: "@fragment fn broken( {",
        entry: "broken",
        target: Resource::Output,
    });
    match broken {
        Err(e @ GpuError::Shader { .. }) => {
            assert!(e.to_string().starts_with("[E-SHADER] broken: "), "{e}");
        }
        Err(e) => panic!("{e}"),
        Ok(_) => panic!("a broken shader compiled"),
    }
    let missing = gpu.fullscreen_pipeline(&FullscreenDesc {
        label: "missing entry",
        wgsl: CLAMP,
        entry: "no_such_entry",
        target: Resource::Output,
    });
    assert!(
        matches!(missing, Err(GpuError::Shader { .. })),
        "{:?}",
        missing.err()
    );
}

#[test]
fn passes_must_match_the_graph() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::new(&gpu, 8, 8).unwrap_or_else(|e| panic!("{e}"));
    let graph = Graph::fixed(Target::Offscreen);
    let mut draw = Clear(BLACK);
    let mut map = tonemap(&gpu, CLAMP, "clamp_fragment");
    assert_eq!(
        graph.render(&gpu, &targets, &mut [&mut map, &mut draw]),
        Err(GpuError::Graph(GraphError::PassMismatch {
            index: 0,
            expected: DRAW,
            found: TONEMAP,
        }))
    );
    assert_eq!(
        graph.render(&gpu, &targets, &mut [&mut draw]),
        Err(GpuError::Graph(GraphError::PassCount {
            expected: 2,
            found: 1,
        }))
    );
}

#[test]
fn a_fullscreen_pass_needs_one_input_and_a_pipeline_for_its_target() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::new(&gpu, 8, 8).unwrap_or_else(|e| panic!("{e}"));
    let graph = Graph::fixed(Target::Offscreen);
    let mut map = tonemap(&gpu, CLAMP, "clamp_fragment");
    // The draw pass reads nothing, so it has no input to draw from.
    let mut no_input = Fullscreen(
        DRAW,
        pipeline(&gpu, CLAMP, "clamp_fragment", Resource::Scene),
    );
    match graph.render(&gpu, &targets, &mut [&mut no_input, &mut map]) {
        Err(GpuError::Pass { pass: "draw", .. }) => {}
        other => panic!("{other:?}"),
    }
    // A pipeline made for the scene's format cannot draw into the output.
    let mut draw = Clear(BLACK);
    let mut wrong = Fullscreen(
        TONEMAP,
        pipeline(&gpu, CLAMP, "clamp_fragment", Resource::Scene),
    );
    match graph.render(&gpu, &targets, &mut [&mut draw, &mut wrong]) {
        Err(
            e @ GpuError::Pass {
                pass: "tonemap", ..
            },
        ) => {
            assert!(e.to_string().starts_with("[E-GPU] pass `tonemap`: "), "{e}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn targets_have_a_tier_0_size() {
    let Some(gpu) = gpu() else { return };
    for (width, height) in [(0, 10), (10, 0), (8193, 1), (1, 8193)] {
        assert!(
            matches!(
                Targets::new(&gpu, width, height),
                Err(GpuError::Size { .. })
            ),
            "{width}x{height}"
        );
    }
    let targets = Targets::new(&gpu, 8192, 1).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(targets.size(), (8192, 1));
}
