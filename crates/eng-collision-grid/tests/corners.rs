//! The contact rule at corners and edges, where rounding decides: cases the
//! proptests and the review found, each a rule of the crate docs. Every
//! trace here passes `conformance::check`.

use eng_collision_api::{BoxHull, CONTACT_GAP, CollisionWorld, Trace, conformance};
use eng_collision_grid::{Grid, Point2, Slab, Wall};
use eng_math::{Vec3, WorldPos};

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

/// Traces once and checks the trace with the conformance suite.
fn checked(world: &Grid, start: WorldPos, end: WorldPos) -> Trace {
    conformance::check(world, start, end, &HULL);
    world.trace(start, end, &HULL)
}

fn off_axes(n: Vec3) -> bool {
    n.z == 0.0 && n.x != 0.0 && n.y != 0.0
}

#[test]
fn a_corner_hit_on_a_slab_ends_on_both_faces() {
    // Step 3: Y is entered with X, and rounding left the stop an ulp short of
    // Y's face; the stop is put on it.
    let world = Grid::new(vec![], vec![rect(4.0, 4.0, 6.0, 6.0, 0.0, 3.0)]).unwrap();
    let s = at(1.9107109421119168, 0.9598307116654166, 1.5);
    let e = at(5.129701861931396, 5.842983561071814, 1.5);
    let tr = checked(&world, s, e);
    assert_eq!(
        (tr.end, tr.normal),
        (at(3.75, 3.75, 1.5), Vec3::new(-1.0, 0.0, 0.0))
    );
}

#[test]
fn passing_a_corner_by_less_than_graze_is_not_a_hit() {
    // Step 2: landing on a low slab's corner as the hull leaves it, past a
    // low wall's end, and past a wall's end off the axes.
    let cases = [
        (
            Grid::new(vec![], vec![rect(4.0, 4.0, 6.0, 6.0, 0.0, 0.4)]).unwrap(),
            at(6.497430782865544, 5.919109978180762, 2.246565913001959),
            at(6.014044005840844, 1.681484128956141, 0.3484911871571814),
        ),
        (
            Grid::new(vec![wall((4.0, 4.0), (6.0, 4.0), 0.0, 0.4)], vec![]).unwrap(),
            at(2.5918357697132497, 2.052196958812823, 3.1119313142909766),
            at(3.984955248640782, 4.695865401902239, 0.9023441689885243),
        ),
        (
            Grid::new(vec![wall((6.0, 0.0), (0.0, 6.0), 0.0, 0.4)], vec![]).unwrap(),
            at(-0.4993001745800876, 7.8676225827760184, 0.275),
            at(-0.10555565632254743, 4.523050983053482, 0.275),
        ),
    ];
    for (world, s, e) in cases {
        let tr = checked(&world, s, e);
        assert!(!tr.hit(), "{tr:?}");
    }
}

#[test]
fn a_corner_on_a_corner_is_blocked_by_the_edge_off_the_axes() {
    // Step 4: the hull's corner meets a corner of the primitive; the hit on
    // X leaves an edge off the axes still ahead, which is what blocks.
    let wall_end = Grid::new(
        vec![wall((0.0, 6.486769911324833), (11.0, 9.0), 0.0, 0.2)],
        vec![],
    )
    .unwrap();
    let tr = checked(
        &wall_end,
        at(12.069157778327087, 8.75, 0.0),
        at(3.3283378264540016, 8.75, 0.0),
    );
    assert!(tr.hit() && off_axes(tr.normal), "{tr:?}");
    let diamond = slab(
        &[
            (-0.015556462896603626, -2.444448605549062),
            (1.1788921426524583, -1.25),
            (-0.015556462896603626, -0.05555139445093804),
            (-1.2100050684456656, -1.25),
        ],
        0.4,
        0.4,
    );
    let world = Grid::new(vec![], vec![diamond]).unwrap();
    let tr = checked(&world, at(-4.0, -1.0, H), at(-1.25, -1.0, H));
    assert!(tr.hit() && off_axes(tr.normal), "{tr:?}");
}

#[test]
fn a_hull_passing_a_slabs_apex_lands_on_the_floor_beyond() {
    // Step 2: the aimed apex of a triangle is touched, not hit.
    let triangle = slab(
        &[
            (10.570361444591944, 2.7447057596531828),
            (11.435910575284222, 2.755569216744784),
            (10.993727980123834, 3.499725023602033),
        ],
        0.0,
        0.2,
    );
    let world = Grid::new(
        vec![wall((5.5, 5.5), (5.5, 0.0), 0.0, 3.0)],
        vec![rect(-1.0, -1.0, 13.0, 10.0, -0.5, 0.0), triangle],
    )
    .unwrap();
    let s = at(5.75, 5.6909748558388245, 1.9247062253324272);
    let e = at(19.2751517791447, 0.911761411482964, -0.1672076274983707);
    let tr = checked(&world, s, e);
    assert_eq!((tr.end.z, tr.normal), (H, Vec3::Z), "{tr:?}");
}

/// The 8 m x 6 m x 3 m room: four walls, a floor and a ceiling.
fn room() -> Grid {
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
    Grid::new(walls, slabs).unwrap()
}

#[test]
fn a_stop_inside_what_ties_with_the_hit_moves_onto_its_face() {
    // Step 6: from outside, the hull meets the room's corner, where two walls
    // and the ceiling's side tie. The first wall's stop was an ulp inside the
    // ceiling's side; it moves onto that face, the walls' corner.
    let s = at(-0.25, 13.66186128048594, 2.783695152422334);
    let e = at(13.472308138449689, 0.948380734201896, 2.783695152422334);
    let tr = checked(&room(), s, e);
    assert_eq!((tr.end.x, tr.end.y), (7.75, 6.25), "{tr:?}");
    assert_eq!(tr.normal, Vec3::new(-1.0, 0.0, 0.0));
}

#[test]
fn a_stop_inside_a_face_it_slides_along_moves_on_along_the_path() {
    // Step 6: sliding along a wall off the axes from within rounding of it,
    // the hull hits a pillar's side. On that face it is an ulp inside the
    // wall; a little further along the path, still on the face, it is not.
    let pillar = rect(
        10.261020399793358,
        1.25,
        10.511020399793358,
        5.068771695107662,
        0.0,
        3.0,
    );
    let world = Grid::new(
        vec![wall(
            (6.25, 6.105651719435156),
            (10.573701768313462, 5.477879564087373),
            0.0,
            3.0,
        )],
        vec![pillar],
    )
    .unwrap();
    let s = at(6.561027806000479, 5.774194280661772, 0.8893709704611121);
    let e = at(10.323701768313462, 5.227879564087373, 3.875);
    let tr = checked(&world, s, e);
    assert_eq!(
        (tr.end.x, tr.normal),
        (10.011020399793358, Vec3::new(-1.0, 0.0, 0.0))
    );
}

#[test]
fn sliding_along_a_face_off_the_axes_from_contact_keeps_the_contract() {
    // Found by the oracle's proptest: a hull stopped against a diamond's
    // 45-degree edge, then moved a quarter meter along it. From within
    // rounding of the face, the motion seems to leave it by 8e-17 m, while
    // the end is inside by rounding, so the hit's normal faces along the
    // motion by that much: within CONTACT_GAP, which the check allows.
    let diamond = slab(
        &[
            (-0.33300485509596217, 0.1179840021497991),
            (0.7990111427542388, 1.25),
            (-0.33300485509596217, 2.3820159978502007),
            (-1.465020852946163, 1.25),
        ],
        0.0,
        0.0,
    );
    let world = Grid::new(vec![], vec![diamond]).unwrap();
    let start = at(-0.8609263422872409, -2.1436119714588755, H);
    let first = at(
        start.x + 0.25,
        start.y + 10.00731697825094,
        start.z - 0.04809485806552371,
    );
    let mut from = start;
    for to in [first, at(-2.25, 8.75, H)] {
        conformance::check(&world, from, to, &HULL);
        from = world.trace(from, to, &HULL).end;
    }
    let along = at(from.x - 0.25, from.y + 0.25, from.z);
    let tr = world.trace(from, along, &HULL);
    assert!(tr.hit() && tr.normal.x < 0.0 && tr.normal.y < 0.0, "{tr:?}");
    conformance::check(&world, from, along, &HULL);
}

#[test]
fn a_hull_corner_on_a_walls_end_is_solid_one_ulp_on_along_the_hit_axis() {
    // Found by the contract's proptest: the hull's corner rests on the end
    // of a wall off the axes, and the hull moves down and a little west. The
    // hit is on Y, exactly on the face. One ulp down is inside the wall, but
    // one ulp down and west is not: the wall's thin wedge there is narrower
    // than an ulp. The check accepts the hit axis alone.
    let world = Grid::new(vec![wall((9.25, 7.5), (1.0, 2.0), 0.0, 0.2)], vec![]).unwrap();
    let start = at(9.0, 7.75, H);
    let end = at(9.0 - 0.019906980178925053, 7.75 - 0.8194253849434967, H);
    let tr = world.trace(start, end, &HULL);
    assert_eq!((tr.fraction, tr.end, tr.normal), (0.0, start, Vec3::Y));
    let down = at(9.0, 7.75f64.next_down(), H);
    assert!(world.overlaps(down, &HULL));
    assert!(!world.overlaps(at(9.0f64.next_down(), down.y, H), &HULL));
    conformance::check(&world, start, end, &HULL);
}

#[test]
fn a_hit_with_less_than_an_ulp_of_solid_ahead_goes_to_an_edge_or_grazes() {
    // Step 4's last rule: the hull's corner, just under a wall's end, moves
    // north and up. The solid ahead of the hit is thinner than an ulp, so
    // one ulp on is still free on every probe the check makes.
    let world = Grid::new(
        vec![wall((0.75, 8.5), (4.598790175350227, 6.5), 0.0, 0.2)],
        vec![],
    )
    .unwrap();
    let s = at(4.348790175350227, 6.25, -0.875);
    let e = at(4.348790175350227, 13.665689115504318, -0.8086997260162662);
    checked(&world, s, e);
}

#[test]
fn sliding_along_a_wall_off_the_axes_within_rounding_is_not_a_hit() {
    // Step 2's slide rule: from a stop against a wall, a motion along it,
    // and rising off its top. Points on the way flicker in and out of the
    // wall by rounding, so its entry there is rounding too.
    let world = Grid::new(
        vec![wall(
            (3.4095210008002677, 5.25),
            (7.0, 7.5500215837233),
            0.0,
            0.2,
        )],
        vec![],
    )
    .unwrap();
    let s = at(5.096146934296498, 5.920286819442317, 0.9966780703380544);
    let e = at(2.6627921091414875, 4.361506401796184, 1.1293396358957157);
    let tr = checked(&world, s, e);
    assert!(!tr.hit(), "{tr:?}");
}

/// The grid, but a hit off the axes after more than 1 mm of travel stops
/// `4 * CONTACT_GAP` short along the normal: a world that breaks the contact
/// rule.
struct Gappy(Grid);

impl CollisionWorld for Gappy {
    fn trace(&self, start: WorldPos, end: WorldPos, hull: &BoxHull) -> Trace {
        let mut tr = self.0.trace(start, end, hull);
        let off_axes = tr.normal.z == 0.0 && tr.normal.x != 0.0 && tr.normal.y != 0.0;
        if tr.hit() && off_axes && start.distance(tr.end) > 1e-3 {
            tr.end = tr.end.add_scaled(tr.normal, 4.0 * CONTACT_GAP);
        }
        tr
    }

    fn overlaps(&self, at: WorldPos, hull: &BoxHull) -> bool {
        self.0.overlaps(at, hull)
    }
}

#[test]
#[should_panic(expected = "more than CONTACT_GAP")]
fn a_gap_off_the_axes_wider_than_contact_gap_fails_the_check() {
    let world = Grid::new(vec![wall((6.0, 0.0), (0.0, 6.0), 0.0, 3.0)], vec![]).unwrap();
    conformance::check(&Gappy(world), at(1.0, 1.5, H), at(4.0, 3.0, H), &HULL);
}
