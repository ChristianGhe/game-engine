//! The renderer: rung 1b's three passes over eng-gpu's fixed graph.

use eng_gpu::{
    Attribute, DRAW, FrameCounters, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, Graph,
    InstancedDesc, InstancedPipeline, LinearRgba, PRESENT, Pass, PassDecl, PassRecorder, Resource,
    TONEMAP, Target, Targets,
};

use crate::{LinearRgb, Quad, RenderError, TopDownScene};

const TOP_DOWN_WGSL: &str = include_str!("top_down.wgsl");
const TONEMAP_WGSL: &str = include_str!("tonemap.wgsl");
const PRESENT_WGSL: &str = include_str!("present.wgsl");

/// The top-down pass's instance, one [`Quad`]: centre and half extents,
/// colour, and 1 for a disc or 0 for a rectangle. 32 bytes.
const INSTANCE: &[Attribute] = &[
    Attribute::F32x2,
    Attribute::F32x2,
    Attribute::F32x3,
    Attribute::U32,
];

/// Draws top-down scenes: the top-down pass into the scene, the tone map
/// into the output, and, for targets with a surface, the present blit.
/// Building one compiles its pipelines; each frame then costs one call to
/// [`Renderer::render`].
pub struct Renderer {
    top_down: InstancedPipeline,
    tonemap: FullscreenPipeline,
    present: FullscreenPipeline,
    offscreen: Graph,
    surface: Graph,
}

impl Renderer {
    /// Compiles the three passes' pipelines on `gpu`.
    pub fn new(gpu: &Gpu) -> Result<Self, GpuError> {
        let fullscreen = |label: &str, wgsl: &str, entry: &str, target: Resource| {
            gpu.fullscreen_pipeline(&FullscreenDesc {
                label,
                wgsl,
                entry,
                target,
            })
        };
        Ok(Self {
            top_down: gpu.instanced_pipeline(&InstancedDesc {
                label: "top-down",
                wgsl: TOP_DOWN_WGSL,
                vertex_entry: "top_down_vertex",
                fragment_entry: "top_down_fragment",
                instance: INSTANCE,
                vertices: 4,
                target: Resource::Scene,
            })?,
            tonemap: fullscreen(
                "tonemap",
                TONEMAP_WGSL,
                "tonemap_fragment",
                Resource::Output,
            )?,
            present: fullscreen(
                "present",
                PRESENT_WGSL,
                "present_fragment",
                Resource::Surface,
            )?,
            offscreen: Graph::fixed(Target::Offscreen),
            surface: Graph::fixed(Target::Surface),
        })
    }

    /// Renders one frame of `scene` into `targets`, presenting it too if
    /// they have a surface, and returns the frame's counters. A scene that
    /// cannot be drawn fails before any GPU work, leaving the targets as
    /// they were.
    pub fn render(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        scene: &TopDownScene,
    ) -> Result<FrameCounters, RenderError> {
        let (width, height) = targets.size();
        let quads = scene.quads(width, height)?;
        let mut draw = TopDown {
            pipeline: &self.top_down,
            background: scene.background,
            instances: instances(&quads),
        };
        let mut tonemap = Fullscreen(TONEMAP, &self.tonemap);
        let counters = if targets.has_surface() {
            let mut present = Fullscreen(PRESENT, &self.present);
            self.surface
                .render(gpu, targets, &mut [&mut draw, &mut tonemap, &mut present])?
        } else {
            self.offscreen
                .render(gpu, targets, &mut [&mut draw, &mut tonemap])?
        };
        Ok(counters)
    }
}

/// The quads as the top-down pipeline reads them: [`INSTANCE`]'s layout,
/// little-endian, as WGSL lays out host-shareable values.
fn instances(quads: &[Quad]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(quads.len() * 32);
    for quad in quads {
        let floats = [
            quad.centre.x,
            quad.centre.y,
            quad.half_extents.x,
            quad.half_extents.y,
            quad.colour.r,
            quad.colour.g,
            quad.colour.b,
        ];
        for value in floats {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&u32::from(quad.disc).to_le_bytes());
    }
    bytes
}

/// The draw pass: the background, then every quad in one instanced draw.
struct TopDown<'a> {
    pipeline: &'a InstancedPipeline,
    background: LinearRgb,
    instances: Vec<u8>,
}

impl Pass for TopDown<'_> {
    fn decl(&self) -> PassDecl {
        DRAW
    }

    fn record(&mut self, rec: &mut PassRecorder) {
        rec.clear(LinearRgba {
            r: self.background.r,
            g: self.background.g,
            b: self.background.b,
            a: 1.0,
        });
        rec.instanced(self.pipeline, &self.instances);
    }
}

/// The tone map or the present blit: one fullscreen draw.
struct Fullscreen<'a>(PassDecl, &'a FullscreenPipeline);

impl Pass for Fullscreen<'_> {
    fn decl(&self) -> PassDecl {
        self.0
    }

    fn record(&mut self, rec: &mut PassRecorder) {
        rec.fullscreen(self.1);
    }
}
