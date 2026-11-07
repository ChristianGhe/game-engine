//! No pass-through at vertex contacts: a hull that starts where its corner
//! touches an edge off the axes, or a primitive's corner, must not sweep
//! through the solid there. Cases found by an independent review (an exact
//! rational oracle written from decision 2 and ADR-0024), and the axis
//! sliver that keeps the graze rule's depth test.

use eng_collision_api::{BoxHull, CollisionWorld, Trace, conformance};
use eng_collision_grid::{Grid, Point2, Slab, Wall};
use eng_math::{Vec3, WorldPos};

#[path = "vertices/cases.rs"]
mod cases;

use cases::{Case, TUNNELS};

/// How far inside solid a point on the way may be (m): the hull shrunk by
/// this on every side must stay free before the hit.
const THROUGH: f32 = 1.0e-6;

/// Points sampled on the way, from the start to the hit.
const SAMPLES: u32 = 64;

/// The first point before the hit where the hull is more than `THROUGH`
/// inside solid, if any.
fn passes_through(
    world: &Grid,
    from: WorldPos,
    to: WorldPos,
    hull: &BoxHull,
    tr: &Trace,
) -> Option<WorldPos> {
    let h = hull.half_extents;
    let shrunk = BoxHull {
        half_extents: Vec3::new(h.x - THROUGH, h.y - THROUGH, h.z - THROUGH),
    };
    (0..=SAMPLES).find_map(|i| {
        let t = tr.fraction * f64::from(i) / f64::from(SAMPLES);
        let p = WorldPos::new(
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t,
            from.z + (to.z - from.z) * t,
        );
        world.overlaps(p, &shrunk).then_some(p)
    })
}

impl Case {
    fn world(&self) -> Grid {
        let walls = self.walls.iter().map(|w| Wall {
            a: Point2::new(w[0], w[1]),
            b: Point2::new(w[2], w[3]),
            bottom: w[4],
            top: w[5],
        });
        let slabs = self.slabs.iter().map(|&(bottom, top, p)| Slab {
            points: p.chunks(2).map(|c| Point2::new(c[0], c[1])).collect(),
            bottom,
            top,
        });
        Grid::new(walls.collect(), slabs.collect()).unwrap()
    }

    /// Checks the trace with the conformance suite and for pass-through,
    /// and returns it.
    fn run(&self) -> Trace {
        let world = self.world();
        let hull = BoxHull {
            half_extents: Vec3::new(
                self.hull[0] as f32,
                self.hull[1] as f32,
                self.hull[2] as f32,
            ),
        };
        let t = self.trace;
        let (s, e) = (
            WorldPos::new(t[0], t[1], t[2]),
            WorldPos::new(t[3], t[4], t[5]),
        );
        conformance::check(&world, s, e, &hull);
        let tr = world.trace(s, e, &hull);
        let through = passes_through(&world, s, e, &hull, &tr);
        assert!(
            through.is_none(),
            "{tr:?} passes through solid at {through:?}"
        );
        tr
    }
}

#[test]
fn a_hull_at_a_walls_end_does_not_pass_through_it() {
    // The hull's corner touches the end of a wall off the axes and the hull
    // moves along the wall's far side: exactly, it is blocked at once.
    let case = Case {
        hull: [0.25, 0.25, 0.875],
        walls: &[[
            2.5882537698672854,
            3.0,
            3.481933063471426,
            3.4468396468020703,
            0.3,
            0.7,
        ]],
        slabs: &[],
        trace: [
            3.231933063471426,
            3.6968396468020703,
            1.575,
            -1.2466224341623917,
            0.6354366111526017,
            1.5069392195383364,
        ],
    };
    let tr = case.run();
    assert!(tr.hit() && tr.fraction == 0.0, "{tr:?}");
}

#[test]
fn hulls_starting_at_vertex_contacts_do_not_pass_through() {
    for (i, case) in TUNNELS.iter().enumerate() {
        let tr = case.run();
        assert!(tr.hit(), "case {i}: {tr:?}");
    }
}

#[test]
fn a_hull_an_ulp_below_a_wall_top_slides_over_it() {
    // The hull's bottom is one ulp below the room's walls' top as it moves
    // across their corner: the solid ahead is a 4.4e-16 m sliver, a graze by
    // the depth test, not a 12 m block.
    let case = Case {
        hull: [0.25, 0.25, 0.875],
        walls: &[
            [0.75, 0.0, 0.0, 0.0, 0.0, 0.2],
            [0.0, 0.0, 0.25, 0.0, 0.0, 0.2],
            [0.0, 0.0, 12.0, 0.0, 0.0, 3.0],
            [12.0, 0.0, 12.0, 9.0, 0.0, 3.0],
            [12.0, 9.0, 0.0, 9.0, 0.0, 3.0],
            [0.0, 9.0, 0.0, 0.0, 0.0, 3.0],
        ],
        slabs: &[(-0.5, 0.0, &[-1.0, -1.0, 13.0, -1.0, 13.0, 10.0, -1.0, 10.0])],
        trace: [12.25, 9.25, 3.8749999999999996, 0.25, 8.75, 3.875],
    };
    let tr = case.run();
    assert!(!tr.hit(), "{tr:?}");
}
