//! Mesh pipelines (rung 2; ADR-0020): indexed triangles from a scene's
//! megabuffers, each instance a record of the frame, into a pass's colour
//! targets and its depth target. The first-person `opaque` and `sprites`
//! passes draw this way. They sit beside 1b's instanced pipelines, which
//! stay as they are.

use std::ops::Range;

use crate::error::GpuError;
use crate::gpu::Gpu;
use crate::graph::Resource;
use crate::instanced::Attribute;
use crate::layout::{self, Layouts};

/// A mesh pipeline: both stages, the vertex layout, and the pass it draws
/// in, what that pass reads and writes.
#[derive(Clone, Copy, Debug)]
pub struct MeshDesc<'a> {
    /// Names the pipeline in errors and GPU debuggers.
    pub label: &'a str,
    /// A WGSL module with both entry points. It binds the frame and the
    /// scene, and the pass's reads if it has any (`layout.rs`):
    /// - `@group(0) @binding(0) var<uniform> frame: Frame`, the caller's
    ///   per-frame uniform, and `@group(0) @binding(1) var<storage, read>
    ///   instances: array<Instance>` ([`crate::InstanceRecord`]), indexed by
    ///   `@builtin(instance_index)`;
    /// - `@group(1)`: the material table, the six texture arrays and their
    ///   sampler ([`crate::SceneDesc`]);
    /// - `@group(2)`: the pass's reads, then a sampler.
    ///
    /// Each colour target is `@location(i)` of the fragment stage, in the
    /// order the pass writes them, `vec4<u32>` for
    /// [`crate::Format::Rg32Uint`]. Load it from a `.wgsl` file with
    /// `include_str!`, so that the shader test checks it.
    pub wgsl: &'a str,
    pub vertex_entry: &'a str,
    pub fragment_entry: &'a str,
    /// One vertex's attributes in order, at `@location(0)` onwards, packed
    /// with no padding in the scene's vertex buffer.
    pub vertex: &'a [Attribute],
    /// What the pass it draws in reads, in order.
    pub reads: &'a [Resource],
    /// What that pass writes, in order: up to four colour targets and one
    /// depth target, never the surface. Draws replace what is there,
    /// without blending.
    pub writes: &'a [Resource],
    /// Whether draws write the depth target. They always test it when the
    /// pass has one: eng-gpu's depth is reversed, so a fragment passes when
    /// its depth is greater than or equal to what is there.
    pub depth_write: bool,
    /// Whether triangles whose corners run clockwise on screen are
    /// dropped: counter-clockwise ones face the camera.
    pub cull_back: bool,
}

/// A compiled mesh pipeline, for [`crate::PassRecorder::mesh`]. Cheap to
/// clone.
#[derive(Clone, Debug)]
pub struct MeshPipeline {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) reads: Vec<Resource>,
    pub(crate) writes: Vec<Resource>,
    pub(crate) reads_layout: Option<wgpu::BindGroupLayout>,
}

/// One draw of a mesh pipeline: a range of the scene's indices, offset by
/// `base_vertex`, once for each of the frame's instance records in
/// `instances`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshDraw {
    pub indices: Range<u32>,
    /// Added to each index before it reads the vertex buffer, so that a
    /// chunk's indices may count from its own first vertex.
    pub base_vertex: i32,
    /// Indexes into [`crate::FrameInput::instances`].
    pub instances: Range<u32>,
}

impl Gpu {
    /// Compiles a mesh pipeline for passes that read `desc.reads` and write
    /// `desc.writes`. Fails with `[E-SHADER]` if the shader does not compile
    /// against the layout, the writes break a pass's rules (at least one,
    /// at most four colour targets and one depth target, no surface), or the
    /// reads are more textures than a stage samples beside the scene's.
    pub fn mesh_pipeline(&self, desc: &MeshDesc<'_>) -> Result<MeshPipeline, GpuError> {
        let shader_error = |message: String| GpuError::Shader {
            label: desc.label.to_string(),
            message,
        };
        let (colours, depth) = layout::attachments(desc.writes)
            .map_err(|e| shader_error(format!("its pass cannot draw: {e}")))?;
        layout::check_reads(desc.reads, true).map_err(shader_error)?;
        self.scoped(|| {
            let reads_layout = self.layouts.reads(&self.device, desc.reads);
            pipeline(
                &self.device,
                &self.layouts,
                desc,
                &colours,
                depth,
                reads_layout,
            )
        })
        .map_err(shader_error)
    }
}

fn pipeline(
    device: &wgpu::Device,
    layouts: &Layouts,
    desc: &MeshDesc<'_>,
    colours: &[wgpu::TextureFormat],
    depth: Option<wgpu::TextureFormat>,
    reads_layout: Option<wgpu::BindGroupLayout>,
) -> MeshPipeline {
    let mut attributes = Vec::with_capacity(desc.vertex.len());
    let mut stride = 0;
    for (location, attribute) in (0..).zip(desc.vertex) {
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
    let layout = layout::pipeline_layout(
        device,
        desc.label,
        &[
            Some(&layouts.frame),
            Some(&layouts.scene),
            reads_layout.as_ref(),
        ],
    );
    let buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: stride,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &attributes,
    })];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(desc.label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(desc.vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: if desc.vertex.is_empty() {
                &[]
            } else {
                &buffers
            },
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: desc.cull_back.then_some(wgpu::Face::Back),
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: depth.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(desc.depth_write),
            depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(desc.fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &layout::colour_states(colours),
        }),
        multiview_mask: None,
        cache: None,
    });
    MeshPipeline {
        pipeline,
        reads: desc.reads.to_vec(),
        writes: desc.writes.to_vec(),
        reads_layout,
    }
}
