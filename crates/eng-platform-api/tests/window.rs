//! The window seam (ADR-0016). A windowed platform hands its app an
//! `Arc<dyn Window>`, which wgpu's safe surface creation takes as it is.
//! This crate is inside the seam, so its tests may name
//! `raw-window-handle`'s traits; the headless platform never does.

use std::sync::Arc;

use eng_platform_api::window::Window;
use eng_platform_api::{
    App, Control, Instant, PhysicalSize, Platform, PlatformError, Wake, conformance,
};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// A window with no handles, as on a system the crate cannot describe.
struct FakeWindow {
    size: PhysicalSize,
}

impl HasWindowHandle for FakeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for FakeWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl Window for FakeWindow {
    fn size(&self) -> PhysicalSize {
        self.size
    }
}

/// The bounds of wgpu 30's `SurfaceTarget<'static>`: both handle traits,
/// `Send + Sync` (its `WasmNotSendSync` on native targets) and `'static`.
fn surface_target<W>(window: W) -> W
where
    W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
{
    window
}

#[test]
fn an_arc_dyn_window_is_a_wgpu_surface_target() {
    let window: Arc<dyn Window> = Arc::new(FakeWindow {
        size: PhysicalSize::new(640, 480),
    });
    let window = surface_target(window);
    assert_eq!(window.size(), PhysicalSize::new(640, 480));
    assert!(matches!(
        window.window_handle(),
        Err(HandleError::Unavailable)
    ));
    assert!(matches!(
        window.display_handle(),
        Err(HandleError::Unavailable)
    ));
}

/// Where a fake windowed platform calls [`App::window`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    /// As the contract says: once, right after `start`.
    AfterStart,
    BeforeStart,
    Twice,
}

/// The smallest windowed platform: a virtual clock, timed wakes, no
/// events, and one window.
struct Windowed {
    order: Order,
    /// The size each run's window had when the app got it.
    seen: Vec<PhysicalSize>,
}

struct Clock {
    now: Instant,
    exit: bool,
}

impl Control for Clock {
    fn now(&self) -> Instant {
        self.now
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Platform for Windowed {
    fn name(&self) -> &'static str {
        "fake-windowed"
    }

    fn run(&mut self, app: &mut dyn App) -> Result<(), PlatformError> {
        let window: Arc<dyn Window> = Arc::new(FakeWindow {
            size: PhysicalSize::new(320, 240),
        });
        self.seen.push(window.size());
        let mut ctx = Clock {
            now: Instant::ZERO,
            exit: false,
        };
        if self.order == Order::BeforeStart {
            app.window(&mut ctx, Arc::clone(&window));
        }
        app.start(&mut ctx);
        if !ctx.exit && self.order != Order::BeforeStart {
            app.window(&mut ctx, Arc::clone(&window));
            if self.order == Order::Twice && !ctx.exit {
                app.window(&mut ctx, window);
            }
        }
        let mut result = Ok(());
        while !ctx.exit {
            match app.frame(&mut ctx) {
                _ if ctx.exit => {}
                Wake::At(at) => ctx.now = ctx.now.max(at),
                Wake::OnEvent => {
                    result = Err(PlatformError::Idle { at: ctx.now });
                    break;
                }
            }
        }
        app.stop(&ctx);
        result
    }
}

#[test]
fn a_windowed_platform_passes_the_suite() {
    let mut platform = Windowed {
        order: Order::AfterStart,
        seen: Vec::new(),
    };
    conformance::check(&mut platform);
    assert!(platform.seen.len() >= 2, "the suite runs it more than once");
}

#[test]
fn the_suite_rejects_a_window_out_of_place() {
    for order in [Order::BeforeStart, Order::Twice] {
        let mut platform = Windowed {
            order,
            seen: Vec::new(),
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            conformance::check(&mut platform);
        }));
        let message = panic_message(caught.expect_err("the suite must reject it"));
        let rule = match order {
            Order::BeforeStart => "start comes first, once",
            _ => "the window comes once, right after start",
        };
        assert!(message.contains(rule), "{order:?}: {message}");
        assert!(message.contains("fake-windowed"), "{message}");
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => payload
            .downcast_ref::<&str>()
            .map_or_else(|| "(not a string)".to_string(), |s| (*s).to_string()),
    }
}
