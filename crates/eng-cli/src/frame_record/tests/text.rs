//! The record's text: its layout read back, the strict reader's refusals
//! and the comparison's tolerances.

use super::super::{FrameRecord, RecordView, Tolerances, contrast_tolerance, top_down_entry};
use super::{H, HIDDEN, W, record, targets};

#[test]
fn the_text_reads_back_to_the_same_record_and_bytes() {
    let record = record();
    let text = record.to_text();
    let read = FrameRecord::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(read, record);
    assert_eq!(read.to_text(), text);
    // One object on each line.
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines
            .iter()
            .any(|l| l.trim_start().starts_with("{\"class\":\"sprite\"")
                && l.contains("\"name\":\"lamp\"")),
        "{text}"
    );
    assert!(text.ends_with("\"width\": 16\n}\n"), "{text}");
}

#[test]
fn a_top_down_record_is_reduced() {
    let t = targets();
    let record = FrameRecord {
        view: RecordView::TopDown,
        width: W,
        height: H,
        frames: vec![top_down_entry(0, &t.image), top_down_entry(4, &t.image)],
    };
    let frame = &record.frames[1];
    assert_eq!((frame.camera, frame.classes), (None, None));
    assert!(frame.objects.is_empty() && frame.probes.is_empty());
    assert_eq!(frame.histogram.iter().sum::<u64>(), u64::from(W * H));
    assert_eq!(frame.luminance, record.frames[0].luminance);
    let text = record.to_text();
    assert_eq!(FrameRecord::parse(&text).unwrap(), record);
}

/// Each edit of a good record's text, and what the reader says of it.
#[test]
fn the_reader_refuses_what_version_1_does_not_hold() {
    let text = record().to_text();
    let first_object = text
        .lines()
        .find(|l| l.contains("\"class\":\"floor\""))
        .unwrap()
        .trim()
        .trim_end_matches(',')
        .to_owned();
    let stray = "{\"class\":\"wall\",\"id\":99,\"pixels\":1}";
    let edits: Vec<(String, &str)> = vec![
        (text.replace("\"version\": 1", "\"version\": 2"), "/version"),
        (
            text.replace("\"view\": \"first-person\"", "\"view\": \"sideways\""),
            "/view",
        ),
        (
            text.replace("\"tick\": 5", "\"tick\": 5,\n      \"zz\": 1"),
            "keys",
        ),
        (
            text.replace("\"tick\": 5", "\"tick\": -5"),
            "/frames/0/tick",
        ),
        (
            text.replace("\"tick\": 5", "\"tick\": 5.0"),
            "/frames/0/tick",
        ),
        (
            text.replace("\"x\":1,\"y\":1", "\"x\":2,\"y\":1"),
            "probe at",
        ),
        (
            text.replace(&first_object, &format!("{first_object},\n{first_object}")),
            "order",
        ),
        (text.replacen("\"pixels\":64", "\"pixels\":63", 1), "add up"),
        (
            text.replacen("\"pixels\":12", "\"pixels\":13", 1),
            "screen box",
        ),
        (
            text.replacen("\"luminance\":0.", "\"luminance\":1.", 1),
            "from 0 to 1",
        ),
        (
            text.replace("\"near\":0.1", "\"near\":0.1,\"near\":0.1"),
            "twice",
        ),
        (format!("{text}x"), "after the record"),
        (
            text.replace("\"depth\":0.5", "\"depth\":NaN"),
            "not a JSON value",
        ),
        (
            text.replace("\"edge\":false", "\"edge\":0"),
            "true or false",
        ),
        (
            text.replacen("\"class\":\"floor\"", "\"class\":\"none\"", 1),
            "sky",
        ),
        // The one layout: spacing and number spellings the writer never
        // uses, though the values are the same.
        (text.replace("\"tick\": 5", "\"tick\":  5"), "one layout"),
        (
            text.replace("\"depth\":0.5", "\"depth\":0.50"),
            "one layout",
        ),
        (
            text.replace("\"depth\":0.5", "\"depth\":5e-1"),
            "one layout",
        ),
        (
            text.replace("\"name\":\"lamp\"", "\"name\":\"\\u+06Camp\""),
            "hex digits",
        ),
        (
            text.replace("\"yaw_deg\":90.0", "\"yaw_deg\":360.0"),
            "up to 360",
        ),
        // A probe on the thing's middle, its id one nobody listed.
        (
            text.replace(
                "\"id\":3,\"slope\":0.0,\"x\":7",
                "\"id\":5,\"slope\":0.0,\"x\":7",
            ),
            "neither asked",
        ),
        // Another unlisted sprite pixel: the sprites' thirds no longer add
        // up to their objects and unlisted keys.
        (
            text.replace(
                stray,
                &format!("{stray},\n{{\"class\":\"sprite\",\"id\":9,\"pixels\":1}}"),
            ),
            "add up",
        ),
    ];
    for (edited, says) in edits {
        assert_ne!(edited, text, "the edit for {says:?} changed nothing");
        let err = FrameRecord::parse(&edited).expect_err(says);
        assert!(err.contains(says), "{says:?}: {err}");
    }
}

#[test]
fn records_compare_within_their_tolerances() {
    let a = record();
    let same = a.compare(&a, &Tolerances::DEFAULT);
    assert!(same.failures.is_empty(), "{:?}", same.failures);
    assert_eq!(same.worst.pixels, 0.0);

    // A few edge pixels and a level of light moved: within.
    let mut b = a.clone();
    b.frames[0].objects[0].pixels += 3;
    b.frames[0].objects[0].luminance = b.frames[0].objects[0].luminance.map(|l| l + 0.002);
    let near = a.compare(&b, &Tolerances::DEFAULT);
    assert!(near.failures.is_empty(), "{:?}", near.failures);
    assert!(near.worst.pixels > 0.0 && near.worst.pixels <= 1.0);

    // An object gone, its light changed, a probe off any edge on something
    // else, the camera moved: each fails.
    let fails = |b: &FrameRecord, at: &str| {
        let failures = a.compare(b, &Tolerances::DEFAULT).failures;
        assert!(
            failures.iter().any(|f| f.contains(at)),
            "{at}: {failures:?}"
        );
    };
    let mut gone = a.clone();
    gone.frames[0].objects[1].pixels = 0;
    fails(&gone, "/objects/1/pixels");
    let mut dark = a.clone();
    dark.frames[0].objects[0].luminance = Some(0.0);
    fails(&dark, "/objects/0/luminance");
    let mut probe = a.clone();
    probe.frames[0].probes[11].key = HIDDEN;
    fails(&probe, "off any edge");
    let mut moved = a.clone();
    moved.frames[0].camera.as_mut().unwrap().eye[0] += 1e-9;
    fails(&moved, "camera");
    // A deeper probe beyond 1e-5 and its slope's sixteenth.
    let mut deep = a.clone();
    deep.frames[0].probes[11].depth += 0.01;
    fails(&deep, "/probes/11/depth");
}

/// Michelson's contrast moves by up to `2 t / (L + S)` when `L` and `S`
/// each move by `t`, so the luminance tolerance carries into it.
#[test]
fn a_contrast_is_held_as_loosely_as_its_light_moves_it() {
    let a = record();
    let floor = &a.frames[0].objects[0];
    let (l, s) = (floor.luminance.unwrap(), floor.surround_luminance.unwrap());
    let t = Tolerances::DEFAULT;
    let allowed = contrast_tolerance(&t, floor, floor);
    assert!((allowed - (t.contrast + 2.0 * t.luminance / (l + s))).abs() < 1e-12);
    // Both means moved by the whole luminance tolerance, the darker way:
    // the contrast that follows stays within it.
    let mut b = a.clone();
    let moved = &mut b.frames[0].objects[0];
    let (l2, s2) = (l - t.luminance, s + t.luminance);
    moved.luminance = Some(l2);
    moved.surround_luminance = Some(s2);
    moved.contrast = Some((l2 - s2).abs() / (l2 + s2));
    let failures = a.compare(&b, &t).failures;
    let contrast = failures.iter().any(|f| f.contains("/objects/0/contrast"));
    assert!(!contrast, "{failures:?}");
}
