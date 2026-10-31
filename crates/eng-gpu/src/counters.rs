//! The per-frame counters (spec 6.13): runner-independent, so that budgets
//! can hold them on every adapter.

/// What one frame asked of the GPU, counted from the commands
/// [`crate::Graph::render`] encoded. The counts depend only on what the
/// passes recorded, never on the adapter. From rung 1b's task T9, reports
/// carry them in their `gpu` object and `budgets.toml [gpu]` limits them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCounters {
    /// Draw calls.
    pub draws: u32,
    /// Pipelines bound. Each pass binds one before its first draw, then
    /// again only for a draw that needs another; nothing stays bound from
    /// one pass to the next.
    pub pipeline_switches: u32,
    /// Bind groups bound. A pass binds its input once, before its first
    /// fullscreen draw.
    pub bind_group_switches: u32,
    /// Bytes written into GPU buffers for the frame.
    pub upload_bytes: u64,
}
