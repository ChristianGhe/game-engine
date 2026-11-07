//! The grid: 1 m cells on the XY plane, each listing the primitives whose
//! bounds touch it, and the [`CollisionWorld`] built on them.

use alloc::vec;
use alloc::vec::Vec;

use eng_collision_api::{BoxHull, CollisionWorld, Trace};
use eng_math::{Vec3, WorldPos, floor_f64, max_f64, min_f64};
use eng_schema_core::Schema;

use crate::shape::{Axis, Shape};
use crate::sweep::{self, Sweep, half};
use crate::{BACKOFF_STEPS, GridError, MARGIN, MAX_CELLS, MAX_ENTRIES, NUDGE_STEPS, Slab, Wall};

/// Static walls and slabs over a uniform grid of 1 m cells: Level 1's
/// [`CollisionWorld`]. Built once by [`Grid::new`]; the crate docs give the
/// rules of its traces.
///
/// Its schema encodes the walls and slabs and the index built from them, so
/// a map's sim-data hash covers both, and a change to the index's layout
/// bumps the version (`tests/contract.rs` pins the room's encoding).
/// Decoding does not check the index: decode only what a `Grid` encoded,
/// never bytes from outside.
#[derive(Debug, Clone, PartialEq, Schema)]
#[schema(name = "eng.collision.grid.Grid", version = 1)]
pub struct Grid {
    walls: Vec<Wall>,
    slabs: Vec<Slab>,
    /// One per primitive: the walls, then the slabs.
    shapes: Vec<Shape>,
    axes: Vec<Axis>,
    /// The grid's south-west corner, in whole meters.
    origin_x: f64,
    origin_y: f64,
    width: u32,
    height: u32,
    /// Cell `c` (row-major, `y * width + x`) lists `items[start[c]..start[c + 1]]`,
    /// in primitive order.
    start: Vec<u32>,
    items: Vec<u32>,
}

/// An inclusive range of cells.
#[derive(Debug, Clone, Copy)]
struct Cells {
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
}

impl Grid {
    /// Builds the grid. Primitive indices, which break ties between hits,
    /// are the walls in order, then the slabs.
    pub fn new(walls: Vec<Wall>, slabs: Vec<Slab>) -> Result<Self, GridError> {
        let count = walls.len() + slabs.len();
        if u32::try_from(count).is_err() {
            return Err(GridError::TooMany { primitives: count });
        }
        let mut axes = Vec::new();
        let mut shapes = Vec::with_capacity(count);
        for (i, w) in walls.iter().enumerate() {
            shapes.push(w.shape(i, &mut axes)?);
        }
        for (i, s) in slabs.iter().enumerate() {
            shapes.push(s.shape(walls.len() + i, &mut axes)?);
        }
        let mut grid = Self {
            walls,
            slabs,
            shapes,
            axes,
            origin_x: 0.0,
            origin_y: 0.0,
            width: 0,
            height: 0,
            start: vec![0],
            items: Vec::new(),
        };
        grid.index()?;
        Ok(grid)
    }

    pub fn walls(&self) -> &[Wall] {
        &self.walls
    }

    pub fn slabs(&self) -> &[Slab] {
        &self.slabs
    }

    /// The grid's size in cells, east-west and north-south.
    pub fn cells(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Fills the cells: each primitive is listed in every cell its bounds
    /// touch.
    fn index(&mut self) -> Result<(), GridError> {
        if self.shapes.is_empty() {
            return Ok(());
        }
        let (mut x0, mut x1) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
        for s in &self.shapes {
            x0 = min_f64(x0, s.min_x);
            x1 = max_f64(x1, s.max_x);
            y0 = min_f64(y0, s.min_y);
            y1 = max_f64(y1, s.max_y);
        }
        self.origin_x = floor_f64(x0);
        self.origin_y = floor_f64(y0);
        // Exact: whole numbers of at most 2^21.
        let width = (floor_f64(x1) - self.origin_x) as u64 + 1;
        let height = (floor_f64(y1) - self.origin_y) as u64 + 1;
        let cells = width * height;
        if cells > MAX_CELLS {
            return Err(GridError::TooLarge { cells });
        }
        (self.width, self.height) = (width as u32, height as u32);
        let mut entries = 0u64;
        for i in 0..self.shapes.len() {
            let s = self.shapes[i];
            let r = self
                .range(s.min_x, s.max_x, s.min_y, s.max_y)
                .expect("a primitive lies inside its own grid");
            let shape = &mut self.shapes[i];
            (shape.cell_x0, shape.cell_x1) = (r.x0, r.x1);
            (shape.cell_y0, shape.cell_y1) = (r.y0, r.y1);
            entries += u64::from(r.x1 - r.x0 + 1) * u64::from(r.y1 - r.y0 + 1);
        }
        if entries > MAX_ENTRIES {
            return Err(GridError::TooDense { entries });
        }
        let mut counts = vec![0u32; cells as usize];
        for s in &self.shapes {
            for y in s.cell_y0..=s.cell_y1 {
                for x in s.cell_x0..=s.cell_x1 {
                    counts[(y * self.width + x) as usize] += 1;
                }
            }
        }
        let mut start = Vec::with_capacity(counts.len() + 1);
        let mut total = 0u32;
        start.push(0);
        for c in &counts {
            total += c;
            start.push(total);
        }
        let mut fill: Vec<u32> = start[..counts.len()].to_vec();
        let mut items = vec![0u32; total as usize];
        for (i, s) in self.shapes.iter().enumerate() {
            for y in s.cell_y0..=s.cell_y1 {
                for x in s.cell_x0..=s.cell_x1 {
                    let c = (y * self.width + x) as usize;
                    items[fill[c] as usize] = i as u32;
                    fill[c] += 1;
                }
            }
        }
        self.start = start;
        self.items = items;
        Ok(())
    }

    /// The cells under `[x0, x1] x [y0, y1]`, or `None` off the grid.
    fn range(&self, x0: f64, x1: f64, y0: f64, y1: f64) -> Option<Cells> {
        let (x0, x1) = axis_range(x0, x1, self.origin_x, self.width)?;
        let (y0, y1) = axis_range(y0, y1, self.origin_y, self.height)?;
        Some(Cells { x0, x1, y0, y1 })
    }

    /// The cells a hull touches anywhere between centres `a` and `b`, with a
    /// margin for rounding.
    fn swept(&self, a: [f64; 3], b: [f64; 3], h: [f64; 3]) -> Option<Cells> {
        self.range(
            min_f64(a[0], b[0]) - h[0] - MARGIN,
            max_f64(a[0], b[0]) + h[0] + MARGIN,
            min_f64(a[1], b[1]) - h[1] - MARGIN,
            max_f64(a[1], b[1]) + h[1] + MARGIN,
        )
    }

    /// Calls `f` once for each primitive listed in `cells`, visiting cells in
    /// row-major order and each cell's primitives in index order. A
    /// primitive is visited in the first of `cells` that lists it. Stops
    /// early when `f` returns `true`, and returns whether it did.
    fn each(&self, cells: Cells, mut f: impl FnMut(usize) -> bool) -> bool {
        for y in cells.y0..=cells.y1 {
            for x in cells.x0..=cells.x1 {
                let c = (y * self.width + x) as usize;
                let (from, to) = (self.start[c] as usize, self.start[c + 1] as usize);
                for &p in &self.items[from..to] {
                    let s = &self.shapes[p as usize];
                    let first = s.cell_x0.max(cells.x0) == x && s.cell_y0.max(cells.y0) == y;
                    if first && f(p as usize) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `q` moved onto the face of X, Y or Z through which the motion from `s`
    /// to `e` last enters each primitive `q` is inside, where that face is
    /// on X, Y or Z; the lower index wins an axis.
    fn onto_faces(&self, q: [f64; 3], s: [f64; 3], e: [f64; 3], hull: &BoxHull) -> [f64; 3] {
        let mut faces: [Option<(usize, f64)>; 3] = [None; 3];
        if let Some(cells) = self.swept(q, q, half(hull)) {
            self.each(cells, |p| {
                let shape = &self.shapes[p];
                if sweep::inside(shape, &self.axes, hull, q)
                    && let Some((j, face)) = sweep::entry_face(shape, &self.axes, hull, s, e)
                    && faces[j].is_none_or(|(i, _)| p < i)
                {
                    faces[j] = Some((p, face));
                }
                false
            });
        }
        let mut out = q;
        for (j, f) in faces.iter().enumerate() {
            if let Some((_, face)) = f {
                out[j] = *face;
            }
        }
        out
    }

    fn inside(&self, c: [f64; 3], hull: &BoxHull) -> bool {
        let Some(cells) = self.swept(c, c, half(hull)) else {
            return false;
        };
        self.each(cells, |p| {
            sweep::inside(&self.shapes[p], &self.axes, hull, c)
        })
    }
}

/// Cells `floor(lo) - origin ..= floor(hi) - origin`, limited to the grid.
fn axis_range(lo: f64, hi: f64, origin: f64, size: u32) -> Option<(u32, u32)> {
    if size == 0 {
        return None;
    }
    let (a, b) = (floor_f64(lo) - origin, floor_f64(hi) - origin);
    let last = f64::from(size) - 1.0;
    if !(b >= 0.0 && a <= last) {
        return None;
    }
    let a = if a > 0.0 { a as u32 } else { 0 };
    let b = if b < last { b as u32 } else { size - 1 };
    Some((a, b))
}

fn xyz(p: WorldPos) -> [f64; 3] {
    [p.x, p.y, p.z]
}

fn pos(c: [f64; 3]) -> WorldPos {
    WorldPos::new(c[0], c[1], c[2])
}

impl CollisionWorld for Grid {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let (s, e) = (xyz(start), xyz(end));
        let d = [e[0] - s[0], e[1] - s[1], e[2] - s[2]];
        let mut best: Option<(sweep::Hit, usize)> = None;
        let mut solid = false;
        if let Some(cells) = self.swept(s, e, half(hull)) {
            solid = self.each(cells, |p| {
                match sweep::sweep(&self.shapes[p], &self.axes, hull, s, e, d) {
                    Sweep::Inside => return true,
                    Sweep::Miss => {}
                    Sweep::Hit(hit) => {
                        if best.is_none_or(|(b, i)| hit.t < b.t || (hit.t == b.t && p < i)) {
                            best = Some((hit, p));
                        }
                    }
                }
                false
            });
        }
        if solid {
            return Trace {
                fraction: 0.0,
                end: start,
                normal: Vec3::ZERO,
                start_solid: true,
            };
        }
        let Some((hit, _)) = best else {
            return Trace {
                fraction: 1.0,
                end,
                normal: Vec3::ZERO,
                start_solid: false,
            };
        };
        settle(
            s,
            d,
            hit,
            |q| self.inside(q, hull),
            |q| self.onto_faces(q, s, e, hull),
        )
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        self.inside(xyz(at), hull)
    }
}

/// The trace for the first hit (the crate docs' step 6): its stop; or if
/// rounding left that inside something met at the same time, the stop moved
/// onto that thing's face on an axis (`onto`); or the stop a little further
/// along the path, on the hit face (`NUDGE_STEPS`); or else a point on the
/// path backed off until it is free, set on the hit face if that is free
/// too. The start, at fraction 0, is free, so the back-off ends there at the
/// latest.
fn settle(
    s: [f64; 3],
    d: [f64; 3],
    hit: sweep::Hit,
    inside: impl Fn([f64; 3]) -> bool,
    onto: impl Fn([f64; 3]) -> [f64; 3],
) -> Trace {
    let blocked = |fraction: f64, q: [f64; 3], normal: Vec3| Trace {
        fraction,
        end: pos(q),
        normal,
        start_solid: false,
    };
    let q = hit.q;
    if !inside(q) {
        return blocked(hit.t, q, hit.normal);
    }
    let q = onto(q);
    if !inside(q) {
        return blocked(hit.t, q, hit.normal);
    }
    let mut step = f64::EPSILON;
    for _ in 0..NUDGE_STEPS {
        let t = hit.t + step;
        if t >= 1.0 {
            break;
        }
        let q = sweep::stop(s, d, t, hit.snap);
        if !inside(q) {
            return blocked(hit.t, q, hit.normal);
        }
        step *= 2.0;
    }
    let mut step = hit.t * f64::EPSILON;
    // The start is free, so this ends by fraction 0 at the latest.
    for _ in 0..BACKOFF_STEPS {
        let t = hit.t - step;
        let t = if t > 0.0 { t } else { 0.0 };
        let p = sweep::stop(s, d, t, None);
        if !inside(p) {
            let on = sweep::stop(s, d, t, hit.snap);
            if !inside(on) {
                return blocked(t, on, hit.normal);
            }
            return blocked(t, p, hit.normal);
        }
        step *= 2.0;
    }
    blocked(0.0, s, hit.normal)
}
