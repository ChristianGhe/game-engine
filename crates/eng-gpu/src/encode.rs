//! Encoding a frame: the views each pass draws into and reads, the frame's
//! buffers, one render pass per pass that runs, the counters, and the
//! timestamps when the device has them (`timing.rs`).

use crate::counters::FrameCounters;
use crate::error::GpuError;
use crate::gpu::Gpu;
use crate::graph::{Graph, GraphError, PassDecl, Resource};
use crate::layout::{self, MAX_COLOUR_TARGETS};
use crate::pass::{Command, FrameInput, Pass, PassRecorder};
use crate::scene::{self, InstanceRecord};
use crate::targets::Targets;

mod draw;

use draw::Frame;

/// The views one pass draws into and reads.
struct Views<'a> {
    colours: Vec<&'a wgpu::TextureView>,
    depth: Option<&'a wgpu::TextureView>,
    reads: Vec<&'a wgpu::TextureView>,
}

impl Graph {
    /// Encodes and submits a frame, and returns its counters and how many
    /// debug groups it opened.
    pub(crate) fn encode(
        &self,
        gpu: &Gpu,
        targets: &Targets,
        input: &FrameInput<'_>,
        passes: &mut [&mut dyn Pass],
    ) -> Result<(FrameCounters, u32), GpuError> {
        // Held for the frame. Its times go first, so that a frame that fails
        // leaves none of the last frame's.
        let mut timing = targets.timing();
        timing.forget();
        // Likewise which transients the pool holds, until this frame is in.
        *targets.drawn() = None;
        if passes.len() != self.declared.len() {
            return Err(GraphError::PassCount {
                expected: self.declared.len(),
                found: passes.len(),
            }
            .into());
        }
        let matches = |index: usize, pass: &dyn Pass| {
            let (expected, found) = (self.declared[index], pass.decl());
            if found == expected {
                Ok(())
            } else {
                Err(GraphError::PassMismatch {
                    index,
                    expected,
                    found,
                })
            }
        };
        // The passes that run, in running order: for a Graph::new graph,
        // the order given, as in rung 1b.
        let mut recorded = Vec::with_capacity(self.slots.len());
        for (at, &index) in self.slots.iter().enumerate() {
            matches(index, &*passes[index])?;
            let decl = &self.declared[index];
            limits(decl)?;
            let mut rec = PassRecorder::new();
            passes[index].record(&mut rec);
            rec.check(decl, self.loads[at], input)?;
            recorded.push(rec);
        }
        // Then the culled and bypassed passes, which do not record.
        for (index, pass) in passes.iter().enumerate() {
            if !self.slots.contains(&index) {
                matches(index, &**pass)?;
            }
        }
        let pooled = if self.plan.textures().is_empty() {
            Vec::new()
        } else {
            targets.pooled(gpu, &self.plan)?
        };
        let surface = input.surface.map(|frame| &frame.view);
        // Only a surface graph uses the surface, which the targets may lack.
        // Each resource is drawn into the texture of the resource it takes,
        // if bypassing made it take another's.
        let view = |r: Resource| match (self.aliases.of(r), surface) {
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
            (r, _) => targets.view(r).ok_or(GpuError::NoSurface),
        };
        let mut views = Vec::with_capacity(recorded.len());
        for decl in &self.passes {
            let mut colours = Vec::new();
            let mut depth = None;
            for &write in decl.writes {
                if layout::is_depth(write) {
                    depth = Some(view(write)?);
                } else {
                    colours.push(view(write)?);
                }
            }
            let reads = decl
                .reads
                .iter()
                .map(|&r| view(r))
                .collect::<Result<_, _>>()?;
            views.push(Views {
                colours,
                depth,
                reads,
            });
        }
        // Every 1b instance of the frame, in the order the passes draw them.
        let commands = || recorded.iter().flat_map(|rec| &rec.commands);
        let uploads: Vec<u8> = commands()
            .filter_map(|command| match command {
                Command::Instanced { bytes, .. } => Some(bytes.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        // The frame group is written only in a frame whose passes bind it.
        let binds_frame =
            commands().any(|c| matches!(c, Command::Mesh { .. } | Command::Screen(_)));
        let records: Vec<u8> = if binds_frame {
            input
                .instances
                .iter()
                .flat_map(|record| InstanceRecord::to_bytes(*record))
                .collect()
        } else {
            Vec::new()
        };
        let frame_bytes = if binds_frame {
            (input.uniform.len() + records.len()) as u64
        } else {
            0
        };
        let names: Vec<&'static str> = self.passes.iter().map(|p| p.name).collect();
        let encoded = gpu.scoped(|| {
            let mut frame = Frame {
                gpu,
                instances: upload(gpu, &uploads),
                cursor: 0,
                group: binds_frame.then(|| frame_group(gpu, input.uniform, &records)),
                scene: input.scene,
                counters: FrameCounters {
                    upload_bytes: uploads.len() as u64 + frame_bytes,
                    ..FrameCounters::default()
                },
            };
            // With timing off, or no timestamp queries, none: the frame
            // is encoded with no timestamps, as before rung 2.
            let queries = timing.begin(gpu, self.passes.len());
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("frame"),
                });
            let mut groups = 0;
            let steps = self.passes.iter().zip(&self.loads).zip(&recorded);
            for (at, (((decl, &loads), rec), views)) in steps.zip(&views).enumerate() {
                if gpu.debug {
                    encoder.push_debug_group(decl.name);
                    groups += 1;
                }
                let timestamps = queries.and_then(|q| q.writes(at));
                frame.encode(&mut encoder, decl, rec, loads, views, timestamps);
                if gpu.debug {
                    encoder.pop_debug_group();
                }
            }
            let timed = queries.is_some();
            if let Some(queries) = queries {
                queries.resolve(&mut encoder, self.passes.len());
            }
            gpu.queue.submit([encoder.finish()]);
            if let (true, Some(period)) = (timed, gpu.timestamp_period) {
                timing.finish(names, period);
            }
            (frame.counters, groups)
        });
        let encoded = encoded.map_err(|e| {
            // wgpu refused the frame: it leaves no times either.
            timing.forget();
            GpuError::Wgpu(e)
        })?;
        *targets.drawn() = Some(self.plan.clone());
        Ok(encoded)
    }
}

/// A pass that runs draws into at most four colour targets and one depth
/// target, all of one size.
fn limits(decl: &PassDecl) -> Result<(), GpuError> {
    let depth = decl.writes.iter().filter(|&&r| layout::is_depth(r)).count();
    let colour = decl.writes.len() - depth;
    let first = decl.writes.first().copied();
    let other = decl
        .writes
        .iter()
        .find(|&&r| first.is_some_and(|f| layout::divisor(r) != layout::divisor(f)));
    let message = if colour > MAX_COLOUR_TARGETS {
        format!("it writes {colour} colour targets; a pass draws into at most {MAX_COLOUR_TARGETS}")
    } else if depth > 1 {
        format!("it writes {depth} depth targets; a pass draws into at most one")
    } else if let (Some(first), Some(&other)) = (first, other) {
        format!(
            "it writes `{}` at 1/{} of the targets' size and `{}` at 1/{}; a pass draws into \
             targets of one size",
            first.name(),
            layout::divisor(first),
            other.name(),
            layout::divisor(other)
        )
    } else {
        return Ok(());
    };
    Err(GpuError::Pass {
        pass: decl.name,
        message,
    })
}

/// The frame's 1b instances in one vertex buffer, or `None` without any.
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

/// The frame group: the uniform and the instance records, written for this
/// frame.
fn frame_group(gpu: &Gpu, uniform: &[u8], records: &[u8]) -> wgpu::BindGroup {
    let uniform = scene::buffer(gpu, "frame uniform", uniform, wgpu::BufferUsages::UNIFORM);
    let records = scene::buffer(
        gpu,
        "instance records",
        records,
        wgpu::BufferUsages::STORAGE,
    );
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("frame"),
        layout: &gpu.layouts.frame,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: records.as_entire_binding(),
            },
        ],
    })
}
