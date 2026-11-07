//! GPU time per pass (spec 6.7: timestamp queries "when available, with
//! null otherwise"; rung 2 plan, O11). A device asks for `TIMESTAMP_QUERY`
//! only when its adapter has it ([`crate::Gpu::new`]); the tier stays 0.
//! Then every frame writes a timestamp at the start and end of each pass
//! that runs, resolves them into a buffer and maps it back after the
//! submit, without waiting. [`crate::Targets::frame_stats`] waits for the
//! last frame's and turns them into [`PassTime`]s.
//!
//! The times are measurements, so they differ from run to run and adapter
//! to adapter; they go in reports, never into budgets that must hold
//! everywhere, and never into the simulation.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use crate::error::GpuError;
use crate::gpu::Gpu;
use crate::targets::READBACK_LIMIT;

/// The GPU time of one pass of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassTime {
    /// The pass's name.
    pub pass: &'static str,
    /// From the GPU's timestamp at the pass's start to the one at its end,
    /// in nanoseconds.
    pub nanoseconds: u64,
}

/// What the last frame rendered into a [`crate::Targets`] measured beyond
/// its [`crate::FrameCounters`], which stay as rung 1b fixed them: the GPU
/// time of each pass and the size of the targets' pool. Reports carry them
/// in their own `gpu` keys, beside `per_frame`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// One for each pass that ran, in running order: culled and bypassed
    /// passes have none. `None` when the device cannot time passes (the
    /// adapter has no timestamp queries, [`crate::Capabilities`]), or no
    /// frame has been rendered into the targets yet.
    pub pass_times: Option<Vec<PassTime>>,
    /// The transient textures the targets keep
    /// ([`crate::Targets::transient_textures`]).
    pub transient_textures: usize,
}

/// The timestamps of one frame being encoded: two per pass, at its start
/// and its end.
pub(crate) struct FrameQueries {
    set: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    readback: wgpu::Buffer,
    count: u32,
}

/// Bytes per timestamp.
const TIMESTAMP_BYTES: u64 = 8;

impl FrameQueries {
    /// Queries for `passes` passes, or `None` when the device does not time
    /// passes or there is none to time.
    pub(crate) fn new(gpu: &Gpu, passes: usize) -> Option<Self> {
        gpu.timestamp_period?;
        let count = u32::try_from(passes.checked_mul(2)?).ok()?;
        // wgpu refuses a query set of more than 4,096 queries; a frame of
        // more than 2,048 passes is not timed.
        if count == 0 || count > wgpu::QUERY_SET_MAX_QUERIES {
            return None;
        }
        let size = u64::from(count) * TIMESTAMP_BYTES;
        let set = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pass times"),
            ty: wgpu::QueryType::Timestamp,
            count,
        });
        let buffer = |label: &str, usage: wgpu::BufferUsages| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Some(Self {
            set,
            resolved: buffer(
                "pass times",
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            ),
            readback: buffer(
                "pass times readback",
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
            count,
        })
    }

    /// Where the `pass`-th pass that runs writes its two timestamps.
    pub(crate) fn writes(&self, pass: usize) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let begin = u32::try_from(pass).ok()?.checked_mul(2)?;
        (begin + 1 < self.count).then_some(wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        })
    }

    /// Resolves the frame's timestamps and copies them where the CPU can
    /// map them, at the end of the frame's commands.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.set, 0..self.count, &self.resolved, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolved,
            0,
            &self.readback,
            0,
            u64::from(self.count) * TIMESTAMP_BYTES,
        );
    }

    /// After the submit: starts mapping the timestamps back, and returns
    /// what [`crate::Targets::frame_stats`] waits for.
    pub(crate) fn map(self, passes: Vec<&'static str>, period: f32) -> Timed {
        let (sender, receiver) = channel();
        self.readback
            .map_async(wgpu::MapMode::Read, .., move |mapped| {
                // The receiver may be gone if a later frame replaced this one.
                let _ = sender.send(mapped.is_ok());
            });
        Timed::Pending {
            passes,
            buffer: self.readback,
            mapped: receiver,
            period,
        }
    }
}

/// The last timed frame of some targets.
pub(crate) enum Timed {
    /// No frame has been timed.
    None,
    /// Its timestamps are on their way back.
    Pending {
        passes: Vec<&'static str>,
        buffer: wgpu::Buffer,
        mapped: Receiver<bool>,
        /// Nanoseconds per timestamp tick.
        period: f32,
    },
    /// Read back already.
    Read(Vec<PassTime>),
}

impl Timed {
    /// The frame's times, waiting for the GPU to finish it (at most
    /// [`READBACK_LIMIT`]); `None` if no frame was timed.
    pub(crate) fn times(&mut self, gpu: &Gpu) -> Result<Option<Vec<PassTime>>, GpuError> {
        let read = match self {
            Self::None => return Ok(None),
            Self::Read(times) => return Ok(Some(times.clone())),
            Self::Pending {
                passes,
                buffer,
                mapped,
                period,
            } => {
                let ready = match mapped.try_recv() {
                    Ok(ok) => Some(ok),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => Some(false),
                };
                let ok = match ready {
                    Some(ok) => ok,
                    None => {
                        gpu.device
                            .poll(wgpu::PollType::Wait {
                                submission_index: None,
                                timeout: Some(READBACK_LIMIT),
                            })
                            .map_err(|e| {
                                GpuError::Wgpu(format!(
                                    "waiting for the pass times (at most {} s): {e}",
                                    READBACK_LIMIT.as_secs()
                                ))
                            })?;
                        mapped.try_recv().unwrap_or(false)
                    }
                };
                if !ok {
                    return Err(GpuError::Wgpu(
                        "the pass times were not mapped after the wait".into(),
                    ));
                }
                let times = read_times(buffer, passes, *period)?;
                buffer.unmap();
                times
            }
        };
        *self = Self::Read(read.clone());
        Ok(Some(read))
    }
}

/// Each pass's time from the mapped timestamps, two per pass.
fn read_times(
    buffer: &wgpu::Buffer,
    passes: &[&'static str],
    period: f32,
) -> Result<Vec<PassTime>, GpuError> {
    let mapped = buffer
        .get_mapped_range(..)
        .map_err(|e| GpuError::Wgpu(format!("reading the pass times: {e}")))?;
    let stamps: Vec<u64> = mapped
        .as_chunks::<8>()
        .0
        .iter()
        .map(|bytes| u64::from_le_bytes(*bytes))
        .collect();
    drop(mapped);
    Ok(passes
        .iter()
        .zip(stamps.as_chunks::<2>().0)
        .map(|(&pass, [begin, end])| PassTime {
            pass,
            nanoseconds: nanoseconds(end.saturating_sub(*begin), period),
        })
        .collect())
}

/// `ticks` of `period` nanoseconds each, truncated.
pub(crate) fn nanoseconds(ticks: u64, period: f32) -> u64 {
    // Exact for any frame shorter than 2^53 ticks; the cast saturates.
    (ticks as f64 * f64::from(period)) as u64
}

#[cfg(test)]
mod tests {
    use super::nanoseconds;

    #[test]
    fn ticks_turn_into_nanoseconds() {
        assert_eq!(nanoseconds(0, 1.0), 0);
        assert_eq!(nanoseconds(1_000, 1.0), 1_000);
        // A 24 MHz tick, about 41.67 ns, truncated.
        assert_eq!(nanoseconds(3, 41.666_668), 125);
        assert_eq!(nanoseconds(u64::MAX, 2.0), u64::MAX);
    }
}
