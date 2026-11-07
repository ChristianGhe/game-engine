//! The sweep of a hull against one primitive: a separating-axis test over
//! X, Y, Z and the primitive's axes off the X and Y axes, in f64 with IEEE
//! basic operations only. The crate docs give the rules; this is them.

use eng_collision_api::BoxHull;
use eng_math::{Vec3, max_f64, min_f64};

use crate::shape::{Axis, Shape};
use crate::{GRAZE, NOISE};

/// A hull's half extents in f64.
pub(crate) fn half(hull: &BoxHull) -> [f64; 3] {
    let h = hull.half_extents;
    [f64::from(h.x), f64::from(h.y), f64::from(h.z)]
}

/// One axis of one primitive, grown by the hull: the centre is inside the
/// primitive on this axis when `lo < project(c) < hi`.
#[derive(Clone, Copy)]
struct Grown {
    /// `None` for X, Y or Z by index, `Some((nx, ny))` off the axes.
    normal: Option<(f64, f64)>,
    index: usize,
    lo: f64,
    hi: f64,
}

impl Grown {
    fn project(&self, c: [f64; 3]) -> f64 {
        match self.normal {
            None => c[self.index],
            Some((nx, ny)) => nx * c[0] + ny * c[1],
        }
    }
}

/// Axis `k` of `shape`: X, Y, Z, then its own axes in order.
fn grown(shape: &Shape, axes: &[Axis], h: [f64; 3], k: usize) -> Grown {
    let (min, max, r, normal) = match k {
        0 => (shape.min_x, shape.max_x, h[0], None),
        1 => (shape.min_y, shape.max_y, h[1], None),
        2 => (shape.bottom, shape.top, h[2], None),
        _ => {
            let a = axes[shape.axes_start as usize + k - 3];
            let (nx, ny) = (f64::from(a.nx), f64::from(a.ny));
            (
                a.min,
                a.max,
                nx.abs() * h[0] + ny.abs() * h[1],
                Some((nx, ny)),
            )
        }
    };
    Grown {
        normal,
        index: k,
        lo: min - r,
        hi: max + r,
    }
}

fn axis_count(shape: &Shape) -> usize {
    3 + shape.axes_len as usize
}

/// Whether a hull centred at `c` is inside the primitive: strictly inside on
/// every axis, so touching is free.
pub(crate) fn inside(shape: &Shape, axes: &[Axis], hull: &BoxHull, c: [f64; 3]) -> bool {
    let h = half(hull);
    (0..axis_count(shape)).all(|k| {
        let g = grown(shape, axes, h, k);
        let p = g.project(c);
        g.lo < p && p < g.hi
    })
}

/// Where a sweep met a primitive.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Hit {
    /// The fraction of the motion, in `[0, 1)`.
    pub t: f64,
    /// Where the hull stops: `s + d * t`, exactly on the hit's face for a
    /// hit on X, Y or Z, and on the face of each other axis of X, Y and Z it
    /// entered that rounding left it short of.
    pub q: [f64; 3],
    /// The hit's axis on X, Y or Z and its face, kept when backing off;
    /// `None` off the axes.
    pub snap: Option<(usize, f64)>,
    pub normal: Vec3,
}

pub(crate) enum Sweep {
    /// The start is inside the primitive.
    Inside,
    Miss,
    Hit(Hit),
}

/// The stop at fraction `t`: `s + d * t` on each axis, and the face itself on
/// a snapped axis.
pub(crate) fn stop(s: [f64; 3], d: [f64; 3], t: f64, snap: Option<(usize, f64)>) -> [f64; 3] {
    let mut q = [s[0] + d[0] * t, s[1] + d[1] * t, s[2] + d[2] * t];
    if let Some((axis, face)) = snap {
        q[axis] = face;
    }
    q
}

/// How a centre whose projection moves from `ps` to `pe` meets `g`: when it
/// enters, when it leaves, and whether it enters through `lo`; `None` if it
/// is never inside. Entering at once or never leaving is infinite.
fn crossing(g: &Grown, ps: f64, pe: f64) -> Option<(f64, f64, bool)> {
    let v = pe - ps;
    if v > 0.0 {
        if ps >= g.hi || pe <= g.lo {
            return None;
        }
        let enter = if ps > g.lo {
            f64::NEG_INFINITY
        } else {
            (g.lo - ps) / v
        };
        let leave = if pe < g.hi {
            f64::INFINITY
        } else {
            (g.hi - ps) / v
        };
        Some((enter, leave, true))
    } else if v < 0.0 {
        if ps <= g.lo || pe >= g.hi {
            return None;
        }
        let enter = if ps < g.hi {
            f64::NEG_INFINITY
        } else {
            (g.hi - ps) / v
        };
        let leave = if pe > g.lo {
            f64::INFINITY
        } else {
            (g.lo - ps) / v
        };
        Some((enter, leave, false))
    } else if g.lo < ps && ps < g.hi {
        Some((f64::NEG_INFINITY, f64::INFINITY, true))
    } else {
        None
    }
}

/// How a centre moving from `s` to `e` meets a primitive.
enum Cross {
    Inside,
    Miss,
    /// The latest entry (unclamped), the earliest exit, the entry's axis
    /// and whether through `lo`.
    Enter(f64, f64, usize, bool),
}

fn cross(shape: &Shape, axes: &[Axis], h: [f64; 3], s: [f64; 3], e: [f64; 3]) -> Cross {
    let (mut t_in, mut t_out, mut entry) = (f64::NEG_INFINITY, f64::INFINITY, (0, true));
    let mut start_inside = true;
    for k in 0..axis_count(shape) {
        let g = grown(shape, axes, h, k);
        let (ps, pe) = (g.project(s), g.project(e));
        start_inside &= g.lo < ps && ps < g.hi;
        // Never inside on one axis: never inside, and not at the start.
        let Some((enter, leave, through_lo)) = crossing(&g, ps, pe) else {
            return Cross::Miss;
        };
        if enter > t_in {
            t_in = enter;
            entry = (k, through_lo);
        }
        if leave < t_out {
            t_out = leave;
        }
    }
    if start_inside {
        Cross::Inside
    } else if t_in >= t_out {
        Cross::Miss
    } else {
        Cross::Enter(t_in, t_out, entry.0, entry.1)
    }
}

/// Sweeps the hull from `s` to `e` (`d = e - s`) against one primitive.
pub(crate) fn sweep(
    shape: &Shape,
    axes: &[Axis],
    hull: &BoxHull,
    s: [f64; 3],
    e: [f64; 3],
    d: [f64; 3],
) -> Sweep {
    let h = half(hull);
    let (t_in, t_out, k, through_lo) = match cross(shape, axes, h, s, e) {
        Cross::Inside => return Sweep::Inside,
        Cross::Miss => return Sweep::Miss,
        Cross::Enter(t, out, k, lo) => (t, out, k, lo),
    };
    // A hit always has a fraction in [0, 1), even when the entry rounds to 1
    // or to -0.
    let t = if t_in <= 0.0 {
        0.0
    } else if t_in < 1.0 {
        t_in
    } else {
        1.0f64.next_down()
    };
    // A hull inside the primitive for less than GRAZE of travel (on the
    // motion's longest axis) only grazes an edge or a corner: it does not
    // block, unless `e` is inside.
    let grazed = || !inside(shape, axes, hull, e);
    let longest = max_f64(max_f64(d[0].abs(), d[1].abs()), d[2].abs());
    if (min_f64(t_out, 1.0) - t) * longest < GRAZE && grazed() {
        return Sweep::Miss;
    }
    // Along a face off the axes, within rounding of it, the motion only
    // slides: its entry there is rounding, not a hit, unless `e` is inside.
    if k >= 3 && sliding(shape, axes, h, k, s, e) && grazed() {
        return Sweep::Miss;
    }
    let (mut snap, mut normal) = attribute(shape, axes, h, k, through_lo);
    let mut q = stop(s, d, t, snap);
    // At a corner another face of X, Y or Z is entered at the same time, and
    // rounding may leave the stop short of it: put the stop on it.
    for j in (0..3).filter(|&j| j != k) {
        let g = grown(shape, axes, h, j);
        if let Some((enter, _, lo)) = crossing(&g, s[j], e[j])
            && enter > f64::NEG_INFINITY
        {
            if lo && q[j] < g.lo {
                q[j] = g.lo;
            } else if !lo && q[j] > g.hi {
                q[j] = g.hi;
            }
        }
    }
    // The rest of the motion, from the stop, must meet the primitive again;
    // if it does not, this too is a graze.
    match cross(shape, axes, h, q, e) {
        Cross::Inside => {}
        Cross::Miss => {
            if grazed() {
                return Sweep::Miss;
            }
        }
        Cross::Enter(rest, rest_out, j, lo) => {
            if snap.is_some() {
                if rest > 0.0 && j >= 3 {
                    // An edge off the axes is still ahead: at a corner of
                    // the primitive, it is what blocks, within rounding.
                    (snap, normal) = attribute(shape, axes, h, j, lo);
                } else if !inside(shape, axes, hull, ulp_towards(q, e, None))
                    && !inside(shape, axes, hull, ulp_towards(q, e, Some(k)))
                {
                    // The solid ahead may be thinner than an ulp: an edge at
                    // its entry or touching the stop (a vertex contact)
                    // blocks; or else, if the solid is a sliver by depth, this
                    // is a graze.
                    match edge_at_entry(shape, axes, h, s, e, q) {
                        Some((j, lo)) => (snap, normal) = attribute(shape, axes, h, j, lo),
                        None if grazed() && sliver(shape, axes, h, q, e, rest, rest_out) => {
                            return Sweep::Miss;
                        }
                        None => {}
                    }
                }
            }
        }
    }
    Sweep::Hit(Hit { t, q, snap, normal })
}

/// Of the edges off the axes entered between `s` and `e`, or that `q`
/// touches at their entry face within `4 * NOISE` per meter (a vertex
/// contact, counted as entered at 0), the latest entered whose projection at
/// `q` is nearer its entry face than its exit face, and whether it is
/// entered through `lo`.
fn edge_at_entry(
    shape: &Shape,
    axes: &[Axis],
    h: [f64; 3],
    s: [f64; 3],
    e: [f64; 3],
    q: [f64; 3],
) -> Option<(usize, bool)> {
    let mut best: Option<(f64, usize, bool)> = None;
    for j in 3..axis_count(shape) {
        let g = grown(shape, axes, h, j);
        let Some((enter, _, lo)) = crossing(&g, g.project(s), g.project(e)) else {
            continue;
        };
        let p = g.project(q);
        let (entry, exit) = if lo { (g.lo, g.hi) } else { (g.hi, g.lo) };
        let near = (p - entry).abs() <= (p - exit).abs();
        let touching = (p - entry).abs() <= (q[0].abs() + q[1].abs() + 1.0) * NOISE * 4.0;
        let enter = if enter == f64::NEG_INFINITY && touching {
            0.0
        } else {
            enter
        };
        if enter > f64::NEG_INFINITY && near && best.is_none_or(|(t, ..)| enter > t) {
            best = Some((enter, j, lo));
        }
    }
    best.map(|(_, j, lo)| (j, lo))
}

/// The snap and normal of a hit through axis `k`, entered through `lo` or
/// `hi`.
fn attribute(
    shape: &Shape,
    axes: &[Axis],
    h: [f64; 3],
    k: usize,
    through_lo: bool,
) -> (Option<(usize, f64)>, Vec3) {
    let g = grown(shape, axes, h, k);
    let sign = if through_lo { -1.0 } else { 1.0 };
    match g.normal {
        None => {
            let mut n = Vec3::ZERO;
            match k {
                0 => n.x = sign,
                1 => n.y = sign,
                _ => n.z = sign,
            }
            let face = if through_lo { g.lo } else { g.hi };
            (Some((k, face)), n)
        }
        Some(_) => {
            let a = axes[shape.axes_start as usize + k - 3];
            (None, Vec3::new(sign * a.nx, sign * a.ny, 0.0))
        }
    }
}

/// `q` moved one ulp towards `e` on axis `only`, or on every axis where they
/// differ (as `eng_collision_api::conformance` does).
pub(crate) fn ulp_towards(q: [f64; 3], e: [f64; 3], only: Option<usize>) -> [f64; 3] {
    let mut out = q;
    for i in 0..3 {
        if only.is_some_and(|o| o != i) {
            continue;
        }
        if e[i] > q[i] {
            out[i] = q[i].next_up();
        } else if e[i] < q[i] {
            out[i] = q[i].next_down();
        }
    }
    out
}

/// The face of X, Y or Z through which a hull moving from `s` to `e` enters
/// the primitive last, if its last entry is on X, Y or Z.
pub(crate) fn entry_face(
    shape: &Shape,
    axes: &[Axis],
    hull: &BoxHull,
    s: [f64; 3],
    e: [f64; 3],
) -> Option<(usize, f64)> {
    let h = half(hull);
    match cross(shape, axes, h, s, e) {
        Cross::Enter(_, _, j, lo) if j < 3 => {
            let g = grown(shape, axes, h, j);
            Some((j, if lo { g.lo } else { g.hi }))
        }
        _ => None,
    }
}

/// Whether the motion from `a` to `b` moves along axis `k`, off the X and Y
/// axes, by no more than rounding: a few ulps of the projections, which grow
/// with the coordinates.
fn sliding(shape: &Shape, axes: &[Axis], h: [f64; 3], k: usize, a: [f64; 3], b: [f64; 3]) -> bool {
    let g = grown(shape, axes, h, k);
    let noise = |c: [f64; 3]| (c[0].abs() + c[1].abs() + 1.0) * NOISE;
    (g.project(b) - g.project(a)).abs() <= noise(a) + noise(b)
}

/// Whether the hull, midway through the overlap `(t0, t1)` of the motion
/// from `q` to `e`, is inside the primitive by less than 2^-40 m per meter of
/// coordinate on some axis: a sliver, not solid.
fn sliver(
    shape: &Shape,
    axes: &[Axis],
    h: [f64; 3],
    q: [f64; 3],
    e: [f64; 3],
    t0: f64,
    t1: f64,
) -> bool {
    let t = (max_f64(t0, 0.0) + min_f64(t1, 1.0)) * 0.5;
    let c = [
        q[0] + (e[0] - q[0]) * t,
        q[1] + (e[1] - q[1]) * t,
        q[2] + (e[2] - q[2]) * t,
    ];
    let thin = (c[0].abs() + c[1].abs() + c[2].abs() + 1.0) / 1_099_511_627_776.0;
    (0..axis_count(shape)).any(|k| {
        let g = grown(shape, axes, h, k);
        let p = g.project(c);
        min_f64(p - g.lo, g.hi - p) < thin
    })
}
