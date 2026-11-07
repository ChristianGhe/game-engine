//! Uploading a scene ([`crate::Gpu::scene`]): its megabuffers, material
//! table and texture arrays, and the bind group that holds them.

use super::{
    ColourTag, Scene, SceneDesc, TEXTURE_SIDES, TextureSlot, array, material_records, slots,
};
use crate::error::GpuError;
use crate::gpu::Gpu;
use crate::layout::Layouts;

impl Gpu {
    /// Uploads a scene: its megabuffers, material table and texture arrays.
    /// Fails with `[E-GPU]` on a texture that is not 64, 128 or 256 px
    /// square RGBA8, more than [`crate::MAX_TEXTURE_LAYERS`] textures of one side
    /// and tag, a material naming a texture the scene lacks, or vertices that
    /// are not whole 4-byte words.
    pub fn scene(&self, desc: &SceneDesc<'_>) -> Result<Scene, GpuError> {
        let fail = |message: String| GpuError::Scene {
            label: desc.label.to_string(),
            message,
        };
        if !desc.vertices.len().is_multiple_of(4) {
            return Err(fail(format!(
                "{} bytes of vertices are not whole 4-byte words",
                desc.vertices.len()
            )));
        }
        let (slots, layers) = slots(desc).map_err(fail)?;
        let materials = material_records(desc, &slots).map_err(fail)?;
        let indices: Vec<u8> = desc.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let texels: u64 = desc.textures.iter().map(|t| t.rgba8.len() as u64).sum();
        let bytes = (desc.vertices.len() + indices.len() + materials.len()) as u64 + texels;
        let index_count = u32::try_from(desc.indices.len()).map_err(|_| {
            fail(format!(
                "{} indices are more than a draw can count",
                desc.indices.len()
            ))
        })?;
        let scene = self
            .scoped(|| {
                let buffer = |what: &str, data: &[u8], usage: wgpu::BufferUsages| {
                    buffer(self, &format!("{} {what}", desc.label), data, usage)
                };
                let vertices = buffer("vertices", desc.vertices, wgpu::BufferUsages::VERTEX);
                let index_buffer = buffer("indices", &indices, wgpu::BufferUsages::INDEX);
                let material_buffer = buffer("materials", &materials, wgpu::BufferUsages::STORAGE);
                let arrays = arrays(self, desc, &slots, &layers);
                let group = bind(self, desc.label, &self.layouts, &material_buffer, &arrays);
                Scene {
                    vertices,
                    indices: index_buffer,
                    index_count,
                    group,
                    slots,
                    bytes,
                }
            })
            .map_err(GpuError::Wgpu)?;
        Ok(scene)
    }
}

/// A buffer holding `data`, or 16 zero bytes for none, since a binding or a
/// vertex buffer cannot be empty.
pub(crate) fn buffer(
    gpu: &Gpu,
    label: &str,
    data: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let size = (data.len() as u64).max(16).next_multiple_of(4);
    let made = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    if !data.is_empty() {
        gpu.queue.write_buffer(&made, 0, data);
    }
    made
}

/// The six texture arrays, each with its textures' layers written, or a
/// single empty layer where the scene has none of that side and tag.
fn arrays(
    gpu: &Gpu,
    desc: &SceneDesc<'_>,
    slots: &[TextureSlot],
    layers: &[u32; 6],
) -> Vec<wgpu::TextureView> {
    let mut made = Vec::with_capacity(6);
    for (index, &count) in layers.iter().enumerate() {
        let side = TEXTURE_SIDES[index / 2];
        let tag = if index % 2 == 0 {
            ColourTag::Srgb
        } else {
            ColourTag::Linear
        };
        let format = match tag {
            ColourTag::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            ColourTag::Linear => wgpu::TextureFormat::Rgba8Unorm,
        };
        let (side, count) = if count == 0 { (1, 1) } else { (side, count) };
        let label = format!("{} textures {side} px {tag:?}", desc.label);
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&label),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (texture_desc, slot) in desc.textures.iter().zip(slots) {
            if array(slot.bucket(), slot.tag) != index {
                continue;
            }
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: slot.layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                texture_desc.rgba8,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(slot.side * 4),
                    rows_per_image: Some(slot.side),
                },
                wgpu::Extent3d {
                    width: slot.side,
                    height: slot.side,
                    depth_or_array_layers: 1,
                },
            );
        }
        made.push(texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&label),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..wgpu::TextureViewDescriptor::default()
        }));
    }
    made
}

/// The scene group: materials, the six arrays and the repeating sampler.
fn bind(
    gpu: &Gpu,
    label: &str,
    layouts: &Layouts,
    materials: &wgpu::Buffer,
    arrays: &[wgpu::TextureView],
) -> wgpu::BindGroup {
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: materials.as_entire_binding(),
    }];
    for (binding, view) in (1..).zip(arrays) {
        entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        });
    }
    entries.push(wgpu::BindGroupEntry {
        binding: 7,
        resource: wgpu::BindingResource::Sampler(&layouts.repeating),
    });
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label} scene")),
        layout: &layouts.scene,
        entries: &entries,
    })
}
