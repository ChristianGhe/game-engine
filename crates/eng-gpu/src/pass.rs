//! Passes and how a graph runs them. A pass records commands into a
//! [`PassRecorder`]; once every pass has recorded and each recording fits
//! its declaration, the frame is encoded and submitted at once.

use crate::error::GpuError;
use crate::gpu::Gpu;
use crate::graph::{Graph, GraphError, PassDecl, Resource};
use crate::targets::Targets;

/// The vertex stage of every fullscreen pass.
const FULLSCREEN_WGSL: &str = include_str!("fullscreen.wgsl");

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

/// A fullscreen pass's fragment stage.
pub struct FullscreenDesc<'a> {
    /// Names the pipeline in errors and GPU debuggers.
    pub label: &'a str,
    /// A WGSL module with the fragment entry point. It takes
    /// `@location(0) uv: vec2<f32>`, from (0, 0) at the top left to (1, 1) at
    /// the bottom right, and may bind the pass's input as
    /// `@group(0) @binding(0) var input: texture_2d<f32>` and
    /// `@group(0) @binding(1) var input_sampler: sampler`, which samples the
    /// nearest texel. Load it from a `.wgsl` file with `include_str!`, so
    /// that the shader test checks it.
    pub wgsl: &'a str,
    /// The fragment entry point's name.
    pub entry: &'a str,
    /// What the pipeline draws into; it sets the colour format.
    pub target: Resource,
}

/// A compiled fullscreen pipeline, for [`PassRecorder::fullscreen`]. Cheap
/// to clone.
#[derive(Clone, Debug)]
pub struct FullscreenPipeline {
    pipeline: wgpu::RenderPipeline,
    target: Resource,
}

/// The commands a pass records for one frame.
#[derive(Debug)]
pub struct PassRecorder {
    clear: LinearRgba,
    commands: Vec<Command>,
}

#[derive(Debug)]
enum Command {
    Fullscreen(FullscreenPipeline),
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

    /// Whether the recording fits the pass's declaration.
    fn check(&self, decl: &PassDecl) -> Result<(), GpuError> {
        let fail = |message: String| GpuError::Pass {
            pass: decl.name,
            message,
        };
        for Command::Fullscreen(pipeline) in &self.commands {
            if decl.reads.len() != 1 {
                return Err(fail(format!(
                    "a fullscreen draw takes the pass's one read as its input, but the pass reads {}",
                    decl.reads.len()
                )));
            }
            if decl.writes != [pipeline.target] {
                return Err(fail(format!(
                    "the pipeline draws into `{}`, but the pass writes `{}`",
                    pipeline.target.name(),
                    decl.writes.first().map_or("nothing", |r| r.name())
                )));
            }
        }
        Ok(())
    }
}

impl Graph {
    /// Records `passes`, one for each of the graph's declarations and in the
    /// same order, then submits the frame. Each pass draws into the target
    /// it writes; a later [`Targets::read_output`] sees the frame.
    pub fn render(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        passes: &mut [&mut dyn Pass],
    ) -> Result<(), GpuError> {
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
        // Only a surface graph has the surface, and eng-gpu cannot render one
        // yet. Each pass writes exactly one resource (Graph::new).
        let mut views = Vec::with_capacity(recorded.len());
        for decl in &self.passes {
            let view = |r: Resource| {
                targets.view(r).ok_or_else(|| GpuError::Pass {
                    pass: decl.name,
                    message: "eng-gpu renders offscreen only; it opens no surface yet".into(),
                })
            };
            let input = decl.reads.first().map(|&r| view(r)).transpose()?;
            views.push((view(decl.writes[0])?, input));
        }
        gpu.scoped(|| {
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("frame"),
                });
            for ((decl, rec), (target, input)) in self.passes.iter().zip(&recorded).zip(&views) {
                encode(gpu, &mut encoder, decl, rec, target, *input);
            }
            gpu.queue.submit([encoder.finish()]);
        })
        .map_err(GpuError::Wgpu)
    }
}

fn encode(
    gpu: &Gpu,
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
    for Command::Fullscreen(pipeline) in &rec.commands {
        // `check` made sure a fullscreen pass has its input.
        let Some(input) = input else { continue };
        let bind_group = gpu.shared.bind(&gpu.device, input);
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// What every fullscreen pipeline shares: the vertex stage, the input's
/// layout and its sampler.
pub(crate) struct Shared {
    vertex: wgpu::ShaderModule,
    inputs: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
}

impl Shared {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let vertex = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fullscreen"),
            source: wgpu::ShaderSource::Wgsl(FULLSCREEN_WGSL.into()),
        });
        let inputs = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fullscreen input"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fullscreen"),
            bind_group_layouts: &[Some(&inputs)],
            immediate_size: 0,
        });
        // Nearest texel, clamped at the edges: a same-size pass copies texels
        // exactly.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fullscreen input"),
            ..wgpu::SamplerDescriptor::default()
        });
        Self {
            vertex,
            inputs,
            layout,
            sampler,
        }
    }

    pub(crate) fn pipeline(
        &self,
        device: &wgpu::Device,
        desc: &FullscreenDesc<'_>,
        format: wgpu::TextureFormat,
    ) -> FullscreenPipeline {
        let fragment = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(desc.label),
            source: wgpu::ShaderSource::Wgsl(desc.wgsl.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(desc.label),
            layout: Some(&self.layout),
            vertex: wgpu::VertexState {
                module: &self.vertex,
                entry_point: Some("fullscreen_vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &fragment,
                entry_point: Some(desc.entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        FullscreenPipeline {
            pipeline,
            target: desc.target,
        }
    }

    fn bind(&self, device: &wgpu::Device, input: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fullscreen input"),
            layout: &self.inputs,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}
