//! The palette look's CPU half (spec 6.7; rung 2 plan, decision 8): a 3D
//! lookup table baked from a list of sRGB colours by OKLab distance, and
//! the Bayer matrix for the optional ordered dither. The palette pass
//! itself, and the plain-text palette reader (task T10), live elsewhere.
//!
//! - **What the table maps.** The palette pass runs after the tone map, on
//!   display colours: 8-bit sRGB codes. The table has [`LUT_SIDE`] = 32
//!   cells along each channel, and a code `c` falls in cell `c >> 3`, so
//!   each cell holds exactly 8 codes of each channel and the shader's
//!   index is a shift. The cell's centre is the middle of its codes,
//!   `8i + 3.5` ([`lut_cell_centre`]).
//! - **Why 32³ and not 16³.** At 16 a cell spans 16 codes, and the dark
//!   ramps of a 256-colour palette step by fewer than that, so one cell
//!   would hold two neighbouring shades and every code in it would get
//!   whichever is nearer the centre. At 32 a code sits at most 3.5 codes a
//!   channel from its cell's centre, and the table is 32 KiB of indices
//!   (128 KiB as RGBA8): one small 3D texture, baked in tens of
//!   milliseconds. 64³ would take 8 times the memory and the bake to bring
//!   that 3.5 down to 1.5.
//! - **Choosing a cell's colour.** Each palette colour and each cell centre
//!   is decoded to linear light (the sRGB curve) and converted to OKLab;
//!   the cell takes the colour at the smallest squared OKLab distance, and
//!   among equal distances the lowest palette index.
//! - **Determinism.** f32 arithmetic with eng-math's `pow` and `cbrt` (the
//!   pinned, plain-Rust libm) only, in a fixed order, so the bytes should
//!   not depend on the target; `tests/palette.rs` pins a hash of them.
//! - **Dither.** [`bayer`] builds the ordered-dither matrix of any power of
//!   two up to 16; the look uses [`BAYER_4X4`] through [`bayer4_offset`].

use std::fmt;

use eng_math::{cbrt, pow};

/// Cells along each channel of the lookup table.
pub const LUT_SIDE: usize = 32;
/// Cells in the lookup table.
pub const LUT_CELLS: usize = LUT_SIDE * LUT_SIDE * LUT_SIDE;
/// The shift from an 8-bit code to its cell (8 codes a cell).
const CELL_SHIFT: u32 = 3;
/// The most colours a palette may hold: indices are one byte.
pub const MAX_PALETTE_COLOURS: usize = 256;

/// A display colour as 8-bit sRGB codes, as a palette lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Srgb8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Srgb8 {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// A colour in Björn Ottosson's OKLab space: `l` is lightness, 0 for black
/// and 1 for white; `a` and `b` are its green-red and blue-yellow axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

/// Linear sRGB to cone responses (LMS), as published with OKLab
/// (bottosson.github.io/posts/oklab, public domain or MIT).
const TO_LMS: [[f64; 3]; 3] = [
    [0.412_221_470_8, 0.536_332_536_3, 0.051_445_992_9],
    [0.211_903_498_2, 0.680_699_545_1, 0.107_396_956_6],
    [0.088_302_461_9, 0.281_718_837_6, 0.629_978_700_5],
];

/// The cube roots of LMS to OKLab, from the same source.
const TO_LAB: [[f64; 3]; 3] = [
    [0.210_454_255_3, 0.793_617_785_0, -0.004_072_046_8],
    [1.977_998_495_1, -2.428_592_205_0, 0.450_593_709_9],
    [0.025_904_037_1, 0.782_771_766_2, -0.808_675_766_0],
];

/// `m · v`, in f32, summed left to right.
fn mul3(m: &[[f64; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    m.map(|row| row[0] as f32 * v[0] + row[1] as f32 * v[1] + row[2] as f32 * v[2])
}

/// An sRGB-encoded channel in [0, 1] as linear light, by the sRGB curve
/// (IEC 61966-2-1): linear below 0.04045, a 2.4 power above it.
pub fn srgb_to_linear(encoded: f32) -> f32 {
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        pow((encoded + 0.055) / 1.055, 2.4)
    }
}

impl Oklab {
    /// The OKLab coordinates of a linear sRGB colour (D65 white).
    pub fn from_linear_srgb(rgb: [f32; 3]) -> Self {
        let [l, m, s] = mul3(&TO_LMS, rgb).map(cbrt);
        let [l, a, b] = mul3(&TO_LAB, [l, m, s]);
        Self { l, a, b }
    }

    /// The OKLab coordinates of an 8-bit sRGB colour.
    pub fn from_srgb8(colour: Srgb8) -> Self {
        let decode = |code: u8| srgb_to_linear(f32::from(code) / 255.0);
        Self::from_linear_srgb([decode(colour.r), decode(colour.g), decode(colour.b)])
    }

    /// The squared Euclidean distance to `other`.
    pub fn distance_squared(self, other: Self) -> f32 {
        let (dl, da, db) = (self.l - other.l, self.a - other.a, self.b - other.b);
        dl * dl + da * da + db * db
    }
}

/// The sRGB-encoded value, in [0, 1], at the centre of cell `i` along a
/// channel: the middle of its codes `8i` to `8i + 7`, over 255.
pub fn lut_cell_centre(i: usize) -> f32 {
    assert!(i < LUT_SIDE, "cell {i} is outside a {LUT_SIDE}-cell side");
    (8.0 * i as f32 + 3.5) / 255.0
}

/// Why a palette could not be baked. Each message starts with a log tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteError {
    /// The palette lists no colour. `[E-PALETTE]`
    Empty,
    /// The palette lists more colours than one byte can index.
    /// `[E-PALETTE]`
    TooMany { count: usize },
}

impl fmt::Display for PaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "[E-PALETTE] the palette lists no colour"),
            Self::TooMany { count } => write!(
                f,
                "[E-PALETTE] the palette lists {count} colours; at most \
                 {MAX_PALETTE_COLOURS} fit one-byte indices"
            ),
        }
    }
}

impl std::error::Error for PaletteError {}

/// A baked palette lookup table: for every cell, the index of the palette
/// colour nearest its centre in OKLab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteLut {
    colours: Vec<Srgb8>,
    indices: Vec<u8>,
}

impl PaletteLut {
    /// Bakes the table for `colours` (1 to 256 of them; duplicates are
    /// allowed and the first wins). The same colours always give the same
    /// bytes.
    pub fn bake(colours: &[Srgb8]) -> Result<Self, PaletteError> {
        if colours.is_empty() {
            return Err(PaletteError::Empty);
        }
        if colours.len() > MAX_PALETTE_COLOURS {
            return Err(PaletteError::TooMany {
                count: colours.len(),
            });
        }
        let palette: Vec<Oklab> = colours.iter().map(|&c| Oklab::from_srgb8(c)).collect();
        let centres: [f32; LUT_SIDE] = core::array::from_fn(|i| srgb_to_linear(lut_cell_centre(i)));
        let mut indices = Vec::with_capacity(LUT_CELLS);
        for b in centres {
            for g in centres {
                for r in centres {
                    indices.push(nearest(&palette, Oklab::from_linear_srgb([r, g, b])));
                }
            }
        }
        Ok(Self {
            colours: colours.to_vec(),
            indices,
        })
    }

    /// The palette the table was baked from.
    pub fn colours(&self) -> &[Srgb8] {
        &self.colours
    }

    /// The table: one palette index per cell, [`LUT_CELLS`] bytes, red
    /// fastest, then green, then blue (cell `r + 32 g + 1024 b`), the
    /// layout of a 32x32x32 3D texture with red along x.
    pub fn indices(&self) -> &[u8] {
        &self.indices
    }

    /// The palette index in cell `(r, g, b)`, each below [`LUT_SIDE`].
    pub fn cell(&self, r: usize, g: usize, b: usize) -> u8 {
        assert!(
            r < LUT_SIDE && g < LUT_SIDE && b < LUT_SIDE,
            "cell ({r}, {g}, {b}) is outside the table"
        );
        self.indices[r + LUT_SIDE * (g + LUT_SIDE * b)]
    }

    /// The palette index the table gives an 8-bit sRGB colour.
    pub fn lookup(&self, colour: Srgb8) -> u8 {
        let cell = |code: u8| usize::from(code >> CELL_SHIFT);
        self.cell(cell(colour.r), cell(colour.g), cell(colour.b))
    }

    /// The table as RGBA8 texels in [`PaletteLut::indices`]'s layout: each
    /// cell's palette colour as its sRGB codes, alpha 255.
    pub fn to_rgba8(&self) -> Vec<u8> {
        self.indices
            .iter()
            .flat_map(|&i| {
                let c = self.colours[usize::from(i)];
                [c.r, c.g, c.b, 255]
            })
            .collect()
    }
}

/// The index of the colour nearest `target`; the lowest among equals.
fn nearest(palette: &[Oklab], target: Oklab) -> u8 {
    let mut best = (0, palette[0].distance_squared(target));
    for (index, colour) in palette.iter().enumerate().skip(1) {
        let distance = colour.distance_squared(target);
        // Strictly less, so an equal distance keeps the lower index.
        if distance < best.1 {
            best = (index, distance);
        }
    }
    best.0 as u8
}

/// The Bayer ordered-dither matrix of side `N`, a power of two up to 16,
/// indexed `[y][x]`: each value from 0 to `N² - 1` once, consecutive
/// values far apart, so any threshold lights an evenly scattered set.
///
/// It is the recursive construction written out bit by bit: going from the
/// lowest bit of `x` and `y` to the highest, each level appends two bits
/// to the value, first `x XOR y` and then `y`, so the lowest level of the
/// position decides the highest bits of the value. For side 2 that gives
/// `[[0, 2], [3, 1]]`, and side `2n` is side `n`'s matrix times 4, plus
/// side 2's value for the quadrant.
pub const fn bayer<const N: usize>() -> [[u8; N]; N] {
    assert!(
        N.is_power_of_two() && N <= 16,
        "a Bayer side is 1 to 16, a power of two"
    );
    let levels = N.trailing_zeros();
    let mut matrix = [[0u8; N]; N];
    let mut y = 0;
    while y < N {
        let mut x = 0;
        while x < N {
            let mut value = 0;
            let mut level = 0;
            while level < levels {
                let shift = 2 * (levels - 1 - level);
                let (xb, yb) = ((x >> level) & 1, (y >> level) & 1);
                value |= ((xb ^ yb) << (shift + 1)) | (yb << shift);
                level += 1;
            }
            matrix[y][x] = value as u8;
            x += 1;
        }
        y += 1;
    }
    matrix
}

/// The 4x4 Bayer matrix the palette look dithers with (plan decision 8).
pub const BAYER_4X4: [[u8; 4]; 4] = bayer::<4>();

/// The dither offset at pixel `(x, y)`, in steps: `(m + 0.5) / 16 - 0.5`
/// for [`BAYER_4X4`]'s `m` at `(x mod 4, y mod 4)`, so the 16 offsets lie
/// evenly within (-0.5, 0.5) and average 0. The palette pass adds it,
/// times the look's dither step, to a colour before the lookup.
pub fn bayer4_offset(x: u32, y: u32) -> f32 {
    let m = BAYER_4X4[(y % 4) as usize][(x % 4) as usize];
    (f32::from(m) + 0.5) / 16.0 - 0.5
}
