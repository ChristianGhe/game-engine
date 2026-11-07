//! The first-person view (rung 2, T9 and T10; ADR-0020): the `render-retro`
//! passes on eng-gpu's sorted graph. It sits beside rung 1b's top-down
//! [`crate::Renderer`], sharing only its present blit.
//!
//! The graph, by name, in running order:
//!
//! | Pass | Reads | Writes | Here | Switched off |
//! |---|---|---|---|---|
//! | `opaque` | none | `albedo` (`Rgba16Float`), `info` (`Rg32Uint`), `depth` (`Depth32Float`) | the world's chunks ([`GpuWorld`]) | never |
//! | `light` | `albedo`, `info`, `depth` | `lit` (`Rgba16Float`) | region light, Lambertian, in nits (`light.wgsl`, [`Look`]) | falloff, banding and contrast each |
//! | `sky` | `depth` | continues `light`: `lit` | [`Sky`] where the depth is 0 (`sky.wgsl`) | flat: the horizon's colour |
//! | `fog` | `lit`, `info`, `depth` | `fogged` (`Rgba16Float`) | each region's fog, by distance (`fog.wgsl`) | bypassed: the tone map reads `lit` |
//! | `tonemap` | `fogged` | `toned` (`Rgba8UnormSrgb`) | EV100 exposure, Khronos PBR Neutral (`pbr_neutral.wgsl`) | never |
//! | `palette` | `toned` | `output` | the [`crate::PaletteLut`], with a 4x4 Bayer dither (`palette.wgsl`) | bypassed: the tone map draws into `output` |
//! | `present` | `output` | `surface` | rung 1b's blit, for targets with a surface; k×k for upscaled targets | low resolution is the targets' |
//!
//! **The plain pipeline** ([`FirstPersonRenderer::render_plain`]) is
//! `opaque`, `light` with the region's light alone, a flat `sky`, and the
//! tone map from `lit` into `output`: no fog or palette pass at all. With
//! every look switch off ([`Look::OFF`]), [`FirstPersonRenderer::render`]
//! gives its image byte for byte (plan decision 13).
//!
//! T11's `sprites` continues `opaque`, drawing into all three of its
//! targets.

mod frame;
mod info;
mod look;
mod passes;
mod world;

use std::sync::atomic::{AtomicU8, Ordering};

use eng_gpu::{
    FrameInput, FullscreenDesc, FullscreenPipeline, Gpu, GpuError, Graph, InstanceRecord, MeshDesc,
    MeshPipeline, Pass, PassNode, Resource, ScreenDesc, ScreenPipeline, SurfaceFrame, Target,
    Targets, Texels, Transient,
};
use eng_math::Ev100;

pub use frame::Sky;
pub use info::{Info, SurfaceClass};
pub use look::{
    CONTRAST_STEPS, DITHER_CODES, LIGHT_FALLOFF, LIGHT_STEP_LUX, LIGHT_STEPS, LOW_RESOLUTION, Look,
    MAX_REGIONS, RegionLook, exposure,
};
pub use world::{Chunk, GpuWorld, WorldMesh, WorldVertex};

use crate::{FirstPersonCamera, FrameCounters, RenderError};
use passes::{
    DEPTH, FOG, FOGGED, INFO, LIGHT, LIT, LOOK_NODES, LOOK_NODES_PRESENTED, OPAQUE, Opaque,
    PALETTE, PLAIN_NODES, PLAIN_NODES_PRESENTED, PLAIN_TONEMAP, Present, SKY, Screen, TONEMAP,
};

const OPAQUE_WGSL: &str = include_str!("opaque.wgsl");
const LIGHT_WGSL: &str = include_str!("light.wgsl");
const SKY_WGSL: &str = include_str!("sky.wgsl");
const FOG_WGSL: &str = include_str!("fog.wgsl");
const TONEMAP_WGSL: &str = include_str!("pbr_neutral.wgsl");
const PALETTE_WGSL: &str = include_str!("palette.wgsl");
const PRESENT_WGSL: &str = include_str!("present.wgsl");

/// One frame of the first-person view: where the camera is, how it
/// exposes, what the sky and each region look like, and which look
/// features are on. The world it shows is a [`GpuWorld`], loaded once.
#[derive(Clone, Debug, PartialEq)]
pub struct FirstPersonScene {
    /// Its near plane is normally [`crate::NEAR_PLANE`].
    pub camera: FirstPersonCamera,
    pub sky: Sky,
    /// The camera's exposure value at ISO 100, fixed per camera: the tone
    /// map scales luminance by [`exposure`] of it.
    pub ev100: Ev100,
    pub look: Look,
    /// Each region's light and fog, by the region index that
    /// [`Info::region`] carries; at most [`MAX_REGIONS`]. A region past the
    /// list's end looks as [`RegionLook::DEFAULT`].
    pub regions: Vec<RegionLook>,
}

/// Which of the renderer's graphs a frame runs: the look's, by which of
/// its two bypassable passes are on, or the plain pipeline's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Look { fog: bool, palette: bool },
    Plain,
}

impl Kind {
    const ALL: [Self; 5] = [
        Self::Look {
            fog: true,
            palette: true,
        },
        Self::Look {
            fog: false,
            palette: true,
        },
        Self::Look {
            fog: true,
            palette: false,
        },
        Self::Look {
            fog: false,
            palette: false,
        },
        Self::Plain,
    ];

    fn index(self) -> u8 {
        match self {
            Self::Look { fog, palette } => u8::from(!fog) | u8::from(!palette) << 1,
            Self::Plain => 4,
        }
    }

    fn graph(self, target: Target) -> Result<Graph, GpuError> {
        let presented = target == Target::Surface;
        let nodes: &[PassNode] = match (self, presented) {
            (Self::Plain, false) => &PLAIN_NODES,
            (Self::Plain, true) => &PLAIN_NODES_PRESENTED,
            (Self::Look { .. }, false) => &LOOK_NODES,
            (Self::Look { .. }, true) => &LOOK_NODES_PRESENTED,
        };
        let off: &[&'static str] = match self {
            Self::Plain => &[],
            Self::Look { fog, palette } => match (fog, palette) {
                (true, true) => &[],
                (false, true) => &["fog"],
                (true, false) => &["palette"],
                (false, false) => &["fog", "palette"],
            },
        };
        Ok(Graph::sorted_without(nodes, off, target)?)
    }
}

/// Draws first-person frames of a [`GpuWorld`] through the graph above.
/// Building one compiles its pipelines and checks its graphs: for each
/// [`Kind`], offscreen and presented.
pub struct FirstPersonRenderer {
    opaque: MeshPipeline,
    light: ScreenPipeline,
    plain_light: ScreenPipeline,
    sky: ScreenPipeline,
    plain_sky: ScreenPipeline,
    fog: ScreenPipeline,
    tonemap: ScreenPipeline,
    plain_tonemap: ScreenPipeline,
    palette: ScreenPipeline,
    present: FullscreenPipeline,
    /// By [`Kind::index`].
    offscreen: Vec<Graph>,
    surface: Vec<Graph>,
    /// The [`Kind::index`] of the last frame drawn, which reads back from.
    last: AtomicU8,
}

impl FirstPersonRenderer {
    /// Compiles the passes' pipelines on `gpu`.
    pub fn new(gpu: &Gpu) -> Result<Self, GpuError> {
        let screen = |label: &str, wgsl: &str, entry: &str, node: &PassNode, scene: bool| {
            gpu.screen_pipeline(&ScreenDesc {
                label,
                wgsl,
                entry,
                reads: node.reads,
                writes: node.writes,
                scene,
            })
        };
        let graphs = |target: Target| -> Result<Vec<Graph>, GpuError> {
            let mut graphs: Vec<(u8, Graph)> = Kind::ALL
                .iter()
                .map(|kind| Ok((kind.index(), kind.graph(target)?)))
                .collect::<Result<_, GpuError>>()?;
            graphs.sort_by_key(|(index, _)| *index);
            Ok(graphs.into_iter().map(|(_, graph)| graph).collect())
        };
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
            light: screen("light", LIGHT_WGSL, "light_fragment", &LIGHT, false)?,
            plain_light: screen(
                "plain light",
                LIGHT_WGSL,
                "plain_light_fragment",
                &LIGHT,
                false,
            )?,
            sky: screen("sky", SKY_WGSL, "sky_fragment", &SKY, false)?,
            plain_sky: screen("plain sky", SKY_WGSL, "plain_sky_fragment", &SKY, false)?,
            fog: screen("fog", FOG_WGSL, "fog_fragment", &FOG, false)?,
            tonemap: screen("tonemap", TONEMAP_WGSL, "tonemap_fragment", &TONEMAP, false)?,
            plain_tonemap: screen(
                "plain tonemap",
                TONEMAP_WGSL,
                "tonemap_fragment",
                &PLAIN_TONEMAP,
                false,
            )?,
            palette: screen("palette", PALETTE_WGSL, "palette_fragment", &PALETTE, true)?,
            present: gpu.fullscreen_pipeline(&FullscreenDesc {
                label: "first-person present",
                wgsl: PRESENT_WGSL,
                entry: "present_fragment",
                target: Resource::Surface,
            })?,
            offscreen: graphs(Target::Offscreen)?,
            surface: graphs(Target::Surface)?,
            last: AtomicU8::new(0),
        })
    }

    /// Renders one frame of `world` as `scene` sees it, with its look, into
    /// `targets`, presenting it too if they have a surface, and returns the
    /// frame's counters. A scene that cannot be drawn fails with
    /// `[E-SCENE]` before any GPU work: a camera
    /// [`FirstPersonCamera::view_proj`] refuses at the targets' aspect, a
    /// sky, fog or EV100 the checks refuse ([`FirstPersonScene`]'s fields),
    /// or the palette on for a world loaded without a table.
    ///
    /// A frame draws each chunk with triangles once, with its own instance
    /// record (its origin minus the eye, in f64); then one draw each for
    /// light, sky, fog, the tone map and the palette, those of fog and the
    /// palette only when switched on, and the blit with a surface. It
    /// uploads the 624-byte uniform and 16 bytes per drawn chunk.
    pub fn render(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        world: &GpuWorld,
        scene: &FirstPersonScene,
    ) -> Result<FrameCounters, RenderError> {
        self.draw_frame(gpu, targets, None, world, scene, Self::kind(scene))
    }

    /// Like [`FirstPersonRenderer::render`], into a window's image
    /// (eng-gpu's [`SurfaceFrame`], of the targets'
    /// [`Targets::surface_size`]), which the caller then presents. For a
    /// low resolution look, make the window's targets with
    /// [`Targets::upscaled`] at [`Look::low_resolution`].
    pub fn render_to(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        frame: &SurfaceFrame,
        world: &GpuWorld,
        scene: &FirstPersonScene,
    ) -> Result<FrameCounters, RenderError> {
        self.draw_frame(gpu, targets, Some(frame), world, scene, Self::kind(scene))
    }

    /// Renders one frame of the plain pipeline (see the top of
    /// `first_person.rs`), whatever `scene`'s look says: the image every
    /// look switch off must give. Its checks and readbacks are
    /// [`FirstPersonRenderer::render`]'s, but it needs no palette table.
    pub fn render_plain(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        world: &GpuWorld,
        scene: &FirstPersonScene,
    ) -> Result<FrameCounters, RenderError> {
        self.draw_frame(gpu, targets, None, world, scene, Kind::Plain)
    }

    /// The depth the last frame drew into `targets`: `near / distance
    /// ahead` where something was drawn, 0 where the sky shows. An
    /// `[E-GPU]` error unless the last frame rendered into `targets`
    /// succeeded and was this renderer's last, or pooled its transients as
    /// that one did.
    pub fn read_depth(&self, gpu: &Gpu, targets: &Targets) -> Result<Texels, RenderError> {
        self.read(gpu, targets, DEPTH)
    }

    /// The `info` the last frame drew into `targets`, two words per pixel;
    /// [`Info::unpack`] reads them. Refused as
    /// [`FirstPersonRenderer::read_depth`] is.
    pub fn read_info(&self, gpu: &Gpu, targets: &Targets) -> Result<Texels, RenderError> {
        self.read(gpu, targets, INFO)
    }

    /// The luminance the last frame's light and sky passes drew, in nits,
    /// as half floats (`Rgba16Float`). Refused as
    /// [`FirstPersonRenderer::read_depth`] is.
    pub fn read_lit(&self, gpu: &Gpu, targets: &Targets) -> Result<Texels, RenderError> {
        self.read(gpu, targets, LIT)
    }

    /// What the last frame's fog pass drew, as [`FirstPersonRenderer::read_lit`]
    /// gives `lit`. With fog switched off it is `lit` itself, whose texture
    /// the bypass hands the tone map; the plain pipeline has no fog to read.
    pub fn read_fogged(&self, gpu: &Gpu, targets: &Targets) -> Result<Texels, RenderError> {
        self.read(gpu, targets, FOGGED)
    }

    fn read(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        transient: Transient,
    ) -> Result<Texels, RenderError> {
        let index = usize::from(self.last.load(Ordering::Relaxed));
        let graphs = if targets.has_surface() {
            &self.surface
        } else {
            &self.offscreen
        };
        Ok(targets.read_transient(gpu, &graphs[index], transient)?)
    }

    fn kind(scene: &FirstPersonScene) -> Kind {
        Kind::Look {
            fog: scene.look.fog,
            palette: scene.look.palette,
        }
    }

    fn draw_frame(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        window: Option<&SurfaceFrame>,
        world: &GpuWorld,
        scene: &FirstPersonScene,
        kind: Kind,
    ) -> Result<FrameCounters, RenderError> {
        let (width, height) = targets.size();
        let view = scene.camera.view_proj(width as f32 / height as f32)?;
        frame::check(scene)?;
        if matches!(kind, Kind::Look { palette: true, .. }) && world.palette.is_none() {
            return Err(RenderError::Scene {
                shape: None,
                problem: "the look's palette is on, but the world was loaded without a palette \
                          table (GpuWorld::load_with_palette)"
                    .into(),
            });
        }
        let uniform = frame::uniform(&view, scene, world.palette.unwrap_or(0));
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
        let plain = kind == Kind::Plain;
        let mut light = Screen(
            LIGHT,
            if plain {
                &self.plain_light
            } else {
                &self.light
            },
        );
        let mut sky = Screen(SKY, if plain { &self.plain_sky } else { &self.sky });
        let mut fog = Screen(FOG, &self.fog);
        let mut tonemap = Screen(TONEMAP, &self.tonemap);
        let mut plain_tonemap = Screen(PLAIN_TONEMAP, &self.plain_tonemap);
        let mut palette = Screen(PALETTE, &self.palette);
        let mut present = Present(&self.present);
        let presented = window.is_some() || targets.has_surface();
        let mut passes: Vec<&mut dyn Pass> = vec![&mut opaque, &mut light, &mut sky];
        if plain {
            passes.push(&mut plain_tonemap);
        } else {
            passes.extend([&mut fog as &mut dyn Pass, &mut tonemap, &mut palette]);
        }
        let graphs = if presented {
            passes.push(&mut present);
            &self.surface
        } else {
            &self.offscreen
        };
        let index = kind.index();
        self.last.store(index, Ordering::Relaxed);
        Ok(graphs[usize::from(index)].render_frame(gpu, targets, &input, &mut passes)?)
    }
}
