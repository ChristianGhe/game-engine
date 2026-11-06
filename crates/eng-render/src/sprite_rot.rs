//! Which of a sprite's 8 rotations to show (spec 6.7, 11; rung 2 plan,
//! decision 4), worked out on the CPU each frame.
//!
//! - **The rule, from the geometry.** Around a thing, seen from above, lie
//!   8 sectors of 45°, the first centred on the thing's facing. The
//!   rotation shown is the number of the sector that holds the direction
//!   from the thing to the viewer, counting counter-clockwise (toward +Y
//!   from +X, the way yaw grows). So a thing facing the viewer shows
//!   rotation 0, its front; a viewer a quarter turn counter-clockwise of
//!   the facing sees rotation 2, side-on; one behind it sees rotation 4.
//! - **In binary angles** a sector is 2^29 and half of one 2^28, so the
//!   rotation is `((to_viewer - facing) + 2^28) >> 29`, all wrapping `u32`
//!   arithmetic. A direction exactly on a boundary lands in the sector
//!   counter-clockwise of it: `facing + 2^28` gives 1, `facing - 2^28`
//!   gives 0.
//! - **The direction** from the thing to the viewer is the one float step:
//!   eng-math's `atan2_f64` of the f64 difference of their positions,
//!   rounded to the nearest binary angle. Heights are ignored.
//! - **Sheets.** A thing whose sprite has 8 drawn rotations shows each as
//!   it is. A left-right symmetric thing may draw only rotations 0 to 4
//!   (a five-rotation sheet), and shows rotation `k` above 4 as rotation
//!   `8 - k` mirrored left to right ([`RotationSheet::frame`]).

use eng_math::{BAngle, TAU_F64, WorldPos, atan2_f64, round_f64};

/// How many rotations a sprite has.
pub const SPRITE_ROTATIONS: u8 = 8;

/// Half a 45° sector, in binary angle units (22.5°).
const HALF_SECTOR: u32 = 1 << 28;
/// The shift from a binary angle to its sector (45° is 2^29).
const SECTOR_SHIFT: u32 = 29;

/// The rotation (0 to 7) a thing facing `facing` shows to a viewer in the
/// direction `to_viewer` from it, both as binary angles. See the module
/// docs for the rule and its tie at sector boundaries.
pub const fn sprite_rotation(facing: BAngle, to_viewer: BAngle) -> u8 {
    let from_front = to_viewer.0.wrapping_sub(facing.0);
    (from_front.wrapping_add(HALF_SECTOR) >> SECTOR_SHIFT) as u8
}

/// The direction from `from` to `to` seen from above, as the binary angle
/// nearest it (0 along +X, a quarter turn along +Y). Heights are ignored.
/// `None` when the two share x and y, where no direction exists.
pub fn direction_to(from: WorldPos, to: WorldPos) -> Option<BAngle> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    // atan2 gives [-pi, pi] (-pi when dy is -0.0 and dx is negative); as a
    // signed count of 2^-32 turns that is [-2^31, 2^31], which wraps into
    // the u32 circle, both ends to a half turn.
    let turns = atan2_f64(dy, dx) / TAU_F64;
    let units = round_f64(turns * 4_294_967_296.0) as i64;
    Some(BAngle(units as u32))
}

/// The rotation a thing at `thing`, facing `facing`, shows to a viewer at
/// `viewer`: [`sprite_rotation`] of [`direction_to`] from the thing to the
/// viewer. A viewer straight above or below the thing sees its front, 0.
pub fn sprite_rotation_toward(thing: WorldPos, facing: BAngle, viewer: WorldPos) -> u8 {
    match direction_to(thing, viewer) {
        Some(to_viewer) => sprite_rotation(facing, to_viewer),
        None => 0,
    }
}

/// How many distinct rotations a sprite sheet draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationSheet {
    /// All 8 rotations, each drawn: rotation `k` is frame `k`.
    Eight,
    /// Rotations 0 to 4 drawn, for a thing that is left-right symmetric:
    /// rotation `k` above 4 is frame `8 - k`, mirrored left to right.
    Five,
}

/// The frame of a sheet to draw, and whether to mirror it left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetFrame {
    pub frame: u8,
    pub mirrored: bool,
}

impl RotationSheet {
    /// How many frames the sheet holds.
    pub const fn frames(self) -> u8 {
        match self {
            Self::Eight => 8,
            Self::Five => 5,
        }
    }

    /// The frame that shows `rotation`, taken modulo 8.
    pub const fn frame(self, rotation: u8) -> SheetFrame {
        let rotation = rotation % SPRITE_ROTATIONS;
        match self {
            Self::Five if rotation > 4 => SheetFrame {
                frame: SPRITE_ROTATIONS - rotation,
                mirrored: true,
            },
            Self::Eight | Self::Five => SheetFrame {
                frame: rotation,
                mirrored: false,
            },
        }
    }
}
