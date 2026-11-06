//! The sprite rotation rule (rung 2 plan, decision 4; queue item Q4,
//! done-when 2) and the mirror flags. No GPU is needed.
//!
//! Seen from above with +X to the right and +Y up the page (AGENTS.md:
//! right-handed, Z up, yaw growing toward +Y), "counter-clockwise" is the
//! direction binary angles grow in, so the plan's "rotation k: the viewer
//! is k × 45° counter-clockwise of the facing" and AGENTS.md agree.

use eng_math::{BAngle, TAU_F64, WorldPos, atan2_f64, floor_f64, min_f64};
use eng_render::{
    RotationSheet, SPRITE_ROTATIONS, SheetFrame, direction_to, sprite_rotation,
    sprite_rotation_toward,
};
use proptest::prelude::*;

const SECTOR: u32 = 1 << 29;
const HALF_SECTOR: u32 = 1 << 28;

fn at(x: f64, y: f64) -> WorldPos {
    WorldPos::new(x, y, 0.0)
}

/// The oracle, from the geometry alone and in radians: the viewer's
/// bearing from the thing by f64 atan2, less the facing, measured
/// counter-clockwise in [0, 2π), shifted half a sector and cut into 8.
/// Also how far the direction sits from the nearest sector boundary, in
/// sectors.
fn oracle(thing: WorldPos, facing: BAngle, viewer: WorldPos) -> (u8, f64) {
    let bearing = atan2_f64(viewer.y - thing.y, viewer.x - thing.x);
    let facing = f64::from(facing.0) / 4_294_967_296.0 * TAU_F64;
    let from_front = (bearing - facing).rem_euclid(TAU_F64);
    let sectors = from_front / (TAU_F64 / 8.0) + 0.5;
    let whole = floor_f64(sectors);
    let margin = min_f64(sectors - whole, 1.0 - (sectors - whole));
    ((whole as i64).rem_euclid(8) as u8, margin)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// Away from the sector boundaries, the integer rule picks the sector
    /// the oracle does.
    #[test]
    fn the_rule_matches_the_geometry(
        thing in (-1000.0f64..1000.0, -1000.0f64..1000.0, -5.0f64..5.0),
        offset in (-200.0f64..200.0, -200.0f64..200.0, -5.0f64..5.0),
        facing in any::<u32>(),
    ) {
        let thing = WorldPos::new(thing.0, thing.1, thing.2);
        let viewer = WorldPos::new(thing.x + offset.0, thing.y + offset.1, thing.z + offset.2);
        prop_assume!(offset.0.abs() + offset.1.abs() > 1e-3);
        let (want, margin) = oracle(thing, BAngle(facing), viewer);
        // 1e-6 of a sector is about 540 binary-angle units: far more than
        // the rounding of atan2 and of the angle itself.
        prop_assume!(margin > 1e-6);
        prop_assert_eq!(sprite_rotation_toward(thing, BAngle(facing), viewer), want);
    }

    /// The rule depends only on the angle between the facing and the
    /// direction to the viewer.
    #[test]
    fn turning_both_keeps_the_rotation(
        facing in any::<u32>(),
        to_viewer in any::<u32>(),
        turn in any::<u32>(),
    ) {
        let turned = sprite_rotation(
            BAngle(facing.wrapping_add(turn)),
            BAngle(to_viewer.wrapping_add(turn)),
        );
        prop_assert_eq!(turned, sprite_rotation(BAngle(facing), BAngle(to_viewer)));
        prop_assert!(turned < SPRITE_ROTATIONS);
    }
}

#[test]
fn a_thing_facing_the_viewer_shows_its_front() {
    // Facing +X with the viewer on +X; facing +Y with the viewer on +Y.
    assert_eq!(
        sprite_rotation_toward(at(0.0, 0.0), BAngle::ZERO, at(5.0, 0.0)),
        0
    );
    assert_eq!(
        sprite_rotation_toward(at(3.0, -2.0), BAngle::QUARTER, at(3.0, 40.0)),
        0
    );
    assert_eq!(sprite_rotation(BAngle(0xDEAD_BEEF), BAngle(0xDEAD_BEEF)), 0);
}

#[test]
fn a_viewer_a_quarter_turn_counter_clockwise_sees_rotation_two() {
    // Facing +X; the viewer on +Y is 90° counter-clockwise seen from above.
    assert_eq!(
        sprite_rotation_toward(at(0.0, 0.0), BAngle::ZERO, at(0.0, 5.0)),
        2
    );
    // Clockwise instead (-Y) is rotation 6.
    assert_eq!(
        sprite_rotation_toward(at(0.0, 0.0), BAngle::ZERO, at(0.0, -5.0)),
        6
    );
    // 45° each way: 1 and 7.
    assert_eq!(
        sprite_rotation_toward(at(0.0, 0.0), BAngle::ZERO, at(5.0, 5.0)),
        1
    );
    assert_eq!(
        sprite_rotation_toward(at(0.0, 0.0), BAngle::ZERO, at(5.0, -5.0)),
        7
    );
    // Facing +Y, the viewer on -X is a quarter turn counter-clockwise.
    let facing_y = BAngle::QUARTER;
    assert_eq!(
        sprite_rotation_toward(at(1.0, 1.0), facing_y, at(-9.0, 1.0)),
        2
    );
}

#[test]
fn a_viewer_behind_sees_rotation_four() {
    assert_eq!(
        sprite_rotation_toward(at(0.0, 0.0), BAngle::ZERO, at(-5.0, 0.0)),
        4
    );
    assert_eq!(
        sprite_rotation_toward(at(2.0, 2.0), BAngle::QUARTER, at(2.0, -7.0)),
        4
    );
    assert_eq!(sprite_rotation(BAngle(0x1234_5678), BAngle(0x9234_5678)), 4);
}

#[test]
fn a_boundary_goes_to_the_counter_clockwise_sector() {
    for facing in [0, 0x1234_5678, 0xFFFF_FFFF, 0x9000_0000] {
        let at = |delta: u32| sprite_rotation(BAngle(facing), BAngle(facing.wrapping_add(delta)));
        // Exactly 2^28 past the facing: the boundary of sectors 0 and 1.
        assert_eq!(at(HALF_SECTOR), 1, "facing {facing:#x}");
        assert_eq!(at(HALF_SECTOR - 1), 0, "facing {facing:#x}");
        // Exactly 2^28 before it: the boundary of sectors 7 and 0.
        assert_eq!(at(HALF_SECTOR.wrapping_neg()), 0, "facing {facing:#x}");
        assert_eq!(at(HALF_SECTOR.wrapping_neg() - 1), 7, "facing {facing:#x}");
        // Every boundary k × 45° + 22.5° goes to sector k + 1.
        for k in 0..8u32 {
            let boundary = k * SECTOR + HALF_SECTOR;
            assert_eq!(
                u32::from(at(boundary)),
                (k + 1) % 8,
                "facing {facing:#x}, k {k}"
            );
            assert_eq!(u32::from(at(boundary - 1)), k, "facing {facing:#x}, k {k}");
        }
    }
}

#[test]
fn directions_follow_the_yaw_convention() {
    let o = at(0.0, 0.0);
    assert_eq!(direction_to(o, at(3.0, 0.0)), Some(BAngle::ZERO));
    assert_eq!(direction_to(o, at(0.0, 3.0)), Some(BAngle::QUARTER));
    assert_eq!(direction_to(o, at(-3.0, 0.0)), Some(BAngle::HALF));
    assert_eq!(direction_to(o, at(-3.0, -0.0)), Some(BAngle::HALF));
    assert_eq!(direction_to(o, at(0.0, -3.0)), Some(BAngle(3 << 30)));
    assert_eq!(direction_to(o, at(2.0, 2.0)), Some(BAngle(SECTOR)));
    // Heights are ignored; with no horizontal offset there is no direction.
    assert_eq!(
        direction_to(o, WorldPos::new(0.0, 3.0, 9.0)),
        Some(BAngle::QUARTER)
    );
    assert_eq!(direction_to(o, WorldPos::new(0.0, 0.0, 9.0)), None);
    // A viewer straight above sees the front.
    let above = WorldPos::new(4.0, 4.0, 10.0);
    assert_eq!(
        sprite_rotation_toward(at(4.0, 4.0), BAngle(0x7000_0000), above),
        0
    );
}

#[test]
fn the_mirror_table() {
    let frame = |frame, mirrored| SheetFrame { frame, mirrored };
    // Five drawn rotations: 5, 6 and 7 are 3, 2 and 1 mirrored.
    let five = [
        frame(0, false),
        frame(1, false),
        frame(2, false),
        frame(3, false),
        frame(4, false),
        frame(3, true),
        frame(2, true),
        frame(1, true),
    ];
    for (rotation, want) in (0..8).zip(five) {
        assert_eq!(
            RotationSheet::Five.frame(rotation),
            want,
            "rotation {rotation}"
        );
        // Rotations count modulo 8.
        assert_eq!(RotationSheet::Five.frame(rotation + 8), want);
    }
    // Eight drawn rotations: each as it is.
    for rotation in 0..8 {
        assert_eq!(RotationSheet::Eight.frame(rotation), frame(rotation, false));
    }
    assert_eq!(RotationSheet::Five.frames(), 5);
    assert_eq!(RotationSheet::Eight.frames(), 8);
    // A frame is always one the sheet has.
    for sheet in [RotationSheet::Five, RotationSheet::Eight] {
        assert!((0..=255).all(|r| sheet.frame(r).frame < sheet.frames()));
    }
}
