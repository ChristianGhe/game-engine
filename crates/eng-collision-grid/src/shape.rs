//! The primitives a map is made of, and the separating axes each gives a
//! sweep.

use alloc::vec::Vec;

use eng_math::{max_f64, min_f64, sqrt_f64};
use eng_schema_core::Schema;

use crate::{GridError, MAX_COORD, MAX_CORNERS};

/// A point on the XY plane (m).
#[derive(Debug, Clone, Copy, PartialEq, Default, Schema)]
#[schema(name = "eng.collision.grid.Point2", version = 1)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A vertical wall: the segment from `a` to `b`, solid from `bottom` up to
/// `top`. It has no thickness, so it blocks from both sides; its top and
/// bottom edges block vertical motion too. Only a hull with some width
/// across it can meet it.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "eng.collision.grid.Wall", version = 1)]
pub struct Wall {
    pub a: Point2,
    pub b: Point2,
    pub bottom: f64,
    pub top: f64,
}

/// A horizontal slab: a convex polygon, solid from `bottom` up to `top`, for
/// floors, ceilings, steps and pillars. `bottom` may equal `top`. Its sides
/// block too, so a step needs no wall under its edge.
#[derive(Debug, Clone, PartialEq, Schema)]
#[schema(name = "eng.collision.grid.Slab", version = 1)]
pub struct Slab {
    /// Three to [`MAX_CORNERS`](crate::MAX_CORNERS) corners, in either
    /// winding, strictly convex.
    pub points: Vec<Point2>,
    pub bottom: f64,
    pub top: f64,
}

/// A separating axis off the X and Y axes: the unit normal of an edge,
/// rounded to f32, and the primitive's projection onto it in f64.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "eng.collision.grid.Axis", version = 1)]
pub(crate) struct Axis {
    pub nx: f32,
    pub ny: f32,
    pub min: f64,
    pub max: f64,
}

/// One primitive as the sweep sees it: its bounds on X, Y and Z, its axes off
/// the X and Y axes (`axes_len` of them from `axes_start` in the grid's
/// list), and the cells it covers.
#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "eng.collision.grid.Shape", version = 1)]
pub(crate) struct Shape {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub bottom: f64,
    pub top: f64,
    pub axes_start: u32,
    pub axes_len: u32,
    pub cell_x0: u32,
    pub cell_y0: u32,
    pub cell_x1: u32,
    pub cell_y1: u32,
}

/// The unit normal of the edge from `p` to `q`, rounded to f32, or `None`
/// for an edge on the X or Y axis.
pub(crate) fn edge_normal(p: Point2, q: Point2) -> Option<(f32, f32)> {
    let (ux, uy) = (q.x - p.x, q.y - p.y);
    if ux == 0.0 || uy == 0.0 {
        return None;
    }
    let len = sqrt_f64(ux * ux + uy * uy);
    Some(((uy / len) as f32, (-ux / len) as f32))
}

/// A point's projection onto an axis, in f64.
pub(crate) fn project(nx: f32, ny: f32, x: f64, y: f64) -> f64 {
    f64::from(nx) * x + f64::from(ny) * y
}

fn coordinate(x: f64) -> bool {
    x.is_finite() && x.abs() <= MAX_COORD
}

fn heights(primitive: usize, bottom: f64, top: f64) -> Result<(), GridError> {
    if !coordinate(bottom) || !coordinate(top) {
        return Err(GridError::Coordinate { primitive });
    }
    if bottom > top {
        return Err(GridError::Heights { primitive });
    }
    Ok(())
}

/// Adds the axes of the edges through `points` (closing the loop when
/// `closed`) to `axes`, skipping edges on the X or Y axis and an axis already
/// added for this primitive or its exact opposite.
fn add_axes(points: &[Point2], closed: bool, axes: &mut Vec<Axis>) -> u32 {
    let first = axes.len();
    let edges = if closed {
        points.len()
    } else {
        points.len() - 1
    };
    for i in 0..edges {
        let (p, q) = (points[i], points[(i + 1) % points.len()]);
        let Some((nx, ny)) = edge_normal(p, q) else {
            continue;
        };
        if axes[first..]
            .iter()
            .any(|a| (a.nx == nx && a.ny == ny) || (a.nx == -nx && a.ny == -ny))
        {
            continue;
        }
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in points {
            let p = project(nx, ny, v.x, v.y);
            min = min_f64(min, p);
            max = max_f64(max, p);
        }
        axes.push(Axis { nx, ny, min, max });
    }
    (axes.len() - first) as u32
}

/// A shape without its cells, which the grid fills in.
fn shape(points: &[Point2], bottom: f64, top: f64, axes_start: usize, axes_len: u32) -> Shape {
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in points {
        min_x = min_f64(min_x, p.x);
        max_x = max_f64(max_x, p.x);
        min_y = min_f64(min_y, p.y);
        max_y = max_f64(max_y, p.y);
    }
    Shape {
        min_x,
        max_x,
        min_y,
        max_y,
        bottom,
        top,
        axes_start: axes_start as u32,
        axes_len,
        cell_x0: 0,
        cell_y0: 0,
        cell_x1: 0,
        cell_y1: 0,
    }
}

impl Wall {
    pub(crate) fn shape(&self, primitive: usize, axes: &mut Vec<Axis>) -> Result<Shape, GridError> {
        let ends = [self.a, self.b];
        if !ends.iter().all(|p| coordinate(p.x) && coordinate(p.y)) {
            return Err(GridError::Coordinate { primitive });
        }
        heights(primitive, self.bottom, self.top)?;
        if self.a == self.b {
            return Err(GridError::ZeroLength { primitive });
        }
        let start = axes.len();
        let len = add_axes(&ends, false, axes);
        Ok(shape(&ends, self.bottom, self.top, start, len))
    }
}

/// The z component of `(b - a) x (c - a)`.
fn cross(a: Point2, b: Point2, c: Point2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

impl Slab {
    pub(crate) fn shape(&self, primitive: usize, axes: &mut Vec<Axis>) -> Result<Shape, GridError> {
        let pts = &self.points;
        if !pts.iter().all(|p| coordinate(p.x) && coordinate(p.y)) {
            return Err(GridError::Coordinate { primitive });
        }
        heights(primitive, self.bottom, self.top)?;
        if pts.len() < 3 || pts.len() > MAX_CORNERS || !strictly_convex(pts) {
            return Err(GridError::NotConvex { primitive });
        }
        let start = axes.len();
        let len = add_axes(pts, true, axes);
        Ok(shape(pts, self.bottom, self.top, start, len))
    }
}

/// Every other corner strictly on the same side of every edge.
fn strictly_convex(pts: &[Point2]) -> bool {
    let n = pts.len();
    let side = cross(pts[0], pts[1], pts[2]);
    if side == 0.0 {
        return false;
    }
    (0..n).all(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        (0..n).filter(|&j| j != i && j != (i + 1) % n).all(|j| {
            let c = cross(a, b, pts[j]);
            if side > 0.0 { c > 0.0 } else { c < 0.0 }
        })
    })
}
