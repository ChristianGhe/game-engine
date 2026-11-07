//! A known room for the first-person tests (rung 2, T9), and what a CPU
//! ray cast in f64 says each pixel shows: the oracle for depth, `info` and
//! colour. The room is axis-aligned quads, so the cast is exact up to f64.
// Each test file uses some of these.
#![allow(dead_code)]

use eng_math::{BAngle, Ev100, WorldPos, sin_cos_f64, sqrt_f64};
use eng_render::{
    Chunk, ColourTag, FirstPersonCamera, FirstPersonScene, Info, LIGHT_STEP_LUX, LinearRgb, Look,
    Material, NEAR_PLANE, RegionLook, SceneTexture, Sky, SurfaceClass,
};

/// A quad of the room: an axis-aligned rectangle facing one way along its
/// axis, in the room's own coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Face {
    /// 0, 1 or 2: the plane is `axis = at`.
    pub axis: usize,
    pub at: f64,
    /// The ranges of the next two axes in turn, `(axis + 1) % 3` then
    /// `(axis + 2) % 3`.
    pub spans: [(f64, f64); 2],
    /// Whether it faces toward + along its axis.
    pub plus: bool,
    pub material: u32,
    pub info: Info,
}

impl Face {
    /// Its corners, counter-clockwise seen from its front.
    pub fn corners(&self) -> [[f64; 3]; 4] {
        let [(u0, u1), (v0, v1)] = self.spans;
        let uv = if self.plus {
            [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
        } else {
            [(u0, v0), (u0, v1), (u1, v1), (u1, v0)]
        };
        uv.map(|(u, v)| {
            let mut p = [0.0; 3];
            p[self.axis] = self.at;
            p[(self.axis + 1) % 3] = u;
            p[(self.axis + 2) % 3] = v;
            p
        })
    }
}

/// The textures' colours, sRGB bytes, by material. The floor's texture is
/// two halves, `FLOOR_A` for u below one half, `FLOOR_B` above.
pub const FLOOR_A: [u8; 3] = [200, 200, 200];
pub const FLOOR_B: [u8; 3] = [60, 60, 60];
pub const SOLID: [[u8; 3]; 4] = [
    [180, 40, 40],  // 1: the walls across X
    [40, 40, 180],  // 2: the walls across Y
    [40, 180, 40],  // 3: the step's top
    [200, 200, 40], // 4: the step's sides
];

/// The floor's texture repeats every 2 m: u is the room's x over 2.
pub const FLOOR_REPEAT: f64 = 2.0;

pub const fn info(region: u16, class: SurfaceClass, id: u32) -> Info {
    Info { region, class, id }
}

pub const fn face(
    axis: usize,
    at: f64,
    spans: [(f64, f64); 2],
    plus: bool,
    material: u32,
    info: Info,
) -> Face {
    Face {
        axis,
        at,
        spans,
        plus,
        material,
        info,
    }
}

/// The room: 6 m along X, 4 m along Y, walls 3 m high and no ceiling, so
/// the sky shows above them; a step 3/8 m high from x 3 to 4 and y 1.5 to
/// 2.5 on the floor; and a quad above the walls that faces away from the
/// eye, so only back-face culling keeps it out of the sky. Two chunks,
/// each with its own origin: the step's middle, drawn first, so that the
/// floor drawn after it must lose the depth test; and the room's corner.
/// Every coordinate is a multiple of 1/8, exact in f64 a billion meters
/// out.
pub fn chunks() -> [([f64; 3], Vec<Face>); 2] {
    use SurfaceClass::{Floor, Wall, WallFacingX, WallFacingY};
    let (top, x, y) = (0.375, (3.0, 4.0), (1.5, 2.5));
    let step = vec![
        face(2, top, [x, y], true, 3, info(1, Floor, 10)),
        face(0, x.0, [y, (0.0, top)], false, 4, info(1, WallFacingX, 11)),
        face(0, x.1, [y, (0.0, top)], true, 4, info(1, WallFacingX, 12)),
        face(1, y.0, [(0.0, top), x], false, 4, info(1, WallFacingY, 13)),
        face(1, y.1, [(0.0, top), x], true, 4, info(1, WallFacingY, 14)),
    ];
    let room = vec![
        face(2, 0.0, [(0.0, 6.0), (0.0, 4.0)], true, 0, info(0, Floor, 1)),
        face(
            0,
            6.0,
            [(0.0, 4.0), (0.0, 3.0)],
            false,
            1,
            info(0, WallFacingX, 2),
        ),
        face(
            0,
            0.0,
            [(0.0, 4.0), (0.0, 3.0)],
            true,
            1,
            info(0, WallFacingX, 3),
        ),
        face(
            1,
            4.0,
            [(0.0, 3.0), (0.0, 6.0)],
            false,
            2,
            info(0, WallFacingY, 4),
        ),
        face(
            1,
            0.0,
            [(0.0, 3.0), (0.0, 6.0)],
            true,
            2,
            info(0, WallFacingY, 5),
        ),
        face(0, 4.0, [(1.0, 3.0), (3.5, 4.5)], true, 1, info(0, Wall, 20)),
    ];
    [([3.5, 2.0, 0.0], step), ([0.0; 3], room)]
}

/// The ids of the faces the eye sees from behind: no pixel may show them.
pub const FACING_AWAY: [u32; 4] = [3, 12, 13, 20];

/// Every face, in one list, with its chunk's.
pub fn faces() -> Vec<Face> {
    chunks().into_iter().flat_map(|(_, faces)| faces).collect()
}

pub fn add(origin: [f64; 3], p: [f64; 3]) -> WorldPos {
    WorldPos::new(origin[0] + p[0], origin[1] + p[1], origin[2] + p[2])
}

/// The room's chunks with its corner at `corner`.
pub fn world_chunks(corner: [f64; 3]) -> Vec<Chunk> {
    build(corner, &chunks())
}

/// Chunks of `faces` at their origins, offset by `corner`.
pub fn build(corner: [f64; 3], chunks: &[([f64; 3], Vec<Face>)]) -> Vec<Chunk> {
    chunks
        .iter()
        .map(|(origin, faces)| {
            let mut chunk = Chunk::new(add(corner, *origin));
            for face in faces {
                let corners = face.corners();
                // World-anchored on the floor; any on the rest, whose
                // textures are flat.
                let uv = if face.material == 0 {
                    corners.map(|p| [(p[0] / FLOOR_REPEAT) as f32, (p[1] / FLOOR_REPEAT) as f32])
                } else {
                    [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]
                };
                chunk.quad(
                    corners.map(|p| add(corner, p)),
                    uv,
                    face.material,
                    face.info,
                );
            }
            chunk
        })
        .collect()
}

/// The textures' texels: 64 px squares, sRGB.
pub fn texels() -> Vec<Vec<u8>> {
    let mut floor = Vec::with_capacity(64 * 64 * 4);
    for _row in 0..64 {
        for column in 0..64 {
            let [r, g, b] = if column < 32 { FLOOR_A } else { FLOOR_B };
            floor.extend_from_slice(&[r, g, b, 255]);
        }
    }
    let mut all = vec![floor];
    for [r, g, b] in SOLID {
        all.push([r, g, b, 255].repeat(64 * 64));
    }
    all
}

pub fn textures(texels: &[Vec<u8>]) -> Vec<SceneTexture<'_>> {
    texels
        .iter()
        .map(|rgba8| SceneTexture {
            side: 64,
            tag: ColourTag::Srgb,
            rgba8,
        })
        .collect()
}

/// Material n shows texture n.
pub fn materials() -> Vec<Material> {
    (0..5)
        .map(|texture| Material {
            texture,
            alpha_test: false,
        })
        .collect()
}

/// Where the camera stands in the room: at x 1, y 3, eyes 1 5/8 m up.
pub const EYE: [f64; 3] = [1.0, 3.0, 1.625];

/// The sky, in nits.
pub const SKY: Sky = Sky {
    horizon: LinearRgb {
        r: 10.0,
        g: 16.0,
        b: 28.0,
    },
    zenith: LinearRgb {
        r: 0.8,
        g: 2.4,
        b: 12.0,
    },
};

/// The camera's exposure: 38.4 nits map to 1.
pub const EV100: f64 = 5.0;

/// The room's light, region 0, and the step's, region 1: Doom levels 200
/// and 160, light steps 25 and 20.
pub const REGIONS: [RegionLook; 2] = [
    RegionLook {
        light: 200,
        ..RegionLook::DEFAULT
    },
    RegionLook {
        light: 160,
        ..RegionLook::DEFAULT
    },
];

/// The camera at [`EYE`] with the room's corner at `corner`: yaw -22.5°
/// (a sixteenth of a turn clockwise of +X), pitch 5.625° up, a 90°
/// vertical field of view and the near plane eng-render picked, every look
/// feature off.
pub fn scene(corner: [f64; 3]) -> FirstPersonScene {
    FirstPersonScene {
        camera: FirstPersonCamera {
            eye: add(corner, EYE),
            yaw: BAngle(0u32.wrapping_sub(1 << 28)),
            pitch: BAngle(1 << 26),
            fov_y: BAngle::QUARTER,
            near: NEAR_PLANE,
        },
        sky: SKY,
        ev100: Ev100(EV100 as f32),
        look: Look::OFF,
        regions: REGIONS.to_vec(),
        sprites: Vec::new(),
    }
}

/// The lux of region `region`'s light with every look feature off: its
/// level over 8 is a whole step here.
pub fn region_lux(region: u16) -> f64 {
    let step = usize::from(REGIONS[usize::from(region)].light / 8);
    f64::from(LIGHT_STEP_LUX[step])
}

/// The camera in f64, worked out from AGENTS.md's conventions rather than
/// from eng-render: forward is yaw's heading raised by the pitch, right is
/// level and a quarter turn clockwise of it, up is `right x forward`.
#[derive(Clone, Copy, Debug)]
pub struct Camera64 {
    pub eye: [f64; 3],
    pub forward: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
    /// Half the image's width and height, at one unit of distance ahead.
    pub half_width: f64,
    pub half_height: f64,
    pub near: f64,
    pub width: u32,
    pub height: u32,
}

impl Camera64 {
    /// `camera`, with its eye in the room's coordinates, on an image of
    /// `width` by `height` pixels.
    pub fn new(camera: &FirstPersonCamera, eye: [f64; 3], width: u32, height: u32) -> Self {
        let (sy, cy) = sin_cos_f64(camera.yaw.radians_f64());
        let (sp, cp) =
            sin_cos_f64(f64::from(camera.pitch.0 as i32) * std::f64::consts::TAU / 4_294_967_296.0);
        let forward = [cp * cy, cp * sy, sp];
        let right = [sy, -cy, 0.0];
        let up = cross(right, forward);
        let (s, c) = sin_cos_f64(camera.fov_y.radians_f64() / 2.0);
        let half_height = s / c;
        Self {
            eye,
            forward,
            right,
            up,
            half_width: half_height * f64::from(width) / f64::from(height),
            half_height,
            near: f64::from(camera.near),
            width,
            height,
        }
    }

    /// The ray through the image at (`px`, `py`) pixels from its top left
    /// corner, scaled so that its part along forward is 1.
    pub fn ray(&self, px: f64, py: f64) -> [f64; 3] {
        let x = px / f64::from(self.width) * 2.0 - 1.0;
        let y = 1.0 - py / f64::from(self.height) * 2.0;
        let (a, b) = (x * self.half_width, y * self.half_height);
        [0, 1, 2].map(|i| self.forward[i] + self.right[i] * a + self.up[i] * b)
    }
}

pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Where `ray` from `eye` meets `face`'s plane: the distance along the ray
/// (forward distance, for the rays above), whatever the face's bounds and
/// facing; `None` if it runs parallel.
pub fn plane_t(face: &Face, eye: [f64; 3], ray: [f64; 3]) -> Option<f64> {
    let d = ray[face.axis];
    (d != 0.0).then(|| (face.at - eye[face.axis]) / d)
}

/// The nearest face that `ray` from `eye` meets from the front, at least
/// `near` ahead, and where: its index in `faces` and the distance.
pub fn cast(faces: &[Face], eye: [f64; 3], ray: [f64; 3], near: f64) -> Option<(usize, f64)> {
    let mut best: Option<(usize, f64)> = None;
    for (index, face) in faces.iter().enumerate() {
        let towards = if face.plus { -1.0 } else { 1.0 };
        if ray[face.axis] * towards <= 0.0 {
            continue; // seen from behind, or edge on
        }
        let Some(t) = plane_t(face, eye, ray) else {
            continue;
        };
        if t < near {
            continue;
        }
        let inside = (1..3).all(|k| {
            let axis = (face.axis + k) % 3;
            let (lo, hi) = face.spans[k - 1];
            let p = eye[axis] + ray[axis] * t;
            (lo..=hi).contains(&p)
        });
        if inside && best.is_none_or(|(_, b)| t < b) {
            best = Some((index, t));
        }
    }
    best
}

/// The sRGB bytes a face shows where `ray` meets it `t` ahead.
pub fn face_colour(face: &Face, eye: [f64; 3], ray: [f64; 3], t: f64) -> [u8; 3] {
    if face.material == 0 {
        if floor_half(eye, ray, t) {
            FLOOR_B
        } else {
            FLOOR_A
        }
    } else {
        SOLID[face.material as usize - 1]
    }
}

/// Whether the floor shows its second half where `ray` meets it `t` ahead.
pub fn floor_half(eye: [f64; 3], ray: [f64; 3], t: f64) -> bool {
    let u = (eye[0] + ray[0] * t) / FLOOR_REPEAT;
    u - eng_math::floor_f64(u) >= 0.5
}

/// The sky's linear colour along `ray`, as `Sky` documents it.
pub fn sky_colour(sky: &Sky, ray: [f64; 3]) -> [f64; 3] {
    let length = sqrt_f64(ray[0] * ray[0] + ray[1] * ray[1] + ray[2] * ray[2]);
    let rise = eng_math::clamp_f64(ray[2] / length, 0.0, 1.0);
    let mix = |h: f32, z: f32| f64::from(h) + (f64::from(z) - f64::from(h)) * rise;
    [
        mix(sky.horizon.r, sky.zenith.r),
        mix(sky.horizon.g, sky.zenith.g),
        mix(sky.horizon.b, sky.zenith.b),
    ]
}

/// What the CPU says a pixel shows, from rays through its centre and a
/// quarter pixel either way across and down.
pub enum Seen {
    Sky,
    /// The face, by index, and the distance ahead at the centre.
    Face(usize, f64),
    /// The rays disagree: an edge, left out.
    Edge,
}

pub const AROUND: [(f64, f64); 4] = [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)];

pub fn seen(faces: &[Face], camera: &Camera64, x: u32, y: u32) -> Seen {
    let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
    let cast = |px: f64, py: f64| cast(faces, camera.eye, camera.ray(px, py), camera.near);
    let centre = cast(cx, cy);
    let same = AROUND
        .iter()
        .all(|(dx, dy)| cast(cx + dx, cy + dy).map(|(i, _)| i) == centre.map(|(i, _)| i));
    match (same, centre) {
        (false, _) => Seen::Edge,
        (true, None) => Seen::Sky,
        (true, Some((index, t))) => Seen::Face(index, t),
    }
}

/// The CPU's depth of `face` through the image at (`px`, `py`).
pub fn depth_at(face: &Face, camera: &Camera64, px: f64, py: f64) -> f64 {
    let t = plane_t(face, camera.eye, camera.ray(px, py)).expect("the face is in view");
    camera.near / t
}

/// The depth tolerance at the pixel whose centre is (`cx`, `cy`): see the
/// top of `first_person.rs`.
pub fn depth_tolerance(face: &Face, camera: &Camera64, cx: f64, cy: f64) -> f64 {
    const SNAP: f64 = 1.0 / 16.0;
    let d = depth_at(face, camera, cx, cy);
    let change = |dx: f64, dy: f64| (depth_at(face, camera, cx + dx, cy + dy) - d).abs();
    let across = eng_math::max_f64(change(SNAP, 0.0), change(-SNAP, 0.0));
    let down = eng_math::max_f64(change(0.0, SNAP), change(0.0, -SNAP));
    1e-5 * d + across + down
}
