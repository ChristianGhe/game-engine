//! What the tests of rung 2's scene layout share (T8): an adapter or a
//! skip, a pass that records a script, scenes of quads, and pixels.
// Each test file uses some of these.
#![allow(dead_code)]

use eng_gpu::{
    Attribute, FrameInput, FullscreenPipeline, Gpu, GpuError, GpuOptions, Graph, Image,
    InstanceRecord, LinearRgba, MeshDesc, MeshDraw, MeshPipeline, Pass, PassDecl, PassRecorder,
    Resource, Scene, SceneDesc, ScreenDesc, ScreenPipeline, Targets,
};

/// An adapter, or `None` to skip, unless `ENG_REQUIRE_GPU=1` (CI) makes a
/// missing one fail.
pub fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(gpu) => Some(gpu),
        Err(e @ GpuError::NoAdapter { .. }) if !eng_gpu::gpu_required() => {
            eprintln!("skipped: {e}");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

pub fn targets(gpu: &Gpu, width: u32, height: u32) -> Targets {
    Targets::new(gpu, width, height).unwrap_or_else(|e| panic!("{e}"))
}

pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> LinearRgba {
    LinearRgba { r, g, b, a }
}

/// What a scripted pass records, in order.
#[derive(Clone)]
pub enum Step {
    Clear(LinearRgba),
    Mesh(MeshPipeline, MeshDraw),
    Screen(ScreenPipeline),
    Fullscreen(FullscreenPipeline),
}

/// A pass that records its steps each frame.
pub struct Script(pub PassDecl, pub Vec<Step>);

impl Pass for Script {
    fn decl(&self) -> PassDecl {
        self.0
    }
    fn record(&mut self, rec: &mut PassRecorder) {
        for step in &self.1 {
            match step {
                Step::Clear(colour) => rec.clear(*colour),
                Step::Mesh(pipeline, draw) => rec.mesh(pipeline, draw.clone()),
                Step::Screen(pipeline) => rec.screen(pipeline),
                Step::Fullscreen(pipeline) => rec.fullscreen(pipeline),
            }
        }
    }
}

/// Renders `passes` through `graph` with `input`, and returns the error.
pub fn render_err(
    gpu: &Gpu,
    graph: &Graph,
    targets: &Targets,
    input: &FrameInput<'_>,
    passes: &mut [Script],
) -> String {
    let mut passes: Vec<&mut dyn Pass> = passes.iter_mut().map(|p| p as &mut dyn Pass).collect();
    graph
        .render_frame(gpu, targets, input, &mut passes)
        .expect_err("the frame should fail")
        .to_string()
}

/// Renders `passes` through `graph` with `input`, and returns the output.
pub fn render(
    gpu: &Gpu,
    graph: &Graph,
    targets: &Targets,
    input: &FrameInput<'_>,
    passes: &mut [Script],
) -> (eng_gpu::FrameCounters, Image) {
    let mut passes: Vec<&mut dyn Pass> = passes.iter_mut().map(|p| p as &mut dyn Pass).collect();
    let counters = graph
        .render_frame(gpu, targets, input, &mut passes)
        .unwrap_or_else(|e| panic!("{e}"));
    let image = targets.read_output(gpu).unwrap_or_else(|e| panic!("{e}"));
    (counters, image)
}

/// The vertex of the quads below: a position relative to its chunk's
/// origin, and texture coordinates.
pub const VERTEX: &[Attribute] = &[Attribute::F32x3, Attribute::F32x2];

/// A quad's four vertices from (x0, y0) to (x1, y1) at height `z`, its
/// coordinates from (0, 0) to (`uv`, `uv`), as vertex bytes.
pub fn quad(x0: f32, y0: f32, x1: f32, y1: f32, z: f32, uv: f32) -> Vec<u8> {
    [
        [x0, y0, z, 0.0, uv],
        [x1, y0, z, uv, uv],
        [x1, y1, z, uv, 0.0],
        [x0, y1, z, 0.0, 0.0],
    ]
    .iter()
    .flatten()
    .flat_map(|v| v.to_le_bytes())
    .collect()
}

/// Two counter-clockwise triangles over a quad's four vertices.
pub const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

/// A scene of `quads`, each its own four vertices and six indices counted
/// from its first vertex, so that quad i is drawn with base vertex `4 * i`.
pub fn scene_of(gpu: &Gpu, quads: &[Vec<u8>], desc: SceneDesc<'_>) -> Scene {
    let vertices: Vec<u8> = quads.concat();
    let indices: Vec<u32> = quads.iter().flat_map(|_| QUAD_INDICES).collect();
    gpu.scene(&SceneDesc {
        vertices: &vertices,
        indices: &indices,
        ..desc
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// The draw of quad `index` of [`scene_of`], once for each record in
/// `instances`.
pub fn draw_quad(index: u32, instances: std::ops::Range<u32>) -> MeshDraw {
    MeshDraw {
        indices: index * 6..index * 6 + 6,
        base_vertex: i32::try_from(index * 4).expect("a small scene"),
        instances,
    }
}

/// A record at `offset` from the eye, with `material`.
pub fn at(offset: [f32; 3], material: u32) -> InstanceRecord {
    InstanceRecord { offset, material }
}

/// The uniform of the test shaders: a scale for positions, then a colour.
pub fn uniform(scale: [f32; 4], colour: [f32; 4]) -> Vec<u8> {
    scale
        .iter()
        .chain(&colour)
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

/// The identity scale and a black colour.
pub fn plain_uniform() -> Vec<u8> {
    uniform([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0])
}

pub fn mesh(
    gpu: &Gpu,
    wgsl: &str,
    fragment: &str,
    reads: &[Resource],
    writes: &[Resource],
) -> MeshPipeline {
    gpu.mesh_pipeline(&MeshDesc {
        label: fragment,
        wgsl,
        vertex_entry: "mesh_vertex",
        fragment_entry: fragment,
        vertex: VERTEX,
        reads,
        writes,
        depth_write: true,
        cull_back: true,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

pub fn screen(
    gpu: &Gpu,
    wgsl: &str,
    entry: &str,
    reads: &[Resource],
    writes: &[Resource],
) -> ScreenPipeline {
    gpu.screen_pipeline(&ScreenDesc {
        label: entry,
        wgsl,
        entry,
        reads,
        writes,
        scene: false,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// The pixel of `image` whose centre is at clip-space (`x`, `y`).
pub fn at_clip(image: &Image, x: f32, y: f32) -> [u8; 4] {
    let px = (x + 1.0) / 2.0 * image.width as f32;
    let py = (1.0 - y) / 2.0 * image.height as f32;
    image.pixel(px as u32, py as u32)
}

/// Whether each channel is within one level of `expected`.
pub fn assert_close(found: [u8; 4], expected: [u8; 4], what: &str) {
    let near = found.iter().zip(expected).all(|(&f, e)| f.abs_diff(e) <= 1);
    assert!(near, "{what}: found {found:?}, expected {expected:?}");
}
