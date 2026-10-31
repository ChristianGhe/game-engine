//! Instanced pipelines: a primitive drawn once per instance, each instance
//! reading its own attributes from bytes a pass records with the frame.
//! eng-render's top-down pass draws its shapes this way.

use crate::graph::Resource;

/// The type of one per-instance attribute: what the vertex stage reads at
/// `@location(n)`, `n` being its place in [`InstancedDesc::instance`].
/// Each is stored little-endian.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribute {
    /// `vec2<f32>`, 8 bytes.
    F32x2,
    /// `vec3<f32>`, 12 bytes.
    F32x3,
    /// `u32`, 4 bytes.
    U32,
}

impl Attribute {
    /// Its size in bytes.
    pub const fn size(self) -> u64 {
        match self {
            Self::F32x2 => 8,
            Self::F32x3 => 12,
            Self::U32 => 4,
        }
    }

    fn format(self) -> wgpu::VertexFormat {
        match self {
            Self::F32x2 => wgpu::VertexFormat::Float32x2,
            Self::F32x3 => wgpu::VertexFormat::Float32x3,
            Self::U32 => wgpu::VertexFormat::Uint32,
        }
    }
}

/// An instanced pipeline: both stages and the instance layout.
pub struct InstancedDesc<'a> {
    /// Names the pipeline in errors and GPU debuggers.
    pub label: &'a str,
    /// A WGSL module with both entry points. The vertex stage takes
    /// `@builtin(vertex_index)`, from 0 to `vertices - 1`, and the instance's
    /// attributes at `@location(0)` onwards. Load it from a `.wgsl` file with
    /// `include_str!`, so that the shader test checks it.
    pub wgsl: &'a str,
    pub vertex_entry: &'a str,
    pub fragment_entry: &'a str,
    /// One instance's attributes in order, packed with no padding: an
    /// instance takes the sum of their sizes. At least one.
    pub instance: &'a [Attribute],
    /// The vertices of each instance's primitive, a triangle strip: 4 make a
    /// quad.
    pub vertices: u32,
    /// What the pipeline draws into: the scene or the output. It sets the
    /// colour format; draws replace what is there, without blending.
    pub target: Resource,
}

/// A compiled instanced pipeline, for [`crate::PassRecorder::instanced`].
/// Cheap to clone.
#[derive(Clone, Debug)]
pub struct InstancedPipeline {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) target: Resource,
    /// Bytes per instance.
    pub(crate) stride: u64,
    pub(crate) vertices: u32,
}

/// Compiles `desc`, whose instance layout is not empty, for `format`.
pub(crate) fn pipeline(
    device: &wgpu::Device,
    desc: &InstancedDesc<'_>,
    format: wgpu::TextureFormat,
) -> InstancedPipeline {
    let mut attributes = Vec::with_capacity(desc.instance.len());
    let mut stride = 0;
    for (location, attribute) in (0..).zip(desc.instance) {
        attributes.push(wgpu::VertexAttribute {
            format: attribute.format(),
            offset: stride,
            shader_location: location,
        });
        stride += attribute.size();
    }
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(desc.label),
        source: wgpu::ShaderSource::Wgsl(desc.wgsl.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(desc.label),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(desc.label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(desc.vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: stride,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &attributes,
            })],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(desc.fragment_entry),
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
    InstancedPipeline {
        pipeline,
        target: desc.target,
        stride,
        vertices: desc.vertices,
    }
}
