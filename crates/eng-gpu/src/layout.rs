//! What rung 2's pipelines bind (ADR-0020), by how often it changes, and
//! the targets a pass draws into.
//!
//! - **Group 0, the frame:** `@binding(0)`, the per-frame uniform, whose
//!   layout the caller's shaders fix; `@binding(1)`, the frame's
//!   [`crate::InstanceRecord`]s as a read-only `array` in storage.
//! - **Group 1, the scene ([`crate::Scene`]):** `@binding(0)`, the material
//!   table, a read-only `array` in storage; `@binding(1)` to `@binding(6)`,
//!   the six `texture_2d_array<f32>`s, 64, 128 then 256 px, each sRGB then
//!   linear; `@binding(7)`, a sampler for the nearest texel that repeats.
//! - **Group 2, the pass's reads:** `@binding(i)` is its i-th read, a
//!   `texture_2d<f32>` for colour, a `texture_2d<u32>` for
//!   [`crate::Format::Rg32Uint`] and a `texture_depth_2d` for
//!   [`crate::Format::Depth32Float`]; `@binding(n)`, n the number of reads,
//!   a sampler for the nearest texel, clamped at the edges.
//!
//! Three of Tier 0's four groups. 1b's fullscreen pipelines keep their own
//! group 0, the input and its sampler.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use crate::graph::{Format, Resource, Transient};
use crate::targets;

/// The most colour targets a pass draws into. Tier 0 allows 8, and 32 bytes
/// per sample in all; four keeps a pass of four 8-byte formats inside it.
pub const MAX_COLOUR_TARGETS: usize = 4;

/// The textures one shader stage may sample at Tier 0.
const MAX_SAMPLED_TEXTURES: usize = 16;

/// The scene's texture arrays, which a pipeline binding the scene samples.
const SCENE_TEXTURES: usize = 6;

/// Whether a pipeline's stage can sample `reads`, beside the scene's arrays
/// when it binds the `scene`: Tier 0 allows 16 textures a stage.
pub(crate) fn check_reads(reads: &[Resource], scene: bool) -> Result<(), String> {
    let room = MAX_SAMPLED_TEXTURES - if scene { SCENE_TEXTURES } else { 0 };
    if reads.len() > room {
        let beside = if scene {
            " beside the scene's six arrays"
        } else {
            ""
        };
        return Err(format!(
            "its pass reads {} textures; a stage samples at most {room}{beside} (Tier 0: \
             {MAX_SAMPLED_TEXTURES})",
            reads.len()
        ));
    }
    Ok(())
}

/// The divisor of the targets' size `resource` is drawn at: 1 for the
/// built-in resources.
pub(crate) fn divisor(resource: Resource) -> u32 {
    match resource {
        Resource::Transient(transient) => transient.divisor,
        _ => 1,
    }
}

/// The group of the frame's uniform and instance records.
pub(crate) const FRAME_GROUP: u32 = 0;
/// The group of the scene's materials, textures and sampler.
pub(crate) const SCENE_GROUP: u32 = 1;
/// The group of the pass's reads.
pub(crate) const READS_GROUP: u32 = 2;

/// How a pass reads a resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ReadKind {
    /// A float texture, read without filtering.
    Colour,
    /// An unsigned-integer texture, read with `textureLoad`.
    Uint,
    /// A depth texture.
    Depth,
}

/// How `resource` is read.
pub(crate) fn read_kind(resource: Resource) -> ReadKind {
    match resource {
        Resource::Transient(Transient {
            format: Format::Rg32Uint,
            ..
        }) => ReadKind::Uint,
        Resource::Transient(Transient {
            format: Format::Depth32Float,
            ..
        }) => ReadKind::Depth,
        _ => ReadKind::Colour,
    }
}

/// Whether `resource` is drawn as a depth target.
pub(crate) fn is_depth(resource: Resource) -> bool {
    matches!(resource, Resource::Transient(t) if t.format.is_depth())
}

/// The formats of a pass's colour targets, in the order it writes them,
/// and of its depth target: `writes` without the surface, with at least one
/// write, at most [`MAX_COLOUR_TARGETS`] colour targets and one depth
/// target. The error says which rule `writes` breaks.
pub(crate) fn attachments(
    writes: &[Resource],
) -> Result<(Vec<wgpu::TextureFormat>, Option<wgpu::TextureFormat>), String> {
    if writes.contains(&Resource::Surface) {
        return Err("only the present blit draws into `surface`".into());
    }
    let (depth, colour): (Vec<Resource>, Vec<Resource>) =
        writes.iter().copied().partition(|&r| is_depth(r));
    if colour.len() > MAX_COLOUR_TARGETS {
        return Err(format!(
            "it writes {} colour targets; a pass draws into at most {MAX_COLOUR_TARGETS}",
            colour.len()
        ));
    }
    if depth.len() > 1 {
        return Err(format!(
            "it writes {} depth targets; a pass draws into at most one",
            depth.len()
        ));
    }
    if writes.is_empty() {
        return Err("it writes nothing".into());
    }
    Ok((
        colour.into_iter().map(targets::format).collect(),
        depth.first().map(|&r| targets::format(r)),
    ))
}

/// The bind-group layouts and samplers rung 2's pipelines share.
pub(crate) struct Layouts {
    pub(crate) frame: wgpu::BindGroupLayout,
    pub(crate) scene: wgpu::BindGroupLayout,
    /// The reads' layouts, one for each list of kinds, so that pipelines
    /// reading alike share one.
    reads: Mutex<BTreeMap<Vec<ReadKind>, wgpu::BindGroupLayout>>,
    /// Nearest, clamped: for the pass's reads.
    pub(crate) clamped: wgpu::Sampler,
    /// Nearest, repeating, no mipmaps: for the scene's textures (spec 6.11's
    /// world-anchored coordinates repeat).
    pub(crate) repeating: wgpu::Sampler,
}

/// Every stage that may bind rung 2's groups.
const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::VERTEX_FRAGMENT;

fn storage(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: STAGES,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture(
    binding: u32,
    sample_type: wgpu::TextureSampleType,
    view_dimension: wgpu::TextureViewDimension,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: STAGES,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler(binding: u32, kind: wgpu::SamplerBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: STAGES,
        ty: wgpu::BindingType::Sampler(kind),
        count: None,
    }
}

impl Layouts {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: STAGES,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1),
            ],
        });
        let mut entries = vec![storage(0)];
        for binding in 1..=6 {
            entries.push(texture(
                binding,
                wgpu::TextureSampleType::Float { filterable: true },
                wgpu::TextureViewDimension::D2Array,
            ));
        }
        entries.push(sampler(7, wgpu::SamplerBindingType::Filtering));
        let scene = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene"),
            entries: &entries,
        });
        let nearest = |label: &str, address: wgpu::AddressMode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: address,
                address_mode_v: address,
                address_mode_w: address,
                ..wgpu::SamplerDescriptor::default()
            })
        };
        Self {
            frame,
            scene,
            reads: Mutex::new(BTreeMap::new()),
            clamped: nearest("reads", wgpu::AddressMode::ClampToEdge),
            repeating: nearest("scene textures", wgpu::AddressMode::Repeat),
        }
    }

    /// The layout of a pass that reads `reads`, or `None` without any.
    pub(crate) fn reads(
        &self,
        device: &wgpu::Device,
        reads: &[Resource],
    ) -> Option<wgpu::BindGroupLayout> {
        if reads.is_empty() {
            return None;
        }
        let kinds: Vec<ReadKind> = reads.iter().map(|&r| read_kind(r)).collect();
        let mut cache = self.reads.lock().unwrap_or_else(PoisonError::into_inner);
        let layout = cache.entry(kinds).or_insert_with_key(|kinds| {
            let mut entries: Vec<wgpu::BindGroupLayoutEntry> = (0..)
                .zip(kinds)
                .map(|(binding, kind)| {
                    let sample_type = match kind {
                        ReadKind::Colour => wgpu::TextureSampleType::Float { filterable: false },
                        ReadKind::Uint => wgpu::TextureSampleType::Uint,
                        ReadKind::Depth => wgpu::TextureSampleType::Depth,
                    };
                    texture(binding, sample_type, wgpu::TextureViewDimension::D2)
                })
                .collect();
            let at = u32::try_from(kinds.len()).unwrap_or(u32::MAX);
            entries.push(sampler(at, wgpu::SamplerBindingType::NonFiltering));
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("reads"),
                entries: &entries,
            })
        });
        Some(layout.clone())
    }
}

/// A pipeline layout of `groups` in order, a `None` leaving its group
/// unused.
pub(crate) fn pipeline_layout(
    device: &wgpu::Device,
    label: &str,
    groups: &[Option<&wgpu::BindGroupLayout>],
) -> wgpu::PipelineLayout {
    let used = groups
        .iter()
        .rposition(Option::is_some)
        .map_or(0, |i| i + 1);
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &groups[..used],
        immediate_size: 0,
    })
}

/// The colour target states of `formats`: replacing what is there, all
/// channels.
pub(crate) fn colour_states(
    formats: &[wgpu::TextureFormat],
) -> Vec<Option<wgpu::ColorTargetState>> {
    formats
        .iter()
        .map(|&format| {
            Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        })
        .collect()
}
