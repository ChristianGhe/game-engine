//! Encoding one pass of a frame: its render pass, its draws, and what it
//! binds, counted.

use super::Views;
use crate::counters::FrameCounters;
use crate::gpu::Gpu;
use crate::graph::PassDecl;
use crate::layout::{FRAME_GROUP, READS_GROUP, SCENE_GROUP};
use crate::pass::{Command, LinearRgba, PassRecorder};
use crate::scene::Scene;

/// What is bound at one of a render pass's groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bound {
    /// 1b's fullscreen input.
    Input,
    Frame,
    Scene,
    Reads,
}

/// One frame being encoded: where the next instances start in the frame's
/// buffer, the frame's own group and scene, and what has been counted so
/// far.
pub(super) struct Frame<'a> {
    pub(super) gpu: &'a Gpu,
    pub(super) instances: Option<wgpu::Buffer>,
    pub(super) cursor: u64,
    pub(super) group: Option<wgpu::BindGroup>,
    pub(super) scene: Option<&'a Scene>,
    pub(super) counters: FrameCounters,
}

/// What one render pass has bound so far: nothing carries over between
/// passes.
#[derive(Default)]
struct PassState<'p> {
    pipeline: Option<&'p wgpu::RenderPipeline>,
    groups: [Option<Bound>; 3],
    /// Whether vertex slot 0 and the index buffer hold the scene's.
    scene_buffers: bool,
    /// The bind groups made for the pass: 1b's input and the reads.
    input: Option<wgpu::BindGroup>,
    reads: Option<wgpu::BindGroup>,
}

impl Frame<'_> {
    /// Encodes one pass, which `loads` its targets if it continues an
    /// earlier pass, and clears them otherwise.
    pub(super) fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        decl: &PassDecl,
        rec: &PassRecorder,
        loads: bool,
        views: &Views<'_>,
        timestamps: Option<wgpu::RenderPassTimestampWrites<'_>>,
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
        let attachments: Vec<Option<wgpu::RenderPassColorAttachment<'_>>> = views
            .colours
            .iter()
            .map(|&view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })
            })
            .collect();
        // Reversed depth: the far plane is 0.
        let depth_load = if loads {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(0.0)
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(decl.name),
            color_attachments: &attachments,
            depth_stencil_attachment: views.depth.map(|view| {
                wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: depth_load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }
            }),
            timestamp_writes: timestamps,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        // Upscaled targets' present pass: the clear covers the whole window
        // image, the draws only the scaled output.
        if let Some([x, y, width, height]) = views.viewport {
            pass.set_viewport(x, y, width, height, 0.0, 1.0);
        }
        let mut state = PassState::default();
        for command in &rec.commands {
            match command {
                Command::Fullscreen(fullscreen) => {
                    // `check` made sure a fullscreen pass has its input.
                    let Some(&input) = views.reads.first() else {
                        continue;
                    };
                    self.pipeline(&mut pass, &mut state, &fullscreen.pipeline);
                    if state.groups[0] != Some(Bound::Input) {
                        let group = state.input.get_or_insert_with(|| {
                            self.gpu.shared.bind(&self.gpu.device, input, decl.name)
                        });
                        self.bind(&mut pass, &mut state.groups, 0, Bound::Input, group);
                    }
                    pass.draw(0..3, 0..1);
                }
                Command::Instanced { pipeline, bytes } => {
                    // `upload` made the buffer from every instanced command.
                    let Some(instances) = self.instances.clone() else {
                        continue;
                    };
                    self.pipeline(&mut pass, &mut state, &pipeline.pipeline);
                    let len = bytes.len() as u64;
                    // Tier 0 buffers hold at most 256 MiB, far fewer instances
                    // than a u32 counts; a larger upload already failed.
                    let count = u32::try_from(len / pipeline.stride).unwrap_or(u32::MAX);
                    pass.set_vertex_buffer(0, instances.slice(self.cursor..self.cursor + len));
                    state.scene_buffers = false;
                    self.cursor += len;
                    pass.draw(0..pipeline.vertices, 0..count);
                }
                Command::Mesh { pipeline, draw } => {
                    // `check` made sure the frame has a scene.
                    let Some(scene) = self.scene else { continue };
                    self.pipeline(&mut pass, &mut state, &pipeline.pipeline);
                    self.frame_groups(&mut pass, &mut state, Some(scene));
                    self.reads(
                        &mut pass,
                        &mut state,
                        pipeline.reads_layout.as_ref(),
                        views,
                        decl,
                    );
                    if !state.scene_buffers {
                        pass.set_vertex_buffer(0, scene.vertices.slice(..));
                        pass.set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
                        state.scene_buffers = true;
                    }
                    pass.draw_indexed(
                        draw.indices.clone(),
                        draw.base_vertex,
                        draw.instances.clone(),
                    );
                }
                Command::Screen(screen) => {
                    self.pipeline(&mut pass, &mut state, &screen.pipeline);
                    let scene = if screen.scene { self.scene } else { None };
                    self.frame_groups(&mut pass, &mut state, scene);
                    self.reads(
                        &mut pass,
                        &mut state,
                        screen.reads_layout.as_ref(),
                        views,
                        decl,
                    );
                    pass.draw(0..3, 0..1);
                }
            }
            self.counters.draws += 1;
        }
    }

    /// Binds `pipeline` unless the pass has it bound already.
    fn pipeline<'p>(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        state: &mut PassState<'p>,
        pipeline: &'p wgpu::RenderPipeline,
    ) {
        if state.pipeline != Some(pipeline) {
            pass.set_pipeline(pipeline);
            self.counters.pipeline_switches += 1;
            state.pipeline = Some(pipeline);
        }
    }

    /// Binds `group` at `index` unless `what` is bound there already.
    fn bind(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        groups: &mut [Option<Bound>; 3],
        index: u32,
        what: Bound,
        group: &wgpu::BindGroup,
    ) {
        let slot = &mut groups[index as usize];
        if *slot != Some(what) {
            pass.set_bind_group(index, group, &[]);
            self.counters.bind_group_switches += 1;
            *slot = Some(what);
        }
    }

    /// Binds the frame's group, and the scene's when the pipeline takes it.
    fn frame_groups(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        state: &mut PassState<'_>,
        scene: Option<&Scene>,
    ) {
        // `encode` made the frame group for a frame with such a pipeline.
        if let Some(frame) = self.group.clone() {
            self.bind(pass, &mut state.groups, FRAME_GROUP, Bound::Frame, &frame);
        }
        if let Some(scene) = scene {
            self.bind(
                pass,
                &mut state.groups,
                SCENE_GROUP,
                Bound::Scene,
                &scene.group,
            );
        }
    }

    /// Binds the pass's reads, made once for the pass, when the pipeline
    /// has any.
    fn reads(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        state: &mut PassState<'_>,
        layout: Option<&wgpu::BindGroupLayout>,
        views: &Views<'_>,
        decl: &PassDecl,
    ) {
        let Some(layout) = layout else { return };
        if state.groups[READS_GROUP as usize] == Some(Bound::Reads) {
            return;
        }
        let gpu = self.gpu;
        let group = state.reads.get_or_insert_with(|| {
            let mut entries: Vec<wgpu::BindGroupEntry<'_>> = (0..)
                .zip(&views.reads)
                .map(|(binding, &view)| wgpu::BindGroupEntry {
                    binding,
                    resource: wgpu::BindingResource::TextureView(view),
                })
                .collect();
            entries.push(wgpu::BindGroupEntry {
                binding: u32::try_from(views.reads.len()).unwrap_or(u32::MAX),
                resource: wgpu::BindingResource::Sampler(&gpu.layouts.clamped),
            });
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("{} reads", decl.name)),
                layout,
                entries: &entries,
            })
        });
        let group = group.clone();
        self.bind(pass, &mut state.groups, READS_GROUP, Bound::Reads, &group);
    }
}
