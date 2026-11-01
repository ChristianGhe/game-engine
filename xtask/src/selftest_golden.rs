//! `cargo xtask selftest golden-edit` (rung 1b, criterion 12; amendment 58
//! of `docs/rungs/1b/PROGRESS.md`): a golden with one level of one pixel
//! changed must fail the shots harness (`crates/game-testbed/tests/shots.rs`)
//! with `[E-GOLDEN] 1 of N pixels differ from the golden`.
//!
//! Only an exact comparison sees one level; FLIP passes it by design. So the
//! case runs the harness with `ENG_SHOT_MODE=exact`, which needs the pinned
//! lavapipe (Mesa 25.2.7 from gfx-rs/ci-build build26, Linux x86_64), and
//! `ENG_REQUIRE_GPU=1`. Anywhere else, with no adapter or another one, it
//! says that it could not run ([`NOT_RUN`]): never a pass.
//!
//! 1. An unedited run renders the shots and checks them, exactly, against
//!    `golden/`. Before the owner commits goldens (G6) there are none, and
//!    the run saves each shot as a candidate instead.
//! 2. The goldens, or else those candidates, and `thresholds.toml` are
//!    copied to `target/selftest/golden-edit/golden/`; `golden/` itself is
//!    the owner's and is never written.
//! 3. One level of one pixel of the first golden changes.
//! 4. A run with `ENG_GOLDEN_DIR` set to the copy must fail on that golden,
//!    for one pixel, and on nothing else.

use std::fs;
use std::path::Path;

use crate::selftest::NOT_RUN;
use crate::util;

pub fn run(root: &Path) -> Result<String, String> {
    let work = root.join("target/selftest/golden-edit");
    let _ = fs::remove_dir_all(&work);
    let refs = work.join("golden");
    fs::create_dir_all(&refs).map_err(|e| format!("{}: {e}", refs.display()))?;
    let (succeeded, output) = shots(root, None, "unedited")?;
    preflight(succeeded, &output)?;
    let golden = root.join("crates/game-testbed/golden");
    let candidates = root.join("target/golden-candidates");
    let from = if pngs(&golden).is_empty() {
        candidates
    } else {
        golden.clone()
    };
    let names = pngs(&from);
    let Some(edited) = names.first().cloned() else {
        return Err(format!(
            "the unedited run left no golden or candidate to edit in {}",
            from.display()
        ));
    };
    for name in &names {
        let file = format!("{name}.png");
        copy(&from.join(&file), &refs.join(&file))?;
    }
    if golden.join("thresholds.toml").is_file() {
        copy(
            &golden.join("thresholds.toml"),
            &refs.join("thresholds.toml"),
        )?;
    }
    let path = refs.join(format!("{edited}.png"));
    let read = |path: &Path| {
        fs::read(path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|bytes| read_png(&bytes))
    };
    let original = read(&path)?;
    let mut image = original.clone();
    let (at, old, new) = nudge(&mut image);
    fs::write(&path, write_png(&image)?).map_err(|e| e.to_string())?;
    // What the harness will decode: one byte away from the original.
    let back = read(&path)?;
    let changed = (back.rgba.iter().zip(&original.rgba))
        .filter(|(a, b)| a != b)
        .count();
    if (back.width, back.height, changed) != (original.width, original.height, 1) {
        return Err(format!(
            "the edited {edited}.png decodes {changed} bytes away from the original, not 1"
        ));
    }
    let (succeeded, output) = shots(root, Some(&refs), "edited")?;
    judge(succeeded, &output, &edited)
        .map(|caught| format!("{caught} (byte {at} of {edited}.png: {old} -> {new})"))
}

/// The run of the harness's one GPU test, `shots_match_their_goldens`, in
/// exact mode and requiring an adapter; with `ENG_GOLDEN_DIR` set to `refs`
/// or unset. Returns whether it passed and its output, which is also logged.
fn shots(root: &Path, refs: Option<&Path>, label: &str) -> Result<(bool, String), String> {
    let mut cmd = util::cargo(root);
    cmd.args(["test", "-p", "game-testbed", "--test", "shots"]);
    if util::on_ci() {
        cmd.arg("--locked");
    }
    cmd.args(["--", "--exact", "shots_match_their_goldens"]);
    cmd.env("ENG_REQUIRE_GPU", "1")
        .env("ENG_SHOT_MODE", "exact");
    match refs {
        Some(dir) => cmd.env("ENG_GOLDEN_DIR", dir),
        None => cmd.env_remove("ENG_GOLDEN_DIR"),
    };
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run cargo test: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let log = root.join(format!(
        "target/agent-logs/xtask-selftest-golden-edit-{label}.log"
    ));
    let _ = fs::create_dir_all(log.parent().expect("has parent"));
    let _ = fs::write(&log, &text);
    Ok((out.status.success(), text))
}

/// The names of the PNG files directly in `dir`, sorted.
fn pngs(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|e| e == "png"))
        .filter_map(|path| Some(path.file_stem()?.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", from.display()))
}

/// The first line of `output` that holds `text`, trimmed.
fn line_with<'a>(output: &'a str, text: &str) -> Option<&'a str> {
    output.lines().find(|l| l.contains(text)).map(str::trim)
}

/// What the unedited run says about this machine.
fn preflight(succeeded: bool, output: &str) -> Result<(), String> {
    if succeeded {
        return Ok(());
    }
    let why = line_with(output, "[E-NO-ADAPTER]")
        .or_else(|| line_with(output, "ENG_SHOT_MODE=exact needs the pinned configuration"));
    match why {
        Some(why) => Err(format!(
            "{NOT_RUN} golden-edit could not run here: one level of one pixel shows only in \
             an exact comparison, which needs the pinned lavapipe (Mesa 25.2.7, gfx-rs/ci-build \
             build26, Linux x86_64). The shots harness says: {why}"
        )),
        None => Err(format!(
            "the shots harness fails before any golden is edited:\n{}",
            util::tail(output, 15)
        )),
    }
}

/// Whether the run against the edited copy failed on `edited` alone, for
/// one differing pixel.
fn judge(succeeded: bool, output: &str, edited: &str) -> Result<String, String> {
    if succeeded {
        return Err(format!(
            "the shots harness passed an edited golden ({edited}.png)"
        ));
    }
    let expected = format!("{edited}: [E-GOLDEN] 1 of ");
    let caught = output
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with(&expected) && l.ends_with("pixels differ from the golden"));
    let Some(caught) = caught else {
        return Err(format!(
            "failed, but not with `{expected}N pixels differ from the golden`:\n{}",
            util::tail(output, 15)
        ));
    };
    let mut others: Vec<&str> = output
        .lines()
        .filter_map(|l| l.trim().split_once(": [E-GOLDEN]"))
        .map(|(shot, _)| shot)
        .filter(|shot| *shot != edited)
        .collect();
    others.sort_unstable();
    others.dedup();
    if others.is_empty() {
        Ok(format!("rejected: {caught}"))
    } else {
        Err(format!(
            "{edited} failed as it should, but unedited shots failed too: {}",
            others.join(", ")
        ))
    }
}

/// Moves the red channel of the middle pixel by one level. Returns the byte's
/// index and its old and new values.
fn nudge(image: &mut Image) -> (usize, u8, u8) {
    let (x, y) = (image.width as usize / 2, image.height as usize / 2);
    let at = (y * image.width as usize + x) * 4;
    let old = image.rgba[at];
    let new = if old == 255 { 254 } else { old + 1 };
    image.rgba[at] = new;
    (at, old, new)
}

/// Decoded pixels, row after row, four bytes each: 8-bit RGBA, which is
/// what `shot` writes and the harness compares.
#[derive(Debug, Clone, PartialEq)]
struct Image {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Decodes as the harness does (`crates/game-testbed/tests/shots/compare.rs`).
fn read_png(bytes: &[u8]) -> Result<Image, String> {
    let error = |e: png::DecodingError| format!("PNG: {e}");
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(error)?;
    let size = reader
        .output_buffer_size()
        .ok_or("PNG: too large to decode")?;
    let mut rgba = vec![0; size];
    let info = reader.next_frame(&mut rgba).map_err(error)?;
    if (info.color_type, info.bit_depth) != (png::ColorType::Rgba, png::BitDepth::Eight) {
        let what = format!("{:?} {:?}", info.color_type, info.bit_depth);
        return Err(format!("PNG: {what}, not 8-bit RGBA"));
    }
    rgba.truncate(info.buffer_size());
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}

fn write_png(image: &Image) -> Result<Vec<u8>, String> {
    let error = |e: png::EncodingError| format!("PNG: {e}");
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(error)?;
    writer.write_image_data(&image.rgba).map_err(error)?;
    writer.finish().map_err(error)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{Image, NOT_RUN, judge, nudge, preflight, read_png, write_png};

    #[test]
    fn a_png_written_reads_back_the_same() {
        let image = Image {
            width: 3,
            height: 2,
            rgba: (0..24).collect(),
        };
        let bytes = write_png(&image).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(read_png(&bytes).unwrap(), image);
    }

    #[test]
    fn only_8_bit_rgba_is_read() {
        // RGB, as the FLIP maps are: never a golden.
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[1, 2, 3, 4, 5, 6]).unwrap();
        writer.finish().unwrap();
        let err = read_png(&bytes).unwrap_err();
        assert!(err.contains("not 8-bit RGBA"), "{err}");
        assert!(read_png(b"GIF89a").is_err());
    }

    const NO_ADAPTER: &str = "thread 'shots_match_their_goldens' panicked at crates/game-testbed/tests/shots.rs:372:62:\n\
        ENG_REQUIRE_GPU=1: [E-NO-ADAPTER] no GPU adapter matches ENG_GPU_ADAPTER=none\n";
    const NOT_PINNED: &str = "thread 'shots_match_their_goldens' panicked at crates/game-testbed/tests/shots.rs:381:76:\n\
        [E-GOLDEN] ENG_SHOT_MODE=exact needs the pinned configuration (linux x86_64, vulkan, \
        `llvmpipe (LLVM 18.1.3, 256 bits)`, `Mesa 25.2.7 (LLVM 18.1.3)`), not macos aarch64 metal `Apple M2 Pro` ``\n";
    const CAUGHT: &str = "dot-square-0: [E-GOLDEN] 1 of 76800 pixels differ from the golden\n\
        dot-square-120: exact\ndot-square-240: exact\nten-dots-0: exact\n\
        thread 'shots_match_their_goldens' panicked at crates/game-testbed/tests/shots.rs:384:5:\n\
        dot-square-0: [E-GOLDEN] 1 of 76800 pixels differ from the golden\n\
        test result: FAILED. 0 passed; 1 failed";

    #[test]
    fn the_unedited_run_tells_a_machine_that_cannot_run_the_case() {
        assert_eq!(preflight(true, "test result: ok. 1 passed"), Ok(()));
        for (output, why) in [
            (NO_ADAPTER, "[E-NO-ADAPTER]"),
            (NOT_PINNED, "needs the pinned configuration"),
        ] {
            let err = preflight(false, output).unwrap_err();
            assert!(err.starts_with(NOT_RUN) && err.contains(why), "{err}");
        }
        // Any other failure is the harness's, before anything was edited.
        let err = preflight(false, "error[E0425]: cannot find value").unwrap_err();
        assert!(!err.starts_with(NOT_RUN) && err.contains("before"), "{err}");
    }

    #[test]
    fn the_edited_run_must_fail_on_the_edited_golden_alone() {
        let caught = judge(false, CAUGHT, "dot-square-0").unwrap();
        assert_eq!(
            caught,
            "rejected: dot-square-0: [E-GOLDEN] 1 of 76800 pixels differ from the golden"
        );
        let passed = judge(true, "test result: ok. 1 passed", "dot-square-0").unwrap_err();
        assert!(passed.contains("passed an edited golden"), "{passed}");
        // Failing for another reason, or on another shot, is not this case.
        for (output, edited) in [(NOT_PINNED, "dot-square-0"), (CAUGHT, "ten-dots-0")] {
            let err = judge(false, output, edited).unwrap_err();
            assert!(err.contains("but not with"), "{err}");
        }
        // One level of one pixel differs by exactly one pixel; more is
        // another edit, or another render.
        let more = CAUGHT.replace("] 1 of", "] 2 of");
        let err = judge(false, &more, "dot-square-0").unwrap_err();
        assert!(err.contains("but not with"), "{err}");
        // The harness prints each failure twice: as it goes, then in its panic.
        let other = "ten-dots-0: [E-GOLDEN] 3 of 76800 pixels differ from the golden";
        let two = format!("{CAUGHT}\n{other}\n{other}");
        let err = judge(false, &two, "dot-square-0").unwrap_err();
        assert_eq!(
            err,
            "dot-square-0 failed as it should, but unedited shots failed too: ten-dots-0"
        );
    }

    #[test]
    fn the_nudge_moves_one_channel_of_one_pixel_by_one_level() {
        let mut image = Image {
            width: 4,
            height: 2,
            rgba: vec![200; 32],
        };
        let before = image.clone();
        let (at, from, to) = nudge(&mut image);
        // The middle pixel (2, 1), its red byte.
        assert_eq!((at, from, to), ((4 + 2) * 4, 200, 201));
        let changed: Vec<usize> = (0..32)
            .filter(|&i| image.rgba[i] != before.rgba[i])
            .collect();
        assert_eq!(changed, [at]);
        let mut white = Image {
            width: 4,
            height: 2,
            rgba: vec![255; 32],
        };
        assert_eq!(nudge(&mut white).2, 254);
    }
}
