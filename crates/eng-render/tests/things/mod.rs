//! Things for the sprite tests (rung 2, T11): a test texture for each frame
//! of a sheet, a hall with a pillar to stand them in, things in it, a
//! frame's output, depth and `info`, and what a CPU ray cast in f64 says
//! each pixel shows, billboards included. The billboard is written from
//! plan decision 4 and AGENTS.md's axes, not from eng-render's shader:
//! upright on its feet, across the camera's level right axis, its
//! texture's left edge on the image's left unless mirrored.
// Each test file uses some of these.
#![allow(dead_code)]

use eng_gpu::{Gpu, Image, Targets};
use eng_math::{BAngle, WorldPos, floor_f64};
use eng_render::{
    ColourTag, FirstPersonCamera, FirstPersonRenderer, FirstPersonScene, FrameCounters, GpuWorld,
    Info, Material, NEAR_PLANE, RotationSheet, SceneTexture, Sprite, SurfaceClass, Texels,
    WorldMesh, sprite_rotation_toward,
};

use crate::room::{self, Camera64, Face};

/// The side of the frames' textures.
pub const SIDE: usize = 64;

/// The colour of each frame's left half, sRGB: far apart, so that a frame
/// is known by its colour.
pub const FRAME_COLOURS: [[u8; 3]; 8] = [
    [220, 40, 40],
    [40, 200, 40],
    [40, 60, 220],
    [220, 200, 40],
    [200, 40, 200],
    [40, 200, 200],
    [240, 140, 40],
    [110, 60, 40],
];

/// Every frame's right half: the marker of which way a texture runs.
pub const MARKER: [u8; 3] = [235, 235, 235];

/// What the transparent texels hold, which must never show.
pub const CUT: [u8; 3] = [255, 0, 255];

/// The first of the frames' materials, after the room's five.
pub const FIRST_FRAME: u32 = 5;

/// Whether texel (`column`, `row`), rows from the top, is opaque: all but
/// a border 8 px wide at the sides and the top, and a 16 px hole in the
/// middle that the alpha test must open.
pub fn opaque(column: usize, row: usize) -> bool {
    let border = !(8..56).contains(&column) || row < 8;
    let hole = (24..40).contains(&column) && (24..40).contains(&row);
    !(border || hole)
}

/// The colour of an opaque texel of `frame`.
pub fn texel_colour(frame: usize, column: usize) -> [u8; 3] {
    if column < SIDE / 2 {
        FRAME_COLOURS[frame]
    } else {
        MARKER
    }
}

/// The room's five textures, then the eight frames', as RGBA8.
pub fn texels() -> Vec<Vec<u8>> {
    let mut all = room::texels();
    for frame in 0..8 {
        let mut texels = Vec::with_capacity(SIDE * SIDE * 4);
        for row in 0..SIDE {
            for column in 0..SIDE {
                let (rgb, alpha) = if opaque(column, row) {
                    (texel_colour(frame, column), 255)
                } else {
                    (CUT, 0)
                };
                texels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
            }
        }
        all.push(texels);
    }
    all
}

pub fn textures(texels: &[Vec<u8>]) -> Vec<SceneTexture<'_>> {
    texels
        .iter()
        .map(|rgba8| SceneTexture {
            side: SIDE as u32,
            tag: ColourTag::Srgb,
            rgba8,
        })
        .collect()
}

/// The room's materials, then one per frame, alpha-tested.
pub fn materials() -> Vec<Material> {
    let mut all = room::materials();
    all.extend((0..8).map(|frame| Material {
        texture: FIRST_FRAME + frame,
        alpha_test: true,
    }));
    all
}

/// The frames' materials in order, as a sheet names them.
pub const FRAMES: [u32; 8] = [5, 6, 7, 8, 9, 10, 11, 12];

/// A hall 16 m by 16 m, floor and a far wall at x 12, with a pillar 1 m
/// square and 4 m high from x 4 to 5 and y -0.5 to 0.5, all region 0: two
/// chunks, the pillar's at its middle. Multiples of 1/8.
pub fn hall() -> Vec<([f64; 3], Vec<Face>)> {
    use SurfaceClass::{Floor, WallFacingX, WallFacingY};
    let walls = vec![
        room::face(
            2,
            0.0,
            [(-2.0, 14.0), (-8.0, 8.0)],
            true,
            3,
            room::info(0, Floor, 1),
        ),
        room::face(
            0,
            12.0,
            [(-8.0, 8.0), (0.0, 4.0)],
            false,
            1,
            room::info(0, WallFacingX, 2),
        ),
    ];
    let (x, y, z) = ((4.0, 5.0), (-0.5, 0.5), (0.0, 4.0));
    let pillar = vec![
        room::face(0, x.0, [y, z], false, 4, room::info(0, WallFacingX, 3)),
        room::face(1, y.0, [z, x], false, 2, room::info(0, WallFacingY, 4)),
        room::face(1, y.1, [z, x], true, 2, room::info(0, WallFacingY, 5)),
        room::face(0, x.1, [y, z], true, 4, room::info(0, WallFacingX, 6)),
    ];
    vec![([0.0; 3], walls), ([4.5, 0.0, 0.0], pillar)]
}

/// A camera at `eye` (room coordinates, offset by `corner`) with `yaw` and
/// `pitch`, a 90° vertical field of view, every look feature off, the
/// room's sky, exposure and regions, and no sprites yet.
pub fn scene(corner: [f64; 3], eye: [f64; 3], yaw: u32, pitch: u32) -> FirstPersonScene {
    FirstPersonScene {
        camera: FirstPersonCamera {
            eye: room::add(corner, eye),
            yaw: BAngle(yaw),
            pitch: BAngle(pitch),
            fov_y: BAngle::QUARTER,
            near: NEAR_PLANE,
        },
        ..room::scene(corner)
    }
}

/// A sprite as the CPU sees it, in room coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Billboard {
    pub feet: [f64; 3],
    /// Negative when mirrored: the texture then runs right to left.
    pub width: f64,
    pub height: f64,
    pub frame: usize,
    pub info: Info,
}

impl Billboard {
    /// `sprite` standing on `feet` (room coordinates) of an eight-rotation
    /// sheet whose frame `k` is material `FRAMES[k]`, seen from `eye`: its
    /// rotation by Q4's rule, which is its frame.
    pub fn eight(sprite: &Sprite, feet: [f64; 3], eye: [f64; 3]) -> Self {
        let at = |p: [f64; 3]| WorldPos::new(p[0], p[1], p[2]);
        let rotation = sprite_rotation_toward(at(feet), sprite.facing, at(eye));
        Self::shown(sprite, feet, usize::from(rotation), false)
    }

    /// `sprite` on `feet` showing `frame`, mirrored or not.
    pub fn shown(sprite: &Sprite, feet: [f64; 3], frame: usize, mirrored: bool) -> Self {
        let width = f64::from(sprite.width);
        Self {
            feet,
            width: if mirrored { -width } else { width },
            height: f64::from(sprite.height),
            frame,
            info: Info {
                region: sprite.region,
                class: SurfaceClass::Sprite,
                id: sprite.id,
            },
        }
    }

    /// Where `ray` from the eye meets the billboard's plane, upright through
    /// its feet across the camera's right axis: the distance ahead, and how
    /// far across (0 at the texture's left edge, 1 at its right) and up (0
    /// at the feet, 1 at the top), whatever its bounds.
    pub fn plane(&self, camera: &Camera64, ray: [f64; 3]) -> Option<(f64, f64, f64)> {
        let right = camera.right;
        let normal = [-right[1], right[0], 0.0];
        let along = dot(ray, normal);
        if along == 0.0 {
            return None;
        }
        let t = dot(sub(self.feet, camera.eye), normal) / along;
        let p = [0, 1, 2].map(|i| camera.eye[i] + ray[i] * t);
        let d = sub(p, self.feet);
        Some((t, dot(d, right) / self.width + 0.5, d[2] / self.height))
    }

    /// The texel `ray` meets, ahead of the near plane and inside the
    /// billboard: its distance ahead, column and row.
    pub fn texel(&self, camera: &Camera64, ray: [f64; 3]) -> Option<(f64, usize, usize)> {
        let (t, across, up) = self.plane(camera, ray)?;
        let inside = (0.0..=1.0).contains(&across) && (0.0..=1.0).contains(&up);
        if t < camera.near || !inside {
            return None;
        }
        let cell = |x: f64| (floor_f64(x * SIDE as f64) as usize).min(SIDE - 1);
        Some((t, cell(across), cell(1.0 - up)))
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// What a ray shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Sky,
    /// A face, by index, and the distance ahead.
    Face(usize, f64),
    /// A billboard, by index, the distance ahead and its texel's colour.
    Sprite(usize, f64, [u8; 3]),
}

impl Hit {
    /// Whether two hits show the same thing, distance aside.
    fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Sky, Self::Sky) => true,
            (Self::Face(a, _), Self::Face(b, _)) => a == b,
            (Self::Sprite(a, _, ca), Self::Sprite(b, _, cb)) => a == b && ca == cb,
            _ => false,
        }
    }
}

/// The nearest of `faces` (seen from the front) and of the billboards'
/// opaque texels along `ray`.
pub fn cast(faces: &[Face], billboards: &[Billboard], camera: &Camera64, ray: [f64; 3]) -> Hit {
    let mut best = match room::cast(faces, camera.eye, ray, camera.near) {
        Some((index, t)) => Hit::Face(index, t),
        None => Hit::Sky,
    };
    for (index, billboard) in billboards.iter().enumerate() {
        let Some((t, column, row)) = billboard.texel(camera, ray) else {
            continue;
        };
        let nearer = match best {
            Hit::Sky => true,
            Hit::Face(_, b) | Hit::Sprite(_, b, _) => t < b,
        };
        if opaque(column, row) && nearer {
            best = Hit::Sprite(index, t, texel_colour(billboard.frame, column));
        }
    }
    best
}

/// What the CPU says pixel (`x`, `y`) shows, from rays through its centre
/// and a quarter pixel either way; `None` where they disagree: an edge of
/// a face, a billboard or a texel.
pub fn seen(
    faces: &[Face],
    billboards: &[Billboard],
    camera: &Camera64,
    x: u32,
    y: u32,
) -> Option<Hit> {
    let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
    let hit = |px: f64, py: f64| cast(faces, billboards, camera, camera.ray(px, py));
    let centre = hit(cx, cy);
    room::AROUND
        .iter()
        .all(|(dx, dy)| hit(cx + dx, cy + dy).same(centre))
        .then_some(centre)
}

/// The depth tolerance of a billboard at the pixel centred on (`cx`,
/// `cy`), as `room::depth_tolerance` gives it for a face.
pub fn depth_tolerance(billboard: &Billboard, camera: &Camera64, cx: f64, cy: f64) -> f64 {
    const SNAP: f64 = 1.0 / 16.0;
    let depth = |px: f64, py: f64| {
        let (t, _, _) = billboard
            .plane(camera, camera.ray(px, py))
            .expect("the billboard is in view");
        camera.near / t
    };
    let d = depth(cx, cy);
    let change = |dx: f64, dy: f64| (depth(cx + dx, cy + dy) - d).abs();
    let across = eng_math::max_f64(change(SNAP, 0.0), change(-SNAP, 0.0));
    let down = eng_math::max_f64(change(0.0, SNAP), change(0.0, -SNAP));
    1e-5 * d + across + down
}

pub const W: u32 = 320;
pub const H: u32 = 240;

/// The hall's corner, far from the origin: f32 steps by 64 m and more
/// there, so only offsets taken in f64 draw it in place.
pub const FAR: [f64; 3] = [1.0e9, -2.0e9, 250.0];

/// The eye in the hall, a multiple of 1/8 m.
pub const EYE: [f64; 3] = [0.0, 0.0, 1.625];

/// Loads `chunks` at `corner` with the room's textures and the frames'.
pub fn load(gpu: &Gpu, corner: [f64; 3], chunks: &[([f64; 3], Vec<Face>)]) -> GpuWorld {
    let texels = texels();
    let textures = textures(&texels);
    let materials = materials();
    let mesh = WorldMesh {
        label: "hall",
        chunks: &room::build(corner, chunks),
        materials: &materials,
        textures: &textures,
    };
    GpuWorld::load_with_palette(gpu, &mesh, &crate::look::lut()).unwrap_or_else(|e| panic!("{e}"))
}

pub struct Frame {
    pub image: Image,
    pub depth: Texels,
    pub info: Texels,
    pub counters: FrameCounters,
}

pub fn render(
    gpu: &Gpu,
    renderer: &FirstPersonRenderer,
    targets: &Targets,
    world: &GpuWorld,
    scene: &FirstPersonScene,
) -> Frame {
    let counters = renderer
        .render(gpu, targets, world, scene)
        .unwrap_or_else(|e| panic!("{e}"));
    Frame {
        image: targets.read_output(gpu).unwrap_or_else(|e| panic!("{e}")),
        depth: renderer
            .read_depth(gpu, targets)
            .unwrap_or_else(|e| panic!("{e}")),
        info: renderer
            .read_info(gpu, targets)
            .unwrap_or_else(|e| panic!("{e}")),
        counters,
    }
}

/// A thing 1.5 m wide and 2 m high, of an eight-rotation sheet of the test
/// frames, standing on `feet` in room coordinates offset by `corner`.
pub fn sprite(id: u32, region: u16, corner: [f64; 3], feet: [f64; 3], facing: u32) -> Sprite {
    Sprite {
        id,
        region,
        feet: room::add(corner, feet),
        facing: BAngle(facing),
        width: 1.5,
        height: 2.0,
        sheet: RotationSheet::Eight,
        materials: FRAMES,
    }
}

/// The hall's things, in room coordinates: 100 behind the pillar, hidden;
/// 101 in front of the far wall, in region 1; 102 half behind the pillar;
/// 103 behind 101 in part, drawn after it, so only depth keeps it behind.
pub fn hall_sprites() -> Vec<([f64; 3], Sprite)> {
    let at = |id, region, feet: [f64; 3], facing, width| {
        let s = sprite(id, region, [0.0; 3], feet, facing);
        (feet, Sprite { width, ..s })
    };
    vec![
        (
            [7.0, 0.0, 0.0],
            Sprite {
                height: 1.5,
                ..at(100, 0, [7.0, 0.0, 0.0], 0, 1.0).1
            },
        ),
        at(101, 1, [9.0, 2.5, 0.0], 0x9000_0000, 1.5),
        at(102, 0, [7.0, -1.25, 0.0], 0x4000_0000, 1.0),
        at(103, 1, [11.0, 2.25, 0.0], 0x8000_0000, 1.5),
    ]
}

/// The pixels whose `info` is sprite `id`.
pub fn pixels_of(frame: &Frame, id: u32) -> Vec<(u32, u32)> {
    let mut pixels = Vec::new();
    for y in 0..H {
        for x in 0..W {
            let info = Info::unpack(frame.info.uint2(x, y));
            if info.is_some_and(|i| i.class == SurfaceClass::Sprite && i.id == id) {
                pixels.push((x, y));
            }
        }
    }
    pixels
}
