//! What `shot` and `video` are asked to draw beside the map (rung 2, T16
//! and plan addition A3): `--view`, `--pose` and `--look`.
//!
//! - `--view top-down|first-person` names the view the command expects, by
//!   [`MapView::name`]. A map has one view, so the flag only confirms it: a
//!   view the map lacks is refused with `[E-USAGE] [E-MAP]` (exit 64)
//!   before the session or a GPU opens. The testbed's top-down view reads
//!   the dot room's resources, so drawing the room top-down would panic.
//! - `--pose X,Y,Z,YAW,PITCH` sets a first-person camera independent of
//!   movement ([`Pose`]): the eye at (X, Y, Z) in meters, absolute world
//!   coordinates, heading YAW and elevation PITCH in degrees (yaw 0 looks
//!   along +X, positive yaw turns towards +Y, positive pitch looks up, at
//!   most 90 either way). It replaces the eye and angles of the camera the
//!   game's scene picks, and keeps its field of view and near plane, as
//!   `play`'s live view angles do. A top-down view refuses it.
//! - `--look look.toml` switches look features with the file `play --look`
//!   reads (`look_file.rs`, [`crate::LookSwitches`], version 1): each key
//!   given sets its feature over the scene's look, the rest stay as the
//!   game's scene has them. A top-down view refuses it. Shots ignore
//!   `low_resolution`, which is a window's business.
//!
//! A shot's or video's report names the view it drew ([`view_json`]): its
//! kind, the pose and look asked for, and the camera of the last frame.

use eng_math::{BAngle, WorldPos};
use eng_render::{FirstPersonCamera, Look};
use eng_schema_json::JsonValue;

use crate::GameDef;
use crate::args::RunArgs;
use crate::maps::MapChoice;
use crate::view::MapView;

/// The views `--view` can name, as [`MapView::name`] writes them.
const VIEWS: [&str; 2] = ["top-down", "first-person"];

/// `--view`'s value: one of [`VIEWS`].
pub(crate) fn parse_view(raw: &str) -> Result<&'static str, String> {
    VIEWS
        .into_iter()
        .find(|view| *view == raw)
        .ok_or_else(|| format!("--view is top-down or first-person, not {raw:?}"))
}

/// A full turn in [`BAngle`] units.
const TURN: f64 = 4_294_967_296.0;

/// The binary angle nearest to `deg` degrees (2^32 to a turn), wrapping;
/// `deg` must be finite. Exact IEEE arithmetic only, so every target gives
/// the same angle.
pub fn degrees_to_bangle(deg: f64) -> BAngle {
    let turns = deg.rem_euclid(360.0) / 360.0;
    // A value that rounds up to a whole turn wraps to 0.
    BAngle((turns * TURN).round() as u64 as u32)
}

/// An angle in degrees, `signed` from -180 to 180 (as a pitch reads), else
/// from 0 to 360.
fn bangle_degrees(angle: BAngle, signed: bool) -> f64 {
    let units = if signed {
        f64::from(angle.0 as i32)
    } else {
        f64::from(angle.0)
    };
    units * 360.0 / TURN
}

/// `--pose`: a first-person camera's eye and angles, independent of
/// movement (plan addition A3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// The eye, absolute, in meters.
    pub eye: WorldPos,
    /// The heading in degrees: 0 along +X, 90 along +Y.
    pub yaw_deg: f64,
    /// The elevation in degrees, -90 to 90: positive looks up.
    pub pitch_deg: f64,
}

impl Pose {
    /// `X,Y,Z,YAW,PITCH`: five finite decimal numbers, the pitch from -90
    /// to 90.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let fail = || {
            format!(
                "--pose needs X,Y,Z,YAW,PITCH: the eye in meters, then yaw and pitch in \
                 degrees (pitch -90 to 90), all finite; got {raw:?}"
            )
        };
        let numbers: Vec<f64> = raw
            .split(',')
            .map(|part| part.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect::<Option<_>>()
            .ok_or_else(fail)?;
        let [x, y, z, yaw_deg, pitch_deg] = <[f64; 5]>::try_from(numbers).map_err(|_| fail())?;
        if !(-90.0..=90.0).contains(&pitch_deg) {
            return Err(fail());
        }
        Ok(Self {
            eye: WorldPos::new(x, y, z),
            yaw_deg,
            pitch_deg,
        })
    }

    pub fn yaw(&self) -> BAngle {
        degrees_to_bangle(self.yaw_deg)
    }

    /// Read as a signed angle, as [`FirstPersonCamera::pitch`] is.
    pub fn pitch(&self) -> BAngle {
        degrees_to_bangle(self.pitch_deg)
    }

    /// `camera` with this pose's eye and angles; its field of view and
    /// near plane stay.
    pub fn apply(&self, camera: FirstPersonCamera) -> FirstPersonCamera {
        FirstPersonCamera {
            eye: self.eye,
            yaw: self.yaw(),
            pitch: self.pitch(),
            ..camera
        }
    }

    fn json(&self) -> JsonValue {
        JsonValue::object([
            ("eye", vec3(self.eye)),
            ("yaw_deg", JsonValue::F64(self.yaw_deg)),
            ("pitch_deg", JsonValue::F64(self.pitch_deg)),
        ])
    }
}

fn vec3(p: WorldPos) -> JsonValue {
    JsonValue::Array(vec![
        JsonValue::F64(p.x),
        JsonValue::F64(p.y),
        JsonValue::F64(p.z),
    ])
}

/// Why `shot` or `video` cannot draw `map` as `a` asks, if they cannot:
/// `--view` names a view the map lacks, or `--pose` or `--look` asks a
/// view that is not first person. A usage error (exit 64), found before
/// the session or a GPU opens. A map with no view at all is
/// [`MapChoice::no_view`]'s.
pub(crate) fn refusal(game: &GameDef, map: MapChoice, a: &RunArgs) -> Option<String> {
    let view = map.view();
    if matches!(view, MapView::None) {
        return None;
    }
    let (name, has) = (map.name(), view.name());
    if let Some(asked) = a.view.filter(|asked| *asked != has) {
        return Some(format!(
            "[E-USAGE] [E-MAP] {}'s map {name:?} has no {asked} view (its view is {has})",
            game.name
        ));
    }
    let first_person = matches!(view, MapView::FirstPerson(_));
    let first_person_only = [(a.pose.is_some(), "--pose"), (a.look.is_some(), "--look")];
    let (_, flag) = first_person_only
        .into_iter()
        .find(|(given, _)| *given && !first_person)?;
    Some(format!(
        "[E-USAGE] {flag} needs a first-person view; {}'s map {name:?} is drawn {has}",
        game.name
    ))
}

/// The look as the report names it.
fn look_json(look: &Look) -> JsonValue {
    JsonValue::object([
        ("falloff", JsonValue::Bool(look.falloff)),
        ("banding", JsonValue::Bool(look.banding)),
        ("contrast", JsonValue::Bool(look.contrast)),
        ("sky_gradient", JsonValue::Bool(look.sky_gradient)),
        ("fog", JsonValue::Bool(look.fog)),
        ("palette", JsonValue::Bool(look.palette)),
        ("dither", JsonValue::Bool(look.dither)),
        ("low_resolution", JsonValue::U64(look.low_resolution.into())),
    ])
}

/// A first-person camera as the report names it, angles in degrees.
pub(crate) fn camera_json(camera: &FirstPersonCamera) -> JsonValue {
    JsonValue::object([
        ("eye", vec3(camera.eye)),
        ("yaw_deg", JsonValue::F64(bangle_degrees(camera.yaw, false))),
        (
            "pitch_deg",
            JsonValue::F64(bangle_degrees(camera.pitch, true)),
        ),
        (
            "fov_y_deg",
            JsonValue::F64(bangle_degrees(camera.fov_y, false)),
        ),
    ])
}

/// The report's `view` for `shot` and `video`: the view drawn (`kind`,
/// [`MapView::name`]), whose camera (`camera_from`: `"pose"` for `--pose`,
/// else `"game"`, the scene's own), the pose asked for, the look the last
/// frame drew with `--look`'s switches set (each `null` without its flag),
/// and the last frame's camera (`null` for a top-down view).
pub(crate) fn view_json(
    kind: &'static str,
    pose: Option<&Pose>,
    look: Option<&Look>,
    camera: Option<&FirstPersonCamera>,
) -> JsonValue {
    let or_null = |value: Option<JsonValue>| value.unwrap_or(JsonValue::Null);
    JsonValue::object([
        ("kind", JsonValue::str(kind)),
        (
            "camera_from",
            JsonValue::str(if pose.is_some() { "pose" } else { "game" }),
        ),
        ("pose", or_null(pose.map(Pose::json))),
        ("look", or_null(look.map(look_json))),
        ("camera", or_null(camera.map(camera_json))),
    ])
}

#[cfg(test)]
mod tests;
