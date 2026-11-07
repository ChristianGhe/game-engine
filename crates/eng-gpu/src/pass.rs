//! Passes and how a graph runs them. A pass records commands into a
//! [`PassRecorder`]; once every pass has recorded and each recording fits
//! its declaration, the frame's instances, uniform and instance records are
//! uploaded, and the frame is encoded, counted and submitted at once
//! (`encode.rs`). Each render pass is labelled with its pass's name and,
//! with `WGPU_DEBUG=1`, wrapped in a debug group of that name.

use crate::counters::FrameCounters;
use crate::error::GpuError;
use crate::fullscreen::FullscreenPipeline;
use crate::gpu::Gpu;
use crate::graph::{Graph, PassDecl, Resource, Target};
use crate::instanced::InstancedPipeline;
use crate::layout::{self, ReadKind};
use crate::mesh::{MeshDraw, MeshPipeline};
use crate::scene::{InstanceRecord, Scene};
use crate::screen::ScreenPipeline;
use crate::surface::SurfaceFrame;
use crate::targets::Targets;

/// A colour in linear, scene-referred units (AGENTS.md conventions). The
/// scene holds values above 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl LinearRgba {
    /// What a pass's target starts as unless the pass clears it or continues
    /// an earlier pass.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
}

/// One pass of a graph. [`Graph::render`] runs it where the graph declares
/// the same [`PassDecl`].
pub trait Pass {
    /// What the pass reads and writes. For a [`Graph::sorted`] graph, its
    /// [`crate::PassNode::decl`].
    fn decl(&self) -> PassDecl;
    /// Records this frame's work. It reaches the GPU after every pass has
    /// recorded.
    fn record(&mut self, rec: &mut PassRecorder);
}

/// What a frame brings beside its passes ([`Graph::render_frame`]): rung
/// 2's per-frame data (ADR-0020). Nothing of it is uploaded unless a pass
/// of the frame draws a mesh or screen pipeline, which bind it; so a frame
/// of rung 1b's pipelines uploads and counts what it did in 1b.
#[derive(Clone, Copy, Default)]
pub struct FrameInput<'a> {
    /// A window's image, for a [`Target::Surface`] graph's present pass to
    /// draw into, as [`Graph::render_to`] does. It must be the targets'
    /// [`Targets::surface_size`]: their size, unless they are
    /// [`Targets::upscaled`].
    /// Without one a surface graph draws into the targets' offscreen
    /// surface.
    pub surface: Option<&'a SurfaceFrame>,
    /// The per-frame uniform, `@group(0) @binding(0)` of mesh and screen
    /// pipelines: the camera's projection and rotation (the eye is the
    /// origin), EV100 exposure and the look switches, laid out as the
    /// caller's shaders declare them. A whole number of 16-byte rows, and
    /// not empty in a frame that draws a mesh or screen pipeline.
    pub uniform: &'a [u8],
    /// This frame's record of each drawn chunk or sprite,
    /// `@group(0) @binding(1)`, which a [`MeshDraw`]'s instances index.
    pub instances: &'a [InstanceRecord],
    /// The scene that mesh pipelines draw from, and screen pipelines with
    /// [`crate::ScreenDesc::scene`] bind.
    pub scene: Option<&'a Scene>,
}

/// The commands a pass records for one frame, drawn in order.
#[derive(Debug)]
pub struct PassRecorder {
    pub(crate) clear: Option<LinearRgba>,
    pub(crate) commands: Vec<Command>,
}

#[derive(Debug)]
pub(crate) enum Command {
    Fullscreen(FullscreenPipeline),
    Instanced {
        pipeline: InstancedPipeline,
        bytes: Vec<u8>,
    },
    Mesh {
        pipeline: MeshPipeline,
        draw: MeshDraw,
    },
    Screen(ScreenPipeline),
}

impl PassRecorder {
    pub(crate) fn new() -> Self {
        Self {
            clear: None,
            commands: Vec::new(),
        }
    }

    /// Starts the pass's colour targets, every one of them, as `colour`
    /// rather than [`LinearRgba::TRANSPARENT`]. An integer target
    /// ([`crate::Format::Rg32Uint`]) takes its components as integers; how a
    /// backend converts a value that is not whole is its own, so clear a
    /// pass with one to whole values (or 0). A depth target always starts
    /// at 0, the far plane. A pass that continues another draws on top of
    /// its targets, so it cannot clear.
    pub fn clear(&mut self, colour: LinearRgba) {
        self.clear = Some(colour);
    }

    /// Draws `pipeline` over the whole target, with the pass's one read as
    /// its input.
    pub fn fullscreen(&mut self, pipeline: &FullscreenPipeline) {
        self.commands.push(Command::Fullscreen(pipeline.clone()));
    }

    /// Draws `pipeline`'s primitive once for each instance in `instances`:
    /// whole instances, laid out as the pipeline's
    /// [`crate::InstancedDesc::instance`] says, one after another. The bytes
    /// are uploaded with the frame and counted in its
    /// [`FrameCounters::upload_bytes`]. With no bytes it records nothing.
    pub fn instanced(&mut self, pipeline: &InstancedPipeline, instances: &[u8]) {
        if instances.is_empty() {
            return;
        }
        self.commands.push(Command::Instanced {
            pipeline: pipeline.clone(),
            bytes: instances.to_vec(),
        });
    }

    /// Draws `draw.indices` of the frame's scene through `pipeline`, once
    /// for each of the frame's instance records in `draw.instances`
    /// ([`FrameInput`]). With no indices or no instances it records nothing.
    pub fn mesh(&mut self, pipeline: &MeshPipeline, draw: MeshDraw) {
        if draw.indices.is_empty() || draw.instances.is_empty() {
            return;
        }
        self.commands.push(Command::Mesh {
            pipeline: pipeline.clone(),
            draw,
        });
    }

    /// Draws `pipeline` over the whole of the pass's targets, with every
    /// read of the pass bound.
    pub fn screen(&mut self, pipeline: &ScreenPipeline) {
        self.commands.push(Command::Screen(pipeline.clone()));
    }

    /// Whether the recording fits the pass's declaration and the frame's
    /// input, and whether it `loads` its target, continuing an earlier pass.
    pub(crate) fn check(
        &self,
        decl: &PassDecl,
        loads: bool,
        input: &FrameInput<'_>,
    ) -> Result<(), GpuError> {
        let fail = |message: String| GpuError::Pass {
            pass: decl.name,
            message,
        };
        if loads && self.clear.is_some() {
            return Err(fail(
                "the pass continues an earlier pass's target, so it draws on top of it and cannot clear it"
                    .into(),
            ));
        }
        let target = |target: Resource| match decl.writes {
            [only] if *only == target => Ok(()),
            [_, _, ..] => Err(fail(format!(
                "the pipeline draws into `{}` alone, but the pass writes {}",
                target.name(),
                names(decl.writes)
            ))),
            _ => Err(fail(format!(
                "the pipeline draws into `{}`, but the pass writes `{}`",
                target.name(),
                decl.writes.first().map_or("nothing", |r| r.name())
            ))),
        };
        let shape = |reads: &[Resource], writes: &[Resource]| {
            if reads == decl.reads && writes == decl.writes {
                Ok(())
            } else {
                Err(fail(format!(
                    "the pipeline was made for a pass that reads {} and writes {}, but this pass \
                     reads {} and writes {}",
                    names(reads),
                    names(writes),
                    names(decl.reads),
                    names(decl.writes)
                )))
            }
        };
        let frame = || {
            if input.uniform.is_empty() {
                Err(fail(
                    "the pipeline binds the frame's uniform, but the frame has none".into(),
                ))
            } else if !input.uniform.len().is_multiple_of(16) {
                Err(fail(format!(
                    "the frame's uniform has {} bytes, not a whole number of 16-byte rows",
                    input.uniform.len()
                )))
            } else {
                Ok(())
            }
        };
        let scene = || {
            input
                .scene
                .ok_or_else(|| fail("the pipeline binds the scene, but the frame has none".into()))
        };
        for command in &self.commands {
            match command {
                Command::Fullscreen(pipeline) => {
                    let [read] = decl.reads else {
                        return Err(fail(format!(
                            "a fullscreen draw takes the pass's one read as its input, but the pass reads {}",
                            decl.reads.len()
                        )));
                    };
                    if layout::read_kind(*read) != ReadKind::Colour {
                        return Err(fail(format!(
                            "a fullscreen draw filters its input, and `{}` is not colour; a screen \
                             pipeline reads it",
                            read.name()
                        )));
                    }
                    target(pipeline.target)?;
                }
                Command::Instanced { pipeline, bytes } => {
                    target(pipeline.target)?;
                    let len = bytes.len() as u64;
                    if !len.is_multiple_of(pipeline.stride) {
                        return Err(fail(format!(
                            "{len} bytes of instances are not a whole number of {}-byte instances",
                            pipeline.stride
                        )));
                    }
                }
                Command::Mesh { pipeline, draw } => {
                    shape(&pipeline.reads, &pipeline.writes)?;
                    frame()?;
                    let scene = scene()?;
                    if draw.indices.end > scene.index_count {
                        return Err(fail(format!(
                            "the draw takes indices {:?}, but the scene has {}",
                            draw.indices, scene.index_count
                        )));
                    }
                    let records = input.instances.len();
                    if draw.instances.end as usize > records {
                        return Err(fail(format!(
                            "the draw takes instance records {:?}, but the frame has {records}",
                            draw.instances
                        )));
                    }
                }
                Command::Screen(pipeline) => {
                    shape(&pipeline.reads, &pipeline.writes)?;
                    frame()?;
                    if pipeline.scene {
                        scene()?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Resources as `[scene, output]`.
fn names(resources: &[Resource]) -> String {
    let names: Vec<&str> = resources.iter().map(|r| r.name()).collect();
    format!("[{}]", names.join(", "))
}

impl Graph {
    /// Records `passes`, one for each of the graph's declarations and in the
    /// order they were declared, culled and bypassed ones included, then
    /// submits the frame and returns what it counted. The passes that run
    /// are checked against their declarations and record in running order;
    /// then the others are checked, and do not record. Each pass draws into
    /// the targets it writes, transients into the targets' pool: up to four
    /// colour targets and one depth target. A later [`Targets::read_output`]
    /// sees the frame, and [`Targets::frame_stats`] what it measured.
    pub fn render(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        passes: &mut [&mut dyn Pass],
    ) -> Result<FrameCounters, GpuError> {
        self.render_frame(gpu, targets, &FrameInput::default(), passes)
    }

    /// Like [`Graph::render`], with a window's image as the
    /// [`Resource::Surface`]: a [`Target::Surface`] graph's present pass
    /// draws into `frame`, which the caller then presents
    /// ([`crate::WindowSurface::present`]). `frame` must be the targets'
    /// [`Targets::surface_size`]: their size, unless they are
    /// [`Targets::upscaled`], whose present pass draws each texel as k×k
    /// pixels of it.
    pub fn render_to(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        frame: &SurfaceFrame,
        passes: &mut [&mut dyn Pass],
    ) -> Result<FrameCounters, GpuError> {
        let input = FrameInput {
            surface: Some(frame),
            ..FrameInput::default()
        };
        self.render_frame(gpu, targets, &input, passes)
    }

    /// Like [`Graph::render`], with what the frame brings beside its passes
    /// ([`FrameInput`]): a window's image, as [`Graph::render_to`] takes it,
    /// and the uniform, instance records and scene that mesh and screen
    /// pipelines draw with. Its [`FrameCounters::upload_bytes`] then also
    /// counts the uniform and the records, if a pass binds them.
    pub fn render_frame(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        input: &FrameInput<'_>,
        passes: &mut [&mut dyn Pass],
    ) -> Result<FrameCounters, GpuError> {
        if let Some(frame) = input.surface {
            if self.target() != Target::Surface {
                return Err(GpuError::Surface(
                    "a window's image needs a surface graph, and this one renders offscreen".into(),
                ));
            }
            if frame.size() != targets.surface_size() {
                let ((fw, fh), (tw, th)) = (frame.size(), targets.surface_size());
                let message = if targets.upscale() == 1 {
                    format!("the window's image is {fw}x{fh}, but the targets are {tw}x{th}")
                } else {
                    format!(
                        "the window's image is {fw}x{fh}, but the targets present into {tw}x{th} \
                         (upscaled {}x)",
                        targets.upscale()
                    )
                };
                return Err(GpuError::Surface(message));
            }
        }
        self.encode(gpu, targets, input, passes)
            .map(|(counters, _)| counters)
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameInput, LinearRgba, Pass, PassRecorder};
    use crate::{DRAW, Gpu, GpuError, GpuOptions, Graph, PassDecl, TONEMAP, Target, Targets};

    struct Clear(PassDecl);

    impl Pass for Clear {
        fn decl(&self) -> PassDecl {
            self.0
        }
        fn record(&mut self, rec: &mut PassRecorder) {
            rec.clear(LinearRgba::TRANSPARENT);
        }
    }

    /// `WGPU_DEBUG=1` wraps each pass in a debug group: one opened per pass,
    /// none without it, and wgpu fails the frame if a group is left open or
    /// closed twice. Set here directly, so that the path runs without the
    /// variable.
    #[test]
    fn debug_groups_open_and_close_around_each_pass() {
        let mut gpu = match Gpu::new(&GpuOptions::from_env()) {
            Ok(gpu) => gpu,
            Err(e @ GpuError::NoAdapter { .. }) if !crate::gpu_required() => {
                eprintln!("skipped: {e}");
                return;
            }
            Err(e) => panic!("{e}"),
        };
        let targets = Targets::new(&gpu, 4, 4).unwrap_or_else(|e| panic!("{e}"));
        let groups = |gpu: &Gpu| {
            let (_, groups) = Graph::fixed(Target::Offscreen)
                .encode(
                    gpu,
                    &targets,
                    &FrameInput::default(),
                    &mut [&mut Clear(DRAW), &mut Clear(TONEMAP)],
                )
                .unwrap_or_else(|e| panic!("{e}"));
            groups
        };
        gpu.debug = true;
        assert_eq!(groups(&gpu), 2);
        gpu.debug = false;
        assert_eq!(groups(&gpu), 0);
    }
}
