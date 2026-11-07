//! Low internal resolution (rung 2, T10; ADR-0020): targets smaller than
//! the window they present into, whose present pass draws each texel as k×k
//! of the window's pixels. Every pass, the output included, runs at the
//! targets' size; only the present pass sees the window's. Targets made
//! with [`Targets::new`] present 1:1, as in rung 1b.
//!
//! **A window size that k does not divide.** The targets take the window's
//! size divided by k, rounded down, so the image is exactly what a shot of
//! that size shows, scaled. The present pass draws it centred, at the
//! window's half of the remainder (rounded down) from the top left, and
//! clears the rest, under k pixels across and down, to opaque black. A
//! window smaller than k on a side takes the largest scale that fits.

use super::{MAX_SIDE, Targets};
use crate::error::GpuError;
use crate::gpu::Gpu;

/// The window image targets present into, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Present {
    /// Window pixels across and down per texel, at least 1.
    scale: u32,
    width: u32,
    height: u32,
    /// Where the image's top left corner lands in the window.
    offset: (u32, u32),
}

impl Present {
    /// Presenting 1:1 into an image of `width` by `height`.
    pub(crate) const fn same(width: u32, height: u32) -> Self {
        Self {
            scale: 1,
            width,
            height,
            offset: (0, 0),
        }
    }

    /// Presenting into a window of `width` by `height` at `scale`, and the
    /// targets' size that takes.
    fn fit(width: u32, height: u32, scale: u32) -> Result<(Self, (u32, u32)), GpuError> {
        let fits = |side: u32| (1..=MAX_SIDE).contains(&side);
        if !fits(width) || !fits(height) {
            return Err(GpuError::Size { width, height });
        }
        if scale == 0 {
            return Err(GpuError::Surface(
                "an upscale of 0: each texel presents as at least 1x1 pixels".into(),
            ));
        }
        let scale = scale.min(width).min(height);
        let targets = (width / scale, height / scale);
        let present = Self {
            scale,
            width,
            height,
            offset: (
                (width - targets.0 * scale) / 2,
                (height - targets.1 * scale) / 2,
            ),
        };
        Ok((present, targets))
    }

    pub(crate) const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl Targets {
    /// Targets for a window of `width` by `height` pixels that shows each
    /// texel as `scale` by `scale` pixels: rung 2's low internal resolution
    /// (ADR-0020). The targets are the window's size divided by `scale`,
    /// rounded down; a surface graph's present pass draws the output into
    /// the window's image (a [`crate::SurfaceFrame`] of
    /// [`Targets::surface_size`]) scaled by `scale` and centred, the
    /// remainder black (see the top of `upscale.rs`). A window smaller than
    /// `scale` on a side takes the largest scale that fits. Scale 1 is
    /// [`Targets::new`]. Fails with `[E-GPU]` for a side outside 1 to 8,192
    /// or a scale of 0.
    pub fn upscaled(gpu: &Gpu, width: u32, height: u32, scale: u32) -> Result<Self, GpuError> {
        let (present, (w, h)) = Present::fit(width, height, scale)?;
        Self::make(gpu, w, h, false, present)
    }

    /// Like [`Targets::upscaled`], with an offscreen stand-in for the
    /// window's image, of the window's size, as
    /// [`Targets::with_offscreen_surface`] has: [`Targets::read_surface`]
    /// reads back the scaled image the present pass drew.
    pub fn upscaled_with_offscreen_surface(
        gpu: &Gpu,
        width: u32,
        height: u32,
        scale: u32,
    ) -> Result<Self, GpuError> {
        let (present, (w, h)) = Present::fit(width, height, scale)?;
        Self::make(gpu, w, h, true, present)
    }

    /// The size of the window image these targets present into: their
    /// [`Targets::size`] unless they are [`Targets::upscaled`].
    pub fn surface_size(&self) -> (u32, u32) {
        self.present.size()
    }

    /// How many window pixels across and down each texel presents as: 1
    /// unless the targets are [`Targets::upscaled`].
    pub fn upscale(&self) -> u32 {
        self.present.scale
    }

    /// Where the present pass draws, `[x, y, width, height]` in the window
    /// image's pixels, for upscaled targets; `None` for 1:1, where it draws
    /// the whole image as in rung 1b.
    pub(crate) fn present_viewport(&self) -> Option<[f32; 4]> {
        let p = self.present;
        (p.scale > 1).then(|| {
            [
                p.offset.0 as f32,
                p.offset.1 as f32,
                (self.width * p.scale) as f32,
                (self.height * p.scale) as f32,
            ]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Present;

    #[test]
    fn a_window_fits_the_largest_whole_image_centred() {
        let fit = |w, h, k| Present::fit(w, h, k).map(|(p, t)| (p.scale, p.offset, t));
        assert_eq!(fit(1920, 1080, 4), Ok((4, (0, 0), (480, 270))));
        assert_eq!(fit(1001, 702, 4), Ok((4, (0, 1), (250, 175))));
        assert_eq!(fit(1003, 703, 4), Ok((4, (1, 1), (250, 175))));
        assert_eq!(fit(640, 480, 1), Ok((1, (0, 0), (640, 480))));
        // Smaller than the scale: the largest that fits.
        assert_eq!(fit(3, 100, 4), Ok((3, (0, 0), (1, 33))));
        assert!(fit(640, 480, 0).is_err_and(|e| e.to_string().contains("an upscale of 0")));
        assert!(fit(0, 480, 2).is_err());
        assert!(fit(9000, 480, 2).is_err());
    }
}
