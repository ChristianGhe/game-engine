//! Offscreen targets and readback: the engine-owned textures a graph renders
//! into, and the output's pixels read back for `shot`, `video` and goldens.

use crate::Resource;
use crate::error::GpuError;
use crate::gpu::Gpu;

/// The scene's format: linear, scene-referred HDR (spec 6.7).
const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The output's format. The GPU encodes sRGB on write, so tone mapping
/// writes linear values (plan decision 2).
const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The longest side a target may have: Tier 0's largest 2D texture.
pub(crate) const MAX_SIDE: u32 = 8192;

/// The texture format of an engine-owned resource; `None` for the surface.
pub(crate) fn format(resource: Resource) -> Option<wgpu::TextureFormat> {
    match resource {
        Resource::Scene => Some(SCENE_FORMAT),
        Resource::Output => Some(OUTPUT_FORMAT),
        Resource::Surface => None,
    }
}

/// The engine-owned textures of one size that a graph renders into: the
/// [`Resource::Scene`] and the [`Resource::Output`].
pub struct Targets {
    width: u32,
    height: u32,
    scene: wgpu::TextureView,
    output_texture: wgpu::Texture,
    output: wgpu::TextureView,
}

impl Targets {
    /// Targets of `width` by `height` pixels, each from 1 to 8,192.
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Result<Self, GpuError> {
        let fits = |side: u32| (1..=MAX_SIDE).contains(&side);
        if !fits(width) || !fits(height) {
            return Err(GpuError::Size { width, height });
        }
        let create = |label: &str, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let drawn = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let (scene, output_texture) = gpu
            .scoped(|| {
                (
                    create("scene", SCENE_FORMAT, drawn),
                    create(
                        "output",
                        OUTPUT_FORMAT,
                        drawn | wgpu::TextureUsages::COPY_SRC,
                    ),
                )
            })
            .map_err(GpuError::Wgpu)?;
        Ok(Self {
            width,
            height,
            scene: scene.create_view(&wgpu::TextureViewDescriptor::default()),
            output: output_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            output_texture,
        })
    }

    /// Width and height in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The view a pass draws into or reads; `None` for the surface.
    pub(crate) fn view(&self, resource: Resource) -> Option<&wgpu::TextureView> {
        match resource {
            Resource::Scene => Some(&self.scene),
            Resource::Output => Some(&self.output),
            Resource::Surface => None,
        }
    }

    /// Copies the output back to the CPU, waiting for the GPU to finish the
    /// frames submitted before.
    pub fn read_output(&self, gpu: &Gpu) -> Result<Image, GpuError> {
        let row = self.width * 4;
        let stride =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu
            .scoped(|| {
                let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("readback"),
                    size: u64::from(stride) * u64::from(self.height),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("readback"),
                        });
                encoder.copy_texture_to_buffer(
                    self.output_texture.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(stride),
                            rows_per_image: Some(self.height),
                        },
                    },
                    wgpu::Extent3d {
                        width: self.width,
                        height: self.height,
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
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| GpuError::Wgpu(format!("waiting for the readback: {e}")))?;
        receiver
            .try_recv()
            .map_err(|_| GpuError::Wgpu("the readback was not mapped after the wait".into()))?
            .map_err(|e| GpuError::Wgpu(format!("mapping the readback: {e}")))?;
        let mapped = buffer
            .get_mapped_range(..)
            .map_err(|e| GpuError::Wgpu(format!("reading the readback: {e}")))?;
        let mut rgba8 = Vec::with_capacity(row as usize * self.height as usize);
        for line in mapped.chunks_exact(stride as usize) {
            rgba8.extend_from_slice(&line[..row as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(Image {
            width: self.width,
            height: self.height,
            rgba8,
        })
    }
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
