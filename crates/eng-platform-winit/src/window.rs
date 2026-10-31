//! The window seam (ADR-0016) over winit's window.

use eng_platform_api::PhysicalSize;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// winit's window as the contract's `window::Window`. Private: the app
/// sees it only as an `Arc<dyn Window>`, so no winit type and no
/// re-export of the handle traits leaves this crate.
///
/// winit's window is `Send + Sync`. On macOS it gives out its handles only
/// on the main thread, where the app's callbacks run, and `size` from
/// another thread waits for the main thread.
pub(crate) struct WinitWindow(pub(crate) winit::window::Window);

impl HasWindowHandle for WinitWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.0.window_handle()
    }
}

impl HasDisplayHandle for WinitWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}

impl eng_platform_api::window::Window for WinitWindow {
    fn size(&self) -> PhysicalSize {
        let size = self.0.inner_size();
        PhysicalSize::new(size.width, size.height)
    }
}
