//! The GCMouse shim (spec 6.10; rung 2 plan, decision 11; ADR-0021).
//! winit's macOS mouse deltas arrive accelerated and coalesced, one per
//! display refresh. On macOS this crate reads raw deltas from the Game
//! Controller framework's `GCMouse` instead: each mouse's moved handler
//! adds its deltas into a sum that `eng-platform-winit` drains each
//! `about_to_wait`, and counts its own calls. The drain follows the frames
//! by design, so the callback count is what shows the mouse's rate
//! (criterion 25, `gcmouse-rate`).
//!
//! - **The API** is [`Shim`]: [`Shim::start`] on macOS (`None` elsewhere,
//!   where the crate does nothing), [`Shim::set_focused`], [`Shim::drain`]
//!   into a [`Motion`], and [`Shim::callbacks`], a [`Callbacks`] count
//!   readable at any time. [`Shim::with_source`] runs the same logic on a
//!   fake [`Source`] that tests feed through a [`Feed`], since CI's macOS
//!   runner has no mouse.
//! - **Deltas** are rounded to the nearest 1/256 count, ties away from
//!   zero, as `eng_input::MouseLook` does, and summed exactly in `i64`
//!   ([`Motion`]), so a drained sum reaches mouse look as the yaw the
//!   separate deltas would give.
//! - **Focus.** Motion is summed only while the shim is told the window has
//!   focus; a focus change drops what was not drained. Every callback is
//!   counted, focused or not.
//! - **Signs and units.** GCMouse's `deltaY` is taken as positive up (SDL 3
//!   sends `-deltaY` as its motion), so the shim negates it into the
//!   contract's `dy`, positive towards the user; deltas are taken as
//!   counts. Neither is verified on a real mouse yet (rung 2, T14's carry):
//!   `examples/events.rs` in `eng-platform-winit` checks them.
//!
//! Only `eng-platform-winit` may depend on this crate, and only this crate
//! on `objc2-game-controller` (both in `[floor]` in `architecture.toml`;
//! the second also in `deny.toml`'s wrapper ban). The bindings it needs are
//! `unsafe fn`, so the root does not forbid `unsafe_code`: the workspace
//! lint still denies it, the safe module forbids it, and `src/macos.rs`
//! alone allows it (ADR-0021). Each `unsafe` block there makes one unsafe
//! call and carries a `// SAFETY:` comment, which the `forbid`s below make
//! clippy check.
#![forbid(clippy::undocumented_unsafe_blocks)]
#![forbid(clippy::multiple_unsafe_ops_per_block)]

#[cfg(target_os = "macos")]
mod macos;
mod shim;

pub use shim::{Callbacks, Feed, Motion, Shim, Source};
