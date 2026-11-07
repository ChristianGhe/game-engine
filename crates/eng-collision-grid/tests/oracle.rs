//! The brute-force oracle (rung 2, decision 2 and criterion 3): every
//! primitive, no grid. It follows the rules in `eng_collision_grid`'s crate
//! docs from the walls and slabs alone, and the grid must give its `Trace`
//! bits and its `overlaps` on proptest maps and sweeps. This file and
//! `oracle/brute.rs` are protected from gate G5 on.

#[path = "oracle/brute.rs"]
mod brute;

use brute::Brute;
use eng_collision_api::{BoxHull, CollisionWorld, Trace, conformance};
use eng_collision_grid::{Grid, Point2, Slab, Wall};
use eng_math::{TAU_F64, Vec3, WorldPos, sin_cos_f64};
use proptest::prelude::*;

fn bits(t: &Trace) -> ([u64; 4], [u32; 3], bool) {
    let (e, n) = (t.end, t.normal);
    (
        [
            t.fraction.to_bits(),
            e.x.to_bits(),
            e.y.to_bits(),
            e.z.to_bits(),
        ],
        [n.x.to_bits(), n.y.to_bits(), n.z.to_bits()],
        t.start_solid,
    )
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

const HULL: BoxHull = BoxHull {
    half_extents: Vec3::new(0.25, 0.25, 0.875),
};

#[test]
fn the_oracle_stops_on_a_wall_on_an_axis_where_the_room_does() {
    let walls = [wall((8.0, -1.0), (8.0, 7.0), 0.0, 3.0)];
    let floor = [slab(
        &[(0.0, 0.0), (8.0, 0.0), (8.0, 6.0), (0.0, 6.0)],
        -0.5,
        0.0,
    )];
    let brute = Brute::new(&walls, &floor);
    let tr = brute.trace(
        WorldPos::new(7.0, 3.0, 0.875),
        WorldPos::new(9.0, 3.0, 0.875),
        &HULL,
    );
    assert_eq!(
        (tr.fraction, tr.end.x, tr.normal),
        (0.375, 7.75, Vec3::new(-1.0, 0.0, 0.0))
    );
    let land = brute.trace(
        WorldPos::new(4.0, 3.0, 2.0),
        WorldPos::new(4.0, 3.0, 0.0),
        &HULL,
    );
    assert_eq!((land.end.z, land.normal), (0.875, Vec3::Z));
}

/// A map: walls and slabs on and off the axes, on a quarter-meter lattice
/// or anywhere, in a `size` x `size` square from `-size / 4`.
fn map(size: f64, walls: usize, slabs: usize) -> impl Strategy<Value = (Vec<Wall>, Vec<Slab>)> {
    let lo = -size / 4.0;
    let steps = (size * 4.0) as u32;
    let coord = move || {
        prop_oneof![
            (0..=steps).prop_map(move |q| lo + f64::from(q) / 4.0),
            lo..lo + size
        ]
    };
    let z = || prop_oneof![Just(0.0), Just(0.4), Just(1.0), -1.0f64..3.0];
    let height = || prop_oneof![Just(0.0), Just(0.2), Just(0.4), Just(3.0), 0.0f64..3.0];
    let point = move || (coord(), coord());
    let ends = prop_oneof![
        (coord(), coord(), coord()).prop_map(|(x0, y, x1)| ((x0, y), (x1, y))),
        (coord(), coord(), coord()).prop_map(|(x, y0, y1)| ((x, y0), (x, y1))),
        (point(), point()),
    ];
    let walls = prop::collection::vec((ends, z(), height()), 0..=walls).prop_map(|ws| {
        ws.into_iter()
            .filter(|((a, b), ..)| a != b)
            .map(|((a, b), bottom, h)| wall(a, b, bottom, bottom + h))
            .collect::<Vec<_>>()
    });
    let polygon = prop_oneof![
        (point(), 0.25f64..5.0, 0.25f64..5.0).prop_map(|((x, y), w, d)| vec![
            (x, y),
            (x + w, y),
            (x + w, y + d),
            (x, y + d)
        ]),
        (point(), 0.3f64..4.0, 3u32..9, 0.0..TAU_F64).prop_map(|((x, y), r, k, phase)| {
            (0..k)
                .map(|i| {
                    let (s, c) = sin_cos_f64(phase + TAU_F64 * f64::from(i) / f64::from(k));
                    (x + r * c, y + r * s)
                })
                .collect()
        }),
        (point(), 0.25f64..3.0).prop_map(|((x, y), r)| vec![
            (x, y - r),
            (x + r, y),
            (x, y + r),
            (x - r, y)
        ]),
    ];
    let slabs = prop::collection::vec((polygon, z(), height()), 0..=slabs).prop_map(|ss| {
        ss.into_iter()
            .map(|(pts, bottom, h)| slab(&pts, bottom, bottom + h))
            .collect::<Vec<_>>()
    });
    (walls, slabs)
}

/// A position in or around a map from `-size / 4`: on the quarter-meter
/// lattice, at rest at a usual height, or anywhere.
fn place(size: f64) -> impl Strategy<Value = WorldPos> {
    let lo = -size / 4.0 - 1.0;
    let steps = ((size + 2.0) * 4.0) as u32;
    let c = move || {
        prop_oneof![
            (0..=steps).prop_map(move |q| lo + f64::from(q) / 4.0),
            lo..lo + size + 2.0
        ]
    };
    let z = prop_oneof![Just(0.875), Just(1.275), Just(1.875), -1.0f64..5.0];
    (c(), c(), z).prop_map(|(x, y, z)| WorldPos::new(x, y, z))
}

/// Where a trace goes from where the last one stopped.
#[derive(Debug, Clone, Copy)]
enum Step {
    By(f64, f64, f64),
    To(WorldPos),
    /// Through the place where a corner of the hull touches a corner of
    /// primitive `primitive` (on its top, its bottom or at the same height),
    /// and on past it by `over` times the way there: contacts that round.
    Aim {
        primitive: usize,
        corner: usize,
        signs: (bool, bool),
        height: u8,
        over: f64,
    },
}

impl Step {
    fn from(self, p: WorldPos, walls: &[Wall], slabs: &[Slab], hull: &BoxHull) -> WorldPos {
        match self {
            Self::By(dx, dy, dz) => WorldPos::new(p.x + dx, p.y + dy, p.z + dz),
            Self::To(q) => q,
            Self::Aim {
                primitive,
                corner,
                signs,
                height,
                over,
            } => {
                let count = walls.len() + slabs.len();
                if count == 0 {
                    return p;
                }
                let i = primitive % count;
                let (corners, bottom, top) = match walls.get(i) {
                    Some(w) => (vec![w.a, w.b], w.bottom, w.top),
                    None => {
                        let s = &slabs[i - walls.len()];
                        (s.points.clone(), s.bottom, s.top)
                    }
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
                WorldPos::new(
                    x + (x - p.x) * over,
                    y + (y - p.y) * over,
                    z + (z - p.z) * over,
                )
            }
        }
    }
}

/// A motion with each axis still, short, a quarter meter, or up to `reach`;
/// a place in the map; or an aimed contact.
fn step(size: f64, reach: f64) -> impl Strategy<Value = Step> {
    let d = move |r: f64| prop_oneof![Just(0.0), -r..r, -0.1f64..0.1, Just(0.25), Just(-0.25)];
    let over = prop_oneof![Just(0.0), 0.0f64..1.5];
    let aim = (
        any::<usize>(),
        any::<usize>(),
        any::<(bool, bool)>(),
        0u8..3,
        over,
    )
        .prop_map(|(primitive, corner, signs, height, over)| Step::Aim {
            primitive,
            corner,
            signs,
            height,
            over,
        });
    prop_oneof![
        3 => (d(reach), d(reach), d(4.0)).prop_map(|(x, y, z)| Step::By(x, y, z)),
        2 => place(size).prop_map(Step::To),
        3 => aim,
    ]
}

fn hull() -> impl Strategy<Value = BoxHull> {
    prop_oneof![
        Just(HULL),
        (0.05f32..1.5, 0.05f32..1.5, 0.05f32..1.5).prop_map(|(x, y, z)| BoxHull {
            half_extents: Vec3::new(x, y, z)
        }),
    ]
}

/// The first of `starts` where the hull is free, or else the first.
fn free_start(brute: &Brute, starts: &[WorldPos], hull: &BoxHull) -> WorldPos {
    let free = starts.iter().find(|&&p| !brute.overlaps(p, hull));
    *free.unwrap_or(&starts[0])
}

/// Chains traces from `start` through `steps`, each from where the last
/// stopped, and compares every trace, and `overlaps` at both ends and the
/// stop, with the oracle's.
fn agrees(
    (walls, slabs): (&[Wall], &[Slab]),
    starts: &[WorldPos],
    steps: &[Step],
    hull: &BoxHull,
) -> Result<(), TestCaseError> {
    let grid = Grid::new(walls.to_vec(), slabs.to_vec()).unwrap();
    let brute = Brute::new(walls, slabs);
    let mut from = free_start(&brute, starts, hull);
    for step in steps {
        let to = step.from(from, walls, slabs, hull);
        let (got, want) = (grid.trace(from, to, hull), brute.trace(from, to, hull));
        prop_assert_eq!(
            bits(&got),
            bits(&want),
            "from {:?} to {:?}: {:?} vs {:?}",
            from,
            to,
            got,
            want
        );
        for p in [from, to, got.end] {
            prop_assert_eq!(
                grid.overlaps(p, hull),
                brute.overlaps(p, hull),
                "overlaps at {:?}",
                p
            );
        }
        from = got.end;
    }
    Ok(())
}

proptest! {
    // At least 1,024 cases (more with PROPTEST_CASES). Starts inside solid
    // are rejected only by the contract test, which needs a free start;
    // the others try up to three starts for a free one, and compare
    // `start_solid` traces too.
    #![proptest_config(ProptestConfig {
        cases: ProptestConfig::default().cases.max(1024),
        max_global_rejects: 1 << 22,
        ..ProptestConfig::default()
    })]

    #[test]
    fn the_grid_gives_the_oracles_trace_bits_in_a_room(
        (walls, slabs) in map(12.0, 12, 6),
        floor in any::<bool>(),
        starts in prop::collection::vec(place(12.0), 1..4),
        steps in prop::collection::vec(step(12.0, 12.0), 1..8),
        hull in hull(),
    ) {
        let mut slabs = slabs;
        if floor {
            slabs.insert(0, slab(&[(-4.0, -4.0), (16.0, -4.0), (16.0, 16.0), (-4.0, 16.0)], -0.5, 0.0));
        }
        agrees((&walls, &slabs), &starts, &steps, &hull)?;
    }

    #[test]
    fn the_grid_gives_the_oracles_trace_bits_when_crowded(
        (walls, slabs) in map(5.0, 40, 16),
        starts in prop::collection::vec(place(5.0), 1..4),
        steps in prop::collection::vec(step(5.0, 3.0), 1..12),
        hull in hull(),
    ) {
        agrees((&walls, &slabs), &starts, &steps, &hull)?;
    }

    #[test]
    fn the_grid_gives_the_oracles_trace_bits_over_many_cells(
        (walls, slabs) in map(64.0, 24, 12),
        starts in prop::collection::vec(place(64.0), 1..4),
        steps in prop::collection::vec(step(64.0, 80.0), 1..6),
        hull in hull(),
    ) {
        agrees((&walls, &slabs), &starts, &steps, &hull)?;
    }

    #[test]
    fn the_oracle_keeps_the_contract(
        (walls, slabs) in map(12.0, 8, 4),
        start in place(12.0),
        steps in prop::collection::vec(step(12.0, 12.0), 1..5),
        hull in hull(),
    ) {
        let brute = Brute::new(&walls, &slabs);
        prop_assume!(!brute.overlaps(start, &hull));
        let mut from = start;
        for step in steps {
            let to = step.from(from, &walls, &slabs, &hull);
            conformance::check(&brute, from, to, &hull);
            from = brute.trace(from, to, &hull).end;
        }
    }
}
