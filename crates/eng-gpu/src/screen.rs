//! Screen pipelines (rung 2; ADR-0020): rung 2's fullscreen passes. Like
//! 1b's [`crate::FullscreenPipeline`]s they draw one triangle over the
//! target with eng-gpu's vertex stage, but they read every one of the
//! pass's reads, whatever its format, bind the frame's uniform (which
//! ADR-0017 said fullscreen passes could not yet do), may bind the scene,
//! and draw into up to four colour targets. The first-person `light`,
//! `sky`, `fog`, `tonemap` and `palette` passes draw this way; 1b's
//! fullscreen pipelines stay as they are.

use crate::error::GpuError;
use crate::gpu::Gpu;
use crate::graph::Resource;
use crate::layout;

/// A screen pass's fragment stage, and the pass it draws in.
#[derive(Clone, Copy, Debug)]
pub struct ScreenDesc<'a> {
    /// Names the pipeline in errors and GPU debuggers.
    pub label: &'a str,
    /// A WGSL module with the fragment entry point. It takes
    /// `@location(0) uv: vec2<f32>`, from (0, 0) at the top left to (1, 1) at
    /// the bottom right, and `@builtin(position)`, whose `xy` is the pixel's
    /// centre for `textureLoad`. It may bind (`layout.rs`):
    /// - `@group(0) @binding(0) var<uniform> frame: Frame`, the caller's
    ///   per-frame uniform, and `@binding(1)`, the frame's instance records;
    /// - `@group(1)`, the scene, if `scene` is set;
    /// - `@group(2) @binding(i)`, the pass's i-th read: `texture_2d<f32>`
    ///   for colour, `texture_2d<u32>` for [`crate::Format::Rg32Uint`],
    ///   `texture_depth_2d` for [`crate::Format::Depth32Float`]; and
    ///   `@binding(n)`, n the number of reads, a sampler for the nearest
    ///   texel, clamped.
    ///
    /// Each colour target is `@location(i)`, in the order the pass writes
    /// them. Load it from a `.wgsl` file with `include_str!`, so that the
    /// shader test checks it.
    pub wgsl: &'a str,
    /// The fragment entry point's name.
    pub entry: &'a str,
    /// What the pass it draws in reads, in order.
    pub reads: &'a [Resource],
    /// What that pass writes, in order: up to four colour targets, no depth
    /// target and never the surface. Draws replace what is there.
    pub writes: &'a [Resource],
    /// Whether it binds the scene, group 1: then a frame that draws it needs
    /// one ([`crate::FrameInput::scene`]).
    pub scene: bool,
}

/// A compiled screen pipeline, for [`crate::PassRecorder::screen`]. Cheap
/// to clone.
#[derive(Clone, Debug)]
pub struct ScreenPipeline {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) reads: Vec<Resource>,
    pub(crate) writes: Vec<Resource>,
    pub(crate) reads_layout: Option<wgpu::BindGroupLayout>,
    pub(crate) scene: bool,
}

impl Gpu {
    /// Compiles a screen pipeline for passes that read `desc.reads` and
    /// write `desc.writes`. Fails with `[E-SHADER]` if the shader does not
    /// compile against the layout, the writes are not one to four colour
    /// targets, or the reads are more textures than a stage samples.
    pub fn screen_pipeline(&self, desc: &ScreenDesc<'_>) -> Result<ScreenPipeline, GpuError> {
        let shader_error = |message: String| GpuError::Shader {
            label: desc.label.to_string(),
            message,
        };
        let (colours, depth) = layout::attachments(desc.writes)
            .map_err(|e| shader_error(format!("its pass cannot draw: {e}")))?;
        if depth.is_some() {
            return Err(shader_error(
                "a screen pipeline draws colour only; its pass cannot write a depth target".into(),
            ));
        }
        layout::check_reads(desc.reads, desc.scene).map_err(shader_error)?;
        self.scoped(|| {
            let reads_layout = self.layouts.reads(&self.device, desc.reads);
            let fragment = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(desc.label),
                    source: wgpu::ShaderSource::Wgsl(desc.wgsl.into()),
                });
            let layout = layout::pipeline_layout(
                &self.device,
                desc.label,
                &[
                    Some(&self.layouts.frame),
                    desc.scene.then_some(&self.layouts.scene),
                    reads_layout.as_ref(),
                ],
            );
            let pipeline = self
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(desc.label),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &self.shared.vertex,
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
                        targets: &layout::colour_states(&colours),
                    }),
                    multiview_mask: None,
                    cache: None,
                });
            ScreenPipeline {
                pipeline,
                reads: desc.reads.to_vec(),
                writes: desc.writes.to_vec(),
                reads_layout,
                scene: desc.scene,
            }
        })
        .map_err(shader_error)
    }
}
