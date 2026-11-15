//! The void: pixels showing nothing below the horizon (record version 2),
//! against an independent reckoning of each row's ray.

use eng_math::{BAngle, cos_f64, sin_f64};
use eng_render::{FirstPersonCamera, SurfaceClass};

use super::super::first_person_entry;
use super::{H, W, asked, camera, key, shows, targets};

/// The rows below the horizon by an independent reckoning: in f64, the
/// ray through row `y`'s centre rises by `sin(pitch) + cos(pitch) tan(fov
/// / 2) ndc_y`, ndc_y being `1 - (2y + 1) / H`.
fn void_by_hand(pitch_deg: f64) -> u64 {
    let half = 879_750_026.0 / 4_294_967_296.0 * std::f64::consts::PI;
    let p = pitch_deg / 180.0 * std::f64::consts::PI;
    let tan_half = sin_f64(half) / cos_f64(half);
    (0..H)
        .filter(|y| {
            let ndc = 1.0 - f64::from(2 * y + 1) / f64::from(H);
            sin_f64(p) + cos_f64(p) * tan_half * ndc < 0.0
        })
        .map(|y| {
            (0..W)
                .filter(|x| shows(*x, y) == Some(key(SurfaceClass::None, 0)))
                .count() as u64
        })
        .sum()
}

#[test]
fn the_void_is_the_sky_below_the_horizon() {
    let t = targets();
    // Pitches whose horizon falls well between two rows' centres: level
    // (between rows 5 and 6), 11.25 degrees up (between rows 7 and 8), 45
    // down (every row), straight down and straight up.
    for (pitch_deg, turn) in [
        (0.0, 0_i32),
        (11.25, 1 << 27),
        (-45.0, -(1 << 29)),
        (-90.0, -(1 << 30)),
        (90.0, 1 << 30),
    ] {
        let camera = FirstPersonCamera {
            pitch: BAngle(turn as u32),
            ..camera()
        };
        let frame = first_person_entry(0, &camera, &asked(), &t.info, &t.depth, &t.image).unwrap();
        let none = frame.classes.unwrap()[usize::from(SurfaceClass::None.code())];
        let want = void_by_hand(pitch_deg);
        assert_eq!(frame.void, Some(want), "pitch {pitch_deg}");
        match pitch_deg {
            -90.0 => assert_eq!(
                want,
                none.iter().sum::<u64>(),
                "all sky is void looking down"
            ),
            90.0 => assert_eq!(want, 0, "none looking up"),
            _ => {}
        }
    }
}
