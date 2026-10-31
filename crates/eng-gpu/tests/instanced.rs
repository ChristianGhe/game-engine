//! Instanced draws (rung 1b, task T8): a pipeline whose vertex stage reads
//! one instance's attributes per primitive, from bytes a pass records each
//! frame. eng-render's top-down pass draws its shapes this way. Without an
//! adapter each GPU test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes it
//! fail.

use eng_gpu::{
    Attribute, DRAW, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, GpuOptions, Graph, Image,
    InstancedDesc, InstancedPipeline, LinearRgba, Pass, PassDecl, PassRecorder, Resource, TONEMAP,
    Target, Targets,
};

const QUADS: &str = include_str!("wgsl/quads.wgsl");
const CLAMP: &str = include_str!("wgsl/clamp.wgsl");

/// `quads.wgsl`'s instance: centre and half extents in clip space, a
/// colour and a channel mask, 32 bytes.
const QUAD: &[Attribute] = &[
    Attribute::F32x2,
    Attribute::F32x2,
    Attribute::F32x3,
    Attribute::U32,
];

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

fn quads_desc(target: Resource) -> InstancedDesc<'static> {
    InstancedDesc {
        label: "quads",
        wgsl: QUADS,
        vertex_entry: "quad_vertex",
        fragment_entry: "quad_fragment",
        instance: QUAD,
        vertices: 4,
        target,
    }
}

fn quads_pipeline(gpu: &Gpu) -> InstancedPipeline {
    gpu.instanced_pipeline(&quads_desc(Resource::Scene))
        .unwrap_or_else(|e| panic!("{e}"))
}

fn clamp_pipeline(gpu: &Gpu) -> FullscreenPipeline {
    gpu.fullscreen_pipeline(&FullscreenDesc {
        label: "clamp",
        wgsl: CLAMP,
        entry: "clamp_fragment",
        target: Resource::Output,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// One instance as `quads.wgsl` reads it: little-endian, packed.
fn quad(centre: [f32; 2], half: [f32; 2], colour: [f32; 3], mask: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32);
    for value in centre.into_iter().chain(half).chain(colour) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&mask.to_le_bytes());
    bytes
}

/// A draw pass that clears the scene to black and draws its instances.
struct Quads(InstancedPipeline, Vec<u8>);

impl Pass for Quads {
    fn decl(&self) -> PassDecl {
        DRAW
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        rec.clear(LinearRgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
        rec.instanced(&self.0, &self.1);
    }
}

/// Any pass that records one instanced draw for each list of instances.
struct Draws(PassDecl, InstancedPipeline, Vec<Vec<u8>>);

impl Pass for Draws {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        for bytes in &self.2 {
            rec.instanced(&self.1, bytes);
        }
    }
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

fn render(gpu: &Gpu, draw: &mut Quads, width: u32, height: u32) -> Result<Image, GpuError> {
    let targets = Targets::new(gpu, width, height)?;
    let mut map = Fullscreen(TONEMAP, clamp_pipeline(gpu));
    Graph::fixed(Target::Offscreen).render(gpu, &targets, &mut [draw, &mut map])?;
    targets.read_output(gpu)
}

#[test]
fn instances_land_where_their_attributes_say_in_order() {
    let Some(gpu) = gpu() else { return };
    // On 64x32 pixels a clip unit is 32 pixels across and 16 down, so these
    // edges fall on pixel boundaries.
    let mut bytes = Vec::new();
    // Red (the mask drops green): x 8..24, y 4..12.
    bytes.extend(quad([-0.5, 0.5], [0.25, 0.25], [1.0, 1.0, 0.0], 0b001));
    // Cyan (the mask drops red): x 40..56, y 16..32.
    bytes.extend(quad([0.5, -0.5], [0.25, 0.5], [1.0, 1.0, 1.0], 0b110));
    // White, drawn over the red one: x 16..24, y 6..10.
    bytes.extend(quad([-0.375, 0.5], [0.125, 0.125], [1.0, 1.0, 1.0], 0b111));
    let mut draw = Quads(quads_pipeline(&gpu), bytes);
    let image = render(&gpu, &mut draw, 64, 32).unwrap_or_else(|e| panic!("{e}"));
    let inside = |x: u32, y: u32, xs: (u32, u32), ys: (u32, u32)| {
        (xs.0..xs.1).contains(&x) && (ys.0..ys.1).contains(&y)
    };
    for y in 0..32 {
        for x in 0..64 {
            let expected = if inside(x, y, (16, 24), (6, 10)) {
                [255, 255, 255, 255]
            } else if inside(x, y, (8, 24), (4, 12)) {
                [255, 0, 0, 255]
            } else if inside(x, y, (40, 56), (16, 32)) {
                [0, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            };
            assert_eq!(image.pixel(x, y), expected, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn each_draw_reads_its_own_instances() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::new(&gpu, 64, 32).unwrap_or_else(|e| panic!("{e}"));
    let graph = Graph::fixed(Target::Offscreen);
    // The left half red, then the right half green: two draws in one pass.
    let left = quad([-0.5, 0.0], [0.5, 1.0], [1.0, 0.0, 0.0], 0b111);
    let right = quad([0.5, 0.0], [0.5, 1.0], [0.0, 1.0, 0.0], 0b111);
    let mut draw = Draws(DRAW, quads_pipeline(&gpu), vec![left.clone(), right]);
    let mut map = Fullscreen(TONEMAP, clamp_pipeline(&gpu));
    let counters = graph
        .render(&gpu, &targets, &mut [&mut draw, &mut map])
        .unwrap_or_else(|e| panic!("{e}"));
    // Two instances of 32 bytes went up with the frame.
    assert_eq!(counters.upload_bytes, 64);
    let image = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(image.pixel(0, 0), [255, 0, 0, 255]);
    assert_eq!(image.pixel(63, 31), [0, 255, 0, 255]);
    // The scene's draw comes first, then the tone map pass draws a blue top
    // half into the output, from the frame's later bytes.
    let top = quad([0.0, 0.5], [1.0, 0.5], [0.0, 0.0, 1.0], 0b111);
    let output = gpu
        .instanced_pipeline(&quads_desc(Resource::Output))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut draw = Draws(DRAW, quads_pipeline(&gpu), vec![left]);
    let mut overlay = Draws(TONEMAP, output, vec![top]);
    let counters = graph
        .render(&gpu, &targets, &mut [&mut draw, &mut overlay])
        .unwrap_or_else(|e| panic!("{e}"));
    // One instance from each pass.
    assert_eq!(counters.upload_bytes, 64);
    let image = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(image.pixel(0, 0), [0, 0, 255, 255]);
    // Below it, the output starts transparent black: the tone map pass
    // does not clear.
    assert_eq!(image.pixel(0, 31), [0, 0, 0, 0]);
}

#[test]
fn instance_bytes_must_be_whole_instances() {
    let Some(gpu) = gpu() else { return };
    let mut bytes = quad([0.0, 0.0], [0.5, 0.5], [1.0, 1.0, 1.0], 0b111);
    bytes.push(0);
    let mut draw = Quads(quads_pipeline(&gpu), bytes);
    match render(&gpu, &mut draw, 8, 8) {
        Err(e @ GpuError::Pass { pass: "draw", .. }) => {
            assert_eq!(
                e.to_string(),
                "[E-GPU] pass `draw`: 33 bytes of instances are not a whole number of 32-byte instances"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_instanced_pipeline_draws_only_into_its_target() {
    let Some(gpu) = gpu() else { return };
    let for_output = gpu
        .instanced_pipeline(&quads_desc(Resource::Output))
        .unwrap_or_else(|e| panic!("{e}"));
    let bytes = quad([0.0, 0.0], [0.5, 0.5], [1.0, 1.0, 1.0], 0b111);
    let mut draw = Quads(for_output, bytes);
    match render(&gpu, &mut draw, 8, 8) {
        Err(e @ GpuError::Pass { pass: "draw", .. }) => {
            assert_eq!(
                e.to_string(),
                "[E-GPU] pass `draw`: the pipeline draws into `output`, but the pass writes `scene`"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_bad_instanced_pipeline_is_an_error_not_a_panic() {
    let Some(gpu) = gpu() else { return };
    let shader_error = |desc: &InstancedDesc<'_>| match gpu.instanced_pipeline(desc) {
        Err(e @ GpuError::Shader { .. }) => e.to_string(),
        Err(e) => panic!("{e}"),
        Ok(_) => panic!("{} made a pipeline", desc.label),
    };
    let broken = shader_error(&InstancedDesc {
        label: "broken",
        wgsl: "@vertex fn broken( {",
        ..quads_desc(Resource::Scene)
    });
    assert!(broken.starts_with("[E-SHADER] broken: "), "{broken}");
    // The shader reads four attributes; the layout gives three.
    let short = shader_error(&InstancedDesc {
        label: "short",
        instance: &QUAD[..3],
        ..quads_desc(Resource::Scene)
    });
    assert!(short.starts_with("[E-SHADER] short: "), "{short}");
    let none = shader_error(&InstancedDesc {
        label: "none",
        instance: &[],
        ..quads_desc(Resource::Scene)
    });
    assert_eq!(
        none,
        "[E-SHADER] none: an instanced pipeline needs at least one instance attribute"
    );
    let surface = shader_error(&InstancedDesc {
        label: "surface",
        ..quads_desc(Resource::Surface)
    });
    assert_eq!(
        surface,
        "[E-SHADER] surface: instanced pipelines draw into the scene or the output, not `surface`"
    );
}
