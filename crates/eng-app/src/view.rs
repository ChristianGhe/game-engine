//! The window's view: the GPU opened on the window's surface, the renderer
//! and engine-owned targets the window's size, and what stops drawing.
//!
//! - The window comes through the window seam (ADR-0016) as the platform's
//!   `Arc<dyn Window>`, and goes straight to eng-gpu: this crate never names
//!   the handle traits.
//! - Without an adapter, or with a surface that offers neither BGRA format,
//!   `play` exits 64 with eng-gpu's message (`[E-NO-ADAPTER]`, `[E-GPU]`)
//!   before the first tick, writing nothing (plan decision 4).
//! - Once open, a frame that fails is a bug: it panics, and the panic hook
//!   writes `crash.json` with the tick (as `shot` does, T9 decision 2). A
//!   lost surface counts as one in 1b (`[E-GPU]`, exit 12); recreating it
//!   waits for a runner that shows one.
//! - Nothing is acquired while the window is minimised or occluded: on
//!   macOS that can block for a second (plan, "One frame"). Ticks go on.

use std::sync::Arc;

use eng_cli::{PlayGpu, PlayRun};
use eng_gpu::{Gpu, GpuOptions, Targets, WindowSurface};
use eng_platform_api::window::Window;
use eng_render::Renderer;

pub(crate) struct View {
    gpu: Gpu,
    surface: WindowSurface,
    renderer: Renderer,
    targets: Targets,
    /// Kept until the surface made from it is gone; its size is checked
    /// each frame.
    window: Arc<dyn Window>,
    pub(crate) occluded: bool,
    minimised: bool,
    frames: u64,
}

impl View {
    /// Opens the GPU on `window`. The error is the line to print, with its
    /// tag; `play` then exits 64.
    pub(crate) fn open(window: Arc<dyn Window>) -> Result<Self, String> {
        let size = window.size();
        let (gpu, surface) = Gpu::with_window(
            &GpuOptions::from_env(),
            Arc::clone(&window),
            size.width,
            size.height,
        )
        .map_err(|e| e.to_string())?;
        let renderer = Renderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
        let (width, height) = surface.size();
        let targets = Targets::new(&gpu, width, height).unwrap_or_else(|e| panic!("{e}"));
        Ok(Self {
            gpu,
            surface,
            renderer,
            targets,
            window,
            occluded: false,
            minimised: size.width == 0 || size.height == 0,
            frames: 0,
        })
    }

    /// The window's drawable area changed; 0 x 0 while minimised.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.minimised = width == 0 || height == 0;
        if self.minimised {
            return;
        }
        self.surface
            .resize(&self.gpu, width, height)
            .unwrap_or_else(|e| panic!("{e}"));
        self.fit_targets(self.surface.size());
    }

    fn fit_targets(&mut self, (width, height): (u32, u32)) {
        if self.targets.size() != (width, height) {
            self.targets = Targets::new(&self.gpu, width, height).unwrap_or_else(|e| panic!("{e}"));
        }
    }

    /// Draws the run's newest states at `alpha` and presents them, unless
    /// the window cannot be seen or has no image to give now.
    pub(crate) fn draw(&mut self, run: &mut PlayRun<'_>, alpha: f64) {
        if self.occluded || self.minimised {
            return;
        }
        // A size that changed without its event, or a surface that went out
        // of date, is caught here: both are no-ops when nothing changed.
        let size = self.window.size();
        if size.width > 0 && size.height > 0 {
            self.resize(size.width, size.height);
        }
        let Some(top_down) = run.top_down() else {
            return;
        };
        let tick = run.next_tick().0 - 1;
        let Some(frame) = self
            .surface
            .acquire(&self.gpu)
            .unwrap_or_else(|e| panic!("{e} (frame at tick {tick})"))
        else {
            return;
        };
        self.fit_targets(frame.size());
        let (width, height) = self.targets.size();
        let scene = top_down(&run.frame(alpha, width, height));
        let counters = self
            .renderer
            .render_to(&self.gpu, &self.targets, &frame, &scene)
            .unwrap_or_else(|e| panic!("{e} (frame at tick {tick})"));
        self.surface
            .present(&self.gpu, frame)
            .unwrap_or_else(|e| panic!("{e} (frame at tick {tick})"));
        run.observe_frame(&counters);
        self.frames += 1;
    }

    /// Lets the surface go, then the window; returns the report's `gpu`
    /// object: the adapter, the last size, the surface's format and the
    /// frames presented.
    pub(crate) fn close(self) -> PlayGpu {
        let Self {
            gpu,
            surface,
            renderer,
            targets,
            window,
            frames,
            ..
        } = self;
        let (width, height) = targets.size();
        let gpu_object = PlayGpu {
            report: gpu.report().clone(),
            width,
            height,
            surface_format: surface.format(),
            frames,
        };
        drop(surface);
        drop((renderer, targets, gpu));
        drop(window);
        gpu_object
    }
}
