//! The brute-force oracle's world: every primitive, in index order, no grid.
//! It follows the rules in `eng_collision_grid`'s crate docs from the walls
//! and slabs alone. Part of the oracle test (`../oracle.rs`).
//!
//! `eng-bench` includes this file too, by `#[path]`, for its brute-force
//! trace benchmarks (`benches/collision/main.rs`) and their tests, so it
//! keeps to the public items of the crates it names and uses no proptest.

use eng_collision_api::{BoxHull, CollisionWorld, Trace};
use eng_collision_grid::{BACKOFF_STEPS, GRAZE, NOISE, NUDGE_STEPS, Point2, Slab, Wall};
use eng_math::{Vec3, WorldPos, max_f64, min_f64, sqrt_f64};

/// One axis of one primitive for one hull: the centre is inside on it when
/// `lo < project(c) < hi`.
#[derive(Debug, Clone, Copy)]
struct Axis {
    /// `None` on X, Y or Z (by `index`); the f32 unit normal off the axes.
    normal: Option<(f32, f32)>,
    index: usize,
    lo: f64,
    hi: f64,
}

impl Axis {
    fn project(&self, c: [f64; 3]) -> f64 {
        match self.normal {
            None => c[self.index],
            Some((nx, ny)) => f64::from(nx) * c[0] + f64::from(ny) * c[1],
        }
    }

    fn holds(&self, c: [f64; 3]) -> bool {
        let p = self.project(c);
        self.lo < p && p < self.hi
    }
}

/// A primitive: its corners on the XY plane, whether they close a loop
/// (slabs) or not (walls), and its heights.
#[derive(Debug, Clone)]
struct Prim {
    points: Vec<Point2>,
    closed: bool,
    bottom: f64,
    top: f64,
}

impl Prim {
    /// X, Y, Z, then the edges off the X and Y axes in order, each grown by
    /// the hull's half extents `h`.
    fn axes(&self, h: [f64; 3]) -> Vec<Axis> {
        let xs = self.points.iter().map(|p| p.x);
        let ys = self.points.iter().map(|p| p.y);
        let lowest = |v: &mut dyn Iterator<Item = f64>| v.fold(f64::INFINITY, min_f64);
        let highest = |v: &mut dyn Iterator<Item = f64>| v.fold(f64::NEG_INFINITY, max_f64);
        let bounds = [
            (lowest(&mut xs.clone()), highest(&mut xs.clone())),
            (lowest(&mut ys.clone()), highest(&mut ys.clone())),
            (self.bottom, self.top),
        ];
        let mut out: Vec<Axis> = (0..3)
            .map(|i| Axis {
                normal: None,
                index: i,
                lo: bounds[i].0 - h[i],
                hi: bounds[i].1 + h[i],
            })
            .collect();
        let n = self.points.len();
        let edges = if self.closed { n } else { n - 1 };
        let mut normals: Vec<(f32, f32)> = Vec::new();
        for i in 0..edges {
            let (p, q) = (self.points[i], self.points[(i + 1) % n]);
            let (ux, uy) = (q.x - p.x, q.y - p.y);
            if ux == 0.0 || uy == 0.0 {
                continue;
            }
            let len = sqrt_f64(ux * ux + uy * uy);
            let (nx, ny) = ((uy / len) as f32, (-ux / len) as f32);
            if normals
                .iter()
                .any(|&(a, b)| (a, b) == (nx, ny) || (a, b) == (-nx, -ny))
            {
                continue;
            }
            normals.push((nx, ny));
            let proj = self
                .points
                .iter()
                .map(|v| f64::from(nx) * v.x + f64::from(ny) * v.y);
            let r = f64::from(nx).abs() * h[0] + f64::from(ny).abs() * h[1];
            out.push(Axis {
                normal: Some((nx, ny)),
                index: out.len(),
                lo: lowest(&mut proj.clone()) - r,
                hi: highest(&mut proj.clone()) + r,
            });
        }
        out
    }
}

/// The oracle: every primitive, in index order.
pub struct Brute(Vec<Prim>);

impl Brute {
    pub fn new(walls: &[Wall], slabs: &[Slab]) -> Self {
        let walls = walls.iter().map(|w| Prim {
            points: vec![w.a, w.b],
            closed: false,
            bottom: w.bottom,
            top: w.top,
        });
        let slabs = slabs.iter().map(|s| Prim {
            points: s.points.clone(),
            closed: true,
            bottom: s.bottom,
            top: s.top,
        });
        Self(walls.chain(slabs).collect())
    }
}

fn half(hull: &BoxHull) -> [f64; 3] {
    let h = hull.half_extents;
    [f64::from(h.x), f64::from(h.y), f64::from(h.z)]
}

fn xyz(p: WorldPos) -> [f64; 3] {
    [p.x, p.y, p.z]
}

fn pos(c: [f64; 3]) -> WorldPos {
    WorldPos::new(c[0], c[1], c[2])
}

fn inside(axes: &[Axis], c: [f64; 3]) -> bool {
    axes.iter().all(|a| a.holds(c))
}

fn stop(s: [f64; 3], d: [f64; 3], t: f64, snap: Option<(usize, f64)>) -> [f64; 3] {
    let mut q = [0.0; 3];
    for i in 0..3 {
        q[i] = s[i] + d[i] * t;
    }
    if let Some((i, face)) = snap {
        q[i] = face;
    }
    q
}

fn ulp_towards(q: [f64; 3], e: [f64; 3], only: Option<usize>) -> [f64; 3] {
    let mut out = q;
    for i in (0..3).filter(|&i| only.is_none_or(|o| o == i)) {
        if e[i] > q[i] {
            out[i] = q[i].next_up();
        } else if e[i] < q[i] {
            out[i] = q[i].next_down();
        }
    }
    out
}

/// The snap and normal of a hit through `a`, entered through `lo` or `hi`.
fn attribute(a: &Axis, through_lo: bool) -> (Option<(usize, f64)>, Vec3) {
    let sign = if through_lo { -1.0 } else { 1.0 };
    match a.normal {
        None => {
            let mut n = [0.0f32; 3];
            n[a.index] = sign;
            let face = if through_lo { a.lo } else { a.hi };
            (Some((a.index, face)), Vec3::new(n[0], n[1], n[2]))
        }
        Some((nx, ny)) => (None, Vec3::new(sign * nx, sign * ny, 0.0)),
    }
}

/// A hit: fraction, stop, snap, normal.
type Hit = (f64, [f64; 3], Option<(usize, f64)>, Vec3);

/// How a centre moving from `ps` to `pe` meets `(lo, hi)`: entry, exit and
/// whether it enters through `lo`; `None` if it never is inside.
fn crossing(a: &Axis, ps: f64, pe: f64) -> Option<(f64, f64, bool)> {
    let v = pe - ps;
    let (lo, hi) = (a.lo, a.hi);
    if v > 0.0 {
        if ps >= hi || pe <= lo {
            return None;
        }
        let enter = if ps > lo {
            f64::NEG_INFINITY
        } else {
            (lo - ps) / v
        };
        let leave = if pe < hi {
            f64::INFINITY
        } else {
            (hi - ps) / v
        };
        Some((enter, leave, true))
    } else if v < 0.0 {
        if ps <= lo || pe >= hi {
            return None;
        }
        let enter = if ps < hi {
            f64::NEG_INFINITY
        } else {
            (hi - ps) / v
        };
        let leave = if pe > lo {
            f64::INFINITY
        } else {
            (lo - ps) / v
        };
        Some((enter, leave, false))
    } else if lo < ps && ps < hi {
        Some((f64::NEG_INFINITY, f64::INFINITY, true))
    } else {
        None
    }
}

/// How a centre moving from `s` to `e` meets a primitive.
enum Meet {
    Inside,
    Miss,
    /// The latest entry (unclamped), the earliest exit, the entry's axis,
    /// and whether through `lo`.
    Enter(f64, f64, usize, bool),
}

fn meet(axes: &[Axis], s: [f64; 3], e: [f64; 3]) -> Meet {
    let mut cross = Vec::with_capacity(axes.len());
    for a in axes {
        match crossing(a, a.project(s), a.project(e)) {
            Some(c) => cross.push(c),
            None => return Meet::Miss,
        }
    }
    if inside(axes, s) {
        return Meet::Inside;
    }
    let (mut t_in, mut entry) = (f64::NEG_INFINITY, (0, true));
    for (k, &(enter, _, lo)) in cross.iter().enumerate() {
        if enter > t_in {
            (t_in, entry) = (enter, (k, lo));
        }
    }
    let t_out = cross.iter().map(|c| c.1).fold(f64::INFINITY, min_f64);
    if t_in >= t_out {
        Meet::Miss
    } else {
        Meet::Enter(t_in, t_out, entry.0, entry.1)
    }
}

/// Of the edges entered between `s` and `e`, or touched at `q` within
/// `4 * NOISE` per meter, the latest entered whose
/// projection at `q` is nearer its entry face than its exit face.
fn edge_at_entry(axes: &[Axis], s: [f64; 3], e: [f64; 3], q: [f64; 3]) -> Option<(usize, bool)> {
    let mut best: Option<(f64, usize, bool)> = None;
    for (j, a) in axes.iter().enumerate().skip(3) {
        let Some((enter, _, lo)) = crossing(a, a.project(s), a.project(e)) else {
            continue;
        };
        let p = a.project(q);
        let (entry, exit) = if lo { (a.lo, a.hi) } else { (a.hi, a.lo) };
        let near = (p - entry).abs() <= (p - exit).abs();
        // Touching its entry face at `q` (a vertex contact) counts as entered.
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

/// One primitive's hit, for a start outside it.
fn sweep(axes: &[Axis], s: [f64; 3], e: [f64; 3], d: [f64; 3]) -> Option<Hit> {
    let Meet::Enter(t_in, t_out, k, lo) = meet(axes, s, e) else {
        return None;
    };
    let t = if t_in <= 0.0 {
        0.0
    } else if t_in < 1.0 {
        t_in
    } else {
        1.0f64.next_down()
    };
    // Inside for less than GRAZE of travel, on the longest axis: a graze.
    let longest = d.iter().map(|v| v.abs()).fold(0.0, max_f64);
    if (min_f64(t_out, 1.0) - t) * longest < GRAZE && !inside(axes, e) {
        return None;
    }
    // Along a face off the axes within rounding: a slide, not a hit.
    if k >= 3 && sliding(&axes[k], s, e) && !inside(axes, e) {
        return None;
    }
    let (mut snap, mut normal) = attribute(&axes[k], lo);
    let mut q = stop(s, d, t, snap);
    // Another face of X, Y or Z entered at the same time: on it.
    for a in axes.iter().take(3).filter(|a| a.index != k) {
        let i = a.index;
        if let Some((enter, _, lo)) = crossing(a, s[i], e[i])
            && enter > f64::NEG_INFINITY
        {
            if lo && q[i] < a.lo {
                q[i] = a.lo;
            } else if !lo && q[i] > a.hi {
                q[i] = a.hi;
            }
        }
    }
    // The rest of the motion must meet the primitive again, or it grazed.
    match meet(axes, q, e) {
        Meet::Inside => {}
        Meet::Miss => {
            if !inside(axes, e) {
                return None;
            }
        }
        Meet::Enter(rest, rest_out, j, lo) => {
            if snap.is_some() {
                if rest > 0.0 && j >= 3 {
                    (snap, normal) = attribute(&axes[j], lo);
                } else if !inside(axes, ulp_towards(q, e, None))
                    && !inside(axes, ulp_towards(q, e, Some(k)))
                {
                    match edge_at_entry(axes, s, e, q) {
                        Some((j, lo)) => (snap, normal) = attribute(&axes[j], lo),
                        None if !inside(axes, e) && sliver(axes, q, e, rest, rest_out) => {
                            return None;
                        }
                        None => {}
                    }
                }
            }
        }
    }
    Some((t, q, snap, normal))
}

impl CollisionWorld for Brute {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        if self.overlaps(start, hull) {
            return Trace {
                fraction: 0.0,
                end: start,
                normal: Vec3::ZERO,
                start_solid: true,
            };
        }
        let h = half(hull);
        let (s, e) = (xyz(start), xyz(end));
        let d = [e[0] - s[0], e[1] - s[1], e[2] - s[2]];
        let mut best: Option<Hit> = None;
        for p in &self.0 {
            if let Some(hit) = sweep(&p.axes(h), s, e, d)
                && best.is_none_or(|b| hit.0 < b.0)
            {
                best = Some(hit);
            }
        }
        let Some((t, q, snap, normal)) = best else {
            return Trace {
                fraction: 1.0,
                end,
                normal: Vec3::ZERO,
                start_solid: false,
            };
        };
        let blocked = |fraction, q, normal| Trace {
            fraction,
            end: pos(q),
            normal,
            start_solid: false,
        };
        let free = |q: [f64; 3]| !self.overlaps(pos(q), hull);
        if free(q) {
            return blocked(t, q, normal);
        }
        // Inside something met at the same time: onto its last-entered face
        // on X, Y or Z, the lowest index first.
        let mut onto = q;
        let mut set = [false; 3];
        for p in &self.0 {
            let axes = p.axes(h);
            if inside(&axes, q)
                && let Meet::Enter(_, _, j, lo) = meet(&axes, s, e)
                && j < 3
                && !set[j]
            {
                onto[j] = if lo { axes[j].lo } else { axes[j].hi };
                set[j] = true;
            }
        }
        if free(onto) {
            return blocked(t, onto, normal);
        }
        // On along the path a little, on the hit face.
        let mut step = f64::EPSILON;
        for _ in 0..NUDGE_STEPS {
            if t + step >= 1.0 {
                break;
            }
            let on = stop(s, d, t + step, snap);
            if free(on) {
                return blocked(t, on, normal);
            }
            step *= 2.0;
        }
        // Back along the path to a free point, on the hit face if that is
        // free too.
        let mut step = t * f64::EPSILON;
        for _ in 0..BACKOFF_STEPS {
            let back = max_f64(t - step, 0.0);
            let p = stop(s, d, back, None);
            if free(p) {
                let on = stop(s, d, back, snap);
                if free(on) {
                    return blocked(back, on, normal);
                }
                return blocked(back, p, normal);
            }
            step *= 2.0;
        }
        blocked(0.0, s, normal)
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        let (h, c) = (half(hull), xyz(at));
        self.0.iter().any(|p| inside(&p.axes(h), c))
    }
}

/// Whether the motion from `a` to `b` moves along `axis` by no more than
/// rounding (`NOISE` per meter of coordinate at each end).
fn sliding(axis: &Axis, a: [f64; 3], b: [f64; 3]) -> bool {
    let noise = |c: [f64; 3]| (c[0].abs() + c[1].abs() + 1.0) * NOISE;
    (axis.project(b) - axis.project(a)).abs() <= noise(a) + noise(b)
}

/// A sliver: midway through the overlap `(t0, t1)` from `q` to `e`, inside
/// by less than 2^-40 m per meter of coordinate on some axis.
fn sliver(axes: &[Axis], q: [f64; 3], e: [f64; 3], t0: f64, t1: f64) -> bool {
    let t = (max_f64(t0, 0.0) + min_f64(t1, 1.0)) * 0.5;
    let mut c = [0.0; 3];
    for i in 0..3 {
        c[i] = q[i] + (e[i] - q[i]) * t;
    }
    let thin = (c[0].abs() + c[1].abs() + c[2].abs() + 1.0) / 1_099_511_627_776.0;
    axes.iter().any(|a| {
        let p = a.project(c);
        min_f64(p - a.lo, a.hi - p) < thin
    })
}
