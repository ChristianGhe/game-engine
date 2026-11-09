//! `Grid::new`'s limits at their exact boundaries, the counts and messages of
//! its errors, the slabs' axes, and the grid's candidate search at a cell
//! line: what the contract and oracle tests do not reach.

use eng_collision_api::{BoxHull, CollisionWorld, conformance};
use eng_collision_grid::{
    Grid, GridError, MAX_CELLS, MAX_COORD, MAX_CORNERS, MAX_ENTRIES, Point2, Slab, Wall,
};
use eng_math::{TAU_F64, Vec3, WorldPos, sin_cos_f64};
use eng_schema_core::to_bytes;

/// Rung 2's room hull: 0.5 m x 0.5 m x 1.75 m.
const HULL: BoxHull = BoxHull {
    half_extents: Vec3::new(0.25, 0.25, 0.875),
};
const H: f64 = 0.875;

fn at(x: f64, y: f64, z: f64) -> WorldPos {
    WorldPos::new(x, y, z)
}

fn wall(a: (f64, f64), b: (f64, f64), bottom: f64, top: f64) -> Wall {
    Wall {
        a: Point2::new(a.0, a.1),
        b: Point2::new(b.0, b.1),
        bottom,
        top,
    }
}

fn slab(points: &[(f64, f64)], bottom: f64, top: f64) -> Slab {
    Slab {
        points: points.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
        bottom,
        top,
    }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64, bottom: f64, top: f64) -> Slab {
    slab(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], bottom, top)
}

fn refuse(walls: Vec<Wall>, slabs: Vec<Slab>) -> GridError {
    Grid::new(walls, slabs).unwrap_err()
}

#[test]
fn a_grid_of_exactly_the_most_cells_and_entries_builds() {
    // 2048 x 2048 cells is MAX_CELLS; four slabs over all of them list
    // MAX_ENTRIES entries. Only more than either is refused.
    let all = rect(0.0, 0.0, 2047.5, 2047.5, 0.0, 1.0);
    let grid = Grid::new(vec![], vec![all.clone(), all.clone(), all.clone(), all]).unwrap();
    assert_eq!(grid.cells(), (2048, 2048));
    assert_eq!(2048 * 2048, MAX_CELLS);
    assert_eq!(4 * MAX_CELLS, MAX_ENTRIES);
}

#[test]
fn too_many_entries_are_refused_with_their_count() {
    // A wall in the origin's cell, and five slabs over cells 1..=2047 on each
    // axis: MAX_CELLS cells (allowed), and 1 + 5 x 2047^2 entries (too many).
    let origin = wall((0.0, 0.0), (0.5, 0.0), 0.0, 1.0);
    let slabs = vec![rect(1.0, 1.0, 2047.5, 2047.5, 0.0, 1.0); 5];
    assert_eq!(
        refuse(vec![origin], slabs),
        GridError::TooDense {
            entries: 1 + 5 * 2047 * 2047
        }
    );
}

#[test]
fn a_height_that_is_not_a_coordinate_is_refused_on_either_end() {
    let refused = |bottom: f64, top: f64| {
        let walled = refuse(vec![wall((0.0, 0.0), (1.0, 0.0), bottom, top)], vec![]);
        let square = rect(0.0, 0.0, 1.0, 1.0, bottom, top);
        (walled, refuse(vec![], vec![square]))
    };
    let both = GridError::Coordinate { primitive: 0 };
    for (bottom, top) in [
        (0.0, f64::NAN),
        (0.0, MAX_COORD * 2.0),
        (f64::NEG_INFINITY, 1.0),
        (f64::NAN, 1.0),
    ] {
        assert_eq!(refused(bottom, top), (both, both), "{bottom} to {top}");
    }
}

#[test]
fn a_slab_corner_with_one_bad_coordinate_is_refused() {
    for bad in [(f64::NAN, 1.0), (1.0, f64::INFINITY)] {
        let tri = slab(&[(0.0, 0.0), (2.0, 0.0), bad], 0.0, 1.0);
        assert_eq!(
            refuse(vec![wall((0.0, 0.0), (1.0, 0.0), 0.0, 1.0)], vec![tri]),
            GridError::Coordinate { primitive: 1 },
            "{bad:?}"
        );
    }
}

/// A regular polygon of `n` corners of radius 4 m about (5, 5).
fn regular(n: u32) -> Slab {
    let points: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let (s, c) = sin_cos_f64(TAU_F64 * f64::from(i) / f64::from(n));
            (5.0 + 4.0 * c, 5.0 + 4.0 * s)
        })
        .collect();
    slab(&points, 0.0, 1.0)
}

#[test]
fn a_slab_may_have_max_corners_and_no_more() {
    let n = u32::try_from(MAX_CORNERS).unwrap();
    assert!(Grid::new(vec![], vec![regular(n)]).is_ok());
    assert_eq!(
        refuse(vec![], vec![regular(n + 1)]),
        GridError::NotConvex { primitive: 0 }
    );
}

#[test]
fn a_corner_on_the_line_of_an_edge_is_not_strictly_convex_in_either_winding() {
    // (1, 2) lies on the line from (2, 2) to (0, 2), past the first three
    // corners, which do turn (the winding is read from them).
    let ccw = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (1.0, 2.0), (0.0, 2.0)];
    let cw = [(0.0, 0.0), (0.0, 2.0), (1.0, 2.0), (2.0, 2.0), (2.0, 0.0)];
    for (points, winding) in [(ccw, "anticlockwise"), (cw, "clockwise")] {
        assert_eq!(
            refuse(vec![], vec![slab(&points, 0.0, 1.0)]),
            GridError::NotConvex { primitive: 0 },
            "{winding}"
        );
    }
}

#[test]
fn every_error_says_what_was_refused() {
    let says = [
        (
            GridError::Coordinate { primitive: 2 },
            "primitive 2: a coordinate is not finite or beyond 1048576 m",
        ),
        (
            GridError::Heights { primitive: 3 },
            "primitive 3: bottom above top",
        ),
        (
            GridError::ZeroLength { primitive: 4 },
            "primitive 4: a wall's ends are the same point",
        ),
        (
            GridError::NotConvex { primitive: 5 },
            "primitive 5: a slab needs 3 to 64 corners, strictly convex",
        ),
        (
            GridError::TooLarge { cells: 7 },
            "the grid would have 7 cells (max 4194304)",
        ),
        (
            GridError::TooDense { entries: 8 },
            "the grid would hold 8 entries (max 16777216)",
        ),
        (
            GridError::TooMany { primitives: 9 },
            "9 primitives is too many",
        ),
    ];
    for (error, text) in says {
        assert_eq!(error.to_string(), text);
    }
}

#[test]
fn the_grid_keeps_its_walls_and_slabs_in_order() {
    let walls = vec![
        wall((0.0, 0.0), (4.0, 0.0), 0.0, 3.0),
        wall((4.0, 0.0), (4.0, 3.0), 0.5, 2.0),
    ];
    let slabs = vec![rect(0.0, 0.0, 4.0, 3.0, -0.5, 0.0)];
    let grid = Grid::new(walls.clone(), slabs.clone()).unwrap();
    assert_eq!(grid.walls(), walls.as_slice());
    assert_eq!(grid.slabs(), slabs.as_slice());
}

#[test]
fn a_slab_has_one_axis_per_direction_of_its_edges_off_the_axes() {
    // A diamond's four edges run in two directions, each with its opposite:
    // two axes. A kite with the same bounds has no parallel edges: four. An
    // axis is two f32s and two f64s, 24 bytes encoded; everything else in the
    // two grids encodes to the same length.
    let diamond = slab(&[(1.0, 0.0), (2.0, 1.0), (1.0, 2.0), (0.0, 1.0)], 0.0, 1.0);
    let kite = slab(&[(1.0, 0.0), (2.0, 1.0), (1.0, 2.0), (0.0, 1.5)], 0.0, 1.0);
    let len = |s: Slab| to_bytes(&Grid::new(vec![], vec![s]).unwrap()).len();
    assert_eq!(len(kite), len(diamond.clone()) + 2 * 24);
    // The diamond blocks on its own faces: a hull moving south-west onto
    // the north-east face stops in contact with it, off the axes.
    let world = Grid::new(vec![], vec![diamond]).unwrap();
    let (start, end) = (at(2.5, 2.5, 0.5), at(1.0, 1.0, 0.5));
    conformance::check(&world, start, end, &HULL);
    let tr = world.trace(start, end, &HULL);
    assert!(tr.hit() && tr.normal.x > 0.7 && tr.normal.y > 0.7, "{tr:?}");
    assert!(tr.end.x + tr.end.y > 3.0 + 0.49, "{tr:?}");
}

#[test]
fn a_hull_reaching_just_past_a_cell_line_meets_a_wall_before_it() {
    // The hull's swept bounds end 0.5 mm past x = 3 (then y = 3), less than
    // the grid's 1 mm margin. The wall 0.3 mm before the line is listed only
    // in the cells before it, and must still be found, and block.
    let x_wall = Grid::new(vec![wall((2.9997, 0.0), (2.9997, 6.0), 0.0, 3.0)], vec![]).unwrap();
    let y_wall = Grid::new(vec![wall((0.0, 2.9997), (6.0, 2.9997), 0.0, 3.0)], vec![]).unwrap();
    let face = 2.9997 + 0.25;
    for (world, start, end, stop, normal) in [
        (
            &x_wall,
            at(3.5, 3.0, H),
            at(3.2495, 3.0, H),
            at(face, 3.0, H),
            Vec3::X,
        ),
        (
            &y_wall,
            at(3.0, 3.5, H),
            at(3.0, 3.2495, H),
            at(3.0, face, H),
            Vec3::Y,
        ),
    ] {
        conformance::check(world, start, end, &HULL);
        let tr = world.trace(start, end, &HULL);
        assert!(tr.hit(), "{tr:?}");
        assert_eq!((tr.end, tr.normal), (stop, normal), "{tr:?}");
    }
}
