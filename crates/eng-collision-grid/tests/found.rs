//! Traces found by searching against cargo-mutants' mutants of the sweep
//! (rung 2, T19 follow-up): the grid's own proptests run with 128 times
//! their cases (`PROPTEST_CASES=131072`, seed 1), and an aimed search of
//! hull corners at slab and wall corners (up to 4 million cases). Each is the
//! trace a shrunk failure stopped on, with the primitives near it: a rule of
//! the crate docs, mostly about rounding at edges and corners, that the
//! proptests reach only rarely at their usual size. Every case, and its
//! mirror image, is checked with the conformance suite and for no
//! pass-through (as `tests/contract.rs` checks), from a free start; those an
//! oracle search found are also checked against the brute-force oracle
//! (`tests/oracle.rs`). Case 3's mirror image broke the contact rule, in the
//! grid and the oracle alike, until a still axis entered through its nearer
//! face (rung 2, T19b follow-up). Two open breaks of the contact rule, (A)
//! and (B), are checked for what still holds (`OPEN`).

#[path = "oracle/brute.rs"]
mod brute;
#[path = "oracle/through.rs"]
mod through;

use brute::Brute;
use eng_collision_api::{BoxHull, CollisionWorld, Trace, conformance};
use eng_collision_grid::{Grid, Point2, Slab, Wall};
use eng_math::{Vec3, WorldPos};

/// How a case is checked.
enum Check {
    /// Found by an oracle search: the grid's trace bits, and `overlaps` at
    /// both ends and the stop, are also the oracle's.
    Oracle,
    /// Found by a contract search, or made from a found case: the contract
    /// only.
    Contract,
}

/// One trace: walls as `([ax, ay, bx, by], bottom, top)`, slabs as
/// `(corners x0, y0, x1, y1, ..., bottom, top)`, the hull's half extents.
struct Case {
    check: Check,
    walls: &'static [([f64; 4], f64, f64)],
    slabs: &'static [(&'static [f64], f64, f64)],
    hull: [f32; 3],
    from: [f64; 3],
    to: [f64; 3],
}

impl Case {
    /// The primitives, or their mirror image through the origin: every
    /// coordinate negated, which IEEE arithmetic does exactly, and every edge
    /// reversed (a slab's corners in reverse order, a wall's ends swapped),
    /// so that each axis's projections are negated too. The mirror takes
    /// each rule's other branch: a motion up an axis becomes one down it.
    fn parts(&self, mirror: bool) -> (Vec<Wall>, Vec<Slab>) {
        let m = if mirror { -1.0 } else { 1.0 };
        let point = |x: f64, y: f64| Point2::new(m * x, m * y);
        let heights = |bottom: f64, top: f64| {
            if mirror {
                (-top, -bottom)
            } else {
                (bottom, top)
            }
        };
        let walls = self
            .walls
            .iter()
            .map(|&([ax, ay, bx, by], bottom, top)| {
                let (a, b) = (point(ax, ay), point(bx, by));
                let (bottom, top) = heights(bottom, top);
                let (a, b) = if mirror { (b, a) } else { (a, b) };
                Wall { a, b, bottom, top }
            })
            .collect();
        let slabs = self
            .slabs
            .iter()
            .map(|&(corners, bottom, top)| {
                let mut points: Vec<Point2> =
                    corners.chunks(2).map(|c| point(c[0], c[1])).collect();
                if mirror {
                    points.reverse();
                }
                let (bottom, top) = heights(bottom, top);
                Slab {
                    points,
                    bottom,
                    top,
                }
            })
            .collect();
        (walls, slabs)
    }

    fn ends(&self, mirror: bool) -> (WorldPos, WorldPos) {
        let m = if mirror { -1.0 } else { 1.0 };
        let pos = |[x, y, z]: [f64; 3]| WorldPos::new(m * x, m * y, m * z);
        (pos(self.from), pos(self.to))
    }
}

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

#[test]
fn the_found_traces_keep_the_rules() {
    for (i, case, mirror) in CASES
        .iter()
        .enumerate()
        .flat_map(|(i, case)| [(i, case, false), (i, case, true)])
    {
        let (walls, slabs) = case.parts(mirror);
        let grid = Grid::new(walls.clone(), slabs.clone()).unwrap();
        let [hx, hy, hz] = case.hull;
        let hull = BoxHull {
            half_extents: Vec3::new(hx, hy, hz),
        };
        let (from, to) = case.ends(mirror);
        let got = grid.trace(from, to, &hull);
        if let Check::Oracle = case.check {
            let brute = Brute::new(&walls, &slabs);
            let want = brute.trace(from, to, &hull);
            assert_eq!(
                bits(&got),
                bits(&want),
                "case {i} (mirror {mirror}): {got:?} vs {want:?}"
            );
            for p in [from, to, got.end] {
                assert_eq!(
                    grid.overlaps(p, &hull),
                    brute.overlaps(p, &hull),
                    "case {i} (mirror {mirror}): overlaps at {p:?}"
                );
            }
        }
        if grid.overlaps(from, &hull) {
            assert!(got.start_solid, "case {i} (mirror {mirror}): {got:?}");
            continue;
        }
        conformance::check(&grid, from, to, &hull);
        through::assert_no_pass_through(&grid, from, to, &got, &hull);
    }
}

/// The two open breaks of ADR-0024's contact rule in step 4 of the crate
/// docs, (A) and (B) of `docs/rungs/2/patches/t19b-mutants-exclusions.md`,
/// each with its mirror image: a hull beside a slab's edge off the axes. The
/// conformance suite fails both, (A) with "one ulp further on is still free"
/// (held on the top, short of the boundary) and (B) with "the rest of the
/// motion does not hit" (stopped with the edge's normal, short of it).
/// Whether to fix the grid or narrow the rule is the owner's (ADR-0024,
/// Amendment 2). What holds, and is checked here: the stop is free, the
/// trace hits, and nothing is passed through. (A)'s end is inside the slab,
/// so any trace must stop; (B)'s is free, so a fix may make (B) a graze
/// instead, and change this test.
#[test]
fn the_open_contact_breaks_stop_free_and_pass_through_nothing() {
    for (i, case, mirror) in OPEN
        .iter()
        .enumerate()
        .flat_map(|(i, case)| [(i, case, false), (i, case, true)])
    {
        let (walls, slabs) = case.parts(mirror);
        let grid = Grid::new(walls, slabs).unwrap();
        let [hx, hy, hz] = case.hull;
        let hull = BoxHull {
            half_extents: Vec3::new(hx, hy, hz),
        };
        let (from, to) = case.ends(mirror);
        let what = if i == 0 { "(A)" } else { "(B)" };
        assert!(!grid.overlaps(from, &hull), "{what} (mirror {mirror})");
        assert_eq!(grid.overlaps(to, &hull), i == 0, "{what} (mirror {mirror})");
        let tr = grid.trace(from, to, &hull);
        assert!(
            tr.hit() && !tr.start_solid,
            "{what} (mirror {mirror}): {tr:?}"
        );
        assert!(
            !grid.overlaps(tr.end, &hull),
            "{what} (mirror {mirror}): {tr:?} ends inside solid"
        );
        through::assert_no_pass_through(&grid, from, to, &tr, &hull);
        assert_eq!(grid.trace(from, to, &hull), tr, "{what} (mirror {mirror})");
    }
}

/// (A), then (B), as found (the slabs from z 0).
#[rustfmt::skip]
const OPEN: &[Case] = &[
    Case {
        check: Check::Contract,
        walls: &[],
        slabs: &[
            (&[0.8342246031076089, 0.22523066479828902, -0.5587184072985766, 0.5130715888515152, -1.1209500218753434, -1.661382543707628, 0.2649472301686788, -0.6111264719388632], 0.0, 0.7545323315703691),
        ],
        hull: [0.26983458, 0.14706548, 0.72871894],
        from: [-1.1284023191669874, -0.49954267624878423, 1.684434015645078],
        to: [-1.2618060870558636, -1.0154872840786453, 1.007474930349261],
    },
    Case {
        check: Check::Contract,
        walls: &[],
        slabs: &[
            (&[1.8083400316838976, 2.7251160814529642, -0.35575512839208945, 2.2516321346073775, -0.11336571303549503, 1.0940975268012743, 1.710195340442799, 1.3599845674512399], 0.0, 1.4747776309330871),
        ],
        hull: [0.9827255, 0.48495227, 0.99043757],
        from: [-1.2732884835113105, 1.4553536667774594, 2.750962695091877],
        to: [-1.0899426147761797, 0.5797825208421452, 2.4121203546379055],
    },
];

/// The cases, each under the mutant it was found against; one primitive a line.
#[rustfmt::skip]
const CASES: &[Case] = &[
    // Found against `delete ! in settle` (grid.rs, column 16).
    Case {
        check: Check::Contract,
        walls: &[
            ([0.0, 9.0, 0.0, 0.0], 0.0, 3.0),
        ],
        slabs: &[
            (&[1.4572902421833274, 2.2858279021256998, 2.9699811632598188, 5.6320076969989845, -0.6842410052688932, 5.268946565288666], 0.0, 3.0),
        ],
        hull: [0.16390537, 0.16390537, 0.05],
        from: [0.5901069945387071, 3.1015772212633457, 0.938846596875227],
        to: [-0.8481463725241041, 5.105041198033455, 3.050000000745058],
    },
    // Found against `replace || with && in crossing` (sweep.rs, column 23).
    Case {
        check: Check::Oracle,
        walls: &[
            ([16.49845749469758, 5.75, 38.984579797148264, 28.0], 0.0, 0.2),
            ([-2.75, 28.25, 29.417393913255108, -6.235541551003853], 0.0, 0.2),
        ],
        slabs: &[],
        hull: [1.4377105, 1.0083271, 0.96123964],
        from: [39.88836385007174, 26.46335260331952, 0.875],
        to: [-13.56307314355928, -26.426800362343375, 1.5719653506652131],
    },
    // Found against `replace < with == in crossing` (sweep.rs, column 27).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[-1.25, 0.75, -1.0, 0.75, -1.0, 1.0, -1.25, 1.0], 0.0, 0.0),
            (&[-1.25, -1.25, -1.0, -1.25, -1.0, -1.0, -1.25, -1.0], 0.0, 0.0),
            (&[-1.25, 0.6508098869617007, -1.0, 0.6508098869617007, -1.0, 0.9008098869617007, -1.25, 0.9008098869617007], 0.0, 0.0),
            (&[-1.25, 0.5, -1.0, 0.5, -1.0, 0.75, -1.25, 0.75], 0.0, 0.0),
            (&[-0.75, 0.25, -0.5, 0.25, -0.5, 0.5, -0.75, 0.5], 0.0, 0.0),
            (&[-1.8097760338372668, 0.826798996049579, 0.6913667967971897, 0.11612986044427798, 2.806427348524426, 1.628507109037833, 2.942721886521245, 4.225079826765639, 0.9976178437741373, 5.950575796747891, -1.5641817626367132, 5.505661359884639, -2.813589577895371, 3.2253661617344696], 0.0, 0.2),
            (&[1.25, -1.0, 1.5, -1.0, 1.5, -0.75, 1.25, -0.75], 0.0, 0.0),
        ],
        hull: [0.25, 0.25, 0.875],
        from: [6.343163391263422, -4.611787929612978, 0.0],
        to: [-2.0597760338372666, 0.576798996049579, 0.0],
    },
    // Found against `replace < with <= in crossing` (sweep.rs, column 17).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[8.304585504016694, 3.2560200834970097, 10.048565420519685, 5.0, 8.304585504016694, 6.74397991650299, 6.560605587513704, 5.0], 0.0, 0.0),
        ],
        hull: [0.25, 0.25, 0.875],
        from: [8.054585504016694, 3.0060200834970097, 0.875],
        to: [7.8045855040166945, 3.2560200834970097, 0.8166412052730473],
    },
    // Found against `replace || with && in crossing` (sweep.rs, column 23).
    Case {
        check: Check::Oracle,
        walls: &[
            ([-1.25, -0.6943364239543638, 5.750871913866252, 5.5], 0.0, 3.0),
        ],
        slabs: &[
            (&[2.764367116201692, 5.720404268231386, 5.329232810564884, 8.285269962594578, 2.764367116201692, 10.85013565695777, 0.1995014218384994, 8.285269962594578], 1.0, 1.0),
        ],
        hull: [0.7704824, 0.7853425, 0.87611455],
        from: [0.9040599426519782, 2.6786265518537, 1.275],
        to: [6.006223570502053, 7.192995366650307, 1.275],
    },
    // Found against `replace < with <= in sweep` (sweep.rs, column 20).
    Case {
        check: Check::Oracle,
        walls: &[
            ([0.25, -3.0, 0.08631871266407519, -2.25], 1.0, 1.4),
        ],
        slabs: &[],
        hull: [0.05, 1.2130858, 0.05],
        from: [-0.21368128808098286, -2.8165546237050867, 1.43982964214573],
        to: [0.036318711919017144, -2.8165546237050867, 1.43982964214573],
    },
    // Found against, as one case:
    // - `delete ! in sweep` (sweep.rs, column 23)
    // - `replace > with == in sweep` (sweep.rs, column 35)
    Case {
        check: Check::Contract,
        walls: &[
            ([8.0, 0.0, 8.0, 6.0], 0.0, 3.0),
            ([8.0, 6.0, 0.0, 6.0], 0.0, 3.0),
        ],
        slabs: &[
            (&[0.0, 0.0, 8.0, 0.0, 8.0, 6.0, 0.0, 6.0], -0.5, 0.0),
            (&[0.0, 0.0, 8.0, 0.0, 8.0, 6.0, 0.0, 6.0], 3.0, 3.5),
        ],
        hull: [0.25, 0.25, 0.875],
        from: [9.077929950978824, 12.353417671260626, 0.875],
        to: [7.444551641610453, 0.31231440465383287, 6.79353806268502],
    },
    // Found against, as one case:
    // - `replace * with + in edge_at_entry` (sweep.rs, column 77)
    // - `replace * with / in edge_at_entry` (sweep.rs, column 77)
    // - `replace * with + in edge_at_entry` (sweep.rs, column 85)
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[2.9513648883589796, -0.25, -1.4756824441794891, 2.3059569691563, -1.4756824441794911, -2.8059569691562993], 0.0, 0.0),
        ],
        hull: [0.25, 0.25, 0.875],
        from: [3.2013648883589796, -0.5, -0.875],
        to: [2.9513648883589796, -0.5, -0.625],
    },
    // Found against `replace && with || in edge_at_entry` (sweep.rs, column 51).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[9.0, -0.25, 9.25, 0.0, 9.0, 0.25, 8.75, 0.0], 0.0, 0.0),
        ],
        hull: [1.4339308, 1.4739423, 1.3004942],
        from: [-3.25, 48.80779840237807, -0.5635770717191444],
        to: [21.060098443467737, -34.838720858737, -0.5635770717191444],
    },
    // Found against `replace && with || in edge_at_entry` (sweep.rs, column 46).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[23.238301380018598, 2.311829211608833, 20.529467437134155, 1.4165608529359566, 20.543842520191873, -1.4363464573618705, 23.261560752997084, -2.3042717832060946, 24.92682790965829, 0.012228176023174893], 0.0, 0.0),
        ],
        hull: [0.25, 0.25, 0.875],
        from: [20.293842520191873, -1.6863464573618705, 0.875],
        to: [20.293842520191873, -1.6694008139133067, 0.625],
    },
    // Found against `replace + with - in edge_at_entry` (sweep.rs, column 70).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[-3.3095987052198685, 0.2942261561973085, -0.6638175192205131, 0.32862324934004494, 0.102123407689382, 1.1961099405432905], 0.0, 0.2),
        ],
        hull: [0.985622, 0.05, 0.05],
        from: [0.3218044695528328, 0.278623248594987, -0.05000000074505806],
        to: [0.3294638788219318, 0.28729811550701945, -0.04750000074505806],
    },
    // Found against `replace * with / in edge_at_entry` (sweep.rs, column 85).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[2.0202635149234154, -0.3303109181356927, 1.874465937057782, -0.13897512945549306, -1.9148962540106778, -0.18823961320323557, 0.9674129059060844, -3.970805170232505], 0.0, 0.2),
        ],
        hull: [0.25, 0.25, 0.875],
        from: [1.2174129059060845, -4.220805170232503, 1.0750000000000004],
        to: [1.227941411996258, -4.184400227711535, 1.0732482741282932],
    },
    // Found against `replace < with <= in ulp_towards` (sweep.rs, column 24).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[2.5691079017620067, -2.012342577691571, 4.503220066809524, -0.6560223929881127, 2.5691079017620067, -0.6560223929881127], 0.0, 0.2),
        ],
        hull: [0.25, 0.25, 0.875],
        from: [2.8191079017620067, -2.262342577691571, -0.8749999999999998],
        to: [2.8191079017620067, -2.248779375844537, -0.8725],
    },
    // Found against `replace - with + in sliver` (sweep.rs, column 22).
    Case {
        check: Check::Oracle,
        walls: &[
            ([-1.8170289344822261, 0.0, -1.1952894534446314, 0.0], 1.0, 1.4),
        ],
        slabs: &[],
        hull: [0.05, 0.8048521, 0.05],
        from: [-1.1452894526995734, -0.8048521280288696, 0.949999999254942],
        to: [-1.145789452707024, -0.796803606748581, 0.9499999992513896],
    },
    // Found against, as one case:
    // - `replace - with / in sliver` (sweep.rs, column 22)
    // - `replace * with + in sliver` (sweep.rs, column 30)
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[-2.3875976921386743, 2.274209679234359, -3.777550494095978, 2.274209679234359, -2.3875976921386743, 2.0742096792343587], 0.0, 0.2),
        ],
        hull: [0.25, 0.25, 0.875],
        from: [-4.02755049409598, 2.5242096792343593, -0.8749999999999999],
        to: [-4.017748000322358, 2.522232025114422, -0.875000000007321],
    },
    // Found against `replace + with - in sliver` (sweep.rs, column 28).
    Case {
        check: Check::Oracle,
        walls: &[
            ([-2.3478055088529626, 0.0, 0.0, -2.6552624668599796], 0.0, 0.2),
        ],
        slabs: &[],
        hull: [0.25, 0.25, 0.875],
        from: [0.25, -2.9052624668599796, 1.0749999999999997],
        to: [0.2475, -2.9027624668599796, 1.075000000007933],
    },
    // Found against `replace + with - in sliver` (sweep.rs, column 41).
    Case {
        check: Check::Oracle,
        walls: &[
            ([-2.7606675741454807, -2.0272561258926958, 0.0, -0.2727364602273008], 0.0, 3.0),
        ],
        slabs: &[],
        hull: [0.25, 0.25, 0.875],
        from: [0.25, -0.022736460227300803, 3.8749999999999996],
        to: [0.2475, -0.025236460227300802, 3.8750000000063363],
    },
    // Found against `replace - with / in sliver` (sweep.rs, column 19).
    Case {
        check: Check::Oracle,
        walls: &[
            ([-1.8170289344822261, 0.0, -1.1952894534446314, 0.0], 0.0, 0.2),
        ],
        slabs: &[],
        hull: [0.25, 0.25, 0.875],
        from: [-0.9452894534446314, -0.25, -0.8749999999999999],
        to: [-0.9477894534446314, -0.2475, -0.8750000000035524],
    },
    // Found against `replace + with - in sliver` (sweep.rs, column 54).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[-1.7099373567837552, 1.2595736665286001, -0.26335488601339585, 1.2595736665286001, -1.7099373567837552, 1.4595736665286], 1.0, 1.4),
        ],
        hull: [0.13452616, 0.90007466, 0.5235743],
        from: [-0.12882872267378098, 0.3594990057505545, 0.47642570734024053],
        to: [-0.13017398430717714, 0.36849975235833493, 0.4764257073388097],
    },
    // Found against `replace - with + in sliver` (sweep.rs, column 19).
    Case {
        check: Check::Oracle,
        walls: &[
            ([0.4695917483603684, -0.13604016264656482, 1.35889747153354, 0.17019999159428734], 1.6314696032004508, 1.6314696032004508),
        ],
        slabs: &[],
        hull: [0.25, 0.25, 0.875],
        from: [1.60889747153354, 0.42019999159428734, 0.7564696032004509],
        to: [1.6000044143018084, 0.41713759005187884, 0.7564696031934804],
    },
    // The contact-rule break that the oracle's contract proptest found at 16
    // times its cases (seed 1), and `corners.rs` keeps: a hull sliding along
    // a diamond's edge onto its top, held by that edge since a still axis
    // enters through its nearer face (`hi` here; `lo` in the mirror image).
    Case {
        check: Check::Oracle,
        walls: &[],
        slabs: &[
            (&[-0.9795673034842626, -0.2785587047970818, -0.7184421868659554, -0.01743358817877455, -0.9795673034842626, 0.24369152843953268, -1.2406924201025697, -0.01743358817877455], 1.0, 1.4),
        ],
        hull: [0.62900436, 0.8207159, 0.62513196],
        from: [-0.08943782762065511, 0.8032823160570652, 2.125],
        to: [-0.3394378276206551, 1.0532823160570652, 1.875],
    },
    // The same, moved by (0.5, 0.845679012), so that the edge's grown
    // interval lies mostly on the positive side of 0: the nearer face is
    // the one at the smaller distance, whatever the signs.
    Case {
        check: Check::Contract,
        walls: &[],
        slabs: &[
            (&[-0.4795673034842626, 0.5671203072029182, -0.2184421868659554, 0.8282454238212253, -0.4795673034842626, 1.0893705404395326, -0.7406924201025697, 0.8282454238212253], 1.0, 1.4),
        ],
        hull: [0.62900436, 0.8207159, 0.62513196],
        from: [0.4105621723793449, 1.6489613280570652, 2.125],
        to: [0.1605621723793449, 1.8989613280570652, 1.875],
    },
];
