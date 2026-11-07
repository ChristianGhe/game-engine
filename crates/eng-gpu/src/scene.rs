//! The GPU scene layout (spec 6.7 "fixed at rung 2"; rung 2 plan, decision
//! 6; ADR-0020), written once when a scene loads:
//! - one vertex and one `u32` index megabuffer, vertices relative to their
//!   chunk's origin;
//! - a material table in a read-only storage buffer;
//! - `texture_2d_array`s of square textures, bucketed by size (64, 128 and
//!   256 px), one array per bucket and colour tag, sampled at the nearest
//!   texel with no mipmaps. Tier 0 has no binding arrays, so a texture is a
//!   layer of an array, not an index into an array of textures.
//!
//! Each frame then brings its [`InstanceRecord`]s: a chunk's origin minus
//! the eye, subtracted in f64 on the CPU, so static meshes stay
//! camera-relative without being uploaded again.

mod upload;

pub(crate) use upload::buffer;

/// The sides a scene texture may have, in pixels: one bucket each.
pub const TEXTURE_SIDES: [u32; 3] = [64, 128, 256];

/// The most textures of one side and colour tag: Tier 0's array layers.
pub const MAX_TEXTURE_LAYERS: u32 = 256;

/// How a texture's bytes are encoded (AGENTS.md: textures are tagged).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColourTag {
    /// sRGB-encoded colour, decoded to linear when sampled.
    Srgb,
    /// Linear values, as stored: masks, normals, data.
    Linear,
}

/// One square texture of a scene: RGBA8, rows top first, no padding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneTexture<'a> {
    /// 64, 128 or 256 ([`TEXTURE_SIDES`]).
    pub side: u32,
    pub tag: ColourTag,
    /// `side * side * 4` bytes.
    pub rgba8: &'a [u8],
}

/// One entry of a scene's material table. A draw names a material by its
/// index there, through its [`InstanceRecord::material`] or, as the
/// shader chooses, per vertex (eng-render's world chunks do, and leave the
/// record's at 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material {
    /// The material's texture: an index into [`SceneDesc::textures`].
    pub texture: u32,
    /// Whether fragments whose alpha is below one half are discarded
    /// (sprites). The shader reads it as bit 0 of the record's `flags`.
    pub alpha_test: bool,
}

/// Where a scene put a texture: the array of its side and tag, and its
/// layer there. The n-th texture of one side and tag is layer n.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureSlot {
    pub side: u32,
    pub tag: ColourTag,
    pub layer: u32,
}

impl TextureSlot {
    /// The bucket of its side: 0 for 64 px, 1 for 128, 2 for 256.
    pub fn bucket(self) -> u32 {
        bucket(self.side).unwrap_or(0)
    }
}

/// A scene as [`crate::Gpu::scene`] uploads it.
///
/// In WGSL, a material record is
/// `struct Material { bucket: u32, tag: u32, layer: u32, flags: u32 }`:
/// the bucket of its texture's side (0 for 64 px, 1 for 128, 2 for 256), its
/// tag (0 sRGB, 1 linear), its layer, and bit 0 set for an alpha test. The
/// texture is then array `1 + bucket * 2 + tag` of the scene group
/// (`layout.rs`). Without binding arrays the shader picks the array with a
/// branch, so it samples with `textureSampleLevel(..., 0.0)`, which needs no
/// uniform control flow.
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneDesc<'a> {
    /// Names the scene's buffers and textures in GPU debuggers.
    pub label: &'a str,
    /// Every vertex of every mesh, laid out as the scene's pipelines'
    /// [`crate::MeshDesc::vertex`] says, positions relative to their chunk's
    /// origin. A whole number of 4-byte words.
    pub vertices: &'a [u8],
    /// Every mesh's indices into `vertices`.
    pub indices: &'a [u32],
    pub materials: &'a [Material],
    pub textures: &'a [SceneTexture<'a>],
}

/// A scene on the GPU: its megabuffers, material table and texture arrays,
/// and the bind group a mesh pass draws them with. Cheap to clone.
#[derive(Clone, Debug)]
pub struct Scene {
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    pub(crate) index_count: u32,
    pub(crate) group: wgpu::BindGroup,
    slots: Vec<TextureSlot>,
    bytes: u64,
}

impl Scene {
    /// The bytes written into GPU buffers and textures when it loaded: its
    /// vertices, indices, material records and texture texels. A frame's
    /// [`crate::FrameCounters::upload_bytes`] never counts them.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Where texture `index` of [`SceneDesc::textures`] went.
    pub fn slot(&self, index: usize) -> Option<TextureSlot> {
        self.slots.get(index).copied()
    }

    /// The number of indices in its index buffer.
    pub fn index_count(&self) -> u32 {
        self.index_count
    }
}

/// One frame's record of a drawn chunk or sprite, in the frame group's
/// storage array ([`crate::FrameInput::instances`]); a mesh draw's
/// instances index it. In WGSL,
/// `struct Instance { offset: vec3<f32>, material: u32 }`, 16 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InstanceRecord {
    /// The chunk's or sprite's origin minus the eye, in metres.
    pub offset: [f32; 3],
    /// An index into the scene's material table.
    pub material: u32,
}

impl InstanceRecord {
    /// The bytes of one record.
    pub const BYTES: u64 = 16;

    /// A record for something at `origin`, seen from `eye`, both absolute
    /// positions (`WorldPos`, f64): the difference is taken in f64, then
    /// rounded once to f32 (spec 6.7), so that a chunk far from the world's
    /// origin keeps its offset from the eye to f32's precision.
    pub fn relative(origin: [f64; 3], eye: [f64; 3], material: u32) -> Self {
        let offset = |axis: usize| (origin[axis] - eye[axis]) as f32;
        Self {
            offset: [offset(0), offset(1), offset(2)],
            material,
        }
    }

    /// Its 16 bytes, little-endian, as WGSL lays the struct out.
    pub fn to_bytes(self) -> [u8; 16] {
        let mut bytes = [0; 16];
        for (word, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip([
            self.offset[0].to_bits(),
            self.offset[1].to_bits(),
            self.offset[2].to_bits(),
            self.material,
        ]) {
            *word = value.to_le_bytes();
        }
        bytes
    }
}

/// The bucket of a texture side, if it is one of [`TEXTURE_SIDES`].
fn bucket(side: u32) -> Option<u32> {
    (0..)
        .zip(TEXTURE_SIDES)
        .find(|&(_, s)| s == side)
        .map(|(b, _)| b)
}

/// The array a texture of `bucket` and `tag` lives in, 0 to 5.
fn array(bucket: u32, tag: ColourTag) -> usize {
    bucket as usize * 2 + usize::from(tag == ColourTag::Linear)
}

/// Each texture's slot, after checking its side and bytes, and how many
/// layers each of the six arrays needs.
fn slots(desc: &SceneDesc<'_>) -> Result<(Vec<TextureSlot>, [u32; 6]), String> {
    let mut layers = [0u32; 6];
    let mut slots = Vec::with_capacity(desc.textures.len());
    for (index, texture) in desc.textures.iter().enumerate() {
        let Some(bucket) = bucket(texture.side) else {
            return Err(format!(
                "texture {index} is {0}x{0} px; a scene texture is 64, 128 or 256 px square",
                texture.side
            ));
        };
        let expected = texture.side as usize * texture.side as usize * 4;
        if texture.rgba8.len() != expected {
            return Err(format!(
                "texture {index} has {} bytes; {1}x{1} RGBA8 is {expected}",
                texture.rgba8.len(),
                texture.side
            ));
        }
        let layer = &mut layers[array(bucket, texture.tag)];
        if *layer == MAX_TEXTURE_LAYERS {
            return Err(format!(
                "texture {index} would be the {}th of {} px, {:?}; an array holds at most \
                 {MAX_TEXTURE_LAYERS}",
                MAX_TEXTURE_LAYERS + 1,
                texture.side,
                texture.tag
            ));
        }
        slots.push(TextureSlot {
            side: texture.side,
            tag: texture.tag,
            layer: *layer,
        });
        *layer += 1;
    }
    Ok((slots, layers))
}

/// The material table's bytes: four `u32`s each.
fn material_records(desc: &SceneDesc<'_>, slots: &[TextureSlot]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(desc.materials.len() * 16);
    for (index, material) in desc.materials.iter().enumerate() {
        let Some(slot) = slots.get(material.texture as usize) else {
            return Err(format!(
                "material {index} names texture {}, but the scene has {}",
                material.texture,
                slots.len()
            ));
        };
        let tag = u32::from(slot.tag == ColourTag::Linear);
        let flags = u32::from(material.alpha_test);
        for word in [slot.bucket(), tag, slot.layer, flags] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{ColourTag, InstanceRecord, Material, SceneDesc, SceneTexture, slots};

    #[test]
    fn the_offset_is_taken_in_f64() {
        // 2^30 m from the origin, f32 steps by 128 m; the difference keeps
        // the quarter metre.
        let far = 1_073_741_824.0;
        let record = InstanceRecord::relative([far + 0.25, -far, 3.5], [far, -far - 2.0, 0.0], 7);
        assert_eq!(record.offset, [0.25, 2.0, 3.5]);
        assert_eq!(record.material, 7);
        // Subtracting after rounding each to f32 would lose it.
        assert_eq!((far + 0.25) as f32 - far as f32, 0.0);
        let bytes = record.to_bytes();
        assert_eq!(bytes[..4], 0.25f32.to_le_bytes());
        assert_eq!(bytes[12..], 7u32.to_le_bytes());
    }

    #[test]
    fn textures_take_layers_by_side_and_tag_in_order() {
        let (a64, a128) = (vec![0; 64 * 64 * 4], vec![0; 128 * 128 * 4]);
        let texture = |side, tag, rgba8| SceneTexture { side, tag, rgba8 };
        let textures = [
            texture(64, ColourTag::Srgb, &a64[..]),
            texture(128, ColourTag::Srgb, &a128[..]),
            texture(64, ColourTag::Linear, &a64[..]),
            texture(64, ColourTag::Srgb, &a64[..]),
        ];
        let desc = SceneDesc {
            textures: &textures,
            ..SceneDesc::default()
        };
        let (slots, layers) = slots(&desc).unwrap();
        let layers_of: Vec<(u32, u32)> = slots.iter().map(|s| (s.bucket(), s.layer)).collect();
        assert_eq!(layers_of, [(0, 0), (1, 0), (0, 0), (0, 1)]);
        assert_eq!(layers, [2, 1, 1, 0, 0, 0]);
    }

    #[test]
    fn a_scene_refuses_what_its_arrays_cannot_hold() {
        let wrong = |textures: &[SceneTexture<'_>]| {
            slots(&SceneDesc {
                textures,
                ..SceneDesc::default()
            })
            .unwrap_err()
        };
        let pixels = vec![0; 32 * 32 * 4];
        assert_eq!(
            wrong(&[SceneTexture {
                side: 32,
                tag: ColourTag::Srgb,
                rgba8: &pixels
            }]),
            "texture 0 is 32x32 px; a scene texture is 64, 128 or 256 px square"
        );
        assert_eq!(
            wrong(&[SceneTexture {
                side: 64,
                tag: ColourTag::Srgb,
                rgba8: &pixels
            }]),
            "texture 0 has 4096 bytes; 64x64 RGBA8 is 16384"
        );
        let full = vec![0; 64 * 64 * 4];
        let many = vec![
            SceneTexture {
                side: 64,
                tag: ColourTag::Linear,
                rgba8: &full,
            };
            257
        ];
        assert_eq!(
            wrong(&many),
            "texture 256 would be the 257th of 64 px, Linear; an array holds at most 256"
        );
        let materials = [Material {
            texture: 1,
            alpha_test: false,
        }];
        let desc = SceneDesc {
            materials: &materials,
            textures: &many[..1],
            ..SceneDesc::default()
        };
        let (slots, _) = slots(&desc).unwrap();
        assert_eq!(
            super::material_records(&desc, &slots).unwrap_err(),
            "material 0 names texture 1, but the scene has 1"
        );
    }
}
