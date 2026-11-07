//! GPU time per pass (spec 6.7: timestamp queries "when available, with
//! null otherwise"; rung 2 plan, O11). A device asks for `TIMESTAMP_QUERY`
//! only when its adapter has it ([`crate::Gpu::new`]); the tier stays 0.
//!
//! Timing is opt-in, per targets ([`crate::Targets::time_passes`]), so only
//! a caller that wants times pays for them, and a frame with timing off is
//! encoded as it was before: no query set, no timestamp writes, no resolve.
//! With timing on, the targets keep one query set and its two buffers,
//! made on the first timed frame and again only for a frame of more passes.
//! Each frame writes a timestamp at the start and end of each pass that
//! runs, resolves them, and maps them back after the submit, without
//! waiting; [`crate::Targets::frame_stats`] waits for them and turns them
//! into [`PassTime`]s. A frame's times replace the last frame's: the next
//! timed frame aborts a mapping nobody read.
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
    /// in nanoseconds; `None` when the pair is not a measurement (a
    /// timestamp the GPU left unwritten, or an end before its start), which
    /// a report writes as null.
    pub nanoseconds: Option<u64>,
}

/// The time between a pass's two timestamps, or `None` when they are not a
/// measurement: a timestamp of 0, which the GPU left unwritten, or an end
/// before its start. Metal under load has been seen to leave a pass's start
/// unwritten while writing its end, which without this check reads as the
/// GPU clock's whole age (about 45 days on one Mac).
pub(crate) fn pass_nanoseconds(begin: u64, end: u64, period: f32) -> Option<u64> {
    (begin != 0 && end >= begin).then(|| nanoseconds(end - begin, period))
}

/// What the last frame rendered into a [`crate::Targets`] measured beyond
/// its [`crate::FrameCounters`], which stay as rung 1b fixed them: the GPU
/// time of each pass and the size of the targets' pool. Reports carry them
/// in their own `gpu` keys, beside `per_frame`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// One for each pass that ran, in running order: culled and bypassed
    /// passes have none. `None` when timing is off for the targets
    /// ([`crate::Targets::time_passes`]), the device cannot time passes (the
    /// adapter has no timestamp queries, [`crate::Capabilities`]), no frame
    /// has been rendered into the targets since timing went on, or the last
    /// frame failed.
    pub pass_times: Option<Vec<PassTime>>,
    /// The transient textures the targets keep
    /// ([`crate::Targets::transient_textures`]).
    pub transient_textures: usize,
}

/// Bytes per timestamp.
const TIMESTAMP_BYTES: u64 = 8;

/// One targets' pass timing: off until [`crate::Targets::time_passes`]
/// turns it on.
#[derive(Default)]
pub(crate) struct Timing {
    on: bool,
    /// Made on the first timed frame, and again only for a frame of more
    /// passes than it holds.
    queries: Option<Queries>,
    times: Times,
}

/// A query set of `capacity` timestamps, two per pass, and the buffers they
/// are resolved into and read back from.
pub(crate) struct Queries {
    set: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    readback: wgpu::Buffer,
    capacity: u32,
    map: MapState,
}

/// Where the readback buffer's mapping stands.
enum MapState {
    Idle,
    /// Asked for after a submit; the receiver says whether it worked.
    Requested(Receiver<bool>),
}

/// The last frame's times.
#[derive(Default)]
enum Times {
    /// None: timing off, no frame timed yet, or the last frame failed.
    #[default]
    None,
    /// On their way back in the readback buffer.
    Pending {
        passes: Vec<&'static str>,
        /// Nanoseconds per timestamp tick.
        period: f32,
    },
    /// Read back already.
    Read(Vec<PassTime>),
}

impl Timing {
    /// Turns timing on or off; either way the last frame's times are gone.
    pub(crate) fn set(&mut self, on: bool) {
        self.on = on;
        self.forget();
        if !on && let Some(queries) = &mut self.queries {
            queries.release();
        }
    }

    /// Drops the last frame's times, at the start of every frame, so that a
    /// frame that fails leaves none.
    pub(crate) fn forget(&mut self) {
        self.times = Times::None;
    }

    /// The queries a frame of `passes` passes writes its timestamps into, or
    /// `None` when timing is off, the device cannot time passes, or there is
    /// no pass (or more than 2,048, beyond wgpu's 4,096 queries) to time.
    /// Makes them if the targets have none big enough, and takes the
    /// readback buffer back from a mapping nobody read.
    pub(crate) fn begin(&mut self, gpu: &Gpu, passes: usize) -> Option<&Queries> {
        if !self.on {
            return None;
        }
        gpu.timestamp_period?;
        let count = u32::try_from(passes.checked_mul(2)?).ok()?;
        if count == 0 || count > wgpu::QUERY_SET_MAX_QUERIES {
            return None;
        }
        if let Some(queries) = &mut self.queries {
            queries.release();
        }
        if self.queries.as_ref().is_none_or(|q| q.capacity < count) {
            self.queries = Some(Queries::new(gpu, count));
        }
        self.queries.as_ref()
    }

    /// After the submit of a frame that [`Timing::begin`] timed: starts
    /// mapping its timestamps back.
    pub(crate) fn finish(&mut self, passes: Vec<&'static str>, period: f32) {
        let Some(queries) = &mut self.queries else {
            return;
        };
        let (sender, receiver) = channel();
        queries
            .readback
            .map_async(wgpu::MapMode::Read, .., move |mapped| {
                // The receiver is gone if a later frame aborted the mapping.
                let _ = sender.send(mapped.is_ok());
            });
        queries.map = MapState::Requested(receiver);
        self.times = Times::Pending { passes, period };
    }

    /// The last frame's times, waiting for the GPU to finish it (at most
    /// [`READBACK_LIMIT`]); `None` if it was not timed.
    pub(crate) fn times(&mut self, gpu: &Gpu) -> Result<Option<Vec<PassTime>>, GpuError> {
        let (passes, period) = match &self.times {
            Times::None => return Ok(None),
            Times::Read(times) => return Ok(Some(times.clone())),
            Times::Pending { passes, period } => (passes.clone(), *period),
        };
        let lost = || GpuError::Wgpu("the pass times were not mapped after the wait".into());
        let queries = self.queries.as_mut().ok_or_else(lost)?;
        let MapState::Requested(mapped) = &queries.map else {
            return Err(lost());
        };
        let ok = match mapped.try_recv() {
            Ok(ok) => ok,
            Err(TryRecvError::Disconnected) => false,
            Err(TryRecvError::Empty) => {
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
            // A failed mapping leaves the buffer unmapped.
            queries.map = MapState::Idle;
            self.times = Times::None;
            return Err(lost());
        }
        let read = read_times(&queries.readback, &passes, period);
        queries.readback.unmap();
        queries.map = MapState::Idle;
        let read = read?;
        self.times = Times::Read(read.clone());
        Ok(Some(read))
    }
}

impl Queries {
    fn new(gpu: &Gpu, capacity: u32) -> Self {
        let size = u64::from(capacity) * TIMESTAMP_BYTES;
        let set = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pass times"),
            ty: wgpu::QueryType::Timestamp,
            count: capacity,
        });
        let buffer = |label: &str, usage: wgpu::BufferUsages| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Self {
            set,
            resolved: buffer(
                "pass times",
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            ),
            readback: buffer(
                "pass times readback",
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
            capacity,
            map: MapState::Idle,
        }
    }

    /// Takes the readback buffer back for another frame: a mapping nobody
    /// read is aborted (still waiting) or ended (done). A failed one left
    /// the buffer unmapped already.
    fn release(&mut self) {
        if let MapState::Requested(mapped) = &self.map {
            match mapped.try_recv() {
                Ok(false) | Err(TryRecvError::Disconnected) => {}
                Ok(true) | Err(TryRecvError::Empty) => self.readback.unmap(),
            }
        }
        self.map = MapState::Idle;
    }

    /// Where the `pass`-th pass that runs writes its two timestamps.
    pub(crate) fn writes(&self, pass: usize) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let begin = u32::try_from(pass).ok()?.checked_mul(2)?;
        (begin + 1 < self.capacity).then_some(wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        })
    }

    /// Resolves the first `passes` passes' timestamps and copies them where
    /// the CPU can map them, at the end of the frame's commands.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder, passes: usize) {
        let count = u32::try_from(passes * 2)
            .unwrap_or(self.capacity)
            .min(self.capacity);
        encoder.resolve_query_set(&self.set, 0..count, &self.resolved, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolved,
            0,
            &self.readback,
            0,
            u64::from(count) * TIMESTAMP_BYTES,
        );
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
            nanoseconds: pass_nanoseconds(*begin, *end, period),
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
    use super::{nanoseconds, pass_nanoseconds};

    #[test]
    fn a_pair_that_is_not_a_measurement_has_no_time() {
        // The start left unwritten, with a real end: the failure seen on
        // Metal, which once read as 3.9e15 ns.
        assert_eq!(pass_nanoseconds(0, 3_955_064_264_700_375, 1.0), None);
        assert_eq!(pass_nanoseconds(5_000, 0, 1.0), None);
        assert_eq!(pass_nanoseconds(0, 0, 1.0), None);
        assert_eq!(pass_nanoseconds(5_000, 4_999, 1.0), None);
        assert_eq!(pass_nanoseconds(5_000, 5_000, 1.0), Some(0));
        assert_eq!(pass_nanoseconds(1_000, 4_000, 41.666_668), Some(125_000));
    }

    #[test]
    fn ticks_turn_into_nanoseconds() {
        assert_eq!(nanoseconds(0, 1.0), 0);
        assert_eq!(nanoseconds(1_000, 1.0), 1_000);
        // A 24 MHz tick, about 41.67 ns, truncated.
        assert_eq!(nanoseconds(3, 41.666_668), 125);
        assert_eq!(nanoseconds(u64::MAX, 2.0), u64::MAX);
    }
}
