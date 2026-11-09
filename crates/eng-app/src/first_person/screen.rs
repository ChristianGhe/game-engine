//! A first-person window's GPU side (rung 2, T15; the T12 and T10
//! carries): the GPU on the window's surface, eng-render's first-person
//! renderer, the map's world loaded once, and targets at the look's low
//! resolution.
//!
//! - **Opening** builds the world from the run's sim (its constants) and
//!   loads it before the first tick. Without an adapter, or with a surface
//!   that offers neither BGRA format, `play` exits 64 with eng-gpu's
//!   message, writing nothing, as in rung 1b. A world eng-render refuses is
//!   a bug in the game and panics (`[E-SCENE]`, exit 12), as in `shot`.
//! - **Targets** are `Targets::upscaled` at the frame's
//!   `look.low_resolution`, made here, by the caller (ADR-0020 says
//!   eng-render; T10 put it in eng-gpu): the window's size over k, drawn
//!   k×k, centred with a black border; a window narrower than k takes the
//!   largest k that fits. They are made again when the window's image or
//!   the scale changes, compared by `surface_size()`, the window's, not
//!   `size()`, the targets' own.
//! - **Each frame** is the game's scene at the clock's alpha, with `play`'s
//!   lens set on it ([`super::lens`]), drawn and presented after the
//!   submit (Fifo). Nothing is acquired while the window is minimised or
//!   occluded (a locked Mac screen counts as occluded); ticks go on.
//! - **A lost surface** is made again from the kept window and the frame
//!   skipped (rung 1b's T11 left it a panic); any other failure once open
//!   is a bug and panics, so the panic hook writes `crash.json`.

use std::sync::Arc;

use eng_cli::{FirstPersonView, FrameLens, PlayGpu, PlayRun};
use eng_gpu::{Gpu, GpuOptions, Targets, WindowSurface};
use eng_platform_api::window::Window;
use eng_render::{FirstPersonRenderer, GpuWorld};

use super::lens::Lens;

pub(crate) struct Screen {
    gpu: Gpu,
    surface: WindowSurface,
    renderer: FirstPersonRenderer,
    world: GpuWorld,
    targets: Targets,
    /// The scale the targets were made at, as asked: `targets.upscale()`
    /// may be smaller, for a window narrower than it.
    scale: u32,
    view: FirstPersonView,
    /// Kept until the surface made from it is gone, and to make a lost one
    /// again; its size is checked each frame, and 0 x 0 means minimised.
    window: Arc<dyn Window>,
    pub(crate) occluded: bool,
    frames: u64,
    last: Option<FrameLens>,
}

impl Screen {
    /// Opens the GPU on `window` and loads `view`'s world from `run`'s
    /// sim. The error is the line to print, with its tag; `play` then exits
    /// 64. `scale` is the look's low resolution at the start.
    pub(crate) fn open(
        window: Arc<dyn Window>,
        view: FirstPersonView,
        run: &PlayRun<'_>,
        scale: u32,
    ) -> Result<Self, String> {
        let size = window.size();
        let (gpu, surface) = Gpu::with_window(
            &GpuOptions::from_env(),
            Arc::clone(&window),
            size.width,
            size.height,
        )
        .map_err(|e| e.to_string())?;
        let renderer = FirstPersonRenderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
        let built = (view.world)(run.frame(1.0, 1, 1).current);
        let world = built
            .load(&gpu)
            .unwrap_or_else(|e| panic!("{e} (loading the {} world)", built.label));
        let (width, height) = surface.size();
        let scale = scale.max(1);
        let targets =
            Targets::upscaled(&gpu, width, height, scale).unwrap_or_else(|e| panic!("{e}"));
        Ok(Self {
            gpu,
            surface,
            renderer,
            world,
            targets,
            scale,
            view,
            window,
            occluded: false,
            frames: 0,
            last: None,
        })
    }

    /// The window, for its refresh rate.
    pub(crate) fn window(&self) -> &dyn Window {
        &*self.window
    }

    /// The window's drawable area changed; 0 x 0 while minimised, when
    /// there is nothing to set up.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface
            .resize(&self.gpu, width, height)
            .unwrap_or_else(|e| panic!("{e}"));
        self.fit_targets(self.surface.size(), self.scale);
    }

    /// Targets for a window image of `size` at `scale`, made again only
    /// when either changed.
    fn fit_targets(&mut self, size: (u32, u32), scale: u32) {
        let scale = scale.max(1);
        if self.targets.surface_size() != size || self.scale != scale {
            self.targets = Targets::upscaled(&self.gpu, size.0, size.1, scale)
                .unwrap_or_else(|e| panic!("{e}"));
            self.scale = scale;
        }
    }

    /// Draws the run's newest states at `alpha` through `lens`, looking
    /// along `angles` when the run is live, and presents them, unless the
    /// window cannot be seen or has no image to give now.
    pub(crate) fn draw(
        &mut self,
        run: &mut PlayRun<'_>,
        alpha: f64,
        angles: Option<(u16, u16)>,
        lens: &Lens,
    ) {
        if self.occluded {
            return;
        }
        // The window's own size each frame, as rung 1b's view checks it:
        // 0 x 0 is minimised, and a size that changed without its event is
        // caught here.
        let size = self.window.size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.resize(size.width, size.height);
        let tick = run.next_tick().0 - 1;
        let Some(frame) = self
            .surface
            .acquire_remaking(&self.gpu, Arc::clone(&self.window))
            .unwrap_or_else(|e| panic!("{e} (frame at tick {tick})"))
        else {
            return;
        };
        // The window's image and the look's scale decide the targets.
        self.fit_targets(frame.size(), self.scale);
        let (width, height) = self.targets.size();
        let mut scene = (self.view.scene)(&run.frame(alpha, width, height));
        lens.apply(&mut scene, angles, aspect(&self.targets));
        if scale_of(&scene) != self.scale {
            // A scene whose look changed its scale: new targets, and the
            // scene again for their size and aspect.
            self.fit_targets(frame.size(), scale_of(&scene));
            let (width, height) = self.targets.size();
            scene = (self.view.scene)(&run.frame(alpha, width, height));
            lens.apply(&mut scene, angles, aspect(&self.targets));
        }
        let counters = self
            .renderer
            .render_to(&self.gpu, &self.targets, &frame, &self.world, &scene)
            .unwrap_or_else(|e| panic!("{e} (frame at tick {tick})"));
        self.surface
            .present(&self.gpu, frame)
            .unwrap_or_else(|e| panic!("{e} (frame at tick {tick})"));
        run.observe_frame(&counters);
        self.frames += 1;
        self.last = Some(FrameLens {
            fov_y: scene.camera.fov_y,
            near: scene.camera.near,
            upscale: self.targets.upscale(),
        });
    }

    /// Lets the surface go, then the world and the window; returns the
    /// report's `gpu` object (the window's last size) and the last frame's
    /// lens.
    pub(crate) fn close(self) -> (PlayGpu, Option<FrameLens>) {
        let Self {
            gpu,
            surface,
            renderer,
            world,
            targets,
            window,
            frames,
            last,
            ..
        } = self;
        let (width, height) = targets.surface_size();
        let gpu_object = PlayGpu {
            report: gpu.report().clone(),
            width,
            height,
            surface_format: surface.format(),
            frames,
        };
        drop(surface);
        drop((renderer, world, targets, gpu));
        drop(window);
        (gpu_object, last)
    }
}

/// The low resolution a scene's look asks for, at least 1.
fn scale_of(scene: &eng_render::FirstPersonScene) -> u32 {
    scene.look.low_resolution.max(1)
}

/// The targets' width over their height, the image's aspect the renderer
/// projects for.
fn aspect(targets: &Targets) -> f64 {
    let (width, height) = targets.size();
    f64::from(width) / f64::from(height)
}
