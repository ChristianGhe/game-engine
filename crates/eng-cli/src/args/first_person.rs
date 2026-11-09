//! `play`'s first-person flags (rung 2, T15; plan decisions 9 and 10):
//!
//! ```text
//! [--windowed] [--fov DEG] [--look look.toml] [--sensitivity DEG_PER_COUNT]
//! [--pitch-limit DEG] [--invert-pitch]
//! ```
//!
//! Degrees are written as plain decimals, at most six places (`0.066`,
//! `80`, `73.5`), and read exactly, as whole millionths of a degree, so
//! the value a player writes is the value used: no float rounding reaches
//! a setting. A sign, an exponent, a lone `.` or a seventh place is
//! refused.
//!
//! - `--sensitivity`: degrees per mouse count, more than 0 and at most
//!   4294.967295 (`LookSettings::sensitivity_microdeg`, a `u32`).
//! - `--pitch-limit`: 0 to 90 degrees, rounded down to a binary angle
//!   (65,536 a turn), so `80` is 14,563 units, `LookSettings`' default,
//!   where rounding to nearest would give 14,564.
//! - `--fov`: the vertical field of view, more than 0 and less than 180
//!   degrees, rounded to the nearest of 2^32 a turn (`BAngle`). The near
//!   plane follows it and the window's aspect (eng-app's `lens.rs`).

use std::path::PathBuf;

use eng_math::BAngle;

/// `play`'s first-person flags, as given; the map decides whether they
/// apply (`play/first_person.rs`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FirstPersonFlags {
    /// `--windowed`: a window instead of borderless fullscreen.
    pub windowed: bool,
    /// `--fov`: the vertical field of view.
    pub fov_y: Option<BAngle>,
    /// `--look`: the look switches' file.
    pub look: Option<PathBuf>,
    /// `--sensitivity`, in millionths of a degree per count.
    pub sensitivity_microdeg: Option<u32>,
    /// `--pitch-limit`, in binary-angle units.
    pub pitch_limit: Option<u16>,
    /// `--invert-pitch`.
    pub invert_pitch: bool,
}

impl FirstPersonFlags {
    /// The first flag given, by its name, for a message refusing them on
    /// a map without a first-person view.
    pub fn first_given(&self) -> Option<&'static str> {
        [
            (self.windowed, "--windowed"),
            (self.fov_y.is_some(), "--fov"),
            (self.look.is_some(), "--look"),
            (self.sensitivity_microdeg.is_some(), "--sensitivity"),
            (self.pitch_limit.is_some(), "--pitch-limit"),
            (self.invert_pitch, "--invert-pitch"),
        ]
        .into_iter()
        .find_map(|(given, name)| given.then_some(name))
    }
}

/// Millionths of a degree in a turn.
const TURN_MICRODEG: u128 = 360_000_000;

/// A plain decimal with at most six places, in millionths: `0.066` is
/// 66,000. At most 12 digits before the point, so it fits.
pub(crate) fn millionths(flag: &str, raw: &str) -> Result<u64, String> {
    let refuse = || {
        format!(
            "{flag} needs a plain decimal with at most six places, such as 0.066 or 80, got \
             {raw:?}"
        )
    };
    let (whole, places) = raw.split_once('.').unwrap_or((raw, ""));
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || whole.len() > 12 || (raw.contains('.') && !digits(places)) {
        return Err(refuse());
    }
    if places.len() > 6 {
        return Err(refuse());
    }
    let whole: u64 = whole.parse().map_err(|_| refuse())?;
    let fraction: u64 = format!("{places:0<6}").parse().map_err(|_| refuse())?;
    Ok(whole * 1_000_000 + fraction)
}

/// `--sensitivity`: degrees per count in millionths, 1 to `u32::MAX`.
pub(crate) fn sensitivity(raw: &str) -> Result<u32, String> {
    let m = millionths("--sensitivity", raw)?;
    u32::try_from(m).ok().filter(|&m| m > 0).ok_or_else(|| {
        format!(
            "--sensitivity needs more than 0 and at most 4294.967295 degrees per count, got \
                 {raw}"
        )
    })
}

/// `--pitch-limit`: 0 to 90 degrees, rounded down to binary-angle units.
pub(crate) fn pitch_limit(raw: &str) -> Result<u16, String> {
    let m = millionths("--pitch-limit", raw)?;
    if m > 90_000_000 {
        return Err(format!("--pitch-limit needs 0 to 90 degrees, got {raw}"));
    }
    // At most a quarter turn, 16,384 units, so it fits.
    Ok((u128::from(m) * 65_536 / TURN_MICRODEG) as u16)
}

/// `--fov`: more than 0 and less than 180 degrees, to the nearest
/// 2^-32 of a turn (half up).
pub(crate) fn fov(raw: &str) -> Result<BAngle, String> {
    let m = millionths("--fov", raw)?;
    if m == 0 || m >= 180_000_000 {
        return Err(format!(
            "--fov needs a vertical field of view of more than 0 and less than 180 degrees, got \
             {raw}"
        ));
    }
    // Below a half turn, so it fits in a u32; at least 12 units above 0.
    let angle = (u128::from(m) * (1 << 32) + TURN_MICRODEG / 2) / TURN_MICRODEG;
    Ok(BAngle(angle as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_are_read_exactly_in_millionths() {
        for (raw, want) in [
            ("0.066", 66_000),
            ("80", 80_000_000),
            ("0", 0),
            ("1.5", 1_500_000),
            ("0.000001", 1),
            ("4294.967295", 4_294_967_295),
            ("007.25", 7_250_000),
            ("999999999999.999999", 999_999_999_999_999_999),
        ] {
            assert_eq!(millionths("--x", raw), Ok(want), "{raw}");
        }
        for raw in [
            "",
            ".",
            "1.",
            ".5",
            "-1",
            "+1",
            "1e3",
            "0.0000001",
            "1.2.3",
            "0x10",
            " 1",
            "1 ",
            "1,5",
            "NaN",
            "inf",
            "1000000000000",
        ] {
            let err = millionths("--x", raw).unwrap_err();
            assert!(err.starts_with("--x needs a plain decimal"), "{raw}: {err}");
        }
    }

    #[test]
    fn sensitivity_is_millionths_of_a_degree_above_0() {
        assert_eq!(sensitivity("0.066"), Ok(66_000));
        assert_eq!(sensitivity("0.022"), Ok(22_000));
        assert_eq!(sensitivity("4294.967295"), Ok(u32::MAX));
        for bad in ["0", "0.000000", "4294.967296", "0.0660001"] {
            assert!(sensitivity(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_pitch_limit_rounds_down() {
        // 80 x 65536 / 360 = 14563.56: down to the default, where rounding
        // to nearest gives 14564.
        assert_eq!(pitch_limit("80"), Ok(14_563));
        assert_eq!(pitch_limit("90"), Ok(16_384));
        assert_eq!(pitch_limit("0"), Ok(0));
        // 45 degrees is exactly 8192 units; a millionth below it, 8191.
        assert_eq!(pitch_limit("45"), Ok(8_192));
        assert_eq!(pitch_limit("44.999999"), Ok(8_191));
        assert!(pitch_limit("90.000001").is_err());
    }

    #[test]
    fn the_field_of_view_is_a_binary_angle_strictly_inside_a_half_turn() {
        assert_eq!(fov("90"), Ok(BAngle::QUARTER));
        // The room's 2 atan(3/4) = 73.739795...°, to six places.
        assert_eq!(fov("73.739795"), Ok(BAngle(879_750_022)));
        assert_eq!(fov("0.000001"), Ok(BAngle(12)));
        assert_eq!(fov("179.999999"), Ok(BAngle(BAngle::HALF.0 - 12)));
        for bad in ["0", "180", "200"] {
            assert!(fov(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_first_flag_given_names_them() {
        let mut flags = FirstPersonFlags::default();
        assert_eq!(flags.first_given(), None);
        flags.invert_pitch = true;
        assert_eq!(flags.first_given(), Some("--invert-pitch"));
        flags.fov_y = Some(BAngle::QUARTER);
        assert_eq!(flags.first_given(), Some("--fov"));
    }
}
