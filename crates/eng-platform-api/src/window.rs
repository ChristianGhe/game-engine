//! The window seam (ADR-0016): how a window reaches the GPU.
//!
//! `raw-window-handle`'s `HasWindowHandle` and `HasDisplayHandle` may
//! appear in public APIs only between this crate, `eng-platform-winit`,
//! `eng-gpu` and `eng-app`, and none of them re-exports the traits. This
//! module is the only part of the contract that names them, and nothing a
//! platform must implement uses it: the headless platform, outside the
//! seam, implements [`Platform`](crate::Platform) and
//! [`Control`](crate::Control) without it.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::event::PhysicalSize;

/// A window the GPU can draw into. A windowed platform hands its window to
/// the app as an `Arc<dyn Window>` ([`App::window`](crate::App::window)),
/// which is what wgpu's safe surface creation takes: both handle traits,
/// `Send`, `Sync` and `'static`.
pub trait Window: HasWindowHandle + HasDisplayHandle + Send + Sync {
    /// The drawable area now, in physical pixels. Changes also arrive as
    /// [`PlatformEvent::Resized`](crate::PlatformEvent::Resized).
    fn size(&self) -> PhysicalSize;
}
