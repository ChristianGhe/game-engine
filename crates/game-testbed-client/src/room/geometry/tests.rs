//! The room's faces from its grid: one chunk; every face's front toward
//! the region it is lit by; no two faces fighting in one plane; walls faced
//! only where someone stands; regions, materials and world-anchored texture
//! coordinates where the layout says; and headroom over every floor.

use eng_collision_grid::Grid;
use eng_math::Vec3;
use eng_render::{Info, SurfaceClass};
use game_testbed_sim::room::RoomData;
use game_testbed_sim::room::layout::EYE_HEIGHT;

use super::{PROBE, chunks, slab_id, wall_id};
use crate::textures::{REPEAT, Surface};

/// One quad as a chunk holds it, back in world coordinates.
#[derive(Clone, Copy, Debug)]
struct Quad {
    corners: [[f64; 3]; 4],
    uv: [[f32; 2]; 4],
    material: u32,
    info: Info,
}

impl Quad {
    /// Its unit normal, from its winding: counter-clockwise seen from the
    /// front.
    fn normal(&self) -> Vec3 {
        let c = self
            .corners
            .map(|p| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32));
        let sub = |a: Vec3, b: Vec3| Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z);
        sub(c[1], c[0]).cross(sub(c[2], c[0])).normalize_or_zero()
    }

    fn centre(&self) -> [f64; 3] {
        let [a, _, c, _] = self.corners;
        [
            (a[0] + c[0]) / 2.0,
            (a[1] + c[1]) / 2.0,
            (a[2] + c[2]) / 2.0,
        ]
    }

    /// Its extent along each axis.
    fn bounds(&self) -> [(f64, f64); 3] {
        core::array::from_fn(|axis| {
            let v = self.corners.map(|p| p[axis]);
            let lo = v.into_iter().reduce(eng_math::min_f64).unwrap();
            let hi = v.into_iter().reduce(eng_math::max_f64).unwrap();
            (lo, hi)
        })
    }
}

fn room() -> (RoomData, Vec<Quad>) {
    let data = RoomData::room();
    let grid = Grid::new(data.walls.clone(), data.slabs.clone()).unwrap();
    let hull = 2.0 * f64::from(data.movement.hull.half_extents.z);
    let built = chunks(&grid, &data.map, hull);
    assert_eq!(built.len(), 1, "the room fits one chunk");
    let chunk = &built[0];
    assert_eq!(chunk.origin, eng_math::WorldPos::new(0.0, 0.0, 0.0));
    assert_eq!(chunk.vertices.len() % 4, 0);
    assert_eq!(chunk.indices.len(), chunk.vertices.len() / 4 * 6);
    let quads = chunk
        .vertices
        .chunks(4)
        .map(|v| Quad {
            corners: core::array::from_fn(|i| v[i].position.map(f64::from)),
            uv: core::array::from_fn(|i| v[i].uv),
            material: v[0].material,
            info: v[0].info,
        })
        .collect();
    (data, quads)
}

fn region(data: &RoomData, x: f64, y: f64) -> Option<u16> {
    data.map.region_at(x, y).map(|r| r as u16)
}

#[test]
fn every_face_turns_its_front_to_the_region_it_is_lit_by() {
    let (data, quads) = room();
    assert!(quads.len() > 50, "{}", quads.len());
    for q in &quads {
        let n = q.normal();
        let [x, y, _] = q.centre();
        let class = q.info.class;
        if n.z.abs() > 0.99 {
            let expected = if n.z > 0.0 {
                SurfaceClass::Floor
            } else {
                SurfaceClass::Ceiling
            };
            assert_eq!(class, expected, "{q:?}");
            assert_eq!(Some(q.info.region), region(&data, x, y), "{q:?}");
        } else {
            let expected = if n.x.abs() > 0.99 {
                SurfaceClass::WallFacingX
            } else {
                SurfaceClass::WallFacingY
            };
            assert_eq!(class, expected, "{q:?}");
            let probe = (x + f64::from(n.x) * PROBE, y + f64::from(n.y) * PROBE);
            assert_eq!(
                Some(q.info.region),
                region(&data, probe.0, probe.1),
                "{q:?}"
            );
        }
    }
}

/// Two faces in one plane facing the same way would fight over their
/// pixels; facing opposite ways, culling keeps one.
#[test]
fn no_two_faces_facing_the_same_way_overlap_in_one_plane() {
    let (_, quads) = room();
    let key = |q: &Quad| {
        let n = q.normal();
        let axis = [n.x.abs(), n.y.abs(), n.z.abs()]
            .iter()
            .position(|&c| c > 0.99)
            .expect("every face is on an axis");
        let plus = [n.x, n.y, n.z][axis] > 0.0;
        (axis, plus, q.bounds()[axis].0)
    };
    for (i, a) in quads.iter().enumerate() {
        for b in &quads[i + 1..] {
            let (ka, kb) = (key(a), key(b));
            if (ka.0, ka.1) != (kb.0, kb.1) || (ka.2 - kb.2).abs() > 1e-6 {
                continue;
            }
            let (box_a, box_b) = (a.bounds(), b.bounds());
            let overlap = (0..3).filter(|&axis| axis != ka.0).all(|axis| {
                let lo = eng_math::max_f64(box_a[axis].0, box_b[axis].0);
                let hi = eng_math::min_f64(box_a[axis].1, box_b[axis].1);
                hi - lo > 1e-6
            });
            assert!(!overlap, "{a:?}\n{b:?}");
        }
    }
}

#[test]
fn walls_are_faced_only_where_someone_stands() {
    let (_, quads) = room();
    let with = |id: u32| {
        quads
            .iter()
            .filter(|q| q.info.id == id)
            .copied()
            .collect::<Vec<_>>()
    };
    // The hall's south wall, seen from the north: cut at x 3, 5 and 8.
    assert_eq!(with(wall_id(0, 0)).len(), 4);
    assert_eq!(with(wall_id(0, 1)).len(), 0);
    // The alcove's back wall, seen from the west, in the alcove's region.
    let back = with(wall_id(3, 0));
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].info.region, 2);
    assert_eq!(with(wall_id(3, 1)).len(), 0);
    // The hall's east wall north of the alcove: the hall, then the step and
    // platform (region 1) from y 5.25, cut at every region's edge (6 and 8,
    // the skylight's) as the floor and ceiling are, so they share corners.
    let east: Vec<u16> = with(wall_id(5, 0)).iter().map(|q| q.info.region).collect();
    assert_eq!(east, [0, 1, 1, 1]);
}

#[test]
fn regions_and_materials_are_where_the_layout_puts_them() {
    let (_, quads) = room();
    let tops = |slab: usize| quads.iter().filter(move |q| q.info.id == slab_id(slab, 0));
    // The floor under the skylight is region 3; elsewhere 0, or 1 under
    // the step and platform; the alcove's is 2.
    for q in tops(0) {
        let [x, y, _] = q.centre();
        let expected = if (3.0..5.0).contains(&x) && (6.0..8.0).contains(&y) {
            3
        } else if x > 8.0 && y > 5.25 {
            1
        } else {
            0
        };
        assert_eq!(q.info.region, expected, "{:?}", q.centre());
        assert_eq!(q.material, Surface::Floor.material());
    }
    assert!(tops(1).all(|q| q.info.region == 2));
    let material = |id: u32| {
        let found: Vec<u32> = quads
            .iter()
            .filter(|q| q.info.id == id)
            .map(|q| q.material)
            .collect();
        assert!(!found.is_empty(), "{id:#x}");
        assert!(found.windows(2).all(|w| w[0] == w[1]));
        found[0]
    };
    // The step, the platform and the ledge: tread; the pillar: concrete.
    for slab in [2, 3, 4] {
        assert_eq!(material(slab_id(slab, 0)), Surface::Step.material());
        // Each one's west side, edge 3, which faces the hall.
        assert_eq!(material(slab_id(slab, 5)), Surface::Step.material());
    }
    assert_eq!(material(slab_id(5, 2)), Surface::Pillar.material());
    // The ceilings' undersides are ceiling, their edges (the skylight's
    // shaft, the alcove's lintel) brick.
    assert_eq!(material(slab_id(6, 1)), Surface::Ceiling.material());
    assert_eq!(material(slab_id(10, 1)), Surface::Ceiling.material());
    assert_eq!(material(slab_id(7, 2)), Surface::Wall.material());
    assert_eq!(material(slab_id(10, 5)), Surface::Wall.material());
    assert_eq!(material(wall_id(6, 0)), Surface::Wall.material());
    // Nothing below the floor: no floor slab's bottom or sides.
    for slab in [0, 1] {
        for face in 1..6 {
            assert!(quads.iter().all(|q| q.info.id != slab_id(slab, face)));
        }
    }
}

/// Texture coordinates are the world's position over [`REPEAT`], less a
/// whole number of repeats: along X and down Y on tops and bottoms, along
/// the viewer's right and down Z on walls.
#[test]
fn texture_coordinates_are_anchored_to_the_world_and_stay_small() {
    let (_, quads) = room();
    let whole = |v: f64| (v - v.round()).abs() < 1e-5;
    for q in &quads {
        let n = q.normal();
        for (p, uv) in q.corners.iter().zip(q.uv) {
            let (u, v) = (f64::from(uv[0]), f64::from(uv[1]));
            assert!(u.abs() <= 8.0 && v.abs() <= 8.0, "{q:?}");
            let (along, down) = if n.z.abs() > 0.99 {
                (p[0], -p[1])
            } else {
                (p[0] * f64::from(-n.y) + p[1] * f64::from(n.x), -p[2])
            };
            assert!(whole(u - along / REPEAT), "{q:?}");
            assert!(whole(v - down / REPEAT), "{q:?}");
        }
    }
}

/// The eye is 1.6 m above the feet, and a ceiling within 0.15 m of it
/// would cross the near plane (eng-render's `NEAR_PLANE`).
#[test]
fn every_floor_has_headroom_for_the_eye() {
    let (_, quads) = room();
    let standable = [Surface::Floor.material(), Surface::Step.material()];
    let floors = quads
        .iter()
        .filter(|q| q.info.class == SurfaceClass::Floor && standable.contains(&q.material));
    let ceilings: Vec<&Quad> = quads
        .iter()
        .filter(|q| q.info.class == SurfaceClass::Ceiling)
        .collect();
    let mut checked = 0;
    for floor in floors {
        let f = floor.bounds();
        for ceiling in &ceilings {
            let c = ceiling.bounds();
            let over = (0..2).all(|a| {
                eng_math::min_f64(f[a].1, c[a].1) - eng_math::max_f64(f[a].0, c[a].0) > 1e-6
            });
            if over && c[2].0 > f[2].0 {
                assert!(
                    c[2].0 - f[2].0 >= EYE_HEIGHT + 0.15,
                    "{floor:?}\n{ceiling:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 10, "{checked}");
}
