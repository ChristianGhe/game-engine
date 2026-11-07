//! The grid keeps the collision contract (`eng_collision_api::conformance`,
//! with ADR-0024's contact rule): exact on faces square to an axis, within
//! `CONTACT_GAP` off them.

use eng_collision_api::{BoxHull, CollisionWorld, conformance};
use eng_collision_grid::{Grid, GridError, MAX_COORD, Point2, Slab, Wall};
use eng_math::{Fnv1a64, TAU_F64, Vec3, WorldPos, sin_cos_f64};
use eng_schema_core::{Schema, from_bytes, to_bytes};
use proptest::prelude::*;

/// Rung 2's room hull: 0.5 m x 0.5 m x 1.75 m.
const HULL: BoxHull = BoxHull {
    half_extents: Vec3::new(0.25, 0.25, 0.875),
};
/// The hull's half height: its centre's height above the floor it stands on.
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

/// The 8 m x 6 m x 3 m room: four walls, a floor and a ceiling.
fn room_parts() -> (Vec<Wall>, Vec<Slab>) {
    let walls = vec![
        wall((0.0, 0.0), (8.0, 0.0), 0.0, 3.0),
        wall((8.0, 0.0), (8.0, 6.0), 0.0, 3.0),
        wall((8.0, 6.0), (0.0, 6.0), 0.0, 3.0),
        wall((0.0, 6.0), (0.0, 0.0), 0.0, 3.0),
    ];
    let slabs = vec![
        rect(0.0, 0.0, 8.0, 6.0, -0.5, 0.0),
        rect(0.0, 0.0, 8.0, 6.0, 3.0, 3.5),
    ];
    (walls, slabs)
}

fn room() -> Grid {
    let (walls, slabs) = room_parts();
    Grid::new(walls, slabs).unwrap()
}

#[test]
fn a_hit_on_a_wall_on_an_axis_stops_exactly_on_it() {
    let world = room();
    let (start, end) = (at(7.0, 3.0, H), at(9.0, 3.0, H));
    let tr = world.trace(start, end, &HULL);
    assert_eq!(tr.fraction, 0.375);
    assert_eq!(tr.end, at(7.75, 3.0, H));
    assert_eq!(tr.normal, Vec3::new(-1.0, 0.0, 0.0));
    conformance::check(&world, start, end, &HULL);
}

#[test]
fn a_falling_hull_lands_exactly_on_the_floor() {
    let world = room();
    let tr = world.trace(at(4.0, 3.0, 2.0), at(4.0, 3.0, 0.0), &HULL);
    assert_eq!(tr.fraction, 0.5625);
    assert_eq!(tr.end, at(4.0, 3.0, H));
    assert_eq!(tr.normal, Vec3::Z);
}

#[test]
fn touching_is_free_and_sliding_along_the_floor_does_not_catch() {
    let world = room();
    let on = at(4.0, 3.0, H);
    assert!(!world.overlaps(on, &HULL));
    assert!(world.overlaps(at(4.0, 3.0, H.next_down()), &HULL));
    let along = world.trace(on, at(6.0, 4.0, H), &HULL);
    assert!(!along.hit() && !along.start_solid, "{along:?}");
    // kit-fps's ground probe hits at once and moves nothing.
    let probe = world.trace(on, at(4.0, 3.0, H - 1.0 / 1024.0), &HULL);
    assert_eq!(
        (probe.fraction, probe.end, probe.normal),
        (0.0, on, Vec3::Z)
    );
}

#[test]
fn a_corner_hit_touches_both_walls_and_ties_go_to_the_lower_index() {
    let (start, end) = (at(7.0, 5.0, H), at(9.0, 7.0, H));
    let tr = room().trace(start, end, &HULL);
    assert_eq!(tr.end, at(7.75, 5.75, H));
    assert_eq!(tr.normal, Vec3::new(-1.0, 0.0, 0.0), "wall 1 before wall 2");
    let (mut walls, slabs) = room_parts();
    walls.swap(1, 2);
    let swapped = Grid::new(walls, slabs).unwrap();
    let tr = swapped.trace(start, end, &HULL);
    assert_eq!(tr.end, at(7.75, 5.75, H));
    assert_eq!(tr.normal, Vec3::new(0.0, -1.0, 0.0));
    conformance::check(&room(), start, end, &HULL);
    conformance::check(&swapped, start, end, &HULL);
}

#[test]
fn a_wall_off_the_axes_stops_the_hull_within_the_contact_gap() {
    let (walls, mut slabs) = room_parts();
    slabs.truncate(1);
    let mut walls = walls;
    walls.push(wall((6.0, 0.0), (0.0, 6.0), 0.0, 3.0));
    let world = Grid::new(walls, slabs).unwrap();
    let (start, end) = (at(1.0, 1.5, H), at(4.0, 3.0, H));
    let tr = world.trace(start, end, &HULL);
    assert!(tr.hit(), "{tr:?}");
    let n = core::f32::consts::FRAC_1_SQRT_2;
    assert_eq!(tr.normal, Vec3::new(-n, -n, 0.0));
    // The hull's corner nearest the wall is on it: x + y = 6 there.
    let corner = (tr.end.x + 0.25) + (tr.end.y + 0.25);
    assert!((corner - 6.0).abs() < 1e-12, "{tr:?}");
    conformance::check(&world, start, end, &HULL);
}

#[test]
fn a_slab_blocks_from_its_sides_and_holds_from_above() {
    let (walls, mut slabs) = room_parts();
    slabs.push(rect(4.0, 0.0, 6.0, 6.0, 0.0, 0.4));
    let world = Grid::new(walls, slabs).unwrap();
    let tr = world.trace(at(3.0, 3.0, H), at(5.0, 3.0, H), &HULL);
    assert_eq!(
        (tr.end, tr.normal),
        (at(3.75, 3.0, H), Vec3::new(-1.0, 0.0, 0.0))
    );
    let top = at(5.0, 3.0, 0.4 + H);
    assert!(!world.overlaps(top, &HULL));
    let probe = world.trace(top, at(5.0, 3.0, 0.0), &HULL);
    assert_eq!(
        (probe.fraction, probe.end, probe.normal),
        (0.0, top, Vec3::Z)
    );
    // Just above the slab, the hull clears it.
    let over = world.trace(top, at(3.0, 3.0, 0.4 + H), &HULL);
    assert!(!over.hit(), "{over:?}");
}

#[test]
fn a_slab_edge_off_the_axes_blocks_within_the_contact_gap() {
    let (walls, mut slabs) = room_parts();
    slabs.push(slab(&[(4.0, 1.0), (6.0, 3.0), (4.0, 5.0)], 0.0, 2.0));
    let world = Grid::new(walls, slabs).unwrap();
    let (start, end) = (at(6.5, 4.5, H), at(4.5, 2.5, H));
    let tr = world.trace(start, end, &HULL);
    assert!(tr.hit() && tr.normal.z == 0.0, "{tr:?}");
    assert!(tr.normal.x > 0.0 && tr.normal.y > 0.0, "{tr:?}");
    conformance::check(&world, start, end, &HULL);
}

#[test]
fn a_wall_top_holds_a_hull_like_a_floor() {
    let world = Grid::new(vec![wall((2.0, 0.0), (2.0, 6.0), 0.0, 0.3)], vec![]).unwrap();
    let tr = world.trace(at(2.1, 3.0, 2.0), at(2.1, 3.0, 0.0), &HULL);
    assert_eq!((tr.end, tr.normal), (at(2.1, 3.0, 0.3 + H), Vec3::Z));
}

#[test]
fn a_start_inside_solid_does_not_move() {
    let tr = room().trace(at(4.0, 3.0, 0.5), at(5.0, 3.0, 0.5), &HULL);
    assert!(tr.start_solid);
    assert_eq!((tr.fraction, tr.end), (0.0, at(4.0, 3.0, 0.5)));
}

#[test]
fn an_empty_grid_blocks_nothing() {
    let world = Grid::new(vec![], vec![]).unwrap();
    assert_eq!(world.cells(), (0, 0));
    let tr = world.trace(at(0.0, 0.0, 0.0), at(5.0, -3.0, 2.0), &HULL);
    assert!(!tr.hit() && tr.end == at(5.0, -3.0, 2.0));
    assert!(!world.overlaps(at(0.0, 0.0, 0.0), &HULL));
}

#[test]
fn the_cells_are_whole_meters_around_the_primitives() {
    let world = room();
    assert_eq!(world.cells(), (9, 7), "x 0..=8, y 0..=6");
    let world = Grid::new(vec![wall((-0.5, 2.25), (3.5, 2.25), 0.0, 1.0)], vec![]).unwrap();
    assert_eq!(world.cells(), (5, 1), "x -1..=3, y 2");
}

#[test]
fn new_refuses_what_it_cannot_sweep() {
    let ok = wall((0.0, 0.0), (1.0, 0.0), 0.0, 1.0);
    let refuse = |walls: Vec<Wall>, slabs: Vec<Slab>| Grid::new(walls, slabs).unwrap_err();
    let far = wall((0.0, 0.0), (MAX_COORD * 2.0, 0.0), 0.0, 1.0);
    let nan = wall((0.0, f64::NAN), (1.0, 0.0), 0.0, 1.0);
    let upside = wall((0.0, 0.0), (1.0, 0.0), 1.0, 0.0);
    let point = wall((1.0, 1.0), (1.0, 1.0), 0.0, 1.0);
    assert_eq!(
        refuse(vec![ok, far], vec![]),
        GridError::Coordinate { primitive: 1 }
    );
    assert_eq!(
        refuse(vec![nan], vec![]),
        GridError::Coordinate { primitive: 0 }
    );
    assert_eq!(
        refuse(vec![upside], vec![]),
        GridError::Heights { primitive: 0 }
    );
    assert_eq!(
        refuse(vec![point], vec![]),
        GridError::ZeroLength { primitive: 0 }
    );
    let two = slab(&[(0.0, 0.0), (1.0, 0.0)], 0.0, 0.0);
    let line = slab(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], 0.0, 0.0);
    let dent = slab(
        &[(0.0, 0.0), (2.0, 0.0), (1.0, 0.5), (2.0, 2.0), (0.0, 2.0)],
        0.0,
        0.0,
    );
    let star: Vec<(f64, f64)> = (0..5)
        .map(|i| {
            let (s, c) = sin_cos_f64(TAU_F64 * f64::from(i * 2 % 5) / 5.0);
            (c, s)
        })
        .collect();
    for (bad, what) in [
        (two, "two corners"),
        (line, "collinear"),
        (dent, "a dent"),
        (slab(&star, 0.0, 0.0), "a star"),
    ] {
        assert_eq!(
            refuse(vec![ok], vec![bad]),
            GridError::NotConvex { primitive: 1 },
            "{what}"
        );
    }
    let huge = rect(0.0, 0.0, 4096.0, 4096.0, 0.0, 0.0);
    assert!(matches!(
        refuse(vec![], vec![huge]),
        GridError::TooLarge { .. }
    ));
    let both_windings = [
        rect(0.0, 0.0, 1.0, 1.0, 0.0, 0.0),
        slab(&[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)], 0.0, 0.0),
    ];
    assert!(Grid::new(vec![], both_windings.to_vec()).is_ok());
}

#[test]
fn the_grid_is_schema_v1_and_round_trips() {
    assert_eq!((Grid::NAME, Grid::VERSION), ("eng.collision.grid.Grid", 1));
    assert_eq!((Wall::NAME, Wall::VERSION), ("eng.collision.grid.Wall", 1));
    assert_eq!((Slab::NAME, Slab::VERSION), ("eng.collision.grid.Slab", 1));
    assert_eq!(
        (Point2::NAME, Point2::VERSION),
        ("eng.collision.grid.Point2", 1)
    );
    assert_eq!(to_bytes(&wall((0.0, 0.0), (1.0, 0.0), 0.0, 1.0)).len(), 48);
    let world = room();
    let bytes = to_bytes(&world);
    let back: Grid = from_bytes(&bytes).unwrap();
    assert_eq!(back, world);
    assert_eq!(back.walls(), world.walls());
    // The room's encoding, index included: a change here changes every
    // map's sim-data hash, and the index's layout must bump Grid's version.
    assert_eq!(
        (bytes.len(), Fnv1a64::hash(&bytes)),
        (1728, 0xdd95_33c7_114f_73d2),
        "the room's encoding changed"
    );
}

/// A map of walls and slabs, on and off the axes, about 12 m x 9 m.
fn map() -> impl Strategy<Value = Grid> {
    let coord = |hi: f64| {
        prop_oneof![
            (0..=(hi as u32 * 4)).prop_map(|q| f64::from(q) / 4.0),
            0.0..hi
        ]
    };
    let z = prop_oneof![Just(0.0), Just(0.3), Just(1.0), 0.0f64..3.0];
    let height = prop_oneof![Just(0.2), Just(0.4), Just(3.0), 0.0f64..3.0];
    let ends = prop_oneof![
        (coord(12.0), coord(9.0), coord(12.0)).prop_map(|(x0, y, x1)| ((x0, y), (x1, y))),
        (coord(12.0), coord(9.0), coord(9.0)).prop_map(|(x, y0, y1)| ((x, y0), (x, y1))),
        ((coord(12.0), coord(9.0)), (coord(12.0), coord(9.0))),
    ];
    let walls = prop::collection::vec((ends, z.clone(), height.clone()), 0..16).prop_map(|ws| {
        ws.into_iter()
            .filter(|&((a, b), ..)| a != b)
            .map(|((a, b), bottom, h)| wall(a, b, bottom, bottom + h))
            .collect::<Vec<_>>()
    });
    let polygon = prop_oneof![
        (coord(12.0), coord(9.0), 0.25f64..4.0, 0.25f64..4.0)
            .prop_map(|(x, y, w, d)| { vec![(x, y), (x + w, y), (x + w, y + d), (x, y + d)] }),
        (coord(12.0), coord(9.0), 0.3f64..3.0, 3u32..8, 0.0..TAU_F64).prop_map(
            |(x, y, r, k, phase)| {
                (0..k)
                    .map(|i| {
                        let (s, c) = sin_cos_f64(phase + TAU_F64 * f64::from(i) / f64::from(k));
                        (x + r * c, y + r * s)
                    })
                    .collect()
            }
        ),
    ];
    let slabs = prop::collection::vec((polygon, z, height), 0..8).prop_map(|ss| {
        ss.into_iter()
            .map(|(pts, bottom, h)| slab(&pts, bottom, bottom + h))
            .collect::<Vec<_>>()
    });
    (walls, slabs, any::<bool>(), any::<bool>()).prop_map(
        |(mut walls, mut slabs, floor, closed)| {
            if floor {
                slabs.insert(0, rect(-1.0, -1.0, 13.0, 10.0, -0.5, 0.0));
            }
            if closed {
                let corners = [(0.0, 0.0), (12.0, 0.0), (12.0, 9.0), (0.0, 9.0), (0.0, 0.0)];
                for c in corners.windows(2) {
                    walls.push(wall(c[0], c[1], 0.0, 3.0));
                }
            }
            Grid::new(walls, slabs).unwrap()
        },
    )
}

/// A position: on the quarter-meter lattice, at rest on z = 0 or 0.4, or
/// anywhere.
fn place() -> impl Strategy<Value = WorldPos> {
    let q = |n: u32| (0..=n).prop_map(|i| f64::from(i) / 4.0 - 1.0);
    let x = prop_oneof![q(56), -1.0f64..13.0];
    let y = prop_oneof![q(44), -1.0f64..10.0];
    let z = prop_oneof![Just(H), Just(0.4 + H), 0.0f64..4.0];
    (x, y, z).prop_map(|(x, y, z)| at(x, y, z))
}

/// Where a trace goes from where the last one stopped.
#[derive(Debug, Clone, Copy)]
enum Goal {
    /// By a motion.
    By(f64, f64, f64),
    /// To a place.
    To(WorldPos),
    /// Through the place where a corner of the hull touches a corner of a
    /// primitive (on its top, its bottom or at the same height), and on past
    /// it by `over` times the way there: contacts that round.
    Aim {
        primitive: usize,
        corner: usize,
        signs: (bool, bool),
        height: u8,
        over: f64,
    },
}

impl Goal {
    fn from(self, p: WorldPos, world: &Grid, hull: &BoxHull) -> WorldPos {
        match self {
            Self::By(dx, dy, dz) => at(p.x + dx, p.y + dy, p.z + dz),
            Self::To(q) => q,
            Self::Aim {
                primitive,
                corner,
                signs,
                height,
                over,
            } => {
                let (walls, slabs) = (world.walls(), world.slabs());
                let count = walls.len() + slabs.len();
                if count == 0 {
                    return p;
                }
                let i = primitive % count;
                let (corners, bottom, top) = if i < walls.len() {
                    let w = walls[i];
                    (vec![w.a, w.b], w.bottom, w.top)
                } else {
                    let s = &slabs[i - walls.len()];
                    (s.points.clone(), s.bottom, s.top)
                };
                let v = corners[corner % corners.len()];
                let h = hull.half_extents;
                let (hx, hy, hz) = (f64::from(h.x), f64::from(h.y), f64::from(h.z));
                let x = if signs.0 { v.x - hx } else { v.x + hx };
                let y = if signs.1 { v.y - hy } else { v.y + hy };
                let z = match height {
                    0 => top + hz,
                    1 => bottom - hz,
                    _ => p.z,
                };
                at(
                    x + (x - p.x) * over,
                    y + (y - p.y) * over,
                    z + (z - p.z) * over,
                )
            }
        }
    }
}

/// A motion with each axis still, short or up to 12 m (4 m on Z), a place,
/// or an aimed contact.
fn step() -> impl Strategy<Value = Goal> {
    let d = |r: f64| prop_oneof![Just(0.0), -r..r, -0.1f64..0.1];
    let by = (d(12.0), d(12.0), d(4.0)).prop_map(|(x, y, z)| Goal::By(x, y, z));
    let over = prop_oneof![Just(0.0), 0.0f64..1.5];
    let aim = (
        any::<usize>(),
        any::<usize>(),
        any::<(bool, bool)>(),
        0u8..3,
        over,
    )
        .prop_map(|(primitive, corner, signs, height, over)| Goal::Aim {
            primitive,
            corner,
            signs,
            height,
            over,
        });
    prop_oneof![3 => by, 2 => place().prop_map(Goal::To), 3 => aim]
}

/// How far inside solid a point on the way may be (m): the hull shrunk by
/// this on every side must stay free on the traced segment before the hit,
/// at `SAMPLES` evenly spaced points from the start to the hit fraction.
/// Grazes (`GRAZE`), slides (`NOISE`) and slivers go far less deep.
const THROUGH: f32 = 1.0e-6;
const SAMPLES: u32 = 32;

/// Checks each trace of a chain from `start`, each from where the last
/// stopped, often in contact: the conformance suite, and no pass-through.
fn chain(world: &Grid, start: WorldPos, steps: &[Goal], hull: &BoxHull) {
    let h = hull.half_extents;
    let shrunk = BoxHull {
        half_extents: Vec3::new(h.x - THROUGH, h.y - THROUGH, h.z - THROUGH),
    };
    let mut from = start;
    for goal in steps {
        let to = goal.from(from, world, hull);
        conformance::check(world, from, to, hull);
        let tr = world.trace(from, to, hull);
        for i in 0..=SAMPLES {
            let t = tr.fraction * f64::from(i) / f64::from(SAMPLES);
            let p = at(
                from.x + (to.x - from.x) * t,
                from.y + (to.y - from.y) * t,
                from.z + (to.z - from.z) * t,
            );
            assert!(
                !world.overlaps(p, &shrunk),
                "the trace from {from:?} to {to:?} ({tr:?}) passes through solid at {p:?}"
            );
        }
        from = tr.end;
    }
}

proptest! {
    // At least 1,024 cases (more with PROPTEST_CASES); starts inside solid
    // are rejected.
    #![proptest_config(ProptestConfig {
        cases: ProptestConfig::default().cases.max(1024),
        max_global_rejects: 1 << 22,
        ..ProptestConfig::default()
    })]

    #[test]
    fn the_room_keeps_the_contract(start in place(), steps in prop::collection::vec(step(), 1..6)) {
        let world = room();
        prop_assume!(!world.overlaps(start, &HULL));
        chain(&world, start, &steps, &HULL);
    }

    #[test]
    fn random_maps_keep_the_contract(
        world in map(),
        start in place(),
        steps in prop::collection::vec(step(), 1..6),
        half in prop_oneof![Just((0.25f32, 0.875f32)), (0.05f32..1.0, 0.05f32..1.5)],
    ) {
        let hull = BoxHull { half_extents: Vec3::new(half.0, half.0, half.1) };
        prop_assume!(!world.overlaps(start, &hull));
        chain(&world, start, &steps, &hull);
    }
}
