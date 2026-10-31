//! The platform contract (spec 6.1, 6.10): plain-data events for physical
//! keys, raw mouse deltas, mouse buttons, the wheel, gamepad axes and
//! buttons, focus, resize, occlusion and close requests; a monotonic clock;
//! the callback trait the app implements; and the window seam.
//!
//! Implementations: `eng-platform-headless` and `eng-platform-winit`, which
//! will both pass this crate's conformance suite. An empty skeleton until
//! rung 1b's task T3 (`docs/rungs/1b/plan.md`).
#![forbid(unsafe_code)]
