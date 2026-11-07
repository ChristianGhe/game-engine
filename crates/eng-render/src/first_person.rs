//! The first-person view (rung 2, T9; ADR-0020): the `opaque` and `sky`
//! passes on eng-gpu's sorted graph, with what the later passes need of
//! them already in place. It sits beside rung 1b's top-down
//! [`crate::Renderer`], sharing only the clamp and blit shaders and the
//! fullscreen pass that draws them.
//!
//! The graph, by name, in running order:
//!
//! | Pass | Reads | Writes | Here |
//! |---|---|---|---|
//! | `opaque` | none | `albedo` (`Rgba16Float`), `info` (`Rg32Uint`), `depth` (`Depth32Float`) | the world's chunks ([`GpuWorld`]) |
//! | `light` | `albedo`, `info`, `depth` | `lit` (`Rgba16Float`) | albedo as it is, until T10's light model (`unlit.wgsl`) |
//! | `sky` | `depth` | continues `light`: `lit` | [`Sky`] where the depth is 0 |
//! | `tonemap` | `lit` | `output` | rung 1b's clamp (`tonemap.wgsl`), until T10's curve |
//! | `present` | `output` | `surface` | rung 1b's blit, for targets with a surface |
//!
//! T10 puts fog between `sky` and `tonemap`, moves the tone map into
//! `toned` and adds the palette into `output`; T11's `sprites` continues
//! `opaque`, drawing into all three of its targets.

mod frame;
mod info;
mod world;

use eng_gpu::{
    Format, FrameInput, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, Graph, InstanceRecord,
    LinearRgba, MeshDesc, MeshDraw, MeshPipeline, PRESENT, Pass, PassDecl, PassNode, PassRecorder,
    Resource, ScreenDesc, ScreenPipeline, SurfaceFrame, Target, Targets, Texels, Transient,
};

pub use frame::Sky;
pub use info::{Info, SurfaceClass};
pub use world::{Chunk, GpuWorld, WorldMesh, WorldVertex};

use crate::renderer::Fullscreen;
use crate::{FirstPersonCamera, FrameCounters, RenderError};
use world::ChunkDraw;

const OPAQUE_WGSL: &str = include_str!("opaque.wgsl");
const UNLIT_WGSL: &str = include_str!("unlit.wgsl");
const SKY_WGSL: &str = include_str!("sky.wgsl");
const TONEMAP_WGSL: &str = include_str!("tonemap.wgsl");
const PRESENT_WGSL: &str = include_str!("present.wgsl");

/// What `opaque` draws its surfaces' colours into: linear albedo.
pub(crate) const ALBEDO: Transient = Transient::new("albedo", Format::Rgba16Float);
/// Each pixel's [`Info`], packed.
pub(crate) const INFO: Transient = Transient::new("info", Format::Rg32Uint);
/// Reversed-Z depth, `near / distance ahead`, 0 where nothing was drawn.
pub(crate) const DEPTH: Transient = Transient::new("depth", Format::Depth32Float);
/// Luminance, scene-referred: what `light` and `sky` draw.
pub(crate) const LIT: Transient = Transient::new("lit", Format::Rgba16Float);

const OPAQUE_TARGETS: &[Resource] = &[
    Resource::Transient(ALBEDO),
    Resource::Transient(INFO),
    Resource::Transient(DEPTH),
];
const LIT_TARGET: &[Resource] = &[Resource::Transient(LIT)];

pub(crate) const OPAQUE: PassNode = PassNode {
    name: "opaque",
    reads: &[],
    writes: OPAQUE_TARGETS,
    continues: None,
    bypassable: false,
};
pub(crate) const LIGHT: PassNode = PassNode {
    name: "light",
    reads: OPAQUE_TARGETS,
    writes: LIT_TARGET,
    continues: None,
    bypassable: false,
};
pub(crate) const SKY: PassNode = PassNode {
    name: "sky",
    reads: &[Resource::Transient(DEPTH)],
    writes: LIT_TARGET,
    continues: Some("light"),
    bypassable: false,
};
pub(crate) const TONEMAP: PassNode = PassNode {
    name: "tonemap",
    reads: LIT_TARGET,
    writes: &[Resource::Output],
    continues: None,
    bypassable: false,
};

/// One frame of the first-person view: where the camera is and what the
/// sky looks like. The world it shows is a [`GpuWorld`], loaded once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstPersonScene {
    /// Its near plane is normally [`crate::NEAR_PLANE`].
    pub camera: FirstPersonCamera,
    pub sky: Sky,
}

/// Draws first-person frames of a [`GpuWorld`] through the graph above.
/// Building one compiles its pipelines and checks its graphs.
pub struct FirstPersonRenderer {
    opaque: MeshPipeline,
    light: ScreenPipeline,
    sky: ScreenPipeline,
    tonemap: FullscreenPipeline,
    present: FullscreenPipeline,
    offscreen: Graph,
    surface: Graph,
}

impl FirstPersonRenderer {
    /// Compiles the passes' pipelines on `gpu`.
    pub fn new(gpu: &Gpu) -> Result<Self, GpuError> {
        let screen = |label: &str, wgsl: &str, entry: &str, node: &PassNode| {
            gpu.screen_pipeline(&ScreenDesc {
                label,
                wgsl,
                entry,
                reads: node.reads,
                writes: node.writes,
                scene: false,
            })
        };
        let fullscreen = |label: &str, wgsl: &str, entry: &str, target: Resource| {
            gpu.fullscreen_pipeline(&FullscreenDesc {
                label,
                wgsl,
                entry,
                target,
            })
        };
        let passes = [OPAQUE, LIGHT, SKY, TONEMAP];
        let with_present = [OPAQUE, LIGHT, SKY, TONEMAP, PRESENT.node()];
        Ok(Self {
            opaque: gpu.mesh_pipeline(&MeshDesc {
                label: "opaque",
                wgsl: OPAQUE_WGSL,
                vertex_entry: "opaque_vertex",
                fragment_entry: "opaque_fragment",
                vertex: world::VERTEX,
                reads: OPAQUE.reads,
                writes: OPAQUE.writes,
                depth_write: true,
                cull_back: true,
            })?,
            light: screen("light (unlit)", UNLIT_WGSL, "unlit_fragment", &LIGHT)?,
            sky: screen("sky", SKY_WGSL, "sky_fragment", &SKY)?,
            tonemap: fullscreen(
                "first-person tonemap",
                TONEMAP_WGSL,
                "tonemap_fragment",
                Resource::Output,
            )?,
            present: fullscreen(
                "first-person present",
                PRESENT_WGSL,
                "present_fragment",
                Resource::Surface,
            )?,
            offscreen: Graph::sorted(&passes, Target::Offscreen)?,
            surface: Graph::sorted(&with_present, Target::Surface)?,
        })
    }

    /// Renders one frame of `world` as `scene` sees it into `targets`,
    /// presenting it too if they have a surface, and returns the frame's
    /// counters. A scene that cannot be drawn (a camera
    /// [`FirstPersonCamera::view_proj`] refuses at the targets' aspect, a
    /// sky that is not finite) fails with `[E-SCENE]` before any GPU work.
    ///
    /// A frame draws each chunk with triangles once, with its own instance
    /// record (its origin minus the eye, in f64); then one draw each for
    /// light, sky and the tone map, and the blit with a surface. It uploads
    /// the 144-byte uniform and 16 bytes per drawn chunk.
    pub fn render(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        world: &GpuWorld,
        scene: &FirstPersonScene,
    ) -> Result<FrameCounters, RenderError> {
        self.draw_frame(gpu, targets, None, world, scene)
    }

    /// Like [`FirstPersonRenderer::render`], into a window's image
    /// (eng-gpu's [`SurfaceFrame`], the targets' size), which the caller
    /// then presents.
    pub fn render_to(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        frame: &SurfaceFrame,
        world: &GpuWorld,
        scene: &FirstPersonScene,
    ) -> Result<FrameCounters, RenderError> {
        self.draw_frame(gpu, targets, Some(frame), world, scene)
    }

    /// The depth the last frame drew into `targets`: `near / distance
    /// ahead` where something was drawn, 0 where the sky shows. An
    /// `[E-GPU]` error unless the last frame rendered into `targets`
    /// succeeded and pooled its transients as this renderer's graph does,
    /// as its own frames do.
    pub fn read_depth(&self, gpu: &Gpu, targets: &Targets) -> Result<Texels, RenderError> {
        Ok(targets.read_transient(gpu, self.graph(targets), DEPTH)?)
    }

    /// The `info` the last frame drew into `targets`, two words per pixel;
    /// [`Info::unpack`] reads them. Refused as
    /// [`FirstPersonRenderer::read_depth`] is.
    pub fn read_info(&self, gpu: &Gpu, targets: &Targets) -> Result<Texels, RenderError> {
        Ok(targets.read_transient(gpu, self.graph(targets), INFO)?)
    }

    /// The graph a frame into `targets` runs. Both pool alike, so a read
    /// after [`FirstPersonRenderer::render_to`] finds the same textures; a
    /// graph that pooled otherwise would be refused by eng-gpu, never read
    /// wrong.
    fn graph(&self, targets: &Targets) -> &Graph {
        if targets.has_surface() {
            &self.surface
        } else {
            &self.offscreen
        }
    }

    fn draw_frame(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        window: Option<&SurfaceFrame>,
        world: &GpuWorld,
        scene: &FirstPersonScene,
    ) -> Result<FrameCounters, RenderError> {
        let (width, height) = targets.size();
        let view = scene.camera.view_proj(width as f32 / height as f32)?;
        scene.sky.check()?;
        let uniform = frame::uniform(&view, &scene.sky);
        let eye = view.eye();
        let records: Vec<InstanceRecord> = world
            .chunks
            .iter()
            .map(|chunk| {
                let o = chunk.origin;
                InstanceRecord::relative([o.x, o.y, o.z], [eye.x, eye.y, eye.z], 0)
            })
            .collect();
        let input = FrameInput {
            surface: window,
            uniform: &uniform,
            instances: &records,
            scene: Some(&world.scene),
        };
        let mut opaque = Opaque {
            pipeline: &self.opaque,
            chunks: &world.chunks,
        };
        let mut light = Screen(LIGHT.decl(), &self.light);
        let mut sky = Screen(SKY.decl(), &self.sky);
        let mut tonemap = Fullscreen(TONEMAP.decl(), &self.tonemap);
        let counters = if window.is_some() || targets.has_surface() {
            let mut present = Fullscreen(PRESENT, &self.present);
            self.surface.render_frame(
                gpu,
                targets,
                &input,
                &mut [
                    &mut opaque,
                    &mut light,
                    &mut sky,
                    &mut tonemap,
                    &mut present,
                ],
            )?
        } else {
            self.offscreen.render_frame(
                gpu,
                targets,
                &input,
                &mut [&mut opaque, &mut light, &mut sky, &mut tonemap],
            )?
        };
        Ok(counters)
    }
}

/// The opaque pass: everything cleared (albedo and `info` to 0, depth to
/// the far plane), then each chunk drawn with its own record.
struct Opaque<'a> {
    pipeline: &'a MeshPipeline,
    chunks: &'a [ChunkDraw],
}

impl Pass for Opaque<'_> {
    fn decl(&self) -> PassDecl {
        OPAQUE.decl()
    }

    fn record(&mut self, rec: &mut PassRecorder) {
        rec.clear(LinearRgba::TRANSPARENT);
        for (record, chunk) in (0u32..).zip(self.chunks) {
            rec.mesh(
                self.pipeline,
                MeshDraw {
                    indices: chunk.indices.clone(),
                    base_vertex: chunk.base_vertex,
                    instances: record..record + 1,
                },
            );
        }
    }
}

/// A pass of one screen draw.
struct Screen<'a>(PassDecl, &'a ScreenPipeline);

impl Pass for Screen<'_> {
    fn decl(&self) -> PassDecl {
        self.0
    }

    fn record(&mut self, rec: &mut PassRecorder) {
        rec.screen(self.1);
    }
}
