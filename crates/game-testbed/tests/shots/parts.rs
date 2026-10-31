//! The shot harness's parts, tested without a GPU, so these run in CI's
//! test job as well as the native ones (`../shots.rs`).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::compare::{Image, Mode, SIZE, Status, Thresholds, check, encode, read_thresholds};
use super::{
    Config, PINNED_ADAPTER, PINNED_DRIVER_INFO, SHOTS, golden_dir, judge, mode, render,
    shot_command,
};

/// A scratch directory with `refs` (standing in for `golden/`, a protected
/// name) and `candidates`, both empty.
fn scratch(name: &str) -> (PathBuf, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("shots")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let (refs, candidates) = (dir.join("refs"), dir.join("candidates"));
    std::fs::create_dir_all(&refs).unwrap();
    std::fs::create_dir_all(&candidates).unwrap();
    (refs, candidates)
}

fn listing(dir: &Path) -> Vec<String> {
    let entries = std::fs::read_dir(dir).unwrap();
    let mut names: Vec<String> = (entries.map(|e| e.unwrap().file_name()))
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

const FLOOR: [u8; 4] = [53, 56, 60, 255];
const WALL: [u8; 4] = [149, 149, 155, 255];
const DOT: [u8; 4] = [255, 153, 39, 255];

/// The testbed's floor with a wall-coloured square of `side` pixels at (8, 8).
fn picture(width: u32, height: u32, side: u32) -> Image {
    let mut rgba8 = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let inside = (8..8 + side).contains(&x) && (8..8 + side).contains(&y);
            rgba8.extend_from_slice(if inside { &WALL } else { &FLOOR });
        }
    }
    Image {
        width,
        height,
        rgba8,
    }
}

/// The floor with a dot as the testbed draws it at 40 px/m (radius 10 px)
/// centred on (100.3, 80.6), less `cleared` of its edge pixels, spread
/// round it: what another rasterizer might drop.
fn dot(cleared: usize) -> Image {
    let mut image = picture(SIZE.0, SIZE.1, 0);
    let inside = |x: u32, y: u32| {
        let (dx, dy) = (f64::from(x) + 0.5 - 100.3, f64::from(y) + 0.5 - 80.6);
        dx * dx + dy * dy <= 100.0
    };
    let mut edge = Vec::new();
    for y in 60..102 {
        for x in 80..122 {
            if inside(x, y) {
                let at = ((y * SIZE.0 + x) * 4) as usize;
                image.rgba8[at..at + 4].copy_from_slice(&DOT);
                if !(inside(x + 1, y) && inside(x - 1, y) && inside(x, y + 1) && inside(x, y - 1)) {
                    edge.push(at);
                }
            }
        }
    }
    let step = edge.len() / cleared.max(1);
    for at in edge.iter().step_by(step.max(1)).take(cleared) {
        image.rgba8[*at..*at + 4].copy_from_slice(&FLOOR);
    }
    image
}

fn png_of(image: &Image) -> Vec<u8> {
    encode(
        image.width,
        image.height,
        png::ColorType::Rgba,
        &image.rgba8,
    )
}

/// The proposed `golden/thresholds.toml` (t12-oracles.patch), less its
/// comments.
const THRESHOLDS: &str = "version = 1\n[flip]\nmean = 0.01\np99 = 0.2\np999 = 0.5\n";

fn pinned_config() -> Config {
    Config {
        os: "linux".into(),
        arch: "x86_64".into(),
        adapter: PINNED_ADAPTER.into(),
        backend: "vulkan".into(),
        device_type: "cpu".into(),
        driver: "llvmpipe".into(),
        driver_info: PINNED_DRIVER_INFO.into(),
    }
}

#[test]
fn a_missing_golden_saves_a_candidate_and_passes() {
    let (refs, candidates) = scratch("missing");
    let png = png_of(&picture(SIZE.0, SIZE.1, 10));
    for mode in [Mode::Exact, Mode::Flip] {
        let verdict = check("a-shot", &png, mode, &refs, &candidates);
        assert_eq!(verdict.status, Status::NoGolden, "{verdict:?}");
        let tag = "a-shot: [W-NO-GOLDEN] no golden yet";
        assert!(verdict.message.starts_with(tag), "{verdict:?}");
        let saved = std::fs::read(candidates.join("a-shot.png")).unwrap();
        assert_eq!(saved, png, "the candidate is the shot's own file");
        assert!(listing(&refs).is_empty(), "nothing is written into golden/");
    }
}

#[test]
fn exact_comparison_fails_on_one_changed_level() {
    let (refs, candidates) = scratch("exact");
    let golden = png_of(&picture(SIZE.0, SIZE.1, 10));
    std::fs::write(refs.join("a-shot.png"), &golden).unwrap();

    let verdict = check("a-shot", &golden, Mode::Exact, &refs, &candidates);
    assert_eq!((verdict.status, verdict.differing), (Status::Pass, Some(0)));
    assert!(listing(&candidates).is_empty(), "a pass saves nothing");

    let mut shot = picture(SIZE.0, SIZE.1, 10);
    shot.rgba8[((100 * SIZE.0 + 200) * 4 + 1) as usize] += 1;
    let shot = png_of(&shot);
    let verdict = check("a-shot", &shot, Mode::Exact, &refs, &candidates);
    assert_eq!((verdict.status, verdict.differing), (Status::Fail, Some(1)));
    let tag = "a-shot: [E-GOLDEN] 1 of 76800";
    assert!(verdict.message.starts_with(tag), "{verdict:?}");
    let saved = std::fs::read(candidates.join("a-shot.png")).unwrap();
    assert_eq!(saved, shot, "a failure saves the shot");
    assert_eq!(listing(&refs), ["a-shot.png"], "golden/ is left alone");
    assert_eq!(std::fs::read(refs.join("a-shot.png")).unwrap(), golden);
}

#[test]
fn a_shot_of_the_wrong_size_or_not_opaque_fails() {
    let (refs, candidates) = scratch("size");
    // Without a golden, then against a golden of the right size.
    for with_golden in [false, true] {
        if with_golden {
            let golden = png_of(&picture(SIZE.0, SIZE.1, 10));
            std::fs::write(refs.join("a-shot.png"), golden).unwrap();
        }
        for mode in [Mode::Exact, Mode::Flip] {
            let wide = png_of(&picture(SIZE.0 + 1, SIZE.1, 10));
            let verdict = check("a-shot", &wide, mode, &refs, &candidates);
            assert_eq!(verdict.status, Status::Fail, "{verdict:?}");
            assert!(verdict.message.contains("321x240"), "{verdict:?}");

            let mut clear = picture(SIZE.0, SIZE.1, 10);
            clear.rgba8[3] = 254;
            let verdict = check("a-shot", &png_of(&clear), mode, &refs, &candidates);
            assert_eq!(verdict.status, Status::Fail, "{verdict:?}");
            assert!(verdict.message.contains("opaque"), "{verdict:?}");
        }
    }
    // A golden of the wrong size fails a shot of the right one.
    let golden = png_of(&picture(SIZE.0, SIZE.1 + 2, 10));
    std::fs::write(refs.join("b-shot.png"), golden).unwrap();
    let shot = png_of(&picture(SIZE.0, SIZE.1, 10));
    for mode in [Mode::Exact, Mode::Flip] {
        let verdict = check("b-shot", &shot, mode, &refs, &candidates);
        assert_eq!(verdict.status, Status::Fail, "{verdict:?}");
        assert!(verdict.message.contains("320x242"), "{verdict:?}");
    }
    assert_eq!(listing(&refs), ["a-shot.png", "b-shot.png"]);
}

#[test]
fn flip_holds_a_shot_to_each_threshold() {
    let (refs, candidates) = scratch("flip");
    let golden = png_of(&picture(SIZE.0, SIZE.1, 10));
    std::fs::write(refs.join("a-shot.png"), &golden).unwrap();
    // The square grows to 14 pixels: 96 pixels differ, about 0.1%.
    let nudged = png_of(&picture(SIZE.0, SIZE.1, 14));
    // To 74 pixels: about 7%.
    let blotched = png_of(&picture(SIZE.0, SIZE.1, 74));
    let with = |text: String, shot: &[u8]| {
        std::fs::write(refs.join("thresholds.toml"), text).unwrap();
        check("a-shot", shot, Mode::Flip, &refs, &candidates)
    };
    let limits = |mean: f32, p99: f32, p999: f32| {
        format!("version = 1\n[flip]\nmean = {mean:?}\np99 = {p99:?}\np999 = {p999:?}\n")
    };

    // No thresholds yet: measured and saved, but it cannot fail.
    let verdict = check("a-shot", &blotched, Mode::Flip, &refs, &candidates);
    assert_eq!(verdict.status, Status::NoThresholds, "{verdict:?}");
    assert!(verdict.message.starts_with("a-shot: [W-NO-THRESHOLDS]"));
    assert!(candidates.join("a-shot.png").is_file());
    assert!(
        candidates.join("flip").join("a-shot.png").is_file(),
        "the error map"
    );

    let same = with(THRESHOLDS.into(), &golden);
    assert_eq!(
        (same.status, same.differing),
        (Status::Pass, Some(0)),
        "{same:?}"
    );
    // Zero on this Mac and on x86-64 Linux, but FLIP's own rounding leaves
    // about 1e-7 where g++ builds it for arm64 Linux.
    let flip = same.flip.unwrap();
    let tiny = [flip.mean, flip.p99, flip.p999, flip.max];
    assert!(tiny.iter().all(|e| (0.0..1e-4).contains(e)), "{flip:?}");

    let small = with(limits(1.0, 1.0, 1.0), &nudged);
    assert_eq!((small.status, small.differing), (Status::Pass, Some(96)));
    let small = small.flip.unwrap();
    assert!(small.mean > 0.0 && small.p99 < small.p999, "{small:?}");
    let large = with(limits(1.0, 1.0, 1.0), &blotched).flip.unwrap();
    assert!(large.mean > small.mean && large.p99 > 0.2, "{large:?}");

    // Each limit fails a shot on its own, at the value measured or below.
    for (text, shot) in [
        (limits(small.mean / 2.0, 1.0, 1.0), &nudged),
        (limits(1.0, large.p99 / 2.0, 1.0), &blotched),
        (limits(1.0, 1.0, small.p999 / 2.0), &nudged),
    ] {
        let verdict = with(text.clone(), shot);
        assert_eq!(verdict.status, Status::Fail, "{text}");
        assert!(verdict.message.starts_with("a-shot: [E-GOLDEN] FLIP mean"));
    }
    assert_eq!(listing(&refs), ["a-shot.png", "thresholds.toml"]);
}

#[test]
fn the_proposed_thresholds_fail_a_missing_dot_but_not_a_few_edge_pixels() {
    let (refs, candidates) = scratch("dot");
    std::fs::write(refs.join("a-shot.png"), png_of(&dot(0))).unwrap();
    std::fs::write(refs.join("thresholds.toml"), THRESHOLDS).unwrap();
    for cleared in [1, 4, 12] {
        let verdict = check(
            "a-shot",
            &png_of(&dot(cleared)),
            Mode::Flip,
            &refs,
            &candidates,
        );
        eprintln!("{cleared} edge pixels: {}", verdict.message);
        assert_eq!(verdict.status, Status::Pass, "{verdict:?}");
        assert_eq!(verdict.differing, Some(cleared));
    }
    let missing = png_of(&picture(SIZE.0, SIZE.1, 0));
    let verdict = check("a-shot", &missing, Mode::Flip, &refs, &candidates);
    eprintln!("no dot: {}", verdict.message);
    assert_eq!(verdict.status, Status::Fail, "{verdict:?}");
    let flip = verdict.flip.unwrap();
    assert!(
        flip.mean <= 0.01 && flip.p99 <= 0.2,
        "only p999 sees it: {flip:?}"
    );
}

#[test]
fn thresholds_are_read_strictly() {
    let (refs, candidates) = scratch("thresholds");
    assert_eq!(read_thresholds(&refs), Ok(None), "none yet");
    let path = refs.join("thresholds.toml");
    std::fs::write(&path, THRESHOLDS).unwrap();
    let proposed = Thresholds {
        mean: 0.01,
        p99: 0.2,
        p999: 0.5,
    };
    assert_eq!(read_thresholds(&refs), Ok(Some(proposed)));
    for bad in [
        THRESHOLDS.replace("p99 ", "p95 "),
        THRESHOLDS.replace("version = 1", "version = 2"),
        THRESHOLDS.replace("mean = 0.01", "mean = nan"),
        THRESHOLDS.replace("mean = 0.01", "mean = -0.01"),
        THRESHOLDS.replace("p99 = 0.2", "p99 = 1.5"),
        THRESHOLDS.replace("p999 = 0.5", "p999 = inf"),
        THRESHOLDS.replace("p99 = 0.2\n", ""),
        THRESHOLDS.replace("p999 = 0.5\n", ""),
        format!("{THRESHOLDS}[shots]\n"),
    ] {
        std::fs::write(&path, &bad).unwrap();
        let err = read_thresholds(&refs).unwrap_err();
        let named = err.starts_with("[E-GOLDEN]") && err.contains("thresholds.toml");
        assert!(named, "{bad}: {err}");
    }
    // A broken file fails the shot instead of skipping its limits.
    let golden = png_of(&picture(SIZE.0, SIZE.1, 10));
    std::fs::write(refs.join("a-shot.png"), golden).unwrap();
    let shot = png_of(&picture(SIZE.0, SIZE.1, 11));
    let verdict = check("a-shot", &shot, Mode::Flip, &refs, &candidates);
    assert_eq!(verdict.status, Status::Fail, "{verdict:?}");
}

#[test]
fn only_the_pinned_configuration_compares_exactly() {
    let pinned = pinned_config();
    assert_eq!(mode(&pinned, None), Ok(Mode::Exact));
    assert_eq!(mode(&pinned, Some("")), Ok(Mode::Exact));
    assert_eq!(mode(&pinned, Some("exact")), Ok(Mode::Exact));
    type Change = fn(&mut Config);
    let others: [(&str, Change); 7] = [
        ("arm64", |c| c.arch = "aarch64".into()),
        ("macOS", |c| c.os = "macos".into()),
        ("Metal", |c| c.backend = "metal".into()),
        ("128-bit vectors", |c| {
            c.adapter = "llvmpipe (LLVM 18.1.3, 128 bits)".into()
        }),
        ("another LLVM", |c| {
            c.adapter = "llvmpipe (LLVM 19.1.7, 256 bits)".into()
        }),
        ("another Mesa", |c| {
            c.driver_info = "Mesa 25.0.7 (LLVM 18.1.3)".into()
        }),
        ("a GPU", |c| c.device_type = "discrete-gpu".into()),
    ];
    for (what, change) in others {
        let mut config = pinned.clone();
        change(&mut config);
        assert_eq!(mode(&config, None), Ok(Mode::Flip), "{what}");
        let refused = mode(&config, Some("exact")).unwrap_err();
        assert!(refused.starts_with("[E-GOLDEN]"), "{what}: {refused}");
        assert!(refused.contains(&config.adapter), "{what}: {refused}");
    }
    // No way to ask for FLIP: on the pinned runner it would only weaken it.
    for unknown in ["flip", "close-enough"] {
        let refused = mode(&pinned, Some(unknown)).unwrap_err();
        assert!(refused.contains(unknown), "{refused}");
    }
}

#[test]
fn every_shot_is_rendered_at_320x240_with_the_fixed_llvmpipe_environment() {
    for shot in &SHOTS {
        let command = shot_command(shot, Path::new("out"));
        let args: Vec<String> = (command.get_args())
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let (tick, players) = (shot.tick, shot.players);
        let want = format!("shot --at-tick {tick} --seed 1 --players {players} --size 320x240");
        assert!(args.join(" ").starts_with(&want), "{args:?}");
        let envs: Vec<(String, Option<String>)> = (command.get_envs())
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into(),
                    v.map(|v| v.to_string_lossy().into()),
                )
            })
            .collect();
        let width = ("LP_NATIVE_VECTOR_WIDTH".to_owned(), Some("256".to_owned()));
        assert!(envs.contains(&width), "{envs:?}");
        // Every other llvmpipe variable the test inherits is removed.
        for (key, _) in std::env::vars() {
            let llvmpipe = key.starts_with("LP_") || key.starts_with("GALLIVM_");
            if llvmpipe && key != "LP_NATIVE_VECTOR_WIDTH" {
                assert!(envs.contains(&(key.clone(), None)), "{key}: {envs:?}");
            }
        }
    }
}

#[test]
fn without_an_adapter_the_harness_skips_unless_one_is_required() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("shots")
        .join("no-adapter");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let none = [("ENG_GPU_ADAPTER", "no adapter is called this")];
    assert_eq!(render(&SHOTS[0], &dir, false, &none), Ok(None), "skipped");
    let message = render(&SHOTS[0], &dir, true, &none).unwrap_err();
    let tag = "ENG_REQUIRE_GPU=1: [E-NO-ADAPTER]";
    assert!(message.starts_with(tag), "{message}");
}

#[test]
fn a_failing_shot_fails_the_run_and_the_summary_says_so() {
    let (refs, candidates) = scratch("judge");
    let golden = png_of(&picture(SIZE.0, SIZE.1, 10));
    std::fs::write(refs.join("a-shot.png"), &golden).unwrap();
    let changed = png_of(&picture(SIZE.0, SIZE.1, 11));
    let config = pinned_config();
    let shots = [("a-shot", changed)];
    let failures = judge(&shots, &config, Mode::Exact, (&refs, false), &candidates);
    assert_eq!(failures.len(), 1, "{failures:?}");
    let tag = "a-shot: [E-GOLDEN] 21 of 76800";
    assert!(failures[0].starts_with(tag), "{failures:?}");
    let summary = std::fs::read(candidates.join("shots.json")).unwrap();
    let summary: Value = serde_json::from_slice(&summary).unwrap();
    assert_eq!(summary["mode"], "exact");
    assert_eq!(summary["configuration"]["pinned"], true);
    let machine = summary["machine"].as_object().unwrap();
    assert!(machine.contains_key("os_version") && machine.contains_key("cpu"));
    assert_eq!(summary["shots"][0]["status"], "fail");

    let shots = [("a-shot", golden)];
    let passes = judge(&shots, &config, Mode::Exact, (&refs, false), &candidates);
    assert!(passes.is_empty(), "{passes:?}");
    // A golden that no shot names would never be checked.
    std::fs::write(refs.join("stale.png"), b"left behind").unwrap();
    let failures = judge(&shots, &config, Mode::Exact, (&refs, false), &candidates);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].starts_with("stale: [E-GOLDEN]"), "{failures:?}");
    assert!(failures[0].ends_with("is no shot's golden"), "{failures:?}");
}

#[test]
fn only_a_directory_without_goldens_has_no_golden_yet() {
    let (refs, candidates) = scratch("partial");
    let config = pinned_config();
    let golden = png_of(&picture(SIZE.0, SIZE.1, 10));
    let shots = [("a-shot", golden.clone()), ("b-shot", golden.clone())];
    // None yet: both pass as "no golden yet", unless the directory was
    // asked for with ENG_GOLDEN_DIR.
    assert!(judge(&shots, &config, Mode::Exact, (&refs, false), &candidates).is_empty());
    let summary = std::fs::read(candidates.join("shots.json")).unwrap();
    let summary: Value = serde_json::from_slice(&summary).unwrap();
    assert_eq!(summary["shots"][1]["status"], "no-golden");
    let asked = judge(&shots, &config, Mode::Exact, (&refs, true), &candidates);
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(
        asked[0].ends_with("is missing; ENG_GOLDEN_DIR is set"),
        "{asked:?}"
    );
    // One golden: the other shot's is missing, as after a deletion.
    std::fs::write(refs.join("a-shot.png"), &golden).unwrap();
    let partial = judge(&shots, &config, Mode::Exact, (&refs, false), &candidates);
    assert_eq!(partial.len(), 1, "{partial:?}");
    let tag = "b-shot: [E-GOLDEN]";
    assert!(partial[0].starts_with(tag), "{partial:?}");
    assert!(partial[0].ends_with("is missing; other shots have theirs"));
    assert!(candidates.join("b-shot.png").is_file(), "the shot is saved");
}

#[test]
fn the_golden_directory_must_exist_outside_the_candidates() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (refs, candidates) = scratch("golden-dir");
    let default = Ok((manifest.join("golden"), false));
    assert_eq!(golden_dir(None, &candidates), default);
    assert_eq!(
        golden_dir(Some("".into()), &candidates),
        default,
        "empty is unset"
    );
    let asked = golden_dir(Some(refs.clone().into()), &candidates);
    assert_eq!(asked, Ok((refs.clone(), true)));
    // Each run empties the candidates first, so every shot would pass as
    // "no golden yet"; a directory that does not exist would too.
    let inside = candidates.join("from-another-machine");
    std::fs::create_dir_all(&inside).unwrap();
    for bad in [candidates.join("."), inside, refs.join("nowhere")] {
        let err = golden_dir(Some(bad.clone().into()), &candidates).unwrap_err();
        assert!(
            err.starts_with("[E-GOLDEN] ENG_GOLDEN_DIR"),
            "{bad:?}: {err}"
        );
    }
}
