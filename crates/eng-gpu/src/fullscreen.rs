//! Fullscreen pipelines: one triangle over the target, whose fragment stage
//! may sample the pass's one read. Tone mapping and the present blit are
//! fullscreen passes.

use crate::graph::Resource;

/// The vertex stage of every fullscreen pass.
const FULLSCREEN_WGSL: &str = include_str!("fullscreen.wgsl");

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

/// A compiled fullscreen pipeline, for [`crate::PassRecorder::fullscreen`].
/// Cheap to clone.
#[derive(Clone, Debug)]
pub struct FullscreenPipeline {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) target: Resource,
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

    /// The bind group of a pass's input.
    pub(crate) fn bind(&self, device: &wgpu::Device, input: &wgpu::TextureView) -> wgpu::BindGroup {
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
