//! The room (rung 2 plan, "The room"; decisions 2 to 4): its layout, its
//! spawns and things, what can be seen from spawn S0, the marker's turn,
//! and walking over it with `ground_move`: the step and the platform climb,
//! the ledge blocks, walls and corners stop the hull exactly, and the hull
//! is never inside solid.

use eng_collision_api::{BoxHull, CollisionWorld};
use eng_collision_grid::Grid;
use eng_core::{PlayerCommand, Tick};
use eng_math::{BAngle, Vec3, WorldPos};
use eng_sim::{Sim, SimConfig};
use game_testbed_sim::room::layout::{
    self, ALCOVE_CEILING, CEILING, EYE_HEIGHT, HULL, LEDGE_TOP, MARKER_TURN_TICKS, PLATFORM_TOP,
    STEP_TOP,
};
use game_testbed_sim::room::{self, RoomData, RoomMap, Spawn, Thing, ThingKind, Turning, View};
use game_testbed_sim::{TICK_RATE, WallBump};
use kit_fps::MoveState;
use proptest::prelude::*;

/// The hull's half height: its centre above the floor it stands on.
const H: f64 = 0.875;

fn config(players: u8, seed: u64) -> SimConfig {
    SimConfig {
        tick_rate: TICK_RATE,
        players,
        seed,
    }
}

fn sim() -> Sim {
    room::build(config(1, 1)).unwrap()
}

/// The room with player 0's spawn moved to `(x, y)` on floor `z`, facing
/// `yaw`.
fn sim_from(x: f64, y: f64, z: f64) -> Sim {
    let mut data = RoomData::room();
    data.map.spawns[0] = Spawn {
        feet: WorldPos::new(x, y, z),
        yaw: BAngle::ZERO,
    };
    room::build_with(config(1, 1), data).unwrap()
}

fn grid(sim: &Sim) -> &Grid {
    sim.world().constant::<Grid>()
}

/// A 16-bit command angle for `deg` degrees, as input scripts round it.
fn angle(deg: f64) -> u16 {
    let turns = deg.rem_euclid(360.0) / 360.0;
    ((turns * 65536.0).round() as u32 & 0xffff) as u16
}

fn step(sim: &mut Sim, forward: i8, yaw_deg: f64) {
    let tick = sim.tick().next();
    let cmds: Vec<PlayerCommand> = (0..sim.config().players)
        .map(|_| PlayerCommand {
            forward,
            view_yaw: angle(yaw_deg),
            ..PlayerCommand::idle(tick)
        })
        .collect();
    sim.step(&cmds).unwrap();
}

fn player(sim: &Sim) -> MoveState {
    let (_, state) = sim.world().storage::<MoveState>().iter().next().unwrap();
    *state
}

fn things(sim: &Sim) -> Vec<Thing> {
    sim.world()
        .storage::<Thing>()
        .iter()
        .map(|(_, t)| *t)
        .collect()
}

/// Runs `segments` of (ticks, forward, yaw), calling `each` after every
/// tick.
fn walk(sim: &mut Sim, segments: &[(u32, i8, f64)], mut each: impl FnMut(&Sim)) {
    for &(ticks, forward, yaw) in segments {
        for _ in 0..ticks {
            step(sim, forward, yaw);
            each(sim);
        }
    }
}

#[test]
fn the_room_is_built_from_axis_aligned_walls_and_slabs() {
    let sim = sim();
    let grid = grid(&sim);
    assert_eq!((grid.walls().len(), grid.slabs().len()), (8, 11));
    assert_eq!(grid.cells(), (15, 10), "x 0..=14, y 0..=9");
    for w in grid.walls() {
        assert!(w.a.x == w.b.x || w.a.y == w.b.y, "{w:?}");
    }
    for s in grid.slabs() {
        let n = s.points.len();
        for i in 0..n {
            let (p, q) = (s.points[i], s.points[(i + 1) % n]);
            assert!(p.x == q.x || p.y == q.y, "{s:?}");
        }
    }
    let tops: Vec<f64> = grid.slabs().iter().map(|s| s.top).collect();
    for top in [0.0, STEP_TOP, PLATFORM_TOP, LEDGE_TOP, CEILING] {
        assert!(tops.contains(&top), "{top}");
    }
    let bottoms: Vec<f64> = grid.slabs().iter().map(|s| s.bottom).collect();
    assert!(bottoms.contains(&ALCOVE_CEILING));
    assert_eq!(layout::HULL.half_extents, Vec3::new(0.25, 0.25, 0.875));
    assert_eq!(EYE_HEIGHT, 1.6);
}

#[test]
fn four_regions_cover_the_floor_plan_the_later_one_winning() {
    let sim = sim();
    let map = sim.world().constant::<RoomMap>();
    assert_eq!(map.regions.len(), 4);
    let cases = [
        ((1.5, 4.5), Some(0)),
        ((6.0, 2.0), Some(0)),
        ((10.0, 5.5), Some(1)),
        ((10.0, 8.0), Some(1)),
        ((13.0, 2.0), Some(2)),
        ((4.0, 7.0), Some(3)),
        ((13.0, 5.0), None),
        ((-1.0, 4.0), None),
    ];
    for ((x, y), region) in cases {
        assert_eq!(map.region_at(x, y), region, "({x}, {y})");
    }
    let sky: Vec<bool> = map.regions.iter().map(|r| r.sky).collect();
    assert_eq!(sky, [false, false, false, true]);
    let light: Vec<u8> = map.regions.iter().map(|r| r.light).collect();
    assert_eq!(light, [160, 192, 112, 255]);
}

#[test]
fn players_start_on_the_floor_at_fixed_spawns_in_slot_order() {
    for seed in [0, 1, 7] {
        let sim = room::build(config(6, seed)).unwrap();
        let map = sim.world().constant::<RoomMap>();
        let states: Vec<MoveState> = sim
            .world()
            .storage::<MoveState>()
            .iter()
            .map(|(_, s)| *s)
            .collect();
        assert_eq!(states.len(), 6);
        for (p, state) in states.iter().enumerate() {
            let spawn = map.spawns[p % 4];
            let at = WorldPos::new(spawn.feet.x, spawn.feet.y, spawn.feet.z + H);
            assert_eq!((state.pos, state.vel), (at, Vec3::ZERO), "player {p}");
            assert!(!grid(&sim).overlaps(state.pos, &HULL), "player {p}");
            // Standing: the ground probe hits at once.
            let probe = grid(&sim).trace(
                state.pos,
                WorldPos::new(at.x, at.y, at.z - kit_fps::GROUND_PROBE),
                &HULL,
            );
            assert_eq!((probe.fraction, probe.normal), (0.0, Vec3::Z));
        }
        let first = sim.world().entities().next().unwrap();
        assert_eq!(first.index, 0, "player 0 is entity 0");
        let views: Vec<View> = sim
            .world()
            .storage::<View>()
            .iter()
            .map(|(_, v)| *v)
            .collect();
        assert!(views.iter().all(|v| *v
            == View {
                yaw: BAngle::ZERO,
                pitch: BAngle::ZERO
            }));
    }
    let s0 = sim().world().constant::<RoomMap>().spawns[0];
    assert_eq!(s0.feet, WorldPos::new(1.5, 4.5, 0.0));
}

#[test]
fn three_things_stand_on_floors() {
    let sim = sim();
    let things = things(&sim);
    let kinds: Vec<ThingKind> = things.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        [ThingKind::Marker, ThingKind::Crate, ThingKind::Barrel]
    );
    let small = BoxHull {
        half_extents: Vec3::new(0.015625, 0.015625, 0.015625),
    };
    for t in &things {
        let above = WorldPos::new(t.feet.x, t.feet.y, t.feet.z + 0.015625);
        let down = grid(&sim).trace(above, WorldPos::new(above.x, above.y, -0.4), &small);
        assert_eq!(
            (down.end, down.normal),
            (above, Vec3::Z),
            "{t:?} stands on a floor"
        );
    }
    assert_eq!(sim.world().entity_count(), 4);
    assert_eq!(
        sim.world().storage::<Turning>().len(),
        1,
        "only the marker turns"
    );
}

/// From S0's eye: the marker and the barrel show, and the crate is behind
/// the pillar, hidden for a billboard up to 1.5 m wide and as tall as the
/// room. A sight line is a trace of a 2 mm box.
#[test]
fn from_s0_the_marker_and_barrel_show_and_the_crate_is_hidden() {
    let sim = sim();
    let ray = BoxHull {
        half_extents: Vec3::new(0.001, 0.001, 0.001),
    };
    let eye = WorldPos::new(1.5, 4.5, EYE_HEIGHT);
    let seen = |to: WorldPos| !grid(&sim).trace(eye, to, &ray).hit();
    let things = things(&sim);
    for t in [things[0], things[2]] {
        for dz in [0.1, 0.5, 1.0] {
            let to = WorldPos::new(t.feet.x, t.feet.y, t.feet.z + dz);
            assert!(seen(to), "{t:?} at {dz}");
        }
    }
    let crate_ = things[1];
    for dy in [-0.75, -0.4, 0.0, 0.4, 0.75] {
        for z in [0.05, 0.5, 1.0, 1.6, 2.5, 3.9] {
            let to = WorldPos::new(crate_.feet.x, crate_.feet.y + dy, z);
            assert!(!seen(to), "the crate shows at {dy}, {z}");
        }
    }
}

#[test]
fn the_marker_turns_once_every_8_seconds_in_eighths_on_the_second() {
    let turning = Turning {
        from: BAngle::HALF,
        ticks: MARKER_TURN_TICKS,
    };
    assert_eq!(MARKER_TURN_TICKS, 8 * TICK_RATE);
    for k in 0..=8u64 {
        let expected = BAngle(BAngle::HALF.0.wrapping_add((k as u32 % 8) << 29));
        assert_eq!(turning.at(Tick(60 * k)), expected, "{k} s");
    }
    // From S0, due west of the marker, each of the 8 rotations (decision 4:
    // the 45-degree sector holding the direction to the viewer) shows for
    // one second, from half a second before each whole second to half after.
    let to_viewer = BAngle::HALF;
    let mut shown = [0u32; 8];
    for t in 0..480u64 {
        let facing = turning.at(Tick(t));
        let rotation = to_viewer.wrapping_sub(facing).0.wrapping_add(1 << 28) >> 29;
        shown[rotation as usize] += 1;
        if t % 60 == 0 {
            assert_eq!(rotation, ((8 - t / 60) % 8) as u32, "tick {t}");
        }
    }
    assert_eq!(shown, [60; 8]);

    let mut sim = sim();
    walk(&mut sim, &[(60, 0, 0.0)], |_| {});
    assert_eq!(things(&sim)[0].facing, BAngle(0xa000_0000), "an eighth on");
    assert_eq!(things(&sim)[1].facing, BAngle::HALF, "the crate stands");
}

#[test]
fn the_step_then_the_platform_climb() {
    let mut sim = sim_from(9.5, 4.0, 0.0);
    let mut heights = Vec::new();
    walk(&mut sim, &[(120, 127, 90.0)], |sim| {
        let z = player(sim).pos.z;
        if heights.last() != Some(&z) {
            heights.push(z);
        }
    });
    assert_eq!(heights, [H, STEP_TOP + H, PLATFORM_TOP + H]);
    assert_eq!(player(&sim).pos.y, 8.75, "on to the north wall");
}

#[test]
fn the_platforms_open_side_climbs_and_walking_off_it_snaps_down() {
    let mut sim = sim_from(7.0, 7.5, 0.0);
    walk(&mut sim, &[(30, 127, 0.0)], |_| {});
    assert_eq!(player(&sim).pos.z, PLATFORM_TOP + H);
    let mut heights = Vec::new();
    walk(&mut sim, &[(60, 127, 180.0)], |sim| {
        let z = player(sim).pos.z;
        if heights.last() != Some(&z) {
            heights.push(z);
        }
    });
    assert_eq!(heights, [PLATFORM_TOP + H, H], "no falling between");
    assert_eq!(player(&sim).vel.z, 0.0);
}

#[test]
fn the_ledge_blocks_exactly_on_its_face() {
    let mut sim = sim_from(9.0, 3.0, 0.0);
    let mut bumped = false;
    walk(&mut sim, &[(120, 127, 270.0)], |sim| {
        assert_eq!(player(sim).pos.z, H, "never climbs the ledge");
        bumped |= sim
            .world()
            .events::<WallBump>()
            .iter()
            .any(|b| b.normal == Vec3::Y);
    });
    let end = player(&sim);
    assert_eq!((end.pos.x, end.pos.y), (9.0, 1.25));
    // Yaw 270's direction has an x of -1.8e-16 in f32, which friction
    // never quite takes away; it moves x by less than an ulp.
    assert_eq!((end.vel.y, end.vel.z), (0.0, 0.0));
    assert!(end.vel.x.abs() < 1e-12, "{end:?}");
    assert!(bumped);
}

#[test]
fn walls_and_the_pillar_stop_the_hull_exactly() {
    // East from S0 into the pillar's west face.
    let mut east = sim();
    walk(&mut east, &[(90, 127, 0.0)], |_| {});
    assert_eq!(player(&east).pos, WorldPos::new(5.25, 4.5, H));
    // Into the alcove's back corner, as room-corner.input.toml goes.
    let mut sim = sim();
    walk(&mut sim, &[(170, 127, -15.0), (130, 127, -35.0)], |sim| {
        assert_eq!(player(sim).pos.z, H, "over the floors' seam at x = 12");
    });
    let end = player(&sim);
    assert_eq!((end.pos.x, end.pos.y, end.vel), (13.75, 1.25, Vec3::ZERO));
}

/// The last player's hull is free, between the floor and the platform's
/// height, never airborne, and inside the floor plan.
fn check_free(sim: &Sim, tag: &str) {
    let (_, s) = sim.world().storage::<MoveState>().iter().last().unwrap();
    let p = s.pos;
    let tag = format!("{tag}, tick {}: {s:?}", sim.tick().0);
    assert!(!grid(sim).overlaps(p, &HULL), "{tag}");
    assert!((H..=PLATFORM_TOP + H).contains(&p.z), "{tag}");
    assert_eq!(s.vel.z, 0.0, "{tag}");
    let in_hall = (0.25..=11.75).contains(&p.x) && (0.25..=8.75).contains(&p.y);
    let in_alcove = (11.75..=13.75).contains(&p.x) && (1.25..=2.75).contains(&p.y);
    assert!(in_hall || in_alcove, "{tag}");
}

/// The fixed case: from every spawn, runs at 32 headings for 5 s, checked
/// after every tick.
#[test]
fn the_hull_is_never_inside_solid_at_32_headings() {
    for spawn in 0..4u8 {
        for heading in 0..32 {
            let yaw = f64::from(heading) * 11.25 + 3.0;
            let mut sim = room::build(config(spawn + 1, 1)).unwrap();
            walk(&mut sim, &[(300, 127, yaw)], |sim| {
                check_free(sim, &format!("spawn {spawn}, yaw {yaw}"));
            });
        }
    }
}

proptest! {
    // No regressions file, a protected path: a failure prints its case,
    // which becomes a fixed test.
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// Any walk from any spawn, checked after every tick: one to four
    /// segments of up to 4 s, each with its own forward and sideways push
    /// and view yaw.
    #[test]
    fn the_hull_is_never_inside_solid_on_any_walk(
        spawn in 0..4u8,
        segments in prop::collection::vec(
            (1..240u32, any::<i8>(), any::<i8>(), any::<u16>()),
            1..5,
        ),
    ) {
        let mut sim = room::build(config(spawn + 1, 1)).unwrap();
        for (i, &(ticks, forward, side, view_yaw)) in segments.iter().enumerate() {
            for _ in 0..ticks {
                let cmd = PlayerCommand {
                    forward,
                    side,
                    view_yaw,
                    ..PlayerCommand::idle(sim.tick().next())
                };
                sim.step(&vec![cmd; usize::from(spawn) + 1]).unwrap();
                check_free(&sim, &format!("spawn {spawn}, segment {i} of {segments:?}"));
            }
        }
    }
}

#[test]
fn commands_set_the_view_and_the_system_order_is_stable() {
    let mut sim = sim();
    let tick = sim.tick().next();
    let cmd = PlayerCommand {
        view_yaw: 0x1234,
        view_pitch: 0xf000,
        ..PlayerCommand::idle(tick)
    };
    sim.step(&[cmd]).unwrap();
    let (_, view) = sim.world().storage::<View>().iter().next().unwrap();
    assert_eq!(
        *view,
        View {
            yaw: BAngle(0x1234_0000),
            pitch: BAngle(0xf000_0000)
        }
    );
    let order: Vec<_> = sim.system_order().map(|(s, n)| (s.name(), n)).collect();
    assert_eq!(
        order,
        [
            ("Update", room::MOVE_PLAYERS),
            ("Update", room::TURN_THINGS)
        ]
    );
}

#[test]
#[should_panic(expected = "[E-MAP] the room has no spawns")]
fn a_room_without_spawns_is_refused() {
    let mut data = RoomData::room();
    data.map.spawns.clear();
    let _ = room::build_with(config(1, 1), data);
}
