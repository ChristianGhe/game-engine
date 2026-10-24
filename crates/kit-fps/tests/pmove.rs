use eng_collision_api::{BoxHull, BoxRoom, CollisionWorld};
use eng_core::{PlayerCommand, Tick};
use eng_math::{Vec3, WorldPos};
use kit_fps::{MoveConfig, MoveState, pmove};
use proptest::prelude::*;

const ROOM: BoxRoom = BoxRoom {
    min: WorldPos::new(0.0, 0.0, 0.0),
    max: WorldPos::new(8.0, 6.0, 2.0),
};
const CONFIG: MoveConfig = MoveConfig {
    max_speed: 4.0,
    accel: 10.0,
    friction: 6.0,
    stop_speed: 1.0,
    hull: BoxHull {
        half_extents: Vec3::new(0.25, 0.25, 0.5),
    },
};
const DT: f32 = 1.0 / 60.0;

fn cmd(forward: i8, side: i8, yaw: u16) -> PlayerCommand {
    PlayerCommand {
        forward,
        side,
        view_yaw: yaw,
        ..PlayerCommand::idle(Tick(1))
    }
}

fn run(mut s: MoveState, c: PlayerCommand, ticks: usize) -> MoveState {
    for _ in 0..ticks {
        s = pmove(&s, &c, &CONFIG, DT, &ROOM).state;
    }
    s
}

fn start() -> MoveState {
    MoveState {
        pos: WorldPos::new(4.0, 3.0, 1.0),
        vel: Vec3::ZERO,
    }
}

#[test]
fn forward_reaches_top_speed_along_yaw() {
    let east = run(start(), cmd(127, 0, 0), 30);
    assert!((east.vel.x - 4.0).abs() < 1e-3, "{:?}", east.vel);
    assert!(east.vel.y.abs() < 1e-6);
    let north = run(start(), cmd(127, 0, 0x4000), 30);
    assert!(
        north.pos.y > 3.5 && (north.pos.x - 4.0).abs() < 1e-5,
        "{north:?}"
    );
}

#[test]
fn side_moves_right_of_yaw() {
    let s = run(start(), cmd(0, 127, 0), 20);
    assert!(s.pos.y < 3.0, "right of +X is -Y: {s:?}");
}

#[test]
fn diagonal_is_capped_at_top_speed() {
    let s = run(start(), cmd(127, 127, 0), 120);
    assert!(s.vel.length() <= 4.0 + 1e-4, "{:?}", s.vel);
}

#[test]
fn friction_stops_an_idle_hull() {
    let moving = run(start(), cmd(127, 0, 0), 30);
    let stopped = run(moving, cmd(0, 0, 0), 60);
    assert_eq!(stopped.vel, Vec3::ZERO);
}

#[test]
fn wall_clips_velocity_and_reports_the_plane() {
    let mut s = start();
    let mut normals = Vec::new();
    for _ in 0..120 {
        let out = pmove(&s, &cmd(127, 0, 0x2000), &CONFIG, DT, &ROOM);
        normals.extend_from_slice(out.planes());
        s = out.state;
    }
    // Heading north-east into the corner: stuck at x = 7.75, y = 5.75.
    assert_eq!((s.pos.x, s.pos.y), (7.75, 5.75));
    assert!(normals.contains(&Vec3::new(-1.0, 0.0, 0.0)));
    assert!(normals.contains(&Vec3::new(0.0, -1.0, 0.0)));
}

proptest! {
    #[test]
    fn never_leaves_the_room(
        commands in prop::collection::vec((any::<i8>(), any::<i8>(), any::<u16>()), 1..200),
    ) {
        let mut s = start();
        for (f, side, yaw) in commands {
            s = pmove(&s, &cmd(f, side, yaw), &CONFIG, DT, &ROOM).state;
            prop_assert!(!ROOM.overlaps(s.pos, &CONFIG.hull), "left the room: {s:?}");
            prop_assert!(s.vel.is_finite());
        }
    }
}
