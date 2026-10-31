//! Passes and how a graph runs them. A pass records commands into a
//! [`PassRecorder`]; once every pass has recorded and each recording fits
//! its declaration, the frame's instances are uploaded, and the frame is
//! encoded, counted and submitted at once.

use crate::counters::FrameCounters;
use crate::error::GpuError;
use crate::fullscreen::FullscreenPipeline;
use crate::gpu::Gpu;
use crate::graph::{Graph, GraphError, PassDecl, Resource};
use crate::instanced::InstancedPipeline;
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
    /// What a pass's target starts as unless the pass clears it.
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
    /// What the pass reads and writes.
    fn decl(&self) -> PassDecl;
    /// Records this frame's work. It reaches the GPU after every pass has
    /// recorded.
    fn record(&mut self, rec: &mut PassRecorder);
}

/// The commands a pass records for one frame, drawn in order.
#[derive(Debug)]
pub struct PassRecorder {
    clear: LinearRgba,
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
            clear: LinearRgba::TRANSPARENT,
            commands: Vec::new(),
        }
    }

    /// Starts the pass's target as `colour` rather than
    /// [`LinearRgba::TRANSPARENT`].
    pub fn clear(&mut self, colour: LinearRgba) {
        self.clear = colour;
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

    /// Whether the recording fits the pass's declaration.
    fn check(&self, decl: &PassDecl) -> Result<(), GpuError> {
        let fail = |message: String| GpuError::Pass {
            pass: decl.name,
            message,
        };
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
    /// same order, then submits the frame and returns what it counted. Each
    /// pass draws into the target it writes; a later
    /// [`Targets::read_output`] sees the frame.
    pub fn render(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        passes: &mut [&mut dyn Pass],
    ) -> Result<FrameCounters, GpuError> {
        if passes.len() != self.passes.len() {
            return Err(GraphError::PassCount {
                expected: self.passes.len(),
                found: passes.len(),
            }
            .into());
        }
        let mut recorded = Vec::with_capacity(passes.len());
        for (index, (decl, pass)) in self.passes.iter().zip(passes.iter_mut()).enumerate() {
            let found = pass.decl();
            if found != *decl {
                return Err(GraphError::PassMismatch {
                    index,
                    expected: *decl,
                    found,
                }
                .into());
            }
            let mut rec = PassRecorder::new();
            pass.record(&mut rec);
            rec.check(decl)?;
            recorded.push(rec);
        }
        // Only a surface graph uses the surface, which the targets may lack.
        // Each pass writes exactly one resource (Graph::new).
        let mut views = Vec::with_capacity(recorded.len());
        for decl in &self.passes {
            let view = |r: Resource| targets.view(r).ok_or(GpuError::NoSurface);
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
            for ((decl, rec), (target, input)) in self.passes.iter().zip(&recorded).zip(&views) {
                frame.encode(&mut encoder, decl, rec, target, *input);
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
    fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        decl: &PassDecl,
        rec: &PassRecorder,
        target: &wgpu::TextureView,
        input: Option<&wgpu::TextureView>,
    ) {
        let clear = wgpu::Color {
            r: f64::from(rec.clear.r),
            g: f64::from(rec.clear.g),
            b: f64::from(rec.clear.b),
            a: f64::from(rec.clear.a),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(decl.name),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
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
                        let bind_group = self.gpu.shared.bind(&self.gpu.device, input);
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
