//! The world's static meshes for the first-person `opaque` pass (ADR-0020):
//! chunks of triangles whose vertices are relative to the chunk's origin,
//! uploaded once into eng-gpu's scene layout, and drawn each frame with one
//! instance record per drawn chunk, its origin minus the eye in f64.
//!
//! The material goes per vertex, beside `info`, rather than in the record:
//! `info`'s region, class and id differ between the surfaces of one chunk
//! anyway, so they must be per vertex, and with the material there too a
//! chunk is one draw of one record whatever its materials. A chunk's record
//! leaves its `material` at 0, unread; the sprites' records use theirs
//! (`first_person/sprites.rs`).
//!
//! After the chunks' indices the index buffer holds the six of one
//! billboard, `0 1 2 0 2 3`, which every sprite shares: the sprites' shader
//! makes its corners from the vertex index, so they need no vertices.

use std::ops::Range;

use eng_gpu::{Attribute, ColourTag, Gpu, Material, Scene, SceneDesc, SceneTexture, TextureSlot};
use eng_math::WorldPos;

use super::info::{Info, SurfaceClass};
use crate::{PaletteLut, RenderError};

/// The billboard's two triangles over its four corners, counter-clockwise
/// seen from the eye: bottom left, bottom right, top right, top left.
const BILLBOARD: [u32; 6] = [0, 1, 2, 0, 2, 3];

/// The palette table's layers: its 32,768 cells, 64x64 a layer.
const PALETTE_LAYERS: usize = 8;
/// The side of the scene textures the palette table is cut into.
const PALETTE_SIDE: u32 = 64;

/// The layout of [`WorldVertex`] as the opaque pass reads it: position,
/// texture coordinates, material, then `info`'s two words.
pub(crate) const VERTEX: &[Attribute] = &[
    Attribute::F32x3,
    Attribute::F32x2,
    Attribute::U32,
    Attribute::U32,
    Attribute::U32,
];

/// One vertex of the world's meshes. The material and `info` are per
/// vertex, so a chunk with several materials, regions or surfaces is still
/// one draw of one instance record; the three vertices of a triangle must
/// agree on both (the shader takes them from one of the three), which
/// [`GpuWorld::load`] checks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldVertex {
    /// Relative to the chunk's origin, in meters, world axes.
    pub position: [f32; 3],
    /// Texture coordinates: 1 is the texture's side, and the texture
    /// repeats. A caller that anchors them to the world (spec 6.11) wraps
    /// them in f64 first, so they stay small.
    pub uv: [f32; 2],
    /// An index into the world's material table.
    pub material: u32,
    /// What `info` records where the triangle is drawn. Its class is never
    /// [`SurfaceClass::None`], which means nothing was drawn.
    pub info: Info,
}

impl WorldVertex {
    /// The bytes of one vertex in the scene's vertex buffer.
    pub const BYTES: usize = 32;

    /// Its 32 bytes, little-endian, as the opaque pass reads them: the
    /// position, the texture coordinates, the material, then `info`'s two
    /// words ([`Info::pack`]).
    pub fn to_bytes(self) -> [u8; Self::BYTES] {
        let [info0, info1] = self.info.pack();
        let words = [
            self.position[0].to_bits(),
            self.position[1].to_bits(),
            self.position[2].to_bits(),
            self.uv[0].to_bits(),
            self.uv[1].to_bits(),
            self.material,
            info0,
            info1,
        ];
        let mut bytes = [0; Self::BYTES];
        for (out, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(words) {
            *out = word.to_le_bytes();
        }
        bytes
    }
}

/// A piece of the world drawn as one: its origin in the world, and
/// triangles whose vertices are relative to it. Each frame its origin minus
/// the eye, taken in f64, places it; its vertices are never uploaded again.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    /// Finite.
    pub origin: WorldPos,
    pub vertices: Vec<WorldVertex>,
    /// Three per triangle, into this chunk's `vertices`, counter-clockwise
    /// seen from the triangle's front: the back is not drawn.
    pub indices: Vec<u32>,
}

impl Chunk {
    /// An empty chunk at `origin`.
    pub fn new(origin: WorldPos) -> Self {
        Self {
            origin,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Adds a flat quad of four `corners`, absolute positions
    /// counter-clockwise seen from its front, each subtracted from the
    /// origin in f64 and then rounded to f32, with the texture coordinates
    /// `uv` at each corner, one material and one `info`. Two triangles:
    /// corners 0, 1, 2 and 0, 2, 3.
    pub fn quad(&mut self, corners: [WorldPos; 4], uv: [[f32; 2]; 4], material: u32, info: Info) {
        let base = u32::try_from(self.vertices.len()).unwrap_or(u32::MAX);
        for (corner, uv) in corners.into_iter().zip(uv) {
            let offset = corner.relative_to(self.origin);
            self.vertices.push(WorldVertex {
                position: [offset.x, offset.y, offset.z],
                uv,
                material,
                info,
            });
        }
        let index = |i: u32| base.saturating_add(i);
        self.indices
            .extend([index(0), index(1), index(2), index(0), index(2), index(3)]);
    }
}

/// A world as [`GpuWorld::load`] uploads it: its chunks, and the materials
/// and textures they name (eng-gpu's scene layout).
#[derive(Clone, Copy, Debug, Default)]
pub struct WorldMesh<'a> {
    /// Names the world's buffers and textures in GPU debuggers.
    pub label: &'a str,
    pub chunks: &'a [Chunk],
    pub materials: &'a [Material],
    pub textures: &'a [SceneTexture<'a>],
}

/// One chunk with triangles, as a frame draws it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChunkDraw {
    pub(crate) origin: WorldPos,
    pub(crate) indices: Range<u32>,
    pub(crate) base_vertex: i32,
}

/// A world on the GPU: its chunks in eng-gpu's megabuffers, its materials
/// and textures, and where each chunk with triangles lies. Cheap to clone.
#[derive(Clone, Debug)]
pub struct GpuWorld {
    pub(crate) scene: Scene,
    pub(crate) chunks: Vec<ChunkDraw>,
    /// Where the sprites' billboard lies in the index buffer.
    pub(crate) billboard: Range<u32>,
    /// How many materials its table holds, which sprites may name.
    pub(crate) materials: usize,
    /// The palette table's first layer in the 64 px sRGB array, if the
    /// world was loaded with one.
    pub(crate) palette: Option<u32>,
}

impl GpuWorld {
    /// Checks `mesh` and uploads it, once: every chunk's vertices one after
    /// another in one vertex buffer and their indices in one index buffer,
    /// each chunk drawn from its own first vertex, then the sprites' six
    /// billboard indices. A mesh that cannot be drawn is an `[E-SCENE]`
    /// error naming the chunk, before any GPU work:
    /// an origin, position or texture coordinate that is not finite, indices
    /// that are not whole triangles or point past the chunk's vertices, a
    /// material the mesh lacks, a vertex whose class is
    /// [`SurfaceClass::None`], or a triangle whose corners differ in
    /// material or `info`. eng-gpu's own refusals (textures,
    /// [`eng_gpu::Gpu::scene`]) come back as `[E-GPU]`.
    pub fn load(gpu: &Gpu, mesh: &WorldMesh<'_>) -> Result<Self, RenderError> {
        Self::upload(gpu, mesh, None)
    }

    /// Like [`GpuWorld::load`], with the palette look's table, which the
    /// palette pass reads: `palette`'s 32x32x32 cells as eight 64 px sRGB
    /// layers after `mesh`'s own textures (so [`GpuWorld::texture_slot`]
    /// numbers them [`WorldMesh::textures`]' length on). The 64 px sRGB
    /// array then holds 8 layers more, of its 256.
    pub fn load_with_palette(
        gpu: &Gpu,
        mesh: &WorldMesh<'_>,
        palette: &PaletteLut,
    ) -> Result<Self, RenderError> {
        Self::upload(gpu, mesh, Some(palette))
    }

    fn upload(
        gpu: &Gpu,
        mesh: &WorldMesh<'_>,
        palette: Option<&PaletteLut>,
    ) -> Result<Self, RenderError> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut chunks = Vec::new();
        let mut first_vertex = 0usize;
        for (at, chunk) in mesh.chunks.iter().enumerate() {
            check(at, chunk, mesh.materials.len())?;
            let fail = |problem: String| RenderError::Scene {
                shape: None,
                problem: format!("chunk {at}: {problem}"),
            };
            let base_vertex = i32::try_from(first_vertex)
                .map_err(|_| fail("the world has more vertices than a draw can count".into()))?;
            let start = u32::try_from(indices.len())
                .map_err(|_| fail("the world has more indices than a draw can count".into()))?;
            let count = u32::try_from(chunk.indices.len())
                .map_err(|_| fail("more indices than a draw can count".into()))?;
            for vertex in &chunk.vertices {
                vertices.extend_from_slice(&vertex.to_bytes());
            }
            indices.extend_from_slice(&chunk.indices);
            first_vertex += chunk.vertices.len();
            if count > 0 {
                chunks.push(ChunkDraw {
                    origin: chunk.origin,
                    indices: start..start.saturating_add(count),
                    base_vertex,
                });
            }
        }
        let billboard_start = u32::try_from(indices.len()).map_err(|_| RenderError::Scene {
            shape: None,
            problem: "the world has more indices than a draw can count".into(),
        })?;
        indices.extend_from_slice(&BILLBOARD);
        let billboard = billboard_start..billboard_start.saturating_add(BILLBOARD.len() as u32);
        let table = palette.map(PaletteLut::to_rgba8).unwrap_or_default();
        let layer_bytes = (PALETTE_SIDE * PALETTE_SIDE * 4) as usize;
        let mut textures = mesh.textures.to_vec();
        textures.extend(table.chunks_exact(layer_bytes).map(|rgba8| SceneTexture {
            side: PALETTE_SIDE,
            tag: ColourTag::Srgb,
            rgba8,
        }));
        let scene = gpu.scene(&SceneDesc {
            label: mesh.label,
            vertices: &vertices,
            indices: &indices,
            materials: mesh.materials,
            textures: &textures,
        })?;
        // The n-th texture of a side and tag is layer n, so the table's
        // eight follow one another.
        let palette = match palette {
            Some(_) => {
                let first = mesh.textures.len();
                let layers: Vec<Option<u32>> = (first..first + PALETTE_LAYERS)
                    .map(|index| scene.slot(index).map(|slot| slot.layer))
                    .collect();
                let start = layers[0].unwrap_or(0);
                let consecutive = (0u32..)
                    .zip(&layers)
                    .all(|(n, layer)| *layer == Some(start + n));
                if !consecutive {
                    return Err(RenderError::Scene {
                        shape: None,
                        problem: format!(
                            "the palette table's layers are {layers:?}, not eight in a row"
                        ),
                    });
                }
                Some(start)
            }
            None => None,
        };
        Ok(Self {
            scene,
            chunks,
            billboard,
            materials: mesh.materials.len(),
            palette,
        })
    }

    /// Whether it was loaded with a palette table
    /// ([`GpuWorld::load_with_palette`]), which the palette look needs.
    pub fn has_palette(&self) -> bool {
        self.palette.is_some()
    }

    /// The bytes written to the GPU when it loaded ([`Scene::bytes`]): the
    /// billboard's 24 bytes of indices among them.
    pub fn bytes(&self) -> u64 {
        self.scene.bytes()
    }

    /// Where texture `index` of [`WorldMesh::textures`] went: its array and
    /// layer, for a pass that reads a texture as data. The palette table's
    /// eight layers follow the mesh's own textures.
    pub fn texture_slot(&self, index: usize) -> Option<TextureSlot> {
        self.scene.slot(index)
    }

    /// How many chunks a frame draws: those with triangles, one draw and
    /// one instance record each.
    pub fn drawn_chunks(&self) -> usize {
        self.chunks.len()
    }
}

/// Whether chunk `at` can be drawn with `materials` materials.
fn check(at: usize, chunk: &Chunk, materials: usize) -> Result<(), RenderError> {
    let fail = |problem: String| RenderError::Scene {
        shape: None,
        problem: format!("chunk {at}: {problem}"),
    };
    if !chunk.origin.is_finite() {
        return Err(fail("its origin must be finite".into()));
    }
    if !chunk.indices.len().is_multiple_of(3) {
        return Err(fail(format!(
            "{} indices are not whole triangles",
            chunk.indices.len()
        )));
    }
    if let Some(index) = chunk
        .indices
        .iter()
        .find(|&&i| i as usize >= chunk.vertices.len())
    {
        return Err(fail(format!(
            "index {index} is past its {} vertices",
            chunk.vertices.len()
        )));
    }
    for (v, vertex) in chunk.vertices.iter().enumerate() {
        let finite = vertex
            .position
            .iter()
            .chain(&vertex.uv)
            .all(|x| x.is_finite());
        if !finite {
            return Err(fail(format!(
                "vertex {v}'s position and texture coordinates must be finite"
            )));
        }
        if vertex.material as usize >= materials {
            return Err(fail(format!(
                "vertex {v} names material {}, but the world has {materials}",
                vertex.material
            )));
        }
        if vertex.info.class == SurfaceClass::None {
            return Err(fail(format!(
                "vertex {v} has class None, which `info` keeps for nothing drawn"
            )));
        }
    }
    // The shader takes a triangle's material and `info` from one corner, so
    // a corner shared between two surfaces would give one the other's.
    for (n, triangle) in chunk.indices.as_chunks::<3>().0.iter().enumerate() {
        let [a, b, c] = triangle.map(|i| &chunk.vertices[i as usize]);
        let same = |o: &WorldVertex| o.material == a.material && o.info == a.info;
        if !(same(b) && same(c)) {
            return Err(fail(format!(
                "triangle {n}'s corners differ in material or `info`; a surface needs its own \
                 vertices"
            )));
        }
    }
    Ok(())
}
