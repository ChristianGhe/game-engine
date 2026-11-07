//! The room's surface textures, made by code (rung 2 plan, P4 option A): no
//! image file enters the repo. Each is a 64 px square of sRGB albedo, rows
//! top first, that tiles: every pattern repeats exactly across its edges,
//! because world-anchored texture coordinates repeat it (spec 6.11).
//!
//! Each covers [`REPEAT`] meters, 3.125 cm a texel, about Doom's 64 texels
//! to the 64 map units of a wall. The arithmetic is integer hashing and
//! IEEE f32 basic operations only, so every target makes the same bytes.
//!
//! | Texture | Pattern |
//! |---|---|
//! | floor | 1 m stone tiles, two by two, with grout |
//! | wall | bricks 50 by 25 cm in staggered rows, with mortar |
//! | ceiling | 1 m panels with a light frame and a dark seam |
//! | step | grey-green tread plate, raised bars in a herringbone |
//! | pillar | pale concrete with vertical flutes every 50 cm |

/// The side of every surface texture, in texels.
pub const SIDE: usize = 64;

/// How many meters one surface texture covers, along both of its axes.
pub const REPEAT: f64 = 2.0;

/// The room's surfaces, in material order: material `i` is texture `i`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Surface {
    Floor,
    Wall,
    Ceiling,
    Step,
    Pillar,
}

impl Surface {
    /// Every surface, in material order.
    pub const ALL: [Self; 5] = [
        Self::Floor,
        Self::Wall,
        Self::Ceiling,
        Self::Step,
        Self::Pillar,
    ];

    /// Its index in the world's material table, and its texture's.
    pub const fn material(self) -> u32 {
        match self {
            Self::Floor => 0,
            Self::Wall => 1,
            Self::Ceiling => 2,
            Self::Step => 3,
            Self::Pillar => 4,
        }
    }

    /// Its texture: `SIDE * SIDE` RGBA8 texels, sRGB, opaque.
    pub fn texture(self) -> Vec<u8> {
        let paint: fn(usize, usize) -> [f32; 3] = match self {
            Self::Floor => floor,
            Self::Wall => wall,
            Self::Ceiling => ceiling,
            Self::Step => step,
            Self::Pillar => pillar,
        };
        let mut texels = Vec::with_capacity(SIDE * SIDE * 4);
        for y in 0..SIDE {
            for x in 0..SIDE {
                let [r, g, b] = paint(x, y).map(code);
                texels.extend_from_slice(&[r, g, b, 255]);
            }
        }
        texels
    }
}

/// An sRGB code from a value on the 0-255 scale, rounded to nearest and
/// clamped.
pub(crate) fn code(value: f32) -> u8 {
    let v = eng_math::clamp(value, 0.0, 255.0);
    // Half up, then truncated: within 0.5 to 255.5, so the cast is exact.
    (v + 0.5) as u8
}

/// A 32-bit hash of `(x, y)` under `seed`: integer mixing only.
pub(crate) fn hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = x.wrapping_mul(0x8da6_b343)
        ^ y.wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f).wrapping_add(0x9e37_79b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

/// The hash as a number in [-1, 1).
pub(crate) fn signed(h: u32) -> f32 {
    (h >> 8) as f32 / 8_388_608.0 - 1.0
}

/// Smooth noise in [-1, 1] that tiles: value noise on a lattice of `cells`
/// by `cells` (a divisor of [`SIDE`]), wrapped at the texture's edge, eased
/// between lattice points.
pub(crate) fn noise(x: usize, y: usize, cells: usize, seed: u32) -> f32 {
    let step = SIDE / cells;
    let (cx, cy) = (x / step, y / step);
    let ease = |t: usize| {
        let t = t as f32 / step as f32;
        t * t * (3.0 - 2.0 * t)
    };
    let (sx, sy) = (ease(x % step), ease(y % step));
    let at = |i: usize, j: usize| signed(hash((i % cells) as u32, (j % cells) as u32, seed));
    let top = at(cx, cy) + (at(cx + 1, cy) - at(cx, cy)) * sx;
    let bottom = at(cx, cy + 1) + (at(cx + 1, cy + 1) - at(cx, cy + 1)) * sx;
    top + (bottom - top) * sy
}

/// `colour` scaled by `shade` on the 0-255 scale.
fn scaled(colour: [f32; 3], shade: f32) -> [f32; 3] {
    colour.map(|c| c * shade)
}

/// Stone tiles, 32 texels (1 m) square, two by two, each a little lighter
/// or darker than the next, mottled, with 2-texel grout between them.
fn floor(x: usize, y: usize) -> [f32; 3] {
    const STONE: [f32; 3] = [150.0, 141.0, 126.0];
    const GROUT: [f32; 3] = [78.0, 74.0, 70.0];
    let (tx, ty) = (x % 32, y % 32);
    if tx == 0 || ty == 0 || tx == 31 || ty == 31 {
        return scaled(GROUT, 1.0 + 0.05 * signed(hash(x as u32, y as u32, 11)));
    }
    let tile = signed(hash((x / 32) as u32, (y / 32) as u32, 12));
    let shade =
        1.0 + 0.07 * tile + 0.08 * noise(x, y, 8, 13) + 0.03 * signed(hash(x as u32, y as u32, 14));
    // A worn edge: the tile's border texels a little darker.
    let edge = if tx == 1 || ty == 1 || tx == 30 || ty == 30 {
        0.9
    } else {
        1.0
    };
    scaled(STONE, shade * edge)
}

/// Bricks 16 by 8 texels (50 by 25 cm), each row shifted half a brick, with
/// a 1-texel mortar line under each and before each.
fn wall(x: usize, y: usize) -> [f32; 3] {
    const BRICK: [f32; 3] = [146.0, 82.0, 62.0];
    const MORTAR: [f32; 3] = [112.0, 106.0, 98.0];
    let row = y / 8;
    let shifted = (x + (row % 2) * 8) % SIDE;
    let (bx, by) = (shifted % 16, y % 8);
    if bx == 0 || by == 7 {
        return scaled(MORTAR, 1.0 + 0.06 * signed(hash(x as u32, y as u32, 21)));
    }
    let brick = hash((shifted / 16) as u32, row as u32, 22);
    let shade = 1.0
        + 0.12 * signed(brick)
        + 0.06 * noise(x, y, 16, 23)
        + 0.04 * signed(hash(x as u32, y as u32, 24));
    // Light from above: each brick's top row lighter, its bottom darker.
    let relief = match by {
        0 => 1.1,
        6 => 0.88,
        _ => 1.0,
    };
    // A few bricks a shade browner.
    let colour = if brick.is_multiple_of(7) {
        [128.0, 84.0, 66.0]
    } else {
        BRICK
    };
    scaled(colour, shade * relief)
}

/// Panels 32 texels (1 m) square: a 1-texel dark seam, a 2-texel light
/// frame inside it, then the panel, faintly mottled.
fn ceiling(x: usize, y: usize) -> [f32; 3] {
    const PANEL: [f32; 3] = [118.0, 122.0, 132.0];
    const FRAME: [f32; 3] = [140.0, 144.0, 153.0];
    const SEAM: [f32; 3] = [74.0, 76.0, 82.0];
    let (px, py) = (x % 32, y % 32);
    let from_edge = px.min(31 - px).min(py.min(31 - py));
    let (colour, shade) = match from_edge {
        0 => (SEAM, 1.0),
        1 | 2 => (FRAME, 1.0),
        3 => (PANEL, 0.85),
        _ => (PANEL, 1.0 + 0.05 * noise(x, y, 8, 31)),
    };
    scaled(colour, shade + 0.02 * signed(hash(x as u32, y as u32, 32)))
}

/// Grey-green tread plate: raised diagonal bars 3 texels long, one in the
/// middle of each 8-texel cell, alternating in direction from cell to cell,
/// each lit along one side and shadowed along the other.
fn step(x: usize, y: usize) -> [f32; 3] {
    const PLATE: [f32; 3] = [104.0, 116.0, 108.0];
    let (cx, cy) = (x % 8, y % 8);
    let flip = ((x / 8) + (y / 8)) % 2 == 1;
    // Along the bar and across it: the bar is (2, 2) to (4, 4) in a flipped
    // cell, (2, 4) to (4, 2) in the others; its shadow is the next diagonal.
    let (along, across) = if flip {
        (cx as i32 + cy as i32 - 6, cx as i32 - cy as i32)
    } else {
        (cx as i32 - cy as i32, cx as i32 + cy as i32 - 6)
    };
    let shade = if along.abs() <= 3 && across == 0 {
        1.28
    } else if along.abs() <= 3 && across == 1 {
        0.82
    } else {
        1.0 + 0.05 * noise(x, y, 8, 41)
    };
    scaled(PLATE, shade + 0.03 * signed(hash(x as u32, y as u32, 42)))
}

/// Pale concrete with vertical flutes, a 3-texel groove every 16 texels
/// (50 cm), shadowed on its left and lit on its right, and a faint stain.
fn pillar(x: usize, y: usize) -> [f32; 3] {
    const CONCRETE: [f32; 3] = [170.0, 165.0, 154.0];
    let fx = x % 16;
    let groove = match fx {
        0 => 0.74,
        1 => 0.82,
        2 => 1.1,
        _ => 1.0,
    };
    let shade = groove
        + 0.06 * noise(x, y, 4, 51)
        + 0.04 * noise(x, y, 16, 52)
        + 0.03 * signed(hash(x as u32, y as u32, 53));
    scaled(CONCRETE, shade)
}

#[cfg(test)]
mod tests {
    use super::{SIDE, Surface, code, noise};

    #[test]
    fn every_texture_is_an_opaque_square_and_they_differ() {
        let textures: Vec<Vec<u8>> = Surface::ALL.iter().map(|s| s.texture()).collect();
        for (surface, texels) in Surface::ALL.iter().zip(&textures) {
            assert_eq!(texels.len(), SIDE * SIDE * 4, "{surface:?}");
            assert!(texels.chunks(4).all(|t| t[3] == 255), "{surface:?}");
            assert_eq!(
                *texels,
                surface.texture(),
                "{surface:?} is the same each time"
            );
        }
        for (i, a) in textures.iter().enumerate() {
            for b in &textures[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let materials: Vec<u32> = Surface::ALL.iter().map(|s| s.material()).collect();
        assert_eq!(materials, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn noise_tiles_across_the_edges() {
        for cells in [4, 8, 16] {
            for i in 0..SIDE {
                // One texel past the edge is the first texel again.
                assert_eq!(noise(i, 0, cells, 1), noise(i, SIDE, cells, 1));
                assert_eq!(noise(0, i, cells, 1), noise(SIDE, i, cells, 1));
                assert!((-1.0..=1.0).contains(&noise(i, i, cells, 1)));
            }
        }
    }

    #[test]
    fn codes_round_to_nearest_and_clamp() {
        assert_eq!(code(-3.0), 0);
        assert_eq!(code(0.49), 0);
        assert_eq!(code(0.5), 1);
        assert_eq!(code(127.6), 128);
        assert_eq!(code(400.0), 255);
    }
}
