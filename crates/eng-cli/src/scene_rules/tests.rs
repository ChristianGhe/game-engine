//! The rules without a GPU: the file's strict reading, and each rule's
//! judgement of hand-made frames, each check broken in turn.

use eng_render::SurfaceClass;

use super::{Check, Outcome, SceneRules, colour_distance, third_pixels};
use crate::frame_record::{
    CameraRecord, FrameEntry, FrameRecord, HISTOGRAM_BINS, ObjectKey, ObjectRecord, RecordView,
    WorldBox,
};

const RULES: &str = r#"
version = 1
map = "room"

[rule-1]
pixels = 50
contrast = 0.03
light = 0.2

[rule-2]
void = 0
sky = 0

[rule-3]
share = 0.1

[shot.look]
must_see = ["marker"]

[shot.up]
must_see = []
except.rule-3-floor = { share = 0.0, why = "pitched up" }
except.rule-2-sky = { pixels = 40, why = "a skylight" }
"#;

fn rules() -> SceneRules {
    SceneRules::parse(RULES, "room.rules.toml").unwrap()
}

const W: u32 = 30;
const H: u32 = 30;
/// Each third of the 30x30 frame: 300 pixels.
const THIRD: u64 = 300;

fn code(class: SurfaceClass) -> usize {
    usize::from(class.code())
}

fn object(name: &str, pixels: u64, colour: [f64; 3], around: [f64; 3]) -> ObjectRecord {
    let lum = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let drawn = pixels > 0;
    ObjectRecord {
        key: ObjectKey {
            class: SurfaceClass::Sprite,
            id: 0,
        },
        name: Some(name.to_owned()),
        world_box: WorldBox {
            min: [0.0; 3],
            max: [1.0; 3],
        },
        pixels,
        screen_box: drawn.then_some([0, 0, 9, 9]),
        colour: drawn.then_some(colour),
        luminance: drawn.then_some(lum(colour)),
        surround_colour: drawn.then_some(around),
        surround_luminance: drawn.then_some(lum(around)),
        contrast: drawn.then_some(0.5),
    }
}

/// A red marker of 100 pixels on grey, floor filling the lower third and
/// ceiling the upper, no sky; the surfaces' luminance 0.05.
fn frame() -> FrameEntry {
    let mut classes = [[0; 3]; SurfaceClass::ALL.len()];
    classes[code(SurfaceClass::Ceiling)] = [THIRD, 0, 0];
    classes[code(SurfaceClass::Wall)] = [0, THIRD - 100, 0];
    classes[code(SurfaceClass::Sprite)] = [0, 100, 0];
    classes[code(SurfaceClass::Floor)] = [0, 0, THIRD];
    FrameEntry {
        tick: 7,
        camera: Some(CameraRecord {
            eye: [0.0; 3],
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            fov_y_deg: 70.0,
            near: 0.1,
        }),
        objects: vec![object("marker", 100, [0.1, 0.02, 0.02], [0.05, 0.05, 0.04])],
        unlisted: Vec::new(),
        invalid_info: 0,
        classes: Some(classes),
        void: Some(0),
        surface_luminance: Some(0.05),
        probes: Vec::new(),
        luminance: 0.05,
        histogram: [0; HISTOGRAM_BINS],
    }
}

fn record(frame: FrameEntry) -> FrameRecord {
    FrameRecord {
        view: RecordView::FirstPerson,
        width: W,
        height: H,
        frames: vec![frame],
    }
}

/// The failing findings' messages for shot `shot` of `frame`.
fn failures(shot: &str, frame: FrameEntry) -> Vec<String> {
    let findings = rules().judge(shot, &record(frame)).unwrap();
    findings
        .iter()
        .filter(|f| f.fails())
        .map(|f| f.message(shot))
        .collect()
}

#[test]
fn the_file_reads_strictly() {
    let rules = rules();
    assert_eq!(rules.map, "room");
    assert_eq!(
        (rules.must_see.pixels, rules.void, rules.sky, rules.share),
        (50, 0, 0, 0.1)
    );
    assert_eq!(rules.shots["look"].must_see, ["marker"]);
    let up = &rules.shots["up"].except;
    assert_eq!(
        (up[&Check::Floor].bound, up[&Check::Floor].why.as_str()),
        (0.0, "pitched up")
    );
    assert_eq!(up[&Check::Sky].bound, 40.0);
    let floor = "{ share = 0.0, why = \"pitched up\" }";
    for (edit, says) in [
        (RULES.replace("version = 1", "version = 2"), "newer"),
        (RULES.replace("share = 0.1", "share = 1.5"), "share"),
        (RULES.replace("share = 0.1", "share = 0.0"), "share"),
        (RULES.replace("light = 0.2", "light = -1.0"), "light"),
        (RULES.replace("pixels = 50", "pixels = 0"), "above 0"),
        (RULES.replace("sky = 0\n", ""), "sky"),
        (RULES.replace("map = \"room\"", "map = \"\""), "map"),
        (
            RULES.replace("contrast = 0.03", "contrast = 0.03\nhue = 1"),
            "hue",
        ),
        (
            RULES.replace("[\"marker\"]", "[\"marker\", \"marker\"]"),
            "twice",
        ),
        // An exception names a frame-wide check, bounds it by its own
        // measure, looser than the scene's, and gives a reason.
        (
            RULES.replace("except.rule-3-floor", "except.rule-1"),
            "excused from rule-2-void",
        ),
        (
            RULES.replace("except.rule-3-floor", "except.rule-9"),
            "excused from rule-2-void",
        ),
        (
            RULES.replace(floor, "\"pitched up\""),
            "except.rule-3-floor",
        ),
        (RULES.replace("\"pitched up\"", "\" \""), "no reason"),
        (
            RULES.replace("share = 0.0, why", "pixels = 3, why"),
            "by `share` alone",
        ),
        (
            RULES.replace("share = 0.0, why", "share = 0.1, why"),
            "a share from 0 up to below the scene's 0.1",
        ),
        (
            RULES.replace("pixels = 40, why", "pixels = 0, why"),
            "a pixel count above the scene's 0",
        ),
        (
            RULES.replace("pixels = 40, why", "pixels = 40, share = 0.5, why"),
            "by `pixels` alone",
        ),
        (
            RULES.replace("[shot.look]\nmust_see = [\"marker\"]\n", "[shot.look]\n"),
            "must_see",
        ),
    ] {
        assert_ne!(edit, RULES, "{says}");
        let err = SceneRules::parse(&edit, "room.rules.toml").expect_err(says);
        assert!(err.contains(says), "{says}: {err}");
        assert!(err.contains("room.rules.toml"), "{err}");
    }
}

#[test]
fn a_proper_frame_passes_every_check() {
    let findings = rules().judge("look", &record(frame())).unwrap();
    let checks: Vec<Check> = findings.iter().map(|f| f.check).collect();
    assert_eq!(
        checks,
        [
            Check::MustSee,
            Check::Void,
            Check::Sky,
            Check::Floor,
            Check::Above
        ]
    );
    assert!(
        findings.iter().all(|f| f.outcome == Outcome::Pass),
        "{findings:?}"
    );
    let marker = &findings[0];
    assert_eq!(marker.object.as_deref(), Some("marker"));
    let light = marker.measures.iter().find(|m| m.name == "light").unwrap();
    let lum = 0.2126 * 0.1 + 0.7152 * 0.02 + 0.0722 * 0.02;
    assert!((light.value.unwrap() - lum / 0.05).abs() < 1e-12);
    assert_eq!(findings[3].measures[0].value, Some(1.0));
}

#[test]
fn rule_1_fails_an_object_missing_hidden_flat_or_black() {
    let fails_with = |frame: FrameEntry, measure: &str| {
        let failures = failures("look", frame);
        let want = format!("[E-SCENE-RULE] rule-1 look marker (tick 7): {measure}");
        assert!(
            failures.len() == 1 && failures[0].starts_with(&want),
            "{measure}: {failures:?}"
        );
    };
    // Not asked for at all: nothing has its name.
    let mut gone = frame();
    gone.objects.clear();
    fails_with(gone, "pixels none < 50");
    // Asked for, not drawn.
    let mut hidden = frame();
    hidden.objects[0] = object("marker", 0, [0.0; 3], [0.0; 3]);
    fails_with(
        hidden,
        "pixels 0 < 50, contrast none < 0.03, light none < 0.2",
    );
    // Under 50 pixels.
    let mut small = frame();
    small.objects[0].pixels = 49;
    fails_with(small, "pixels 49 < 50");
    // Its surroundings' own colour.
    let mut flat = frame();
    flat.objects[0].colour = flat.objects[0].surround_colour;
    fails_with(flat, "contrast 0 < 0.03");
    // Drawn black: it stands out, but holds a twentieth of the surfaces'
    // light.
    let mut black = frame();
    black.objects[0].colour = Some([0.002; 3]);
    black.objects[0].luminance = Some(0.002);
    fails_with(black, "light 0.04 < 0.2");
}

/// Light is held to the surfaces, not the whole frame: more sky in view,
/// brighter than any surface, leaves it as it was.
#[test]
fn rule_1_light_leaves_the_sky_out() {
    let mut sky = frame();
    sky.luminance = 0.5;
    let findings = rules().judge("look", &record(sky)).unwrap();
    let light = (findings[0].measures.iter())
        .find(|m| m.name == "light")
        .unwrap();
    let lum = 0.2126 * 0.1 + 0.7152 * 0.02 + 0.0722 * 0.02;
    assert!((light.value.unwrap() - lum / 0.05).abs() < 1e-12);
    // Sky alone, no surface: no light to hold it to, which fails.
    let mut none = frame();
    none.surface_luminance = None;
    let failures = failures("look", none);
    assert!(failures[0].contains("light none < 0.2"), "{failures:?}");
}

#[test]
fn rule_1_judges_every_object_of_the_name() {
    let mut two = frame();
    let mut dark = two.objects[0].clone();
    dark.key.id = 1;
    dark.luminance = Some(0.001);
    two.objects.push(dark);
    let failures = failures("look", two);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("light 0.02 < 0.2"), "{failures:?}");
}

#[test]
fn rule_2_fails_a_void_pixel_or_any_sky() {
    let mut hole = frame();
    hole.void = Some(1);
    let found = failures("look", hole);
    assert_eq!(
        found,
        [
            "[E-SCENE-RULE] rule-2-void look (tick 7): pixels 1 > 0 (pixels showing nothing \
          below the horizon)"
        ]
    );
    // A missing ceiling face: sky in the upper third, above the horizon,
    // where rule-3-above still passes (ceiling and sky count together).
    let mut ceiling_hole = frame();
    let classes = ceiling_hole.classes.as_mut().unwrap();
    classes[code(SurfaceClass::Ceiling)] = [THIRD - 25, 0, 0];
    classes[code(SurfaceClass::None)] = [25, 0, 0];
    let found = failures("look", ceiling_hole);
    assert_eq!(
        found,
        [
            "[E-SCENE-RULE] rule-2-sky look (tick 7): pixels 25 > 0 (pixels showing nothing: \
          the sky)"
        ]
    );
}

#[test]
fn rule_3_fails_a_frame_without_floor_below_or_ceiling_above() {
    let floor = code(SurfaceClass::Floor);
    let ceiling = code(SurfaceClass::Ceiling);
    // 29 of 300 floor pixels in the lower third: under a tenth.
    let mut low = frame();
    low.classes.as_mut().unwrap()[floor] = [0, 0, 29];
    let found = failures("look", low);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("[E-SCENE-RULE] rule-3-floor look (tick 7): share 0.096667 < 0.1"),
        "{found:?}"
    );
    // 29 of 300 ceiling pixels in the upper third.
    let mut high = frame();
    high.classes.as_mut().unwrap()[ceiling] = [29, 0, 0];
    let found = failures("look", high);
    assert!(
        found.len() == 1 && found[0].contains("rule-3-above look (tick 7): share 0.096667"),
        "{found:?}"
    );
}

#[test]
fn an_exception_holds_a_shot_to_its_bound_and_fails_once_stale() {
    let (floor, ceiling, none) = (
        code(SurfaceClass::Floor),
        code(SurfaceClass::Ceiling),
        code(SurfaceClass::None),
    );
    // Pitched up: no floor, and 30 pixels of sky, within the shot's 40.
    let mut up = frame();
    let classes = up.classes.as_mut().unwrap();
    classes[floor] = [0, 0, 0];
    classes[ceiling] = [THIRD - 30, 0, 0];
    classes[none] = [30, 0, 0];
    let findings = rules().judge("up", &record(up.clone())).unwrap();
    for check in [Check::Floor, Check::Sky] {
        let excused = findings.iter().find(|f| f.check == check).unwrap();
        assert!(
            matches!(excused.outcome, Outcome::Excused { .. }) && !excused.fails(),
            "{excused:?}"
        );
    }
    let sky = findings.iter().find(|f| f.check == Check::Sky).unwrap();
    assert!(
        sky.message("up").ends_with(
            "pixels 30 <= 40 (pixels showing nothing: the sky); excused from the scene's 0: a \
             skylight"
        ),
        "{}",
        sky.message("up")
    );
    // Past the bound: 41 pixels of sky fail.
    up.classes.as_mut().unwrap()[none] = [41, 0, 0];
    let found = failures("up", up);
    assert!(
        found.len() == 1
            && found[0].starts_with("[E-SCENE-RULE] rule-2-sky up (tick 7): pixels 41 > 40")
            && found[0].contains("past even the bound it is excused to: a skylight"),
        "{found:?}"
    );
    // The same shot showing its floor and no sky: both exceptions stale.
    let found = failures("up", frame());
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].starts_with("[E-SCENE-RULE] rule-2-sky up (tick 7): pixels 0 <= 40")
            && found[1].starts_with("[E-SCENE-RULE] rule-3-floor up (tick 7): share 1 >= 0")
            && found.iter().all(|f| f.ends_with("remove the exception")),
        "{found:?}"
    );
}

#[test]
fn a_shot_without_a_table_or_a_top_down_record_is_refused() {
    let err = rules().judge("other", &record(frame())).unwrap_err();
    assert!(err.contains("no [shot.other]"), "{err}");
    let mut top_down = record(frame());
    top_down.view = RecordView::TopDown;
    let err = rules().judge("look", &top_down).unwrap_err();
    assert!(err.contains("top-down"), "{err}");
}

#[test]
fn colour_distance_is_oklab_and_thirds_are_the_records() {
    assert_eq!(colour_distance([0.2; 3], [0.2; 3]), 0.0);
    // Black to white is OKLab's whole lightness range, 1, to f32's
    // precision.
    let white = colour_distance([0.0; 3], [1.0; 3]);
    assert!((white - 1.0).abs() < 1e-4, "{white}");
    assert_eq!(third_pixels(320, 240), [25_600; 3]);
    // Row y is in third 3y / height: rows 0-3, 4-6 and 7-9 of 10.
    assert_eq!(third_pixels(2, 10), [8, 6, 6]);
}
