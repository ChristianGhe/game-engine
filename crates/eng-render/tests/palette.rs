//! The palette look's CPU half (rung 2 plan, decision 8; queue item Q4,
//! done-when 3): OKLab, the baked lookup table and the Bayer matrix. No
//! GPU is needed.
//!
//! The hand-computed cells below were worked out from the formulas alone
//! (the sRGB curve, Ottosson's two matrices and the cube root, in f64 on a
//! calculator), not from this crate; the comments give the distances.

use eng_math::Fnv1a64;
use eng_render::{
    BAYER_4X4, LUT_CELLS, LUT_SIDE, Oklab, PaletteError, PaletteLut, Srgb8, bayer, bayer4_offset,
    lut_cell_centre, srgb_to_linear,
};
use proptest::prelude::*;

const BLACK: Srgb8 = Srgb8::new(0, 0, 0);
const WHITE: Srgb8 = Srgb8::new(255, 255, 255);
const BLUE: Srgb8 = Srgb8::new(0, 0, 255);

// --- OKLab -----------------------------------------------------------------

/// The CIE XYZ to linear sRGB matrix, derived here from sRGB's published
/// chromaticities (IEC 61966-2-1, the same as ITU-R BT.709): red (0.64,
/// 0.33), green (0.30, 0.60), blue (0.15, 0.06), white D65 (0.3127,
/// 0.3290). The primaries' XYZ, scaled so they add up to the white, form
/// the sRGB to XYZ matrix; this is its inverse.
fn xyz_to_linear_srgb() -> [[f64; 3]; 3] {
    let xyz = |x: f64, y: f64| [x / y, 1.0, (1.0 - x - y) / y];
    let (r, g, b, w) = (
        xyz(0.64, 0.33),
        xyz(0.30, 0.60),
        xyz(0.15, 0.06),
        xyz(0.3127, 0.3290),
    );
    let primaries = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
    let scale = apply(&inverse(&primaries), w);
    let to_xyz = primaries.map(|row| [row[0] * scale[0], row[1] * scale[1], row[2] * scale[2]]);
    inverse(&to_xyz)
}

fn apply(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    m.map(|row| row[0] * v[0] + row[1] * v[1] + row[2] * v[2])
}

/// A 3x3 inverse by cofactors.
fn inverse(m: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let c = |r: usize, k: usize| {
        let (r1, r2, k1, k2) = ((r + 1) % 3, (r + 2) % 3, (k + 1) % 3, (k + 2) % 3);
        m[r1][k1] * m[r2][k2] - m[r1][k2] * m[r2][k1]
    };
    let det = m[0][0] * c(0, 0) + m[0][1] * c(0, 1) + m[0][2] * c(0, 2);
    // The inverse is the transposed cofactor matrix over the determinant.
    core::array::from_fn(|r| core::array::from_fn(|k| c(k, r) / det))
}

#[test]
fn oklab_matches_ottossons_known_answers() {
    // Björn Ottosson, "A perceptual color space for image processing"
    // (2020), https://bottosson.github.io/posts/oklab/, the "Table of
    // example XYZ and corresponding Oklab coordinates", given to 3
    // decimals. Checked against the page on 2026-11-06. Its inputs are
    // XYZ, so they go through the derived XYZ to sRGB matrix first; that
    // path stays within 4.1e-4 of every entry.
    let table = [
        ([0.950, 1.000, 1.089], [1.000, 0.000, 0.000]),
        ([1.000, 0.000, 0.000], [0.450, 1.236, -0.019]),
        ([0.000, 1.000, 0.000], [0.922, -0.671, 0.263]),
        ([0.000, 0.000, 1.000], [0.153, -1.415, -0.449]),
    ];
    let to_srgb = xyz_to_linear_srgb();
    for (xyz, want) in table {
        let rgb = apply(&to_srgb, xyz).map(|c| c as f32);
        let got = Oklab::from_linear_srgb(rgb);
        let got = [got.l, got.a, got.b];
        for (g, w) in got.iter().zip(want) {
            assert!(
                (g - w).abs() <= 1e-3,
                "XYZ {xyz:?}: got {got:?}, want {want:?}"
            );
        }
    }
}

#[test]
fn oklab_of_black_white_and_grey() {
    assert_eq!(
        Oklab::from_srgb8(BLACK),
        Oklab {
            l: 0.0,
            a: 0.0,
            b: 0.0
        }
    );
    // White is L = 1 with no chroma; any grey is L = cube root of its
    // linear value (each matrix row of the first step sums to 1, the
    // second's L row to 1 and its a and b rows to 0).
    for (grey, want_l) in [(1.0f32, 1.0f32), (0.125, 0.5), (0.008, 0.2)] {
        let lab = Oklab::from_linear_srgb([grey; 3]);
        assert!((lab.l - want_l).abs() < 1e-5, "grey {grey}: {lab:?}");
        assert!(
            lab.a.abs() < 1e-5 && lab.b.abs() < 1e-5,
            "grey {grey}: {lab:?}"
        );
    }
    let white = Oklab::from_srgb8(WHITE);
    assert!((white.l - 1.0).abs() < 1e-5 && white.a.abs() < 1e-5 && white.b.abs() < 1e-5);
}

#[test]
fn the_srgb_curve() {
    assert_eq!(srgb_to_linear(0.0), 0.0);
    assert_eq!(srgb_to_linear(1.0), 1.0);
    // The linear toe: 0.04045 / 12.92.
    assert!((srgb_to_linear(0.040_45) - 0.003_130_8).abs() < 1e-7);
    // ((0.5 + 0.055) / 1.055)^2.4 = 0.214041.
    assert!((srgb_to_linear(0.5) - 0.214_041).abs() < 1e-6);
}

// --- The table -------------------------------------------------------------

#[test]
fn cells_and_their_centres() {
    assert_eq!(LUT_SIDE, 32);
    assert_eq!(LUT_CELLS, 32 * 32 * 32);
    // Cell i holds codes 8i to 8i + 7; its centre is code 8i + 3.5.
    assert_eq!(lut_cell_centre(0), 3.5 / 255.0);
    assert_eq!(lut_cell_centre(12), 99.5 / 255.0);
    assert_eq!(lut_cell_centre(31), 251.5 / 255.0);
}

#[test]
fn a_two_colour_palette_splits_by_lightness() {
    // Black and white have no chroma, so a cell goes to white exactly when
    // its OKLab L is above 0.5: a linear grey of 0.125, code 99.09.
    let lut = PaletteLut::bake(&[BLACK, WHITE]).unwrap();
    // Grey cells: code 3.5, L 0.102; code 251.5, L 0.990.
    assert_eq!(lut.cell(0, 0, 0), 0);
    assert_eq!(lut.cell(31, 31, 31), 1);
    // Code 91.5 is L 0.473 (d² 0.2237 to black, 0.2777 to white); code
    // 99.5 is L 0.5015 (d² 0.2515 and 0.2485).
    assert_eq!(lut.cell(11, 11, 11), 0);
    assert_eq!(lut.cell(12, 12, 12), 1);
    // The primaries' cells: blue is dark in OKLab (L 0.449), red (0.622)
    // and green (0.858) light.
    assert_eq!(lut.cell(0, 0, 31), 0);
    assert_eq!(lut.cell(31, 0, 0), 1);
    assert_eq!(lut.cell(0, 31, 0), 1);
    // Mixed cells: (15, 15, 0) L 0.566; (0, 10, 31) L 0.528.
    assert_eq!(lut.cell(15, 15, 0), 1);
    assert_eq!(lut.cell(0, 10, 31), 1);
    // The layout is red fastest, then green, then blue.
    assert_eq!(lut.indices()[31], 1);
    assert_eq!(lut.indices()[31 * 1024], 0);
}

#[test]
fn a_three_colour_palette() {
    // Black, blue (L 0.452, a -0.032, b -0.312) and white.
    let lut = PaletteLut::bake(&[BLACK, BLUE, WHITE]).unwrap();
    assert_eq!(lut.cell(0, 0, 0), 0);
    assert_eq!(lut.cell(31, 31, 31), 2);
    // d² to black, blue, white:
    // (0, 0, 31): 0.2969, 0.00003, 0.3990.
    assert_eq!(lut.cell(0, 0, 31), 1);
    // Up the blue axis: (0, 0, 11) 0.0693, 0.0824, 0.6277; (0, 0, 12)
    // 0.0776, 0.0738, 0.6117.
    assert_eq!(lut.cell(0, 0, 11), 0);
    assert_eq!(lut.cell(0, 0, 12), 1);
    // The mid grey at L 0.5015 lies nearer blue than either: 0.2515,
    // 0.1006, 0.2485. (By distance between sRGB codes it would be black.)
    assert_eq!(lut.cell(12, 12, 12), 1);
    // A lighter grey, L 0.717: 0.5142, 0.1684, 0.0800.
    assert_eq!(lut.cell(20, 20, 20), 2);
    // Pale blues: (16, 16, 31) 0.4765, 0.0706, 0.1404; (20, 20, 31)
    // 0.5810, 0.1293, 0.0774.
    assert_eq!(lut.cell(16, 16, 31), 1);
    assert_eq!(lut.cell(20, 20, 31), 2);
    // An 8-bit colour looks up the cell of its codes shifted by 3.
    assert_eq!(lut.lookup(Srgb8::new(99, 96, 103)), lut.cell(12, 12, 12));
    assert_eq!(lut.lookup(Srgb8::new(0, 0, 95)), 0);
    assert_eq!(lut.lookup(Srgb8::new(0, 0, 96)), 1);
    assert_eq!(lut.lookup(WHITE), 2);
}

#[test]
fn ties_go_to_the_lowest_index() {
    // A colour listed twice is at the same distance from every cell as its
    // twin, so the first one always wins.
    let lut = PaletteLut::bake(&[WHITE, BLACK, BLACK]).unwrap();
    assert!(lut.indices().iter().all(|&i| i < 2));
    assert_eq!(lut.cell(0, 0, 0), 1);
    let same = PaletteLut::bake(&[BLUE; 5]).unwrap();
    assert!(same.indices().iter().all(|&i| i == 0));
    // Doubling every colour changes nothing.
    let once = PaletteLut::bake(&[BLACK, BLUE, WHITE]).unwrap();
    let twice = PaletteLut::bake(&[BLACK, BLUE, WHITE, BLACK, BLUE, WHITE]).unwrap();
    assert_eq!(once.indices(), twice.indices());
}

/// A 256-colour palette: a 6x6x6 cube and a 40-step grey ramp.
fn full_palette() -> Vec<Srgb8> {
    let level = |i: u8| i * 51;
    let mut colours: Vec<Srgb8> = (0..216u16)
        .map(|i| {
            let i = i as u8;
            Srgb8::new(level(i % 6), level(i / 6 % 6), level(i / 36))
        })
        .collect();
    colours.extend((0..40u8).map(|i| {
        let v = 3 + i * 6;
        Srgb8::new(v, v, v)
    }));
    colours
}

#[test]
fn two_bakes_give_the_same_bytes() {
    let colours = full_palette();
    assert_eq!(colours.len(), 256);
    let (a, b) = (
        PaletteLut::bake(&colours).unwrap(),
        PaletteLut::bake(&colours).unwrap(),
    );
    assert_eq!(a.indices().len(), LUT_CELLS);
    assert_eq!(a.indices(), b.indices());
    assert_eq!(a.to_rgba8(), b.to_rgba8());
    // A change detector: the table's FNV-1a-64, taken on
    // aarch64-apple-darwin. CI runs it in the test job (Linux x86-64 and
    // arm64) and the native job (Linux and Windows on x86-64 and arm64,
    // macOS on arm64); the riscv job tests core crates only, so not there.
    // x86-64 should match, as the bake uses IEEE operations and libm's
    // plain-Rust powf and cbrtf, but that was not measured when this was
    // pinned. A difference on any of them is a determinism bug; a
    // deliberate change to the bake moves it, and its commit says why.
    let hash = Fnv1a64::hash(a.indices());
    assert_eq!(hash, 0xb41e_b64d_bbf0_d3f4, "{hash:#018x}");
}

#[test]
fn rgba8_texels_are_the_chosen_colours() {
    let lut = PaletteLut::bake(&[BLACK, BLUE, WHITE]).unwrap();
    let texels = lut.to_rgba8();
    assert_eq!(texels.len(), 4 * LUT_CELLS);
    let at = |r: usize, g: usize, b: usize| {
        let i = 4 * (r + 32 * (g + 32 * b));
        [texels[i], texels[i + 1], texels[i + 2], texels[i + 3]]
    };
    assert_eq!(at(0, 0, 0), [0, 0, 0, 255]);
    assert_eq!(at(12, 12, 12), [0, 0, 255, 255]);
    assert_eq!(at(31, 31, 31), [255, 255, 255, 255]);
    assert_eq!(lut.colours(), &[BLACK, BLUE, WHITE]);
}

#[test]
fn a_palette_holds_one_to_256_colours() {
    let empty = PaletteLut::bake(&[]).unwrap_err();
    assert_eq!(empty, PaletteError::Empty);
    assert!(empty.to_string().starts_with("[E-PALETTE] "), "{empty}");
    let many = PaletteLut::bake(&[BLACK; 257]).unwrap_err();
    assert_eq!(many, PaletteError::TooMany { count: 257 });
    assert!(many.to_string().starts_with("[E-PALETTE] "), "{many}");
    assert!(PaletteLut::bake(&[BLACK; 256]).is_ok());
    assert!(PaletteLut::bake(&[WHITE]).is_ok());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    /// Every cell holds the first colour at the least distance from its
    /// centre, recomputed cell by cell. Colours come from a small set, so
    /// palettes often repeat one and the tie rule is exercised.
    #[test]
    fn every_cell_holds_the_nearest_colour(
        picks in prop::collection::vec(0usize..6, 1..6),
    ) {
        let set = [
            BLACK,
            WHITE,
            BLUE,
            Srgb8::new(200, 40, 40),
            Srgb8::new(90, 160, 60),
            Srgb8::new(128, 128, 128),
        ];
        let colours: Vec<Srgb8> = picks.iter().map(|&i| set[i]).collect();
        let lut = PaletteLut::bake(&colours).unwrap();
        let labs: Vec<Oklab> = colours.iter().map(|&c| Oklab::from_srgb8(c)).collect();
        let centre = |i: usize| srgb_to_linear(lut_cell_centre(i));
        for (cell, &index) in lut.indices().iter().enumerate() {
            let (r, g, b) = (cell % 32, cell / 32 % 32, cell / 1024);
            let target = Oklab::from_linear_srgb([centre(r), centre(g), centre(b)]);
            let distances: Vec<f32> = labs.iter().map(|l| l.distance_squared(target)).collect();
            let least = distances.iter().copied().fold(f32::INFINITY, eng_math::min);
            let first = distances.iter().position(|&d| d == least).unwrap();
            prop_assert_eq!(usize::from(index), first, "cell ({}, {}, {})", r, g, b);
        }
    }
}

// --- Dither ----------------------------------------------------------------

#[test]
fn the_bayer_matrices() {
    assert_eq!(bayer::<1>(), [[0]]);
    assert_eq!(bayer::<2>(), [[0, 2], [3, 1]]);
    // The classic 4x4 ordered-dither matrix, indexed [y][x].
    assert_eq!(
        BAYER_4X4,
        [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
    );
    let eight = bayer::<8>();
    assert_eq!(eight[0], [0, 32, 8, 40, 2, 34, 10, 42]);
    assert_eq!(eight[1], [48, 16, 56, 24, 50, 18, 58, 26]);
    // Side 2n is 4 × side n, plus side 2's value for the quadrant, and
    // holds each value once.
    let (two, mut seen) = (bayer::<2>(), [false; 64]);
    for y in 0..8 {
        for x in 0..8 {
            let want = 4 * BAYER_4X4[y % 4][x % 4] + two[y / 4][x / 4];
            assert_eq!(eight[y][x], want, "({x}, {y})");
            seen[usize::from(eight[y][x])] = true;
        }
    }
    assert!(seen.iter().all(|&s| s));
    let sixteen = bayer::<16>();
    assert_eq!(sixteen[15][15], 85, "the last cell of side 16");
}

#[test]
fn bayer_offsets_are_centred() {
    assert_eq!(bayer4_offset(0, 0), -0.468_75);
    assert_eq!(bayer4_offset(1, 0), 0.031_25);
    assert_eq!(bayer4_offset(3, 3), -0.156_25);
    assert_eq!(bayer4_offset(4, 8), bayer4_offset(0, 0));
    let sum: f32 = (0..4)
        .flat_map(|y| (0..4).map(move |x| bayer4_offset(x, y)))
        .sum();
    assert_eq!(sum, 0.0);
}
