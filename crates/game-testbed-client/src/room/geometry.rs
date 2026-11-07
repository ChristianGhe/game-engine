//! The room's static meshes, built from its sim data: the walls and slabs
//! of its `eng.collision.grid.Grid` constant and the regions of its
//! `testbed.RoomMap` (rung 2, T7's layout), as eng-render's [`Chunk`]s.
//!
//! - **Faces.** A wall has no thickness and blocks from both sides, so it
//!   gets a face on each side a viewer can stand on. A slab is a solid
//!   prism: its top, its bottom and a face on each edge of its polygon.
//!   Each face is a quad of its own vertices ([`Chunk::quad`]), corners
//!   counter-clockwise seen from its front, so back faces are culled:
//!   that is what keeps the room's coplanar faces that point opposite
//!   ways (a pillar's top against the ceiling's underside, a slab's side
//!   against a wall) from fighting.
//! - **Regions.** Each face is cut at every X and Y where any region's
//!   rectangle starts or ends (an axis-aligned top or bottom into a grid of
//!   rectangles), so faces that meet along an edge share their corners
//!   there and no crack opens between them. Each piece takes the region
//!   its front faces: for a vertical face, the one holding the point
//!   [`PROBE`] in front of its middle; for a top or bottom, the one holding
//!   its middle ([`RoomMap::region_at`]).
//!   A piece facing no region faces where nobody stands and is left out,
//!   as are a slab's bottom and sides at or below the floor.
//! - **Materials**, by what a slab is, from the floor level (the lowest
//!   slab top) and the hull's height: a ceiling if its bottom is at least
//!   a hull above the floor, a floor if its top is at the floor, a pillar
//!   if its top is at least a hull above it, else a step; walls, and a
//!   ceiling's sides (the skylight's shaft, the lintel over the alcove),
//!   are wall.
//! - **Classes:** tops [`SurfaceClass::Floor`], bottoms
//!   [`SurfaceClass::Ceiling`], vertical faces by which way they face.
//! - **Ids**, which `info` carries: [`wall_id`] and [`slab_id`], by the
//!   primitive's index in the grid and the face.
//! - **Texture coordinates** are anchored to the world (spec 6.11): one
//!   texture per [`REPEAT`] meters, along X and down Y on tops and bottoms,
//!   and along the viewer's right and down Z on vertical faces. They are
//!   taken in f64 and wrapped there, less a whole number of repeats, before
//!   rounding to f32, so they stay small.
//! - **Chunks:** each face goes to the [`CHUNK_SIDE`] tile holding its
//!   middle, whose south-west corner is the chunk's origin. The room fits
//!   one tile, so a frame draws it with one chunk.

use std::collections::BTreeMap;

use eng_collision_grid::{Grid, Point2, Slab};
use eng_math::{WorldPos, floor_f64, sqrt_f64};
use eng_render::{Chunk, Info, SurfaceClass};
use game_testbed_sim::room::RoomMap;

use crate::textures::{REPEAT, Surface};

/// The side of the square tiles the world is cut into chunks by (m).
pub const CHUNK_SIDE: f64 = 16.0;

/// How far in front of a vertical face its region is looked up (m).
pub const PROBE: f64 = 1.0 / 64.0;

/// The id of side `side` of wall `wall`: 0 its left, seen from its first
/// point toward its second, 1 its right. Unique while there are at most
/// 65,536 walls.
pub const fn wall_id(wall: usize, side: u32) -> u32 {
    0x0100_0000 | ((wall as u32 & 0xffff) << 8) | side
}

/// The id of face `face` of slab `slab`: 0 its top, 1 its bottom, and
/// `2 + e` the side on edge `e`, from its point `e` to the next. Unique
/// while there are at most 65,536 slabs.
pub const fn slab_id(slab: usize, face: u32) -> u32 {
    0x0200_0000 | ((slab as u32 & 0xffff) << 8) | face
}

/// One face piece before it joins a chunk.
struct Face {
    corners: [WorldPos; 4],
    uv: [[f32; 2]; 4],
    material: u32,
    info: Info,
}

/// The faces so far, and what places them.
struct Builder<'a> {
    map: &'a RoomMap,
    faces: Vec<Face>,
}

/// The room's chunks: every face of `grid`'s walls and slabs, cut and
/// lit by `map`'s regions, for a hull `hull_height` meters tall.
pub fn chunks(grid: &Grid, map: &RoomMap, hull_height: f64) -> Vec<Chunk> {
    let floor = grid
        .slabs()
        .iter()
        .map(|s| s.top)
        .reduce(eng_math::min_f64)
        .unwrap_or(0.0);
    let mut builder = Builder {
        map,
        faces: Vec::new(),
    };
    for (index, wall) in grid.walls().iter().enumerate() {
        let (left, right) = normals(wall.a, wall.b);
        let material = Surface::Wall.material();
        for (side, normal) in [(0, left), (1, right)] {
            let id = wall_id(index, side);
            builder.vertical(
                [wall.a, wall.b],
                [wall.bottom, wall.top],
                normal,
                material,
                id,
            );
        }
    }
    for (index, slab) in grid.slabs().iter().enumerate() {
        builder.slab(index, slab, floor, surface(slab, floor, hull_height));
    }
    let mut tiles: BTreeMap<(i64, i64), Chunk> = BTreeMap::new();
    for face in builder.faces {
        let c = face.corners;
        let tile = |a: f64, b: f64| floor_f64((a + b) / 2.0 / CHUNK_SIDE) as i64;
        let key = (tile(c[0].x, c[2].x), tile(c[0].y, c[2].y));
        tiles
            .entry(key)
            .or_insert_with(|| {
                let at = |t: i64| t as f64 * CHUNK_SIDE;
                Chunk::new(WorldPos::new(at(key.0), at(key.1), 0.0))
            })
            .quad(face.corners, face.uv, face.material, face.info);
    }
    tiles.into_values().collect()
}

/// What `slab` is drawn as, over a floor at `floor` for a hull
/// `hull_height` tall.
fn surface(slab: &Slab, floor: f64, hull_height: f64) -> Surface {
    if slab.bottom >= floor + hull_height {
        Surface::Ceiling
    } else if slab.top <= floor {
        Surface::Floor
    } else if slab.top >= floor + hull_height {
        Surface::Pillar
    } else {
        Surface::Step
    }
}

/// The unit normals on the left and the right of the way from `a` to `b`.
fn normals(a: Point2, b: Point2) -> ((f64, f64), (f64, f64)) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = sqrt_f64(dx * dx + dy * dy);
    let (dx, dy) = (dx / length, dy / length);
    ((-dy, dx), (dy, -dx))
}

/// `value / REPEAT` for each, less the whole repeats below the smallest,
/// so the texture lands where the world says and the numbers stay small.
fn wrapped<const N: usize>(values: [f64; N]) -> [f32; N] {
    let scaled = values.map(|v| v / REPEAT);
    let least = scaled.into_iter().reduce(eng_math::min_f64).unwrap_or(0.0);
    let whole = floor_f64(least);
    scaled.map(|v| (v - whole) as f32)
}

/// The class of a vertical face with `normal`.
fn facing(normal: (f64, f64)) -> SurfaceClass {
    if normal.1 == 0.0 {
        SurfaceClass::WallFacingX
    } else if normal.0 == 0.0 {
        SurfaceClass::WallFacingY
    } else {
        SurfaceClass::Wall
    }
}

impl Builder<'_> {
    fn region(&self, x: f64, y: f64) -> Option<u16> {
        self.map
            .region_at(x, y)
            .map(|r| u16::try_from(r).unwrap_or(u16::MAX))
    }

    /// The cuts of `0..1` along the segment `ends` where a region's edge
    /// crosses it, with their points, in order: exact on an axis.
    fn pieces(&self, ends: [Point2; 2]) -> Vec<(Point2, Point2)> {
        let [a, b] = ends;
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let mut cuts: Vec<(f64, Point2)> = vec![(0.0, a), (1.0, b)];
        let between = |c: f64, p: f64, q: f64| (p < c && c < q) || (q < c && c < p);
        for region in &self.map.regions {
            for c in [region.min.x, region.max.x] {
                if between(c, a.x, b.x) {
                    let t = (c - a.x) / dx;
                    let y = if dy == 0.0 { a.y } else { a.y + dy * t };
                    cuts.push((t, Point2::new(c, y)));
                }
            }
            for c in [region.min.y, region.max.y] {
                if between(c, a.y, b.y) {
                    let t = (c - a.y) / dy;
                    let x = if dx == 0.0 { a.x } else { a.x + dx * t };
                    cuts.push((t, Point2::new(x, c)));
                }
            }
        }
        // Stable, by a total key; equal cuts are one.
        cuts.sort_by(|p, q| p.0.total_cmp(&q.0));
        cuts.dedup_by(|later, earlier| later.0 == earlier.0);
        cuts.windows(2).map(|w| (w[0].1, w[1].1)).collect()
    }

    /// The faces of the vertical rectangle over the segment `ends`, from
    /// `z[0]` up to `z[1]`, facing `normal`: one per piece between the
    /// regions' edges that faces a region.
    fn vertical(
        &mut self,
        ends: [Point2; 2],
        z: [f64; 2],
        normal: (f64, f64),
        material: u32,
        id: u32,
    ) {
        if z[1] <= z[0] {
            return;
        }
        let class = facing(normal);
        // The viewer's right, looking at the face: its normal turned a
        // quarter turn counter-clockwise.
        let axis = (-normal.1, normal.0);
        let along = |x: f64, y: f64| x * axis.0 + y * axis.1;
        for (p, q) in self.pieces(ends) {
            let middle = ((p.x + q.x) / 2.0, (p.y + q.y) / 2.0);
            let probe = (middle.0 + normal.0 * PROBE, middle.1 + normal.1 * PROBE);
            let Some(region) = self.region(probe.0, probe.1) else {
                continue;
            };
            let (left, right) = if along(p.x, p.y) <= along(q.x, q.y) {
                (p, q)
            } else {
                (q, p)
            };
            let at = |p: Point2, z: f64| WorldPos::new(p.x, p.y, z);
            let corners = [
                at(left, z[0]),
                at(right, z[0]),
                at(right, z[1]),
                at(left, z[1]),
            ];
            let u = wrapped(corners.map(|c| along(c.x, c.y)));
            let v = wrapped(corners.map(|c| -c.z));
            self.faces.push(Face {
                corners,
                uv: [0, 1, 2, 3].map(|i| [u[i], v[i]]),
                material,
                info: Info { region, class, id },
            });
        }
    }

    /// A horizontal face over the convex `points`, at `z`, facing up or
    /// down, in `region`.
    fn horizontal(&mut self, points: &[Point2], z: f64, up: bool, material: u32, info: Info) {
        // Counter-clockwise from above for a top, clockwise for a bottom:
        // counter-clockwise seen from the front either way.
        let mut points = points.to_vec();
        if signed_area(&points) < 0.0 {
            points.reverse();
        }
        if !up {
            points.reverse();
        }
        let at = |p: Point2| WorldPos::new(p.x, p.y, z);
        // A fan of quads from the first corner; a last triangle repeats its
        // final corner.
        let mut i = 1;
        while i + 1 < points.len() {
            let last = (i + 2).min(points.len() - 1);
            let corners = [
                at(points[0]),
                at(points[i]),
                at(points[i + 1]),
                at(points[last]),
            ];
            let u = wrapped(corners.map(|c| c.x));
            let v = wrapped(corners.map(|c| -c.y));
            self.faces.push(Face {
                corners,
                uv: [0, 1, 2, 3].map(|i| [u[i], v[i]]),
                material,
                info,
            });
            i += 2;
        }
    }

    /// Every face of slab `index`, drawn as `surface`, over a floor at
    /// `floor`.
    fn slab(&mut self, index: usize, slab: &Slab, floor: f64, surface: Surface) {
        let material = surface.material();
        let faces = [(slab.top, true, 0), (slab.bottom, false, 1)];
        for (z, up, face) in faces {
            // Nobody stands below the floor to see a bottom there.
            if !up && z <= floor {
                continue;
            }
            let class = if up {
                SurfaceClass::Floor
            } else {
                SurfaceClass::Ceiling
            };
            let id = slab_id(index, face);
            for (cell, centre) in self.cells(&slab.points) {
                let Some(region) = self.region(centre.x, centre.y) else {
                    continue;
                };
                self.horizontal(&cell, z, up, material, Info { region, class, id });
            }
        }
        if slab.top <= floor {
            return;
        }
        // A ceiling's edges, the skylight's shaft and the lintel over the
        // alcove, carry on the walls' bricks.
        let material = match surface {
            Surface::Ceiling => Surface::Wall.material(),
            _ => material,
        };
        let counter_clockwise = signed_area(&slab.points) > 0.0;
        let n = slab.points.len();
        for e in 0..n {
            let (a, b) = (slab.points[e], slab.points[(e + 1) % n]);
            let (left, right) = normals(a, b);
            let outward = if counter_clockwise { right } else { left };
            let id = slab_id(index, 2 + e as u32);
            self.vertical([a, b], [slab.bottom, slab.top], outward, material, id);
        }
    }

    /// The polygon `points` cut where the regions' edges cross it, with
    /// each cell's middle: a grid of rectangles for an axis-aligned
    /// rectangle, else the polygon whole, about its corners' mean.
    fn cells(&self, points: &[Point2]) -> Vec<(Vec<Point2>, Point2)> {
        let xs: Vec<f64> = points.iter().map(|p| p.x).collect();
        let ys: Vec<f64> = points.iter().map(|p| p.y).collect();
        let span = |v: &[f64]| {
            let lo = v.iter().copied().reduce(eng_math::min_f64).unwrap_or(0.0);
            let hi = v.iter().copied().reduce(eng_math::max_f64).unwrap_or(0.0);
            (lo, hi)
        };
        let ((x0, x1), (y0, y1)) = (span(&xs), span(&ys));
        let rectangle = points.len() == 4
            && points
                .iter()
                .all(|p| (p.x == x0 || p.x == x1) && (p.y == y0 || p.y == y1))
            && x0 < x1
            && y0 < y1;
        if !rectangle {
            let n = points.len() as f64;
            let mean = Point2::new(xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
            return vec![(points.to_vec(), mean)];
        }
        let cuts = |lo: f64, hi: f64, edges: &mut dyn Iterator<Item = f64>| {
            let mut v = vec![lo, hi];
            v.extend(edges.filter(|&c| lo < c && c < hi));
            v.sort_by(f64::total_cmp);
            v.dedup();
            v
        };
        let regions = &self.map.regions;
        let xs = cuts(x0, x1, &mut regions.iter().flat_map(|r| [r.min.x, r.max.x]));
        let ys = cuts(y0, y1, &mut regions.iter().flat_map(|r| [r.min.y, r.max.y]));
        let mut cells = Vec::new();
        for y in ys.windows(2) {
            for x in xs.windows(2) {
                let corners = vec![
                    Point2::new(x[0], y[0]),
                    Point2::new(x[1], y[0]),
                    Point2::new(x[1], y[1]),
                    Point2::new(x[0], y[1]),
                ];
                cells.push((
                    corners,
                    Point2::new((x[0] + x[1]) / 2.0, (y[0] + y[1]) / 2.0),
                ));
            }
        }
        cells
    }
}

/// Twice the polygon's signed area: above 0 when counter-clockwise from
/// above.
fn signed_area(points: &[Point2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (p, q) = (points[i], points[(i + 1) % n]);
            p.x * q.y - q.x * p.y
        })
        .sum()
}

#[cfg(test)]
mod tests;
