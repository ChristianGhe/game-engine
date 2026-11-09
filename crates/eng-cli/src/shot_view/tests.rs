//! `--view`, `--pose` and `--look`: their values, their refusals and what
//! the report says.

use eng_math::{BAngle, WorldPos};
use eng_render::{FirstPersonCamera, Look, NEAR_PLANE};
use eng_sim::{ScheduleError, Sim, SimBuilder, SimConfig};

use super::{Pose, degrees_to_bangle, parse_view, refusal, view_json};
use crate::args::RunArgs;
use crate::maps::{GameMap, MapChoice};
use crate::view::MapView;
use crate::view::tests::FIRST_PERSON;
use crate::{GameDef, UNKNOWN};

fn build(config: SimConfig) -> Result<Sim, ScheduleError> {
    SimBuilder::new(config).build()
}

const GAME: GameDef = GameDef {
    name: "views",
    version: "0",
    build_sha: UNKNOWN,
    tick_rate: 60,
    build,
    top_down: None,
    view_yaw: 0,
};

fn choice(view: MapView) -> MapChoice {
    MapChoice {
        map: GameMap {
            name: "m",
            build,
            view,
        },
        first: "m",
    }
}

#[test]
fn degrees_are_binary_angles_rounded_to_nearest_and_wrapped() {
    assert_eq!(degrees_to_bangle(0.0), BAngle(0));
    assert_eq!(degrees_to_bangle(90.0), BAngle(0x4000_0000));
    assert_eq!(degrees_to_bangle(180.0), BAngle(0x8000_0000));
    assert_eq!(degrees_to_bangle(-90.0), BAngle(0xc000_0000));
    assert_eq!(degrees_to_bangle(450.0), BAngle(0x4000_0000));
    assert_eq!(degrees_to_bangle(360.0), BAngle(0));
    // Just under a whole turn rounds up to it, which wraps to 0.
    assert_eq!(degrees_to_bangle(359.999_999_999_9), BAngle(0));
    // One unit is 360 / 2^32 degrees.
    let unit = 360.0 / 4_294_967_296.0;
    assert_eq!(degrees_to_bangle(unit * 0.49), BAngle(0));
    assert_eq!(degrees_to_bangle(unit * 0.51), BAngle(1));
    assert_eq!(degrees_to_bangle(-unit), BAngle(u32::MAX));
}

#[test]
fn a_pose_is_five_finite_numbers_with_the_pitch_within_a_quarter_turn() {
    let pose = Pose::parse("9,3.5,1.6,-90,-15").unwrap();
    assert_eq!(pose.eye, WorldPos::new(9.0, 3.5, 1.6));
    assert_eq!((pose.yaw_deg, pose.pitch_deg), (-90.0, -15.0));
    assert_eq!(pose.yaw(), BAngle(0xc000_0000));
    // A pitch reads as a signed angle: -15 degrees looks down.
    assert!((pose.pitch().0 as i32) < 0);
    assert_eq!(
        Pose::parse(" 1 , 2 ,3,0, 90").unwrap().pitch(),
        BAngle(0x4000_0000)
    );
    for bad in [
        "",
        "1,2,3,4",
        "1,2,3,4,5,6",
        "1,2,3,4,x",
        "1,2,3,nan,0",
        "1,2,inf,0,0",
        "1,2,3,0,90.5",
        "1,2,3,0,-91",
        "1;2;3;0;0",
        "1,,3,0,0",
    ] {
        let err = Pose::parse(bad).unwrap_err();
        assert!(
            err.starts_with("--pose needs X,Y,Z,YAW,PITCH"),
            "{bad}: {err}"
        );
        assert!(err.contains(&format!("{bad:?}")), "{err}");
    }
}

#[test]
fn a_pose_replaces_the_eye_and_angles_and_keeps_the_lens() {
    let game_camera = FirstPersonCamera {
        eye: WorldPos::new(1.5, 4.5, 1.6),
        yaw: BAngle(7),
        pitch: BAngle(9),
        fov_y: BAngle(879_750_026),
        near: NEAR_PLANE,
    };
    let posed = Pose::parse("2,3,1,45,10").unwrap().apply(game_camera);
    assert_eq!(posed.eye, WorldPos::new(2.0, 3.0, 1.0));
    assert_eq!(posed.yaw, BAngle(0x2000_0000));
    assert_eq!(posed.pitch, degrees_to_bangle(10.0));
    assert_eq!((posed.fov_y, posed.near), (game_camera.fov_y, NEAR_PLANE));
}

#[test]
fn view_names_are_the_map_views_names() {
    for view in [MapView::TopDown, FIRST_PERSON] {
        assert_eq!(parse_view(view.name()), Ok(view.name()));
    }
    for bad in ["none", "isometric", "Top-Down", ""] {
        let err = parse_view(bad).unwrap_err();
        assert_eq!(
            err,
            format!("--view is top-down or first-person, not {bad:?}")
        );
    }
}

#[test]
fn a_view_the_map_lacks_and_first_person_flags_on_a_top_down_map_are_refused() {
    let shot = |view, pose: bool, look: bool| RunArgs {
        view,
        pose: pose.then(|| Pose::parse("0,0,1,0,0").unwrap()),
        look: look.then(|| "l.toml".into()),
        ..RunArgs::default()
    };
    let top_down = choice(MapView::TopDown);
    let first_person = choice(FIRST_PERSON);
    // What each map has, or nothing asked: drawn.
    for (map, view) in [
        (top_down, None),
        (top_down, Some("top-down")),
        (first_person, None),
        (first_person, Some("first-person")),
    ] {
        assert_eq!(refusal(&GAME, map, &shot(view, false, false)), None);
    }
    assert_eq!(refusal(&GAME, first_person, &shot(None, true, true)), None);
    assert_eq!(
        refusal(&GAME, first_person, &shot(Some("top-down"), false, false)).as_deref(),
        Some("[E-USAGE] [E-MAP] views's map \"m\" has no top-down view (its view is first-person)")
    );
    assert_eq!(
        refusal(&GAME, top_down, &shot(Some("first-person"), false, false)).as_deref(),
        Some("[E-USAGE] [E-MAP] views's map \"m\" has no first-person view (its view is top-down)")
    );
    for (pose, look, flag) in [(true, false, "--pose"), (false, true, "--look")] {
        assert_eq!(
            refusal(&GAME, top_down, &shot(None, pose, look)),
            Some(format!(
                "[E-USAGE] {flag} needs a first-person view; views's map \"m\" is drawn top-down"
            ))
        );
    }
    // A map with no view is `no_view`'s to refuse.
    assert_eq!(
        refusal(
            &GAME,
            choice(MapView::None),
            &shot(Some("top-down"), true, true)
        ),
        None
    );
}

#[test]
fn the_report_names_the_view_its_pose_its_look_and_the_last_camera() {
    let camera = FirstPersonCamera {
        eye: WorldPos::new(9.0, 3.5, 1.6),
        yaw: BAngle(0xc000_0000),
        pitch: degrees_to_bangle(-15.0),
        fov_y: BAngle(0x4000_0000),
        near: NEAR_PLANE,
    };
    let pose = Pose::parse("9,3.5,1.6,-90,-15").unwrap();
    let json = view_json("first-person", Some(&pose), Some(&Look::OFF), Some(&camera));
    let text = json.to_compact();
    for part in [
        "\"kind\":\"first-person\"",
        "\"camera_from\":\"pose\"",
        "\"pose\":{\"eye\":[9.0,3.5,1.6],\"yaw_deg\":-90.0,\"pitch_deg\":-15.0}",
        "\"falloff\":false",
        "\"low_resolution\":1",
        "\"yaw_deg\":270.0",
        "\"fov_y_deg\":90.0",
    ] {
        assert!(text.contains(part), "{part} in {text}");
    }
    let pitch = json.pointer("/camera/pitch_deg").and_then(|v| v.as_f64());
    assert!(pitch.is_some_and(|p| (p + 15.0).abs() < 1e-6), "{text}");
    let plain = view_json("top-down", None, None, None).to_compact();
    assert_eq!(
        plain,
        "{\"kind\":\"top-down\",\"camera_from\":\"game\",\"pose\":null,\"look\":null,\
         \"camera\":null}"
    );
}
