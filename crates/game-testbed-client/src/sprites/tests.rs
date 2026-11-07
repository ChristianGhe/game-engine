//! The sprite frames: their shape and margins, the marker's eight
//! rotations told apart by the face each shows, the symmetric things'
//! silhouettes mirrored as a five-rotation sheet draws them, and each
//! thing's size against the room.

use eng_collision_grid::Point2;
use eng_render::RotationSheet;
use game_testbed_sim::room::{RoomData, ThingKind};

use super::{KINDS, SIDE, frame, frames, look};

fn alpha(texels: &[u8], column: usize, row: usize) -> u8 {
    texels[(row * SIDE + column) * 4 + 3]
}

/// The mean colour of the opaque texels in columns `columns`.
fn mean(texels: &[u8], columns: std::ops::Range<usize>) -> [f32; 3] {
    let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
    for row in 0..SIDE {
        for column in columns.clone() {
            let at = (row * SIDE + column) * 4;
            if texels[at + 3] == 255 {
                for (s, c) in sum.iter_mut().zip(&texels[at..at + 3]) {
                    *s += f32::from(*c);
                }
                n += 1.0;
            }
        }
    }
    sum.map(|s| s / n)
}

/// Which of red, green, blue and yellow a colour is nearest, by its
/// largest channels.
fn hue(c: [f32; 3]) -> &'static str {
    let [r, g, b] = c;
    if r > 1.5 * b && g > 1.5 * b && (r - g).abs() < 0.35 * r {
        "yellow"
    } else if r > g && r > b {
        "red"
    } else if g > r && g > b {
        "green"
    } else {
        "blue"
    }
}

#[test]
fn each_kind_has_its_sheets_frames_square_cut_out_and_framed_by_a_clear_margin() {
    let mut first = 5;
    for kind in KINDS {
        let look = look(kind);
        assert_eq!(look.first_material, first, "{kind:?}");
        let frames = frames(kind);
        assert_eq!(frames.len(), usize::from(look.sheet.frames()), "{kind:?}");
        first += frames.len() as u32;
        for (k, texels) in frames.iter().enumerate() {
            assert_eq!(texels.len(), SIDE * SIDE * 4);
            let opaque = texels.chunks(4).filter(|t| t[3] == 255).count();
            assert!(texels.chunks(4).all(|t| t[3] == 0 || t[3] == 255));
            assert!(texels.chunks(4).all(|t| t[3] == 255 || t[..3] == [0, 0, 0]));
            assert!(opaque > SIDE * SIDE / 8, "{kind:?} {k}: {opaque}");
            for i in 0..SIDE {
                for (c, r) in [(i, 0), (i, SIDE - 1), (0, i), (SIDE - 1, i)] {
                    assert_eq!(alpha(texels, c, r), 0, "{kind:?} {k} at ({c}, {r})");
                }
            }
            // It stands on the bottom: its lowest opaque row is near it.
            let lowest = (0..SIDE)
                .rev()
                .find(|&r| (0..SIDE).any(|c| alpha(texels, c, r) == 255));
            assert!(
                lowest.is_some_and(|r| r >= SIDE - 4),
                "{kind:?} {k}: {lowest:?}"
            );
        }
        assert_eq!(frames, super::frames(kind), "the same bytes each time");
    }
    assert_eq!(first, 23);
    assert_eq!(look(ThingKind::Marker).sheet, RotationSheet::Eight);
    assert_eq!(look(ThingKind::Crate).sheet, RotationSheet::Five);
    assert_eq!(look(ThingKind::Barrel).sheet, RotationSheet::Five);
}

/// Frame k is seen from k eighths of a turn counter-clockwise of the
/// facing, the image's left on the viewer's left: the front (red) alone at
/// 0, the left side (green) at 2, the back (blue) at 4, the right side
/// (yellow) at 6; between them both, the one counter-clockwise on the
/// image's left.
#[test]
fn the_markers_eight_frames_show_the_faces_their_viewers_see() {
    let frames = frames(ThingKind::Marker);
    let half = SIDE / 2;
    let expected = [
        ("red", "red"),
        ("red", "green"),
        ("green", "green"),
        ("green", "blue"),
        ("blue", "blue"),
        ("blue", "yellow"),
        ("yellow", "yellow"),
        ("yellow", "red"),
    ];
    for (k, (left, right)) in expected.into_iter().enumerate() {
        // The body's middle third of each half, clear of the nose.
        let (l, r) = (half / 2 - 6..half - 4, half + 4..half + half / 2 + 6);
        let seen = (hue(mean(&frames[k], l)), hue(mean(&frames[k], r)));
        assert_eq!(seen, (left, right), "frame {k}");
    }
    for (i, a) in frames.iter().enumerate() {
        for b in &frames[i + 1..] {
            let differ = a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| p != q).count();
            assert!(differ > SIDE * SIDE / 10, "{differ} texels differ");
        }
    }
}

/// A five-rotation sheet draws frame 8 - k mirrored for k above 4, which
/// is right only for a thing symmetric left to right: the crate's and the
/// barrel's silhouettes from 5 to 7 are those of 3 to 1 mirrored. (Their
/// shading is not: the light stays on the viewer's upper left.)
#[test]
fn the_symmetric_things_silhouettes_mirror() {
    for kind in [ThingKind::Crate, ThingKind::Barrel] {
        for k in 5..8u8 {
            let seen = frame(kind, k);
            let mirrored = frame(kind, 8 - k);
            let mut differ = 0;
            for row in 0..SIDE {
                for column in 0..SIDE {
                    let a = alpha(&seen, column, row);
                    let b = alpha(&mirrored, SIDE - 1 - column, row);
                    differ += usize::from(a != b);
                }
            }
            // Rounding at the edges only.
            assert!(differ <= SIDE, "{kind:?} {k}: {differ} texels differ");
        }
    }
}

/// The distance from `p` to the segment from `a` to `b`.
fn to_segment(p: Point2, a: Point2, b: Point2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy);
    let t = eng_math::clamp_f64(t, 0.0, 1.0);
    let (x, y) = (a.x + dx * t - p.x, a.y + dy * t - p.y);
    eng_math::sqrt_f64(x * x + y * y)
}

/// A billboard turns to face the eye and is depth-tested, so a thing must
/// stand at least half its width from anything solid beside it, or it
/// cuts into it as the view turns; and the crate must be at most 1.5 m
/// wide to stay hidden behind the pillar from spawn 0.
#[test]
fn every_thing_clears_the_walls_by_half_its_width() {
    let room = RoomData::room();
    for thing in &room.map.things {
        let look = look(thing.kind);
        let (bottom, top) = (thing.feet.z, thing.feet.z + f64::from(look.size));
        let p = Point2::new(thing.feet.x, thing.feet.y);
        let mut nearest = f64::INFINITY;
        for wall in &room.walls {
            if wall.bottom < top && bottom < wall.top {
                nearest = eng_math::min_f64(nearest, to_segment(p, wall.a, wall.b));
            }
        }
        for slab in &room.slabs {
            if slab.bottom < top && bottom < slab.top {
                let n = slab.points.len();
                for e in 0..n {
                    let (a, b) = (slab.points[e], slab.points[(e + 1) % n]);
                    nearest = eng_math::min_f64(nearest, to_segment(p, a, b));
                }
            }
        }
        let half = f64::from(look.size) / 2.0;
        assert!(
            nearest >= half,
            "{:?}: {nearest} m from a wall, half its width {half}",
            thing.kind
        );
    }
    assert!(look(ThingKind::Crate).size <= 1.5);
}
