//! The room's layout: every size, in meters (rung 2 plan, "The room"; the
//! exact sizes are T7's). Every wall and every slab side is on an axis, so
//! traces stop exactly on them (ADR-0024) and `pmove`'s angled-wall stop
//! (O12) cannot arise.
//!
//! The plan, X east and Y north, 3 columns a meter and 2 rows; S spawns, M
//! the marker, C the crate, B the barrel, `:` the skylight's hole:
//!
//! ```text
//!   9 +-----------------------+-----------+
//!     |                       |           |
//!   8 |        :.....:        |           |
//!     |        :     :        |       B   |
//!   7 |        :sky  :        | platform  |
//!     |        :     :        |           |
//!   6 |   S2   :.....:        +-----------+
//!     |                       |  step     |
//!   5 |               +---+   +-----------+
//!     |   S0      M   |   |   C           |
//!   4 |               +---+               |
//!     |                                   |
//!   3 |   S1                              +-----+
//!     |                                         |
//!   2 |                                         | alcove
//!     |   S3                                    |
//!   1 |                    +-----------+  +-----+
//!     |                    |  ledge    |  |
//!   0 +--------------------+-----------+--+
//!     0   1.5     4           8        11       14
//! ```
//!
//! - **The hall:** x 0-12, y 0-9; floor at 0, ceiling at 4 m, 0.5 m thick.
//! - **The platform:** x 8-12, y 6-9, its top 0.4 m up; **the step** in
//!   front of it, x 8-12, y 5.25-6, its top 0.2 m up. Each rise is under
//!   `GroundConfig::DEFAULT`'s 0.4 m, so the hull walks up; the platform's
//!   open west side, 0.4 m from the floor, climbs too, and a walk off it
//!   snaps down (snap 0.4 m).
//! - **The ledge:** x 7-11, y 0-1, its top 0.6 m up: more than a step, so it
//!   blocks.
//! - **The pillar:** x 5.5-6.5, y 4-5, floor to ceiling.
//! - **The alcove:** a recess in the east wall, x 12-14, y 1-3, with a 2.5 m
//!   ceiling; its ceiling slab, 2.5-4.5 m, closes the opening above 2.5 m.
//! - **The skylight:** a 2 m x 2 m hole in the hall's ceiling, x 3-5, y 6-8;
//!   the four ceiling slabs around it are its shaft, open above 4.5 m.
//! - **Regions** ([`Region`]; a point's is the last that holds it): 0, the
//!   hall, light 160; 1, the platform and its step, 192; 2, the alcove, 112
//!   (its green fog is a look, `game-testbed-client`'s); 3, under the
//!   skylight, 255, open to the sky.
//! - **Spawns**, every one facing east (yaw 0), the hull on the floor:
//!   S0 (1.5, 4.5), S1 (1.5, 3), S2 (1.5, 6), S3 (1.5, 1.5); player `p`
//!   takes spawn `p % 4`.
//! - **Things** ([`ThingSpawn`]): M, the marker, at (4, 4.5), facing west
//!   toward S0 and turning once every 8 s (480 ticks), counter-clockwise, so
//!   from S0 each of its 8 rotations shows for one second, centred on the
//!   whole seconds; C, a crate, at (8, 4.5), facing west, behind the pillar
//!   as seen from S0; B, a barrel, on the platform at (10.5, 7.5), facing
//!   S0 to within 0.2 degrees.

use alloc::vec;
use alloc::vec::Vec;

use eng_collision_api::BoxHull;
use eng_collision_grid::{Point2, Slab, Wall};
use eng_math::{BAngle, Vec3, WorldPos};
use kit_fps::{GroundConfig, MoveConfig};

use super::{Region, RoomMap, Spawn, ThingKind, ThingSpawn};

/// The hall's east-west size (m).
pub const HALL_X: f64 = 12.0;
/// The hall's north-south size (m).
pub const HALL_Y: f64 = 9.0;
/// The hall's ceiling: the underside of its ceiling slabs (m).
pub const CEILING: f64 = 4.0;
/// The top of the ceiling slabs, where the skylight's shaft opens (m).
pub const ROOF: f64 = 4.5;
/// The alcove's ceiling (m).
pub const ALCOVE_CEILING: f64 = 2.5;
/// The top of the step in front of the platform (m).
pub const STEP_TOP: f64 = 0.2;
/// The top of the platform (m).
pub const PLATFORM_TOP: f64 = 0.4;
/// The top of the ledge (m): higher than a step.
pub const LEDGE_TOP: f64 = 0.6;
/// How deep the floor slabs go below the floor (m).
const FLOOR_DEPTH: f64 = 0.5;

/// The player's hull (decision 3): 0.5 m x 0.5 m x 1.75 m.
pub const HULL: BoxHull = BoxHull {
    half_extents: Vec3::new(0.25, 0.25, 0.875),
};

/// The player's eye above its feet (m), decision 3. The sim does not use
/// it; a first-person camera does.
pub const EYE_HEIGHT: f64 = 1.6;

/// The room's movement tuning: the dot's speeds with the room's hull.
pub const MOVE: MoveConfig = MoveConfig {
    max_speed: 4.0,
    accel: 10.0,
    friction: 6.0,
    stop_speed: 1.0,
    hull: HULL,
};

/// The room's ground movement: `kit-fps`'s defaults (decision 3).
pub const GROUND: GroundConfig = GroundConfig::DEFAULT;

/// Ticks per turn of the marker: 8 s at 60 Hz.
pub const MARKER_TURN_TICKS: u32 = 480;

/// 141/256 of a turn, 198.28 degrees: the barrel's facing, toward S0
/// (198.43 degrees from it).
const TOWARD_S0_FROM_B: BAngle = BAngle(0x8d00_0000);

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

fn wall(a: (f64, f64), b: (f64, f64), bottom: f64, top: f64) -> Wall {
    Wall {
        a: p(a.0, a.1),
        b: p(b.0, b.1),
        bottom,
        top,
    }
}

/// An axis-aligned box from `(x0, y0)` to `(x1, y1)`, `bottom` to `top`,
/// corners counter-clockwise from the south-west.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64, bottom: f64, top: f64) -> Slab {
    Slab {
        points: vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)],
        bottom,
        top,
    }
}

/// The room's walls, counter-clockwise around its floor plan from the
/// south-west corner. Their indices come before the slabs' and break ties
/// between hits (decision 2).
pub fn walls() -> Vec<Wall> {
    vec![
        wall((0.0, 0.0), (HALL_X, 0.0), 0.0, CEILING),
        wall((HALL_X, 0.0), (HALL_X, 1.0), 0.0, CEILING),
        // The alcove: south, back, north.
        wall((HALL_X, 1.0), (14.0, 1.0), 0.0, ALCOVE_CEILING),
        wall((14.0, 1.0), (14.0, 3.0), 0.0, ALCOVE_CEILING),
        wall((14.0, 3.0), (HALL_X, 3.0), 0.0, ALCOVE_CEILING),
        wall((HALL_X, 3.0), (HALL_X, HALL_Y), 0.0, CEILING),
        wall((HALL_X, HALL_Y), (0.0, HALL_Y), 0.0, CEILING),
        wall((0.0, HALL_Y), (0.0, 0.0), 0.0, CEILING),
    ]
}

/// The room's slabs: floors, the step, the platform, the ledge, the pillar,
/// then the ceilings.
pub fn slabs() -> Vec<Slab> {
    let (floor, ceiling) = (-FLOOR_DEPTH, CEILING);
    vec![
        rect(0.0, 0.0, HALL_X, HALL_Y, floor, 0.0),
        rect(HALL_X, 1.0, 14.0, 3.0, floor, 0.0),
        rect(8.0, 5.25, HALL_X, 6.0, 0.0, STEP_TOP),
        rect(8.0, 6.0, HALL_X, HALL_Y, 0.0, PLATFORM_TOP),
        rect(7.0, 0.0, 11.0, 1.0, 0.0, LEDGE_TOP),
        rect(5.5, 4.0, 6.5, 5.0, 0.0, ceiling),
        // The hall's ceiling around the skylight: south, north, west, east.
        rect(0.0, 0.0, HALL_X, 6.0, ceiling, ROOF),
        rect(0.0, 8.0, HALL_X, HALL_Y, ceiling, ROOF),
        rect(0.0, 6.0, 3.0, 8.0, ceiling, ROOF),
        rect(5.0, 6.0, HALL_X, 8.0, ceiling, ROOF),
        rect(HALL_X, 1.0, 14.0, 3.0, ALCOVE_CEILING, ROOF),
    ]
}

/// The room's regions, spawns and things.
pub fn map() -> RoomMap {
    let region = |x0, y0, x1, y1, light, sky| Region {
        min: p(x0, y0),
        max: p(x1, y1),
        light,
        sky,
    };
    let spawn = |x, y| Spawn {
        feet: WorldPos::new(x, y, 0.0),
        yaw: BAngle::ZERO,
    };
    let thing = |kind, x, y, z, facing, turn_ticks| ThingSpawn {
        kind,
        feet: WorldPos::new(x, y, z),
        facing,
        turn_ticks,
    };
    RoomMap {
        regions: vec![
            region(0.0, 0.0, HALL_X, HALL_Y, 160, false),
            region(8.0, 5.25, HALL_X, HALL_Y, 192, false),
            region(HALL_X, 1.0, 14.0, 3.0, 112, false),
            region(3.0, 6.0, 5.0, 8.0, 255, true),
        ],
        spawns: vec![
            spawn(1.5, 4.5),
            spawn(1.5, 3.0),
            spawn(1.5, 6.0),
            spawn(1.5, 1.5),
        ],
        things: vec![
            thing(
                ThingKind::Marker,
                4.0,
                4.5,
                0.0,
                BAngle::HALF,
                MARKER_TURN_TICKS,
            ),
            thing(ThingKind::Crate, 8.0, 4.5, 0.0, BAngle::HALF, 0),
            thing(
                ThingKind::Barrel,
                10.5,
                7.5,
                PLATFORM_TOP,
                TOWARD_S0_FROM_B,
                0,
            ),
        ],
    }
}
