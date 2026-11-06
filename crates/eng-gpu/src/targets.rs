//! Offscreen targets and readback: the engine-owned textures a graph renders
//! into, and the output's pixels read back for `shot`, `video` and goldens.
//! An offscreen texture can stand in for a window's surface, so that a
//! surface graph renders whole without a window. The targets also keep the
//! textures that graphs' transients are drawn into (their pool).

use std::sync::{Mutex, PoisonError};

use crate::error::GpuError;
use crate::gpu::Gpu;
use crate::graph::{Format, Plan, PooledTexture};
use crate::{Resource, Transient};

/// The scene's format: linear, scene-referred HDR (spec 6.7).
const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The output's format. The GPU encodes sRGB on write, so tone mapping
/// writes linear values (plan decision 2).
const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The surface's format: BGRA, which every rung 1b backend offers a window,
/// sRGB-encoded like the output, so the present blit reproduces the output's
/// bytes.
const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// The longest side a target may have: Tier 0's largest 2D texture.
pub(crate) const MAX_SIDE: u32 = 8192;

/// How long a readback waits for the GPU: far longer than any frame takes
/// (the four golden shots take 3.5 s together on emulated lavapipe), so only
/// a driver that never finishes reaches it, and the run fails with
/// `[E-GPU]` instead of hanging.
pub const READBACK_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// The texture format a resource is drawn in.
pub(crate) fn format(resource: Resource) -> wgpu::TextureFormat {
    match resource {
        Resource::Scene => SCENE_FORMAT,
        Resource::Output => OUTPUT_FORMAT,
        Resource::Surface => SURFACE_FORMAT,
        Resource::Transient(Transient { format, .. }) => transient_format(format),
    }
}

fn transient_format(format: Format) -> wgpu::TextureFormat {
    match format {
        Format::Rgba16Float => SCENE_FORMAT,
        Format::Rgba8UnormSrgb => OUTPUT_FORMAT,
    }
}

/// The engine-owned textures of one size that a graph renders into: the
/// [`Resource::Scene`] and the [`Resource::Output`], and with
/// [`Targets::with_offscreen_surface`] a stand-in for the
/// [`Resource::Surface`]. They also keep the textures of graphs'
/// [`Resource::Transient`]s ([`Targets::transient_textures`]). A resize
/// makes new targets, so those are made again at the new size.
pub struct Targets {
    width: u32,
    height: u32,
    scene: wgpu::TextureView,
    output_texture: wgpu::Texture,
    output: wgpu::TextureView,
    surface: Option<(wgpu::Texture, wgpu::TextureView)>,
    /// Transient textures, in the order they were made. A render takes the
    /// pool by reference ([`crate::Graph::render`]), so it sits in a mutex.
    pool: Mutex<Vec<Pooled>>,
}

/// One texture of the targets' pool, for transients of one kind.
struct Pooled {
    kind: Kind,
    view: wgpu::TextureView,
}

/// What a pooled texture is made as: format, divisor and whether passes
/// read it.
type Kind = (Format, u32, bool);

fn kind(texture: &PooledTexture) -> Kind {
    (texture.format, texture.divisor, texture.sampled)
}

impl Targets {
    /// Targets of `width` by `height` pixels, each from 1 to 8,192.
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Result<Self, GpuError> {
        Self::make(gpu, width, height, false)
    }

    /// Like [`Targets::new`], plus an offscreen texture in the surface's
    /// format (B8G8R8A8, sRGB) that stands in for a window's surface: a
    /// [`crate::Target::Surface`] graph renders into it, and
    /// [`Targets::read_surface`] reads back what its present pass drew. A
    /// window's own surface ([`crate::WindowSurface`]) presents instead.
    pub fn with_offscreen_surface(gpu: &Gpu, width: u32, height: u32) -> Result<Self, GpuError> {
        Self::make(gpu, width, height, true)
    }

    fn make(gpu: &Gpu, width: u32, height: u32, surface: bool) -> Result<Self, GpuError> {
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
        let (scene, output_texture, surface) = gpu
            .scoped(|| {
                (
                    create("scene", SCENE_FORMAT, drawn),
                    create(
                        "output",
                        OUTPUT_FORMAT,
                        drawn | wgpu::TextureUsages::COPY_SRC,
                    ),
                    // Like a swapchain image, it can be drawn into but not
                    // read by a pass; the copy is for readback.
                    surface.then(|| {
                        create(
                            "surface",
                            SURFACE_FORMAT,
                            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                        )
                    }),
                )
            })
            .map_err(GpuError::Wgpu)?;
        Ok(Self {
            width,
            height,
            scene: labelled_view(&scene, "scene"),
            output: labelled_view(&output_texture, "output"),
            output_texture,
            surface: surface.map(|texture| {
                let surface_view = labelled_view(&texture, "surface");
                (texture, surface_view)
            }),
            pool: Mutex::new(Vec::new()),
        })
    }

    /// Width and height in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Whether the targets have a surface, so that a
    /// [`crate::Target::Surface`] graph can render into them.
    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    /// The view a pass draws into or reads; `None` for a missing surface,
    /// and for a transient, whose view comes from [`Targets::pooled`].
    pub(crate) fn view(&self, resource: Resource) -> Option<&wgpu::TextureView> {
        match resource {
            Resource::Scene => Some(&self.scene),
            Resource::Output => Some(&self.output),
            Resource::Surface => self.surface.as_ref().map(|(_, view)| view),
            Resource::Transient(_) => None,
        }
    }

    /// How many transient textures the targets keep. A graph's first frame
    /// makes those its [`crate::Graph::pool`] needs beyond the ones already
    /// here, and every later frame, of that graph or another, draws into
    /// the same ones: the n-th texture of a kind (format, size and usage)
    /// in a graph's pool is the n-th of that kind here.
    pub fn transient_textures(&self) -> usize {
        self.pool
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// A view of each texture in `plan`, in its order, making those the pool
    /// lacks.
    pub(crate) fn pooled(
        &self,
        gpu: &Gpu,
        plan: &Plan,
    ) -> Result<Vec<wgpu::TextureView>, GpuError> {
        let mut pool = self.pool.lock().unwrap_or_else(PoisonError::into_inner);
        let mut views = Vec::with_capacity(plan.textures().len());
        for (index, texture) in plan.textures().iter().enumerate() {
            let kind = kind(texture);
            let nth = plan.textures()[..index]
                .iter()
                .filter(|t| self::kind(t) == kind)
                .count();
            let found = pool.iter().filter(|p| p.kind == kind).nth(nth);
            let view = match found {
                Some(pooled) => pooled.view.clone(),
                None => {
                    let label = format!(
                        "transient {:?} 1/{} #{nth}",
                        texture.format, texture.divisor
                    );
                    let view = gpu
                        .scoped(|| self.transient(gpu, texture, &label))
                        .map_err(GpuError::Wgpu)?;
                    pool.push(Pooled {
                        kind,
                        view: view.clone(),
                    });
                    view
                }
            };
            views.push(view);
        }
        Ok(views)
    }

    /// A new texture for `texture`'s transients: their format, the targets'
    /// size divided by the divisor and rounded up, drawn into, and read if
    /// they are.
    fn transient(&self, gpu: &Gpu, texture: &PooledTexture, label: &str) -> wgpu::TextureView {
        let side = |pixels: u32| pixels.div_ceil(texture.divisor);
        let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if texture.sampled {
            usage |= wgpu::TextureUsages::TEXTURE_BINDING;
        }
        let made = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: side(self.width),
                height: side(self.height),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: transient_format(texture.format),
            usage,
            view_formats: &[],
        });
        labelled_view(&made, label)
    }

    /// Copies the output back to the CPU, waiting for the GPU to finish the
    /// frames submitted before. A wait longer than [`READBACK_LIMIT`] fails
    /// with [`GpuError::Wgpu`] instead of blocking for ever.
    pub fn read_output(&self, gpu: &Gpu) -> Result<Image, GpuError> {
        self.read(gpu, &self.output_texture)
    }

    /// Copies the offscreen surface back to the CPU, as
    /// [`Targets::read_output`] does the output: RGBA8 in that order,
    /// sRGB-encoded, though the surface stores B, G, R, A. Without a surface
    /// it fails with [`GpuError::NoSurface`].
    pub fn read_surface(&self, gpu: &Gpu) -> Result<Image, GpuError> {
        let (texture, _) = self.surface.as_ref().ok_or(GpuError::NoSurface)?;
        let mut image = self.read(gpu, texture)?;
        for pixel in image.rgba8.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        Ok(image)
    }

    /// The bytes of a texture of this size with four bytes a pixel, as
    /// stored.
    fn read(&self, gpu: &Gpu, texture: &wgpu::Texture) -> Result<Image, GpuError> {
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
                    texture.as_image_copy(),
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
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(READBACK_LIMIT),
            })
            .map_err(|e| {
                GpuError::Wgpu(format!(
                    "waiting for the readback (at most {} s): {e}",
                    READBACK_LIMIT.as_secs()
                ))
            })?;
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

/// A view of all of `texture`, named `label` in GPU debuggers.
fn labelled_view(texture: &wgpu::Texture, label: &str) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some(label),
        ..wgpu::TextureViewDescriptor::default()
    })
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
