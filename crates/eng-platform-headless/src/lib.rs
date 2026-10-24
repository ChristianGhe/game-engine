//! The headless platform (spec 6.10): no window, no devices, no winit.
//!
//! It paces a fixed number of frames on a virtual clock, one frame per tick,
//! so headless runs are as fast as the machine allows and identical
//! everywhere. Rung 1b extracts the platform contract (`eng-platform-api`)
//! that this crate and `eng-platform-winit` both implement.
#![forbid(unsafe_code)]

/// One frame of the virtual clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// 0-based frame number.
    pub index: u64,
    /// Virtual time at the start of the frame, in nanoseconds.
    pub now_ns: u64,
    /// Virtual time since the previous frame.
    pub dt_ns: u64,
}

#[derive(Debug, Clone)]
pub struct HeadlessPlatform {
    frame_ns: u64,
    frames: u64,
    next: u64,
}

impl HeadlessPlatform {
    /// A platform that yields `frames` frames, one per tick at `tick_rate`.
    ///
    /// # Panics
    /// If `tick_rate` is zero.
    pub fn new(tick_rate: u32, frames: u64) -> Self {
        assert!(tick_rate > 0, "tick rate must be positive");
        Self {
            frame_ns: 1_000_000_000 / u64::from(tick_rate),
            frames,
            next: 0,
        }
    }

    pub const fn name(&self) -> &'static str {
        "headless"
    }

    /// Virtual time now, in nanoseconds.
    pub fn now_ns(&self) -> u64 {
        self.next * self.frame_ns
    }

    pub fn next_frame(&mut self) -> Option<Frame> {
        if self.next == self.frames {
            return None;
        }
        let frame = Frame {
            index: self.next,
            now_ns: self.now_ns(),
            dt_ns: self.frame_ns,
        };
        self.next += 1;
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yields_exactly_the_frames_asked_for() {
        let mut p = HeadlessPlatform::new(60, 3);
        let frames: Vec<Frame> = std::iter::from_fn(|| p.next_frame()).collect();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[2].index, 2);
        assert_eq!(frames[1].now_ns, 16_666_666);
        assert_eq!(p.now_ns(), 49_999_998);
        assert!(p.next_frame().is_none());
    }
}
