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
//!   exists every shot needs its own, and one that no shot names fails;
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
use harness::{Mode, Rendered, Shot, candidates, render, shots, tolerances};
use serde_json::{Value, json};

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
        let (status, worst, problems) = match std::fs::read_to_string(&path) {
            Err(_) if explicit || any => {
                let why = format!("[E-GOLDEN] {} is missing", path.display());
                ("fail", Value::Null, vec![why])
            }
            Err(_) => ("no-reference", Value::Null, Vec::new()),
            Ok(text) if mode == Mode::Exact => {
                if text == ours.text {
                    ("pass", Value::Null, Vec::new())
                } else {
                    let why = "[E-GOLDEN] not byte for byte the reference on the pinned \
                               configuration"
                        .to_owned();
                    ("fail", Value::Null, vec![why])
                }
            }
            Ok(text) => {
                let theirs =
                    FrameRecord::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                let tolerance = tolerances.for_shot(&shot.name);
                let comparison = ours.record.compare(&theirs, &tolerance);
                let status = if comparison.failures.is_empty() {
                    "pass"
                } else {
                    "fail"
                };
                let worst = harness::worst_json(&comparison.worst);
                (status, worst, comparison.failures)
            }
        };
        eprintln!("{}: {status} {worst}", shot.name);
        for problem in &problems {
            eprintln!("  {problem}");
            failures.push(format!("{}: {problem}", shot.name));
        }
        results.push(json!({
            "name": shot.name, "status": status, "worst": worst,
            "failures": problems,
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
