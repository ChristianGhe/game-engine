//! The frame record (rung 2, plan addition A1; acceptance `frame-record`),
//! on a real adapter. `shot` and `video` write `<out>.frame.json` beside
//! the picture (eng-cli's `frame_record`), and here:
//! - **every shot in `shots.toml`** has a record that the strict reader
//!   takes, rendered twice by the real binary, the same bytes both times;
//! - **against reference records**, when there are any (`golden/frames/`,
//!   the owner's, or `ENG_FRAME_DIR`): exactly, byte for byte, on the
//!   pinned lavapipe; elsewhere within `frame-tolerances.toml`'s per-shot
//!   tolerances. Without any, each record is saved as a candidate and the
//!   test passes ("no reference yet"), as the golden shots do. Once one
//!   exists every shot needs its own, and one that no shot names fails. A
//!   shot marked `golden = "pending"` (T16's marker) has none yet by the
//!   owner's choice and fails nothing, and a reference for it in
//!   `golden/frames/` fails until the marker is cleared (`harness::judge`);
//! - **a video** has one record entry per frame, the shot's at its tick;
//! - **planted bugs** are `tests/frame_bugs.rs`'s, a binary of its own.
//!
//! Every record goes to `target/frame-candidates/<shot>.frame.json`, with
//! `frames.json` saying where they were rendered and how each compared.
//! The environment, as `tests/shots.rs` reads it:
//! - `ENG_REQUIRE_GPU=1`: no adapter fails instead of skipping;
//! - `ENG_SHOT_MODE=exact`: any configuration but the pinned one fails;
//! - `ENG_FRAME_DIR`: reference records instead of `golden/frames/`, such
//!   as another machine's candidates. It must exist and hold one for every
//!   shot.

// Shared with `frame_bugs.rs`, which uses the rest of it.
#[allow(dead_code)]
#[path = "frame_record/harness.rs"]
mod harness;

use eng_cli::frame_record::{FrameRecord, RecordView};
use harness::{Mode, Rendered, Shot, candidates, judge, render, shots, tolerances};
use serde_json::json;

/// Every shot's record, rendered twice, checked and compared.
#[test]
fn every_shot_has_a_frame_record_and_rerenders_it_exactly() {
    let shots = shots();
    let tolerances = tolerances(&shots);
    let candidates = candidates();
    // Its own scratch directory: the other tests run beside it.
    let tmp = harness::scratch("shots");
    let mut rendered: Vec<(&Shot, Rendered)> = Vec::new();
    for shot in &shots {
        let Some(first) = render(shot, &tmp.join("a")) else {
            return;
        };
        let again = render(shot, &tmp.join("b")).expect("the adapter is still there");
        assert!(
            first.text == again.text,
            "{}: the same shot gave two records",
            shot.name
        );
        assert_eq!(first.config, again.config);
        harness::check_shot(shot, &first);
        let name = format!("{}.frame.json", shot.name);
        std::fs::write(candidates.join(name), &first.text).unwrap();
        rendered.push((shot, first));
    }
    let config = rendered[0].1.config.clone();
    assert!(
        rendered.iter().all(|(_, r)| r.config == config),
        "one adapter"
    );
    let mode = harness::mode(&config).unwrap_or_else(|e| panic!("{e}"));
    let (refs, explicit) = harness::reference_dir(&candidates).unwrap_or_else(|e| panic!("{e}"));
    let any = shots
        .iter()
        .any(|s| refs.join(format!("{}.frame.json", s.name)).exists());
    let mut results = Vec::new();
    let mut failures = Vec::new();
    for (shot, ours) in &rendered {
        let path = refs.join(format!("{}.frame.json", shot.name));
        let reference = std::fs::read_to_string(&path).ok();
        let at = harness::References {
            at: &path,
            explicit,
            complete: any,
        };
        let tolerance = tolerances.for_shot(&shot.name);
        let ours_read = (ours.text.as_str(), &ours.record);
        let verdict = judge(
            shot.pending(),
            ours_read,
            reference.as_deref(),
            at,
            mode,
            &tolerance,
        );
        eprintln!("{}: {} {}", shot.name, verdict.status, verdict.worst);
        for note in &verdict.notes {
            eprintln!("  {note}");
        }
        for problem in &verdict.failures {
            eprintln!("  {problem}");
            failures.push(format!("{}: {problem}", shot.name));
        }
        results.push(json!({
            "name": shot.name, "pending": shot.pending(), "status": verdict.status,
            "worst": verdict.worst, "failures": verdict.failures, "notes": verdict.notes,
        }));
    }
    // A reference no shot names (a shot renamed or dropped) would never be
    // checked; only the owner can remove it.
    let mut files: Vec<_> = match std::fs::read_dir(&refs) {
        Ok(entries) => entries.map(|e| e.unwrap().file_name()).collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    for file in files {
        let file = file.to_string_lossy().into_owned();
        let Some(stem) = file.strip_suffix(".frame.json") else {
            continue;
        };
        if !shots.iter().any(|s| s.name == stem) {
            let why = format!("{stem}: [E-GOLDEN] {file} is no shot's reference");
            eprintln!("{why}");
            failures.push(why);
        }
    }
    let summary = json!({
        "configuration": config.json(),
        "mode": if mode == Mode::Exact { "exact" } else { "tolerances" },
        "reference_dir": refs.display().to_string(),
        "shots": results,
    });
    let text = serde_json::to_string_pretty(&summary).unwrap() + "\n";
    std::fs::write(candidates.join("frames.json"), text).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A video's record has an entry per frame, each the shot's of its tick;
/// the dots' video too, reduced.
#[test]
fn a_video_records_every_frame() {
    let dir = harness::scratch("video");
    // The command, then the room's flags, then the rest.
    let room = |extra: &[&str]| {
        let flags = ["--map", "room", "--seed", "1", "--size", "320x240"];
        let args = [&extra[..1], &flags[..], &extra[1..]].concat();
        harness::probe(&dir, &args)
    };
    let video = ["video", "--ticks", "4", "--every", "2", "--out", "v.apng"];
    let Some(out) = room(&video) else {
        return;
    };
    assert_eq!(out.status.code(), Some(0), "{}", harness::text(&out));
    let video = harness::read_record(&dir.join("v.apng.frame.json"));
    let ticks: Vec<u64> = video.frames.iter().map(|f| f.tick).collect();
    assert_eq!(ticks, [0, 2, 4]);
    let out = room(&["shot", "--at-tick", "2", "--out", "s.png"]).unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", harness::text(&out));
    let shot = harness::read_record(&dir.join("s.png.frame.json"));
    assert_eq!(shot.frames, [video.frames[1].clone()]);

    let dots = [
        "video", "--ticks", "2", "--size", "320x240", "--out", "d.apng",
    ];
    let out = harness::probe(&dir, &dots).unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", harness::text(&out));
    let dots = harness::read_record(&dir.join("d.apng.frame.json"));
    assert_eq!(dots.view, RecordView::TopDown);
    assert_eq!(dots.frames.len(), 3);
    assert!(
        dots.frames
            .iter()
            .all(|f| f.camera.is_none() && f.objects.is_empty())
    );
}

/// `frame-tolerances.toml` reads, names only shots `shots.toml` has, and
/// its `[default]` is eng-cli's.
#[test]
fn the_tolerances_name_only_shots_that_exist() {
    let shots = shots();
    let tolerances = tolerances(&shots);
    assert_eq!(
        tolerances.default(),
        eng_cli::frame_record::Tolerances::DEFAULT
    );
    for shot in &shots {
        let t = tolerances.for_shot(&shot.name);
        assert!(t.luminance > 0.0 && t.histogram > 0.0, "{}", shot.name);
    }
}

/// A 4x2 top-down record of one grey, and its text.
fn grey(level: u8) -> (String, FrameRecord) {
    let image = eng_gpu::Image {
        width: 4,
        height: 2,
        rgba8: [level, level, level, 255].repeat(8),
    };
    let entry = eng_cli::frame_record::top_down_entry(0, &image);
    let record = FrameRecord {
        view: RecordView::TopDown,
        width: 4,
        height: 2,
        frames: vec![entry],
    };
    (record.to_text(), record)
}

fn refs(explicit: bool, complete: bool) -> harness::References<'static> {
    harness::References {
        at: std::path::Path::new("golden/frames/a-shot.frame.json"),
        explicit,
        complete,
    }
}

/// A shot `shots.toml` marks `golden = "pending"` has no reference yet,
/// whatever the others have, and fails nothing; an unmarked one fails as
/// missing once any shot has its reference.
#[test]
fn a_marked_shot_without_a_reference_fails_nothing() {
    let (text, record) = grey(100);
    let ours = (text.as_str(), &record);
    let tol = eng_cli::frame_record::Tolerances::DEFAULT;
    for (explicit, complete) in [(false, false), (false, true), (true, true)] {
        let at = refs(explicit, complete);
        let marked = judge(true, ours, None, at, Mode::Tolerances, &tol);
        assert_eq!((marked.status, marked.failures.len()), ("no-reference", 0));
        assert!(marked.notes[0].contains("[W-NO-GOLDEN]"), "{marked:?}");
        let unmarked = judge(false, ours, None, at, Mode::Tolerances, &tol);
        let fails = explicit || complete;
        assert_eq!(unmarked.status == "fail", fails, "{unmarked:?}");
        if !explicit && complete {
            assert!(unmarked.failures[0].contains("missing; other shots have theirs"));
        }
    }
}

/// A reference for a marked shot in `golden/frames/` fails, telling the
/// owner to clear the marker; in `ENG_FRAME_DIR` it is measured and
/// reported, not judged. Unmarked shots are judged: within tolerances off
/// the pinned configuration, byte for byte on it.
#[test]
fn a_reference_for_a_marked_shot_in_golden_frames_fails() {
    let (text, record) = grey(100);
    let (dark, _) = grey(20);
    let ours = (text.as_str(), &record);
    let tol = eng_cli::frame_record::Tolerances::DEFAULT;
    let tolerances = Mode::Tolerances;

    let committed = judge(true, ours, Some(&text), refs(false, true), tolerances, &tol);
    assert_eq!(committed.status, "fail");
    let says = "exists, but golden = \"pending\" in shots.toml: clear the marker";
    assert!(committed.failures[0].contains(says), "{committed:?}");

    let measured = judge(true, ours, Some(&dark), refs(true, true), tolerances, &tol);
    assert_eq!((measured.status, measured.failures.len()), ("pending", 0));
    assert!(!measured.notes.is_empty(), "the difference is reported");

    let judged = judge(false, ours, Some(&dark), refs(true, true), tolerances, &tol);
    assert_eq!(judged.status, "fail");
    assert!(judged.failures.iter().any(|f| f.contains("histogram")));
    let same = judge(
        false,
        ours,
        Some(&text),
        refs(false, true),
        tolerances,
        &tol,
    );
    assert_eq!((same.status, same.failures.len()), ("pass", 0));

    let exact = Mode::Exact;
    let respaced = text.replace("\"tick\": 0", "\"tick\":  0");
    let byte = judge(false, ours, Some(&respaced), refs(false, true), exact, &tol);
    assert!(byte.failures[0].contains("byte for byte"), "{byte:?}");
    let equal = judge(false, ours, Some(&text), refs(false, true), exact, &tol);
    assert_eq!(equal.status, "pass");
}
