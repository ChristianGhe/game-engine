//! A test-only [`CollisionWorld`] of solid axis-aligned boxes, until rung 2's
//! grid crate arrives. A hull overlaps a box only where their interiors meet,
//! so a hull touching a face is free.
//!
//! - Sweeps are slab tests in f64 with IEEE basic operations only, so every
//!   target gives the same bits.
//! - The earliest hit wins. Ties go to the lower box index, then the lower
//!   axis (X, Y, Z). The hit axis ends exactly on the face.
//! - If rounding would still leave the end inside a box, the trace reports a
//!   hit at its start instead (fraction 0), so it never ends inside solid.

use eng_collision_api::{BoxHull, CollisionWorld, Trace};
use eng_math::{Vec3, WorldPos, max_f64};

/// One solid box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Solid {
    pub min: WorldPos,
    pub max: WorldPos,
}

pub fn solid(min: [f64; 3], max: [f64; 3]) -> Solid {
    Solid {
        min: WorldPos::new(min[0], min[1], min[2]),
        max: WorldPos::new(max[0], max[1], max[2]),
    }
}

/// Solid boxes, in index order. Everything else is open.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Boxes(pub Vec<Solid>);

/// A walled 16 m x 12 m course with its floor at z = 0: a 1.5 m pit, a stair
/// of 0.2 m risers, steps of 0.4 m and 0.41 m, a 1.2 m platform, a step under
/// a ceiling too low to stand on it, and a pillar.
pub fn course() -> Boxes {
    Boxes(vec![
        solid([-1.0, -1.0, -1.0], [9.0, 13.0, 0.0]),
        solid([9.0, -1.0, -1.0], [11.0, 5.0, 0.0]),
        solid([9.0, 7.0, -1.0], [11.0, 13.0, 0.0]),
        solid([11.0, -1.0, -1.0], [17.0, 13.0, 0.0]),
        solid([9.0, 5.0, -2.5], [11.0, 7.0, -1.5]),
        solid([-1.0, -1.0, 0.0], [0.0, 13.0, 3.5]),
        solid([16.0, -1.0, 0.0], [17.0, 13.0, 3.5]),
        solid([0.0, -1.0, 0.0], [16.0, 0.0, 3.5]),
        solid([0.0, 12.0, 0.0], [16.0, 13.0, 3.5]),
        solid([-1.0, -1.0, 3.5], [17.0, 13.0, 4.5]),
        solid([2.0, 1.0, 0.0], [3.0, 3.0, 0.2]),
        solid([3.0, 1.0, 0.0], [4.0, 3.0, 0.4]),
        solid([4.0, 1.0, 0.0], [5.0, 3.0, 0.6]),
        solid([6.0, 1.0, 0.0], [7.0, 3.0, 0.4]),
        solid([8.0, 1.0, 0.0], [9.0, 3.0, 0.41]),
        solid([12.0, 1.0, 0.0], [15.0, 5.0, 1.2]),
        solid([2.0, 8.0, 0.0], [4.0, 10.0, 0.3]),
        solid([2.0, 8.0, 2.0], [4.0, 10.0, 3.5]),
        solid([6.5, 6.5, 0.0], [7.0, 7.0, 3.5]),
    ])
}

/// The open range on each axis where the hull's centre is inside a box.
type Ranges = [(f64, f64); 3];

fn axes(p: WorldPos) -> [f64; 3] {
    [p.x, p.y, p.z]
}

impl Solid {
    fn grown(&self, hull: &BoxHull) -> Ranges {
        let h = hull.half_extents;
        let h = [f64::from(h.x), f64::from(h.y), f64::from(h.z)];
        let (lo, hi) = (axes(self.min), axes(self.max));
        [0, 1, 2].map(|i| (lo[i] - h[i], hi[i] + h[i]))
    }
}

fn within((lo, hi): (f64, f64), x: f64) -> bool {
    lo < x && x < hi
}

fn inside(r: &Ranges, p: [f64; 3]) -> bool {
    r.iter().zip(p).all(|(&range, x)| within(range, x))
}

/// The unit normal on `axis` facing against motion `d` along it.
fn facing(axis: usize, d: f64) -> Vec3 {
    let sign = if d > 0.0 { -1.0 } else { 1.0 };
    let mut n = Vec3::ZERO;
    match axis {
        0 => n.x = sign,
        1 => n.y = sign,
        _ => n.z = sign,
    }
    n
}

/// When a centre moving from `s` to `e` enters `r`, and through which axis.
fn entry(r: &Ranges, s: [f64; 3], e: [f64; 3]) -> Option<(f64, usize)> {
    let (mut enter, mut leave, mut through) = (f64::NEG_INFINITY, f64::INFINITY, None);
    for (axis, &(lo, hi)) in r.iter().enumerate() {
        let d = e[axis] - s[axis];
        if d == 0.0 {
            if !within((lo, hi), s[axis]) {
                return None;
            }
            continue;
        }
        let (at_lo, at_hi) = ((lo - s[axis]) / d, (hi - s[axis]) / d);
        let (t_in, t_out) = if d > 0.0 {
            (at_lo, at_hi)
        } else {
            (at_hi, at_lo)
        };
        if t_in > enter {
            (enter, through) = (t_in, Some(axis));
        }
        if t_out < leave {
            leave = t_out;
        }
    }
    let axis = through?;
    (enter < leave && enter < 1.0 && leave > 0.0).then_some((max_f64(enter, 0.0), axis))
}

impl Boxes {
    /// A hit at `start`: the face of the first box holding `bad` that
    /// `start` is outside of.
    fn blocked(&self, start: WorldPos, bad: WorldPos, hull: &BoxHull) -> Trace {
        let (s, b) = (axes(start), axes(bad));
        let normal = self
            .0
            .iter()
            .map(|solid| solid.grown(hull))
            .filter(|r| inside(r, b))
            .find_map(|r| (0..3).find(|&i| !within(r[i], s[i])))
            .map(|axis| facing(axis, b[axis] - s[axis]))
            .expect("a free start is outside every box that holds the end");
        Trace {
            fraction: 0.0,
            end: start,
            normal,
            start_solid: false,
        }
    }
}

impl CollisionWorld for Boxes {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        if self.overlaps(start, hull) {
            return Trace {
                fraction: 0.0,
                end: start,
                normal: Vec3::ZERO,
                start_solid: true,
            };
        }
        let (s, e) = (axes(start), axes(end));
        let mut first: Option<(f64, usize, f64)> = None;
        for solid in &self.0 {
            let r = solid.grown(hull);
            if let Some((t, axis)) = entry(&r, s, e)
                && first.is_none_or(|(best, ..)| t < best)
            {
                let face = if e[axis] > s[axis] {
                    r[axis].0
                } else {
                    r[axis].1
                };
                first = Some((t, axis, face));
            }
        }
        let Some((t, axis, face)) = first else {
            if self.overlaps(end, hull) {
                return self.blocked(start, end, hull);
            }
            return Trace {
                fraction: 1.0,
                end,
                normal: Vec3::ZERO,
                start_solid: false,
            };
        };
        let mut p = [0.0; 3];
        for (i, v) in p.iter_mut().enumerate() {
            *v = s[i] + (e[i] - s[i]) * t;
        }
        p[axis] = face;
        let stop = WorldPos::new(p[0], p[1], p[2]);
        if self.overlaps(stop, hull) {
            return self.blocked(start, stop, hull);
        }
        Trace {
            fraction: t,
            end: stop,
            normal: facing(axis, e[axis] - s[axis]),
            start_solid: false,
        }
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        let p = axes(at);
        self.0.iter().any(|solid| inside(&solid.grown(hull), p))
    }
}
