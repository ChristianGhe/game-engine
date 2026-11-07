//! The GCMouse shim (spec 6.10; rung 2 plan, decision 11). winit's macOS
//! mouse deltas arrive coalesced, one per display refresh. On macOS this
//! crate reads raw deltas from the Game Controller framework's `GCMouse`
//! instead: its handlers add them into a sum that `eng-platform-winit`
//! drains each frame, and count their own calls.
//!
//! On every other target the crate is empty. Only `eng-platform-winit` may
//! depend on it, and only it may depend on `objc2-game-controller` (both in
//! `[floor]` in `architecture.toml`; the second also in `deny.toml`'s
//! wrapper ban). The bindings it needs are `unsafe fn`, so the root does not
//! forbid `unsafe_code`: the workspace lint still denies it, and task T14
//! allows it in one module file only (ADR-0021). Every `unsafe` block there
//! needs a `// SAFETY:` comment, which the `forbid` below makes clippy
//! check.
//!
//! An empty skeleton until rung 2's task T14 (`docs/rungs/2/plan.md`).
#![forbid(clippy::undocumented_unsafe_blocks)]

#[cfg(target_os = "macos")]
mod macos;
