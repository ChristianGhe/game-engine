//! A window's surface (rung 1b, task T11): the swapchain wgpu makes for the
//! window the platform hands over (the window seam, ADR-0016), presented
//! with Fifo (plan decision 7).
//!
//! The present pass writes `Bgra8UnormSrgb` (T8, decision 7). A surface that
//! offers it is configured in it; one that offers only `Bgra8Unorm` is
//! configured in that and drawn through an sRGB view of each image, which
//! encodes the same way. A surface that offers neither fails with `[E-GPU]`
//! before anything is drawn.
//!
//! A surface the system loses (wgpu's `Lost`) is an `[E-GPU]` error from
//! [`WindowSurface::acquire`], as in rung 1b. A caller that keeps the
//! window asks with [`WindowSurface::acquire_remaking`] instead, which
//! makes the surface again from it and skips the frame (rung 2, T15); one
//! still lost after three remakes in a row with no frame is an error.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::error::GpuError;
use crate::gpu::{Gpu, GpuOptions, instance};
use crate::targets::MAX_SIDE;

const SRGB: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
const UNORM: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

/// The format a surface's images are made in, and the view formats the
/// present pass needs to draw them as `Bgra8UnormSrgb`; `None` if the
/// surface offers neither BGRA format.
pub(crate) fn formats(
    offered: &[wgpu::TextureFormat],
) -> Option<(wgpu::TextureFormat, Vec<wgpu::TextureFormat>)> {
    if offered.contains(&SRGB) {
        Some((SRGB, Vec::new()))
    } else if offered.contains(&UNORM) {
        Some((UNORM, vec![SRGB]))
    } else {
        None
    }
}

/// A window's surface, configured for the device it was opened with
/// ([`Gpu::with_window`]). Drop it before the window: the platform contract
/// asks apps to let the window go by `stop`.
pub struct WindowSurface {
    /// `None` only between a lost surface going and its remaking.
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    remakes: Remakes,
}

/// Remakes of a lost surface in a row, with no frame between them
/// ([`WindowSurface::acquire_remaking`]): a surface that stays lost is an
/// error, not a frame skipped forever.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Remakes(u32);

impl Remakes {
    /// Remakes allowed in a row before a loss is an error.
    const MOST: u32 = 3;

    /// The surface was lost again: one more remake, or the `[E-GPU]` error
    /// once [`Remakes::MOST`] have brought no frame back.
    fn lost(&mut self) -> Result<(), GpuError> {
        if self.0 >= Self::MOST {
            return Err(GpuError::Surface(format!(
                "the window's surface stays lost: made again {} times in a row with no frame",
                Self::MOST
            )));
        }
        self.0 += 1;
        Ok(())
    }

    /// A frame came: the count starts again.
    fn frame(&mut self) {
        self.0 = 0;
    }
}

/// What acquiring an image found.
enum Acquired {
    Frame(SurfaceFrame),
    /// No image now: skip the frame.
    Skip,
    /// The surface is gone and must be made again.
    Lost,
}

/// One image of a window's surface, from [`WindowSurface::acquire`]: a
/// [`crate::Graph::render_to`] frame draws into it, then
/// [`WindowSurface::present`] shows it.
pub struct SurfaceFrame {
    texture: wgpu::SurfaceTexture,
    pub(crate) view: wgpu::TextureView,
    suboptimal: bool,
}

impl SurfaceFrame {
    /// Width and height in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.texture.texture.width(), self.texture.texture.height())
    }
}

impl Gpu {
    /// Opens a device that can present to `window`, as [`Gpu::new`] does
    /// otherwise, and the window's surface at `width` by `height` pixels.
    ///
    /// `window` is what the platform hands the app (`eng-platform-api`'s
    /// `Arc<dyn Window>`): the two handle traits are the window seam
    /// (ADR-0016).
    pub fn with_window<W>(
        options: &GpuOptions,
        window: W,
        width: u32,
        height: u32,
    ) -> Result<(Gpu, WindowSurface), GpuError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let instance = instance();
        let surface = instance
            .create_surface(window)
            .map_err(|e| GpuError::Surface(format!("cannot make the window's surface: {e}")))?;
        let gpu = Gpu::open(instance, options, Some(&surface))?;
        let surface = WindowSurface::new(&gpu, surface, width, height)?;
        Ok((gpu, surface))
    }
}

impl WindowSurface {
    fn new(
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let caps = surface.get_capabilities(&gpu.adapter);
        let Some((format, view_formats)) = formats(&caps.formats) else {
            let offered: Vec<String> = caps.formats.iter().map(|f| format!("{f:?}")).collect();
            return Err(GpuError::Surface(format!(
                "the window's surface offers neither Bgra8UnormSrgb nor Bgra8Unorm, only [{}]",
                offered.join(", ")
            )));
        };
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            wgpu::CompositeAlphaMode::Auto
        };
        let mut surface = Self {
            surface: Some(surface),
            remakes: Remakes::default(),
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: side(width),
                height: side(height),
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode,
                view_formats,
            },
        };
        surface.configure(gpu)?;
        Ok(surface)
    }

    fn configure(&mut self, gpu: &Gpu) -> Result<(), GpuError> {
        let Some(surface) = &self.surface else {
            // Lost and not made again: the next acquire says so.
            return Ok(());
        };
        gpu.scoped(|| surface.configure(&gpu.device, &self.config))
            .map_err(|e| GpuError::Surface(format!("cannot configure the window's surface: {e}")))
    }

    /// The images' width and height in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// The format the surface's images are made in: `"Bgra8UnormSrgb"`, or
    /// `"Bgra8Unorm"` drawn through an sRGB view.
    pub fn format(&self) -> &'static str {
        if self.config.format == SRGB {
            "Bgra8UnormSrgb"
        } else {
            "Bgra8Unorm"
        }
    }

    /// Makes the images `width` by `height` pixels, each side clamped to 1
    /// to 8,192 (Tier 0). A minimised window may report a side of 0: the
    /// caller should then stop drawing until it has a size again.
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) -> Result<(), GpuError> {
        let (width, height) = (side(width), side(height));
        if (width, height) != self.size() {
            self.config.width = width;
            self.config.height = height;
            self.configure(gpu)?;
        }
        Ok(())
    }

    /// The next image to draw into, or `None` when there is none to draw
    /// now: the wait timed out, the window cannot be seen, or the surface
    /// changed and was configured again. Skip the frame then. With Fifo this
    /// waits for the display; the caller should not ask while the window is
    /// minimised or occluded, where macOS can block for a second (plan, "One
    /// frame").
    pub fn acquire(&mut self, gpu: &Gpu) -> Result<Option<SurfaceFrame>, GpuError> {
        match self.next(gpu)? {
            Acquired::Frame(frame) => Ok(Some(frame)),
            Acquired::Skip => Ok(None),
            Acquired::Lost => Err(GpuError::Surface("the window's surface was lost".into())),
        }
    }

    /// Like [`WindowSurface::acquire`], but a surface the system lost is
    /// made again from `window`, the window it was made from (the
    /// platform's `Arc<dyn Window>`), set up as before, and the frame is
    /// skipped (`Ok(None)`). One that cannot be made again, or stays lost
    /// through three remakes in a row with no frame between them, is an
    /// `[E-GPU]` error.
    pub fn acquire_remaking<W>(
        &mut self,
        gpu: &Gpu,
        window: W,
    ) -> Result<Option<SurfaceFrame>, GpuError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        match self.next(gpu)? {
            Acquired::Frame(frame) => {
                self.remakes.frame();
                Ok(Some(frame))
            }
            Acquired::Skip => Ok(None),
            Acquired::Lost => {
                self.remakes.lost()?;
                // The lost surface and its images go first: a window may
                // have only one swapchain.
                self.surface = None;
                let surface = gpu.instance.create_surface(window).map_err(|e| {
                    GpuError::Surface(format!("cannot make the lost window's surface again: {e}"))
                })?;
                self.surface = Some(surface);
                self.configure(gpu)?;
                Ok(None)
            }
        }
    }

    fn next(&mut self, gpu: &Gpu) -> Result<Acquired, GpuError> {
        let Some(surface) = &self.surface else {
            return Ok(Acquired::Lost);
        };
        let (texture, suboptimal) = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => (texture, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => (texture, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(Acquired::Skip);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure(gpu)?;
                return Ok(Acquired::Skip);
            }
            wgpu::CurrentSurfaceTexture::Lost => return Ok(Acquired::Lost),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(GpuError::Surface(
                    "acquiring the window's next image failed validation".into(),
                ));
            }
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("surface"),
            format: Some(SRGB),
            ..wgpu::TextureViewDescriptor::default()
        });
        Ok(Acquired::Frame(SurfaceFrame {
            texture,
            view,
            suboptimal,
        }))
    }

    /// Shows a frame that [`crate::Graph::render_to`] drew, after its
    /// submit. A frame the surface called suboptimal configures it again.
    pub fn present(&mut self, gpu: &Gpu, frame: SurfaceFrame) -> Result<(), GpuError> {
        gpu.queue.present(frame.texture);
        if frame.suboptimal {
            self.configure(gpu)?;
        }
        Ok(())
    }
}

/// A side clamped to what a Tier 0 texture can have.
fn side(pixels: u32) -> u32 {
    pixels.clamp(1, MAX_SIDE)
}

#[cfg(test)]
mod tests {
    use super::{Remakes, SRGB, UNORM, formats};

    #[test]
    fn a_surface_lost_through_three_remakes_in_a_row_is_an_error() {
        let mut remakes = Remakes::default();
        for _ in 0..Remakes::MOST {
            assert_eq!(remakes.lost(), Ok(()));
        }
        let err = remakes.lost().unwrap_err().to_string();
        assert_eq!(
            err,
            "[E-GPU] the window's surface stays lost: made again 3 times in a row with no frame"
        );
        // A frame between losses starts the count again.
        let mut remakes = Remakes::default();
        for _ in 0..10 {
            assert_eq!(remakes.lost(), Ok(()));
            assert_eq!(remakes.lost(), Ok(()));
            remakes.frame();
        }
    }

    #[test]
    fn the_srgb_format_is_taken_first_then_an_srgb_view() {
        let rgba = wgpu::TextureFormat::Rgba8Unorm;
        assert_eq!(formats(&[rgba, UNORM, SRGB]), Some((SRGB, vec![])));
        assert_eq!(formats(&[rgba, UNORM]), Some((UNORM, vec![SRGB])));
        assert_eq!(formats(&[rgba, wgpu::TextureFormat::Rgba8UnormSrgb]), None);
        assert_eq!(formats(&[]), None);
    }
}
