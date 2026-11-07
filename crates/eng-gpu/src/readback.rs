//! Reading a transient back (rung 2, T9): what a graph's last frame left in
//! one of its transients, such as the first-person `depth` and `info`, for
//! depth probes, id checks and frame records (plan addition A1). Rung 1b's
//! readback of the output and the surface is `targets.rs`'s, unchanged, and
//! gives the [`Image`] defined here.

use crate::error::GpuError;
use crate::gpu::Gpu;
use crate::graph::{Format, Graph, Resource, Transient};
use crate::targets::{READBACK_LIMIT, Targets};

/// A transient's texels as the GPU stores them ([`Targets::read_transient`]):
/// rows top first, [`Format::texel_bytes`] bytes a texel, little-endian, no
/// padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texels {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    /// `width * height * format.texel_bytes()` bytes.
    pub bytes: Vec<u8>,
}

impl Texels {
    /// The bytes of the texel in column `x` and row `y`; (0, 0) is the top
    /// left.
    ///
    /// # Panics
    ///
    /// Outside the texels.
    pub fn texel(&self, x: u32, y: u32) -> &[u8] {
        assert!(
            x < self.width && y < self.height,
            "texel ({x}, {y}) is outside {}x{} texels",
            self.width,
            self.height
        );
        let size = self.format.texel_bytes() as usize;
        let at = (y as usize * self.width as usize + x as usize) * size;
        &self.bytes[at..at + size]
    }

    /// The depth at (`x`, `y`) of a [`Format::Depth32Float`] read: 0 on the
    /// far plane, as eng-gpu's depth is reversed.
    ///
    /// # Panics
    ///
    /// Outside the texels, or for another format.
    pub fn depth(&self, x: u32, y: u32) -> f32 {
        assert_eq!(self.format, Format::Depth32Float, "a depth read");
        let mut word = [0; 4];
        word.copy_from_slice(self.texel(x, y));
        f32::from_le_bytes(word)
    }

    /// The two integers at (`x`, `y`) of a [`Format::Rg32Uint`] read.
    ///
    /// # Panics
    ///
    /// Outside the texels, or for another format.
    pub fn uint2(&self, x: u32, y: u32) -> [u32; 2] {
        assert_eq!(self.format, Format::Rg32Uint, "an Rg32Uint read");
        let texel = self.texel(x, y);
        let word = |at: usize| {
            let mut word = [0; 4];
            word.copy_from_slice(&texel[at..at + 4]);
            u32::from_le_bytes(word)
        };
        [word(0), word(4)]
    }
}

impl Targets {
    /// Copies back what the last frame rendered into these targets left in
    /// `transient`, as `graph` draws it, waiting for the GPU as
    /// [`Targets::read_output`] does. It is the transient's texture in the
    /// targets' pool, at its own size (the targets' divided by its divisor).
    /// A transient that a bypass draws into another transient's texture
    /// ([`Graph::sorted_without`]) reads that texture, which is what its
    /// readers read.
    ///
    /// Fails with `[E-GPU]` if no frame has been submitted into these
    /// targets, or the last one failed; if the last frame's graph pooled its
    /// transients otherwise than `graph` does (graphs share the pool, so
    /// only a frame of `graph`, or of a graph that pools alike, says what
    /// the textures hold); if no pass of `graph` that runs draws
    /// `transient` (with its name, format and divisor); if a bypass draws it
    /// into a built-in resource's texture; or if `graph` pools a transient
    /// that the frame uses after it into the same texture
    /// ([`Graph::pool`]), which would have drawn over it.
    pub fn read_transient(
        &self,
        gpu: &Gpu,
        graph: &Graph,
        transient: Transient,
    ) -> Result<Texels, GpuError> {
        let fail = |message: String| GpuError::Readback {
            transient: transient.name,
            message,
        };
        match &*self.drawn() {
            None => {
                return Err(fail(
                    "no frame has been rendered into these targets, or the last one failed".into(),
                ));
            }
            Some(plan) if *plan != graph.plan => {
                return Err(fail(
                    "the last frame rendered into these targets ran a graph that pools its \
                     transients otherwise"
                        .into(),
                ));
            }
            Some(_) => {}
        }
        let drawn = match graph.aliases.of(Resource::Transient(transient)) {
            Resource::Transient(drawn) => drawn,
            other => {
                return Err(fail(format!(
                    "it is drawn into `{}`, bypassed; read that instead",
                    other.name()
                )));
            }
        };
        let index = graph
            .plan
            .texture(&drawn)
            .ok_or_else(|| fail("no pass of the graph that runs draws it".into()))?;
        let pooled = &graph.pool()[index];
        if (pooled.format, pooled.divisor) != (transient.format, transient.divisor) {
            return Err(fail(format!(
                "the graph draws it as {:?} at 1/{}, not {:?} at 1/{}",
                pooled.format, pooled.divisor, transient.format, transient.divisor
            )));
        }
        if let Some(later) = pooled
            .transients
            .iter()
            .skip_while(|&&name| name != drawn.name)
            .nth(1)
        {
            return Err(fail(format!(
                "it shares its texture with `{later}`, which the frame draws after it"
            )));
        }
        let texture = self
            .pooled_texture(&graph.plan, index)
            .ok_or_else(|| fail("the targets' pool lacks its texture".into()))?;
        let (width, height) = self.size();
        let (width, height) = (
            width.div_ceil(transient.divisor),
            height.div_ceil(transient.divisor),
        );
        let bytes = copy_back(gpu, &texture, transient.format, width, height)?;
        Ok(Texels {
            width,
            height,
            format: transient.format,
            bytes,
        })
    }
}

/// The texels of all of `texture`, `width` by `height` in `format`, as
/// stored, without the row padding a copy needs. A wait longer than
/// [`READBACK_LIMIT`] fails with [`GpuError::Wgpu`].
fn copy_back(
    gpu: &Gpu,
    texture: &wgpu::Texture,
    format: Format,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, GpuError> {
    let row = width * format.texel_bytes();
    let stride =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    // A depth texture copies its depth aspect, its only one.
    let aspect = if format.is_depth() {
        wgpu::TextureAspect::DepthOnly
    } else {
        wgpu::TextureAspect::All
    };
    let buffer = gpu
        .scoped(|| {
            let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("transient readback"),
                size: u64::from(stride) * u64::from(height),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("transient readback"),
                });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            gpu.queue.submit([encoder.finish()]);
            buffer
        })
        .map_err(GpuError::Wgpu)?;
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer.map_async(wgpu::MapMode::Read, .., move |mapped| {
        // The receiver waits below, so a failed send cannot happen.
        let _ = sender.send(mapped);
    });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(READBACK_LIMIT),
        })
        .map_err(|e| {
            GpuError::Wgpu(format!(
                "waiting for the transient readback (at most {} s): {e}",
                READBACK_LIMIT.as_secs()
            ))
        })?;
    receiver
        .try_recv()
        .map_err(|_| GpuError::Wgpu("the transient readback was not mapped after the wait".into()))?
        .map_err(|e| GpuError::Wgpu(format!("mapping the transient readback: {e}")))?;
    let mapped = buffer
        .get_mapped_range(..)
        .map_err(|e| GpuError::Wgpu(format!("reading the transient readback: {e}")))?;
    let mut bytes = Vec::with_capacity(row as usize * height as usize);
    for line in mapped.chunks_exact(stride as usize) {
        bytes.extend_from_slice(&line[..row as usize]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(bytes)
}

/// The output's pixels: RGBA8, sRGB-encoded as the output stores them, rows
/// top first, with no padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba8: Vec<u8>,
}

impl Image {
    /// The pixel in column `x` and row `y`; (0, 0) is the top left.
    ///
    /// # Panics
    ///
    /// Outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        assert!(
            x < self.width && y < self.height,
            "pixel ({x}, {y}) is outside a {}x{} image",
            self.width,
            self.height
        );
        let at = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.rgba8[at],
            self.rgba8[at + 1],
            self.rgba8[at + 2],
            self.rgba8[at + 3],
        ]
    }
}
