//! The things' sprite frames, rendered on the CPU from small 3D primitives
//! with a marked front (rung 2 plan, decision 4; P4 option A), so sprites
//! "are rendered from 3D sources" (spec 11) and no image file enters the
//! repo.
//!
//! **The frames** follow eng-render's rule (`first_person/sprites.rs`):
//! frame `k` is the thing seen by a viewer at its facing plus `k` eighths
//! of a turn, counter-clockwise, the image's left on the viewer's left;
//! a [`RotationSheet::Five`] thing, left-right symmetric, draws frames 0
//! to 4. Each is a square [`SIDE`] px texture of sRGB albedo, rows top
//! first, alpha 255 on the thing and 0 around it, with at least a
//! one-texel transparent margin.
//!
//! **The camera** is orthographic, looking down [`ELEVATION`] so the tops
//! show, across the billboard's [`ThingLook::size`] in meters. The image's
//! bottom row is the lowest any frame can reach (the footprint's corner
//! nearest the viewer), so the thing does not jump as its frame changes.
//! Each texel takes four samples: it is opaque when two or more hit, in the
//! mean colour of those that do. Shading is Lambertian from the viewer's
//! upper left, so each frame drawn is lit alike and the region's light does
//! the rest (`light` pass); a five-rotation sheet's mirrored frames (5 to 7)
//! are lit from the upper right, as in Doom.
//!
//! In the thing's own axes its facing is +X, its left +Y and up +Z, from
//! the point it stands on.
//!
//! | Thing | Model | Sheet |
//! |---|---|---|
//! | marker | a post with four faces red (front), green (left), blue (back) and yellow (right), a white top and a white nose on the front | eight |
//! | crate | a 0.8 m wooden cube, framed planks and a cross brace on each side | five |
//! | barrel | a red drum with two hoops, a yellow diamond on its front and a lid | five |

use eng_math::{BAngle, Vec3, atan2, max, sin_cos_f64, sqrt};
use eng_render::RotationSheet;
use game_testbed_sim::room::ThingKind;

use crate::textures::code;

/// The side of every sprite frame, in texels.
pub const SIDE: usize = 64;

/// How far the sprite camera looks down: a twelfth of a quarter turn,
/// 7.5 degrees.
pub const ELEVATION: BAngle = BAngle((1 << 30) / 12);

/// How a kind of thing is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThingLook {
    /// The billboard's width and height, in meters: square, as its
    /// textures are, so texels are square.
    pub size: f32,
    pub sheet: RotationSheet,
    /// The material of its frame 0; frame `k` is this plus `k`.
    pub first_material: u32,
}

/// The kinds of thing, in material order after the room's five surfaces.
pub const KINDS: [ThingKind; 3] = [ThingKind::Marker, ThingKind::Crate, ThingKind::Barrel];

/// How `kind` is drawn: the marker 1 m across, the crate 1.25 m, the barrel
/// 1 m, each at most 1.5 m so the crate stays hidden behind the pillar from
/// spawn 0, and at most twice its distance to the nearest wall.
pub const fn look(kind: ThingKind) -> ThingLook {
    match kind {
        ThingKind::Marker => ThingLook {
            size: 1.0,
            sheet: RotationSheet::Eight,
            first_material: 5,
        },
        ThingKind::Crate => ThingLook {
            size: 1.25,
            sheet: RotationSheet::Five,
            first_material: 13,
        },
        ThingKind::Barrel => ThingLook {
            size: 1.0,
            sheet: RotationSheet::Five,
            first_material: 18,
        },
    }
}

/// One solid of a model, in the thing's axes (m).
#[derive(Clone, Copy, Debug)]
enum Solid {
    /// An axis-aligned box.
    Box { min: [f32; 3], max: [f32; 3] },
    /// A vertical cylinder round the Z axis.
    Drum { radius: f32, bottom: f32, top: f32 },
}

/// What a ray hit: the point and the outward normal, in the thing's axes,
/// and which solid of the model.
#[derive(Clone, Copy, Debug)]
struct Hit {
    t: f32,
    point: Vec3,
    normal: Vec3,
    solid: usize,
}

/// The solids of `kind`'s model.
fn model(kind: ThingKind) -> &'static [Solid] {
    match kind {
        ThingKind::Marker => &[
            Solid::Box {
                min: [-0.2, -0.2, 0.0],
                max: [0.2, 0.2, 0.75],
            },
            // The nose, on the front at head height.
            Solid::Box {
                min: [0.2, -0.08, 0.5],
                max: [0.34, 0.08, 0.64],
            },
        ],
        ThingKind::Crate => &[Solid::Box {
            min: [-0.4, -0.4, 0.0],
            max: [0.4, 0.4, 0.8],
        }],
        ThingKind::Barrel => &[Solid::Drum {
            radius: 0.28,
            bottom: 0.0,
            top: 0.85,
        }],
    }
}

/// The radius of `kind`'s footprint: how far its model reaches from the
/// Z axis.
fn footprint(kind: ThingKind) -> f32 {
    model(kind)
        .iter()
        .map(|solid| match *solid {
            Solid::Box { min, max: hi } => {
                let x = max(-min[0], hi[0]);
                let y = max(-min[1], hi[1]);
                sqrt(x * x + y * y)
            }
            Solid::Drum { radius, .. } => radius,
        })
        .fold(0.0, max)
}

/// The nearest hit of the ray from `origin` along `dir` with `solid`.
fn hit(solid: &Solid, origin: Vec3, dir: Vec3) -> Option<(f32, Vec3)> {
    match *solid {
        Solid::Box { min, max: hi } => {
            let (o, d) = ([origin.x, origin.y, origin.z], [dir.x, dir.y, dir.z]);
            let mut near = (f32::NEG_INFINITY, Vec3::ZERO);
            let mut far = f32::INFINITY;
            for axis in 0..3 {
                if d[axis] == 0.0 {
                    if o[axis] < min[axis] || o[axis] > hi[axis] {
                        return None;
                    }
                    continue;
                }
                let (t0, t1) = (
                    (min[axis] - o[axis]) / d[axis],
                    (hi[axis] - o[axis]) / d[axis],
                );
                let (enter, leave) = if t0 < t1 { (t0, t1) } else { (t1, t0) };
                if enter > near.0 {
                    let mut normal = [0.0; 3];
                    normal[axis] = if d[axis] > 0.0 { -1.0 } else { 1.0 };
                    near = (enter, Vec3::new(normal[0], normal[1], normal[2]));
                }
                if leave < far {
                    far = leave;
                }
            }
            (near.0 <= far && near.0 > 0.0).then_some(near)
        }
        Solid::Drum {
            radius,
            bottom,
            top,
        } => {
            let a = dir.x * dir.x + dir.y * dir.y;
            let b = 2.0 * (origin.x * dir.x + origin.y * dir.y);
            let c = origin.x * origin.x + origin.y * origin.y - radius * radius;
            let mut best: Option<(f32, Vec3)> = None;
            let disc = b * b - 4.0 * a * c;
            if a > 0.0 && disc >= 0.0 {
                let t = (-b - sqrt(disc)) / (2.0 * a);
                let z = origin.z + dir.z * t;
                if t > 0.0 && (bottom..=top).contains(&z) {
                    let (x, y) = (origin.x + dir.x * t, origin.y + dir.y * t);
                    best = Some((t, Vec3::new(x / radius, y / radius, 0.0)));
                }
            }
            if dir.z < 0.0 {
                let t = (top - origin.z) / dir.z;
                let (x, y) = (origin.x + dir.x * t, origin.y + dir.y * t);
                let closer = best.is_none_or(|(b, _)| t < b);
                if t > 0.0 && x * x + y * y <= radius * radius && closer {
                    best = Some((t, Vec3::new(0.0, 0.0, 1.0)));
                }
            }
            best
        }
    }
}

/// The ray's nearest hit with the model.
fn cast(solids: &[Solid], origin: Vec3, dir: Vec3) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    for (index, solid) in solids.iter().enumerate() {
        if let Some((t, normal)) = hit(solid, origin, dir)
            && best.is_none_or(|b| t < b.t)
        {
            let point = Vec3::new(
                origin.x + dir.x * t,
                origin.y + dir.y * t,
                origin.z + dir.z * t,
            );
            best = Some(Hit {
                t,
                point,
                normal,
                solid: index,
            });
        }
    }
    best
}

/// The albedo where `hit` lands on `kind`'s model, as sRGB codes on the
/// 0-255 scale.
fn paint(kind: ThingKind, hit: &Hit) -> [f32; 3] {
    let (p, n) = (hit.point, hit.normal);
    match kind {
        ThingKind::Marker => marker(hit.solid, p, n),
        ThingKind::Crate => crate_wood(p, n),
        ThingKind::Barrel => barrel(p, n),
    }
}

/// The marker's faces: red front (+X), green left (+Y), blue back, yellow
/// right, a white top, each with a dark rim; the nose is white.
fn marker(solid: usize, p: Vec3, n: Vec3) -> [f32; 3] {
    if solid == 1 {
        return [236.0, 236.0, 230.0];
    }
    if n.z > 0.5 {
        return [240.0, 240.0, 236.0];
    }
    let colour = if n.x > 0.5 {
        [214.0, 44.0, 40.0]
    } else if n.y > 0.5 {
        [52.0, 186.0, 64.0]
    } else if n.x < -0.5 {
        [46.0, 76.0, 222.0]
    } else {
        [236.0, 200.0, 44.0]
    };
    // Across the face, from its middle.
    let across = if n.x.abs() > 0.5 { p.y } else { p.x };
    let rim = across.abs() > 0.17 || p.z < 0.03 || p.z > 0.72;
    if rim {
        colour.map(|c| c * 0.55)
    } else {
        colour
    }
}

/// The crate: on each side a dark frame round planks 11 cm wide and a
/// cross brace, all left-right symmetric; the top framed planks.
fn crate_wood(p: Vec3, n: Vec3) -> [f32; 3] {
    const FRAME: [f32; 3] = [112.0, 76.0, 42.0];
    const PLANK: [f32; 3] = [170.0, 124.0, 72.0];
    const GAP: [f32; 3] = [70.0, 46.0, 26.0];
    let (across, up) = if n.z > 0.5 {
        (p.y, p.x + 0.4)
    } else if n.x.abs() > 0.5 {
        (p.y, p.z)
    } else {
        (p.x, p.z)
    };
    let s = across.abs();
    if s > 0.33 || up < 0.07 || up > 0.73 {
        return FRAME;
    }
    let brace = |d: f32| d.abs() < 0.045;
    if n.z <= 0.5 && (brace(s - (up - 0.4)) || brace(s + (up - 0.4))) {
        return FRAME.map(|c| c * 1.12);
    }
    let plank = ((up - 0.07) / 0.11) as u32;
    let within = (up - 0.07) - plank as f32 * 0.11;
    if within > 0.1 {
        return GAP;
    }
    let shade = 1.0 + 0.08 * crate::textures::signed(crate::textures::hash(plank, 0, 61));
    PLANK.map(|c| c * shade)
}

/// The barrel: red, two dark hoops, a yellow diamond with a dark rim on
/// its front, and a grey lid with a lighter rim and a bung.
fn barrel(p: Vec3, n: Vec3) -> [f32; 3] {
    const RED: [f32; 3] = [176.0, 44.0, 38.0];
    const HOOP: [f32; 3] = [74.0, 72.0, 78.0];
    const YELLOW: [f32; 3] = [232.0, 192.0, 44.0];
    if n.z > 0.5 {
        let r = sqrt(p.x * p.x + p.y * p.y);
        let bung = (p.x - 0.12) * (p.x - 0.12) + p.y * p.y < 0.04 * 0.04;
        return if bung {
            [50.0, 50.0, 54.0]
        } else if r > 0.23 {
            [128.0, 128.0, 134.0]
        } else {
            [88.0, 88.0, 94.0]
        };
    }
    let hoop = |z: f32, at: f32| (at..at + 0.05).contains(&z);
    if hoop(p.z, 0.12) || hoop(p.z, 0.68) {
        return HOOP;
    }
    // Arc length from the front, then the diamond's distance.
    let arc = atan2(p.y, p.x).abs() * 0.28;
    let diamond = arc + (p.z - 0.42).abs();
    if diamond < 0.12 {
        YELLOW
    } else if diamond < 0.145 {
        [40.0, 36.0, 30.0]
    } else {
        RED
    }
}

/// The frames of `kind`: as many as its sheet draws, each `SIDE * SIDE`
/// RGBA8 texels.
pub fn frames(kind: ThingKind) -> Vec<Vec<u8>> {
    let count = look(kind).sheet.frames();
    (0..count).map(|k| frame(kind, k)).collect()
}

/// Frame `k` of `kind`: the view from a viewer `k` eighths of a turn
/// counter-clockwise of its facing.
fn frame(kind: ThingKind, k: u8) -> Vec<u8> {
    let size = look(kind).size;
    let solids = model(kind);
    let (sin_e, cos_e) = sin_cos_f64(ELEVATION.radians_f64());
    let (sin_e, cos_e) = (sin_e as f32, cos_e as f32);
    let (sin_v, cos_v) = sin_cos_f64(BAngle((u32::from(k) % 8) << 29).radians_f64());
    let (sin_v, cos_v) = (sin_v as f32, cos_v as f32);
    // Looking at the thing from direction k, down by the elevation; right
    // is level, a quarter turn clockwise of the heading; up completes them.
    let dir = Vec3::new(-cos_v * cos_e, -sin_v * cos_e, -sin_e);
    let right = Vec3::new(-sin_v, cos_v, 0.0);
    let up = Vec3::new(-cos_v * sin_e, -sin_v * sin_e, cos_e);
    // The image's bottom: as low as the footprint's nearest corner reaches.
    let bottom = -footprint(kind) * sin_e;
    let light = {
        let l = Vec3::new(
            -dir.x - right.x * 0.6 + up.x * 0.8,
            -dir.y - right.y * 0.6 + up.y * 0.8,
            -dir.z - right.z * 0.6 + up.z * 0.8,
        );
        let length = l.length();
        Vec3::new(l.x / length, l.y / length, l.z / length)
    };
    let texel = size / SIDE as f32;
    let mut texels = Vec::with_capacity(SIDE * SIDE * 4);
    for row in 0..SIDE {
        for column in 0..SIDE {
            let mut sum = [0.0f32; 3];
            let mut hits = 0u32;
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let u = -size / 2.0 + (column as f32 + dx) * texel;
                let w = bottom + size - (row as f32 + dy) * texel;
                let origin = Vec3::new(
                    right.x * u + up.x * w - dir.x * 4.0,
                    right.y * u + up.y * w - dir.y * 4.0,
                    right.z * u + up.z * w - dir.z * 4.0,
                );
                if let Some(hit) = cast(solids, origin, dir) {
                    let lambert = max(0.0, hit.normal.dot(light));
                    let shade = 0.5 + 0.6 * lambert;
                    let albedo = paint(kind, &hit);
                    for (s, a) in sum.iter_mut().zip(albedo) {
                        *s += a * shade;
                    }
                    hits += 1;
                }
            }
            let margin = row == 0 || column == 0 || row == SIDE - 1 || column == SIDE - 1;
            if hits >= 2 && !margin {
                let [r, g, b] = sum.map(|s| code(s / hits as f32));
                texels.extend_from_slice(&[r, g, b, 255]);
            } else {
                texels.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    texels
}

#[cfg(test)]
mod tests;
