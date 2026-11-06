//! Passes and how a graph runs them. A pass records commands into a
//! [`PassRecorder`]; once every pass has recorded and each recording fits
//! its declaration, the frame's instances are uploaded, and the frame is
//! encoded, counted and submitted at once. Each render pass is labelled with
//! its pass's name and, with `WGPU_DEBUG=1`, wrapped in a debug group of
//! that name.

use crate::counters::FrameCounters;
use crate::error::GpuError;
use crate::fullscreen::FullscreenPipeline;
use crate::gpu::Gpu;
use crate::graph::{Graph, GraphError, PassDecl, Resource, Target};
use crate::instanced::InstancedPipeline;
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

/// The commands a pass records for one frame, drawn in order.
#[derive(Debug)]
pub struct PassRecorder {
    clear: Option<LinearRgba>,
    commands: Vec<Command>,
}

#[derive(Debug)]
enum Command {
    Fullscreen(FullscreenPipeline),
    Instanced {
        pipeline: InstancedPipeline,
        bytes: Vec<u8>,
    },
}

impl PassRecorder {
    fn new() -> Self {
        Self {
            clear: None,
            commands: Vec::new(),
        }
    }

    /// Starts the pass's target as `colour` rather than
    /// [`LinearRgba::TRANSPARENT`]. A pass that continues another draws on
    /// top of its target, so it cannot clear.
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

    /// Whether the recording fits the pass's declaration, and whether it
    /// `loads` its target, continuing an earlier pass.
    fn check(&self, decl: &PassDecl, loads: bool) -> Result<(), GpuError> {
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
        let target = |target: Resource| {
            if decl.writes == [target] {
                Ok(())
            } else {
                Err(fail(format!(
                    "the pipeline draws into `{}`, but the pass writes `{}`",
                    target.name(),
                    decl.writes.first().map_or("nothing", |r| r.name())
                )))
            }
        };
        for command in &self.commands {
            match command {
                Command::Fullscreen(pipeline) => {
                    if decl.reads.len() != 1 {
                        return Err(fail(format!(
                            "a fullscreen draw takes the pass's one read as its input, but the pass reads {}",
                            decl.reads.len()
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
            }
        }
        Ok(())
    }
}

impl Graph {
    /// Records `passes`, one for each of the graph's declarations and in the
    /// order they were declared, culled ones included, then submits the
    /// frame and returns what it counted. The passes record in running
    /// order; a culled pass is checked against its declaration but does not
    /// record. Each pass draws into the target it writes, transients into
    /// the targets' pool; a later [`Targets::read_output`] sees the frame.
    pub fn render(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        passes: &mut [&mut dyn Pass],
    ) -> Result<FrameCounters, GpuError> {
        self.encode(gpu, targets, None, passes)
    }

    /// Like [`Graph::render`], with a window's image as the
    /// [`Resource::Surface`]: a [`Target::Surface`] graph's present pass
    /// draws into `frame`, which the caller then presents
    /// ([`crate::WindowSurface::present`]). `frame` must be the targets' size.
    pub fn render_to(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        frame: &SurfaceFrame,
        passes: &mut [&mut dyn Pass],
    ) -> Result<FrameCounters, GpuError> {
        if self.target() != Target::Surface {
            return Err(GpuError::Surface(
                "a window's image needs a surface graph, and this one renders offscreen".into(),
            ));
        }
        if frame.size() != targets.size() {
            let ((fw, fh), (tw, th)) = (frame.size(), targets.size());
            return Err(GpuError::Surface(format!(
                "the window's image is {fw}x{fh}, but the targets are {tw}x{th}"
            )));
        }
        self.encode(gpu, targets, Some(&frame.view), passes)
    }

    fn encode(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        surface: Option<&wgpu::TextureView>,
        passes: &mut [&mut dyn Pass],
    ) -> Result<FrameCounters, GpuError> {
        if passes.len() != self.declared.len() {
            return Err(GraphError::PassCount {
                expected: self.declared.len(),
                found: passes.len(),
            }
            .into());
        }
        // Where each declared pass runs, if it does.
        let mut runs_at = vec![None; self.declared.len()];
        for (at, &slot) in self.slots.iter().enumerate() {
            runs_at[slot] = Some(at);
        }
        let mut recorded: Vec<Option<PassRecorder>> = std::iter::repeat_with(|| None)
            .take(self.passes.len())
            .collect();
        for (index, (decl, pass)) in self.declared.iter().zip(passes.iter_mut()).enumerate() {
            let found = pass.decl();
            if found != *decl {
                return Err(GraphError::PassMismatch {
                    index,
                    expected: *decl,
                    found,
                }
                .into());
            }
            let Some(at) = runs_at[index] else { continue };
            let mut rec = PassRecorder::new();
            pass.record(&mut rec);
            rec.check(decl, self.loads[at])?;
            recorded[at] = Some(rec);
        }
        // Every pass that runs recorded, in running order.
        let recorded: Vec<PassRecorder> = recorded.into_iter().flatten().collect();
        let pooled = if self.plan.textures().is_empty() {
            Vec::new()
        } else {
            targets.pooled(gpu, &self.plan)?
        };
        // Only a surface graph uses the surface, which the targets may lack.
        // Each pass writes exactly one resource (Graph::new, Graph::sorted).
        let mut views = Vec::with_capacity(recorded.len());
        for decl in &self.passes {
            let view = |r: Resource| match (r, surface) {
                (Resource::Surface, Some(window)) => Ok(window),
                (Resource::Transient(transient), _) => self
                    .plan
                    .texture(&transient)
                    .and_then(|index| pooled.get(index))
                    .ok_or_else(|| {
                        GpuError::Wgpu(format!(
                            "transient `{}` has no texture in the graph's pool",
                            transient.name
                        ))
                    }),
                _ => targets.view(r).ok_or(GpuError::NoSurface),
            };
            let input = decl.reads.first().map(|&r| view(r)).transpose()?;
            views.push((view(decl.writes[0])?, input));
        }
        // Every instance of the frame, in the order the passes draw them.
        let uploads: Vec<u8> = recorded
            .iter()
            .flat_map(|rec| &rec.commands)
            .filter_map(|command| match command {
                Command::Instanced { bytes, .. } => Some(bytes.as_slice()),
                Command::Fullscreen(_) => None,
            })
            .flatten()
            .copied()
            .collect();
        gpu.scoped(|| {
            let mut frame = Frame {
                gpu,
                instances: upload(gpu, &uploads),
                cursor: 0,
                counters: FrameCounters {
                    upload_bytes: uploads.len() as u64,
                    ..FrameCounters::default()
                },
            };
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("frame"),
                });
            let steps = self.passes.iter().zip(&self.loads).zip(&recorded);
            for (((decl, &loads), rec), (target, input)) in steps.zip(&views) {
                if gpu.debug {
                    encoder.push_debug_group(decl.name);
                }
                frame.encode(&mut encoder, decl, rec, loads, target, *input);
                if gpu.debug {
                    encoder.pop_debug_group();
                }
            }
            gpu.queue.submit([encoder.finish()]);
            frame.counters
        })
        .map_err(GpuError::Wgpu)
    }
}

/// The frame's instances in one vertex buffer, or `None` without any.
fn upload(gpu: &Gpu, bytes: &[u8]) -> Option<wgpu::Buffer> {
    if bytes.is_empty() {
        return None;
    }
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instances"),
        size: bytes.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu.queue.write_buffer(&buffer, 0, bytes);
    Some(buffer)
}

/// One frame being encoded: where the next instances start in the frame's
/// buffer, and what has been counted so far.
struct Frame<'a> {
    gpu: &'a Gpu,
    instances: Option<wgpu::Buffer>,
    cursor: u64,
    counters: FrameCounters,
}

impl Frame<'_> {
    /// Encodes one pass, which `loads` its target if it continues an
    /// earlier pass, and clears it otherwise.
    fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        decl: &PassDecl,
        rec: &PassRecorder,
        loads: bool,
        target: &wgpu::TextureView,
        input: Option<&wgpu::TextureView>,
    ) {
        let colour = rec.clear.unwrap_or(LinearRgba::TRANSPARENT);
        let load = if loads {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: f64::from(colour.r),
                g: f64::from(colour.g),
                b: f64::from(colour.b),
                a: f64::from(colour.a),
            })
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(decl.name),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        // What this pass has bound so far: nothing carries over between passes.
        let mut bound: Option<&wgpu::RenderPipeline> = None;
        let mut input_bound = false;
        for command in &rec.commands {
            match command {
                Command::Fullscreen(fullscreen) => {
                    // `check` made sure a fullscreen pass has its input.
                    let Some(input) = input else { continue };
                    set_pipeline(
                        &mut pass,
                        &mut bound,
                        &fullscreen.pipeline,
                        &mut self.counters,
                    );
                    if !input_bound {
                        let bind_group = self.gpu.shared.bind(&self.gpu.device, input, decl.name);
                        pass.set_bind_group(0, &bind_group, &[]);
                        self.counters.bind_group_switches += 1;
                        input_bound = true;
                    }
                    pass.draw(0..3, 0..1);
                }
                Command::Instanced { pipeline, bytes } => {
                    // `upload` made the buffer from every instanced command.
                    let Some(instances) = &self.instances else {
                        continue;
                    };
                    set_pipeline(
                        &mut pass,
                        &mut bound,
                        &pipeline.pipeline,
                        &mut self.counters,
                    );
                    let len = bytes.len() as u64;
                    // Tier 0 buffers hold at most 256 MiB, far fewer instances
                    // than a u32 counts; a larger upload already failed.
                    let count = u32::try_from(len / pipeline.stride).unwrap_or(u32::MAX);
                    pass.set_vertex_buffer(0, instances.slice(self.cursor..self.cursor + len));
                    self.cursor += len;
                    pass.draw(0..pipeline.vertices, 0..count);
                }
            }
            self.counters.draws += 1;
        }
    }
}

/// Binds `pipeline` unless the pass has it bound already.
fn set_pipeline<'p>(
    pass: &mut wgpu::RenderPass<'_>,
    bound: &mut Option<&'p wgpu::RenderPipeline>,
    pipeline: &'p wgpu::RenderPipeline,
    counters: &mut FrameCounters,
) {
    if *bound != Some(pipeline) {
        pass.set_pipeline(pipeline);
        counters.pipeline_switches += 1;
        *bound = Some(pipeline);
    }
}

#[cfg(test)]
mod tests {
    use super::{LinearRgba, Pass, PassRecorder};
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

    /// `WGPU_DEBUG=1` wraps each pass in a debug group; wgpu fails the
    /// frame if a group is left open or closed twice. Set here directly, so
    /// that the path runs without the variable.
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
        gpu.debug = true;
        let targets = Targets::new(&gpu, 4, 4).unwrap_or_else(|e| panic!("{e}"));
        Graph::fixed(Target::Offscreen)
            .render(&gpu, &targets, &mut [&mut Clear(DRAW), &mut Clear(TONEMAP)])
            .unwrap_or_else(|e| panic!("{e}"));
    }
}
