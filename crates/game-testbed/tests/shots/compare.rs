//! The shot harness's comparison (`../shots.rs`): one rendered shot against
//! its golden, exactly or with FLIP, and what it saves. Its tests, which
//! need no GPU, are in `parts.rs`.

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

/// Every shot is this size (plan decision 3).
pub(crate) const SIZE: (u32, u32) = (320, 240);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Mode {
    Exact,
    Flip,
}

/// A decoded image: RGBA8 rows, top first.
pub(crate) struct Image {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba8: Vec<u8>,
}

fn decode(bytes: &[u8]) -> Result<Image, String> {
    let error = |e: png::DecodingError| e.to_string();
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(error)?;
    let size = reader.output_buffer_size().ok_or("too large to decode")?;
    let mut rgba8 = vec![0; size];
    let info = reader.next_frame(&mut rgba8).map_err(error)?;
    if (info.color_type, info.bit_depth) != (png::ColorType::Rgba, png::BitDepth::Eight) {
        let what = format!("{:?} {:?}", info.color_type, info.bit_depth);
        return Err(format!("{what}, not 8-bit RGBA"));
    }
    rgba8.truncate(info.buffer_size());
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba8,
    })
}

pub(crate) fn encode(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(data).unwrap();
    writer.finish().unwrap();
    bytes
}

/// `thresholds.toml`'s limits, which every shot compared with FLIP must
/// meet, each from 0 to 1: FLIP's mean error over the image, and the 99th
/// and 99.9th percentiles of its per-pixel errors (unweighted). The 99.9th
/// is the 77th-worst pixel of a 320x240 shot, so it sees a small area that
/// is wrong, such as a dot (about 314 pixels) that is missing.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Thresholds {
    pub(crate) mean: f32,
    pub(crate) p99: f32,
    pub(crate) p999: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdsFile {
    version: u32,
    flip: Thresholds,
}

/// The thresholds in `dir`, `None` while it has none.
pub(crate) fn read_thresholds(dir: &Path) -> Result<Option<Thresholds>, String> {
    let path = dir.join("thresholds.toml");
    if !path.exists() {
        return Ok(None);
    }
    let at = |e: &dyn std::fmt::Display| format!("[E-GOLDEN] {}: {e}", path.display());
    let text = std::fs::read_to_string(&path).map_err(|e| at(&e))?;
    let file: ThresholdsFile = toml::from_str(&text).map_err(|e| at(&e))?;
    if file.version != 1 {
        return Err(at(&format!("version {}, not 1", file.version)));
    }
    let flip = file.flip;
    for (key, value) in [("mean", flip.mean), ("p99", flip.p99), ("p999", flip.p999)] {
        if !(value.is_finite() && (0.0..=1.0).contains(&value)) {
            return Err(at(&format!("flip.{key} is {value}, not from 0 to 1")));
        }
    }
    Ok(Some(flip))
}

/// FLIP's error between a golden and a shot.
#[derive(Debug)]
pub(crate) struct Flip {
    pub(crate) mean: f32,
    pub(crate) p99: f32,
    pub(crate) p999: f32,
    pub(crate) max: f32,
}

impl Flip {
    fn within(&self, limits: &Thresholds) -> bool {
        self.mean <= limits.mean && self.p99 <= limits.p99 && self.p999 <= limits.p999
    }
}

/// FLIP on the images' RGB (both are opaque) at its default 67 pixels per
/// degree, with the error map (magma) in RGB8.
fn flip(golden: &Image, shot: &Image) -> (Flip, Vec<u8>) {
    let rgb = |image: &Image| {
        let pixels: Vec<u8> = (image.rgba8.chunks(4))
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        nv_flip::FlipImageRgb8::with_data(image.width, image.height, &pixels)
    };
    let error = nv_flip::flip(rgb(golden), rgb(shot), nv_flip::DEFAULT_PIXELS_PER_DEGREE);
    let mut pool = nv_flip::FlipPool::from_image(&error);
    let stats = Flip {
        mean: pool.mean(),
        p99: pool.get_percentile(0.99, false),
        p999: pool.get_percentile(0.999, false),
        max: pool.max_value(),
    };
    (stats, error.apply_color_lut(&nv_flip::magma_lut()).to_vec())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Status {
    Pass,
    Fail,
    NoGolden,
    NoThresholds,
}

/// One shot's result.
#[derive(Debug)]
pub(crate) struct Verdict {
    pub(crate) status: Status,
    pub(crate) message: String,
    /// Pixels that differ from the golden in any channel.
    pub(crate) differing: Option<usize>,
    pub(crate) flip: Option<Flip>,
}

impl Verdict {
    pub(crate) fn json(&self, name: &str) -> Value {
        let status = match self.status {
            Status::Pass => "pass",
            Status::Fail => "fail",
            Status::NoGolden => "no-golden",
            Status::NoThresholds => "no-thresholds",
        };
        let flip = (self.flip.as_ref())
            .map(|f| json!({"mean": f.mean, "p99": f.p99, "p999": f.p999, "max": f.max}));
        json!({
            "name": name, "status": status, "message": self.message,
            "differing_pixels": self.differing, "flip": flip,
        })
    }
}

/// Compares the shot `name` (its PNG file's bytes) with `refs/<name>.png`,
/// the golden. A shot that does not pass is saved as `candidates/<name>.png`
/// and a FLIP comparison leaves its map in `candidates/flip/<name>.png`;
/// nothing is written into `refs`. A missing golden is "no golden yet"
/// (`[W-NO-GOLDEN]`), which the caller may count as a failure.
pub(crate) fn check(name: &str, png: &[u8], mode: Mode, refs: &Path, candidates: &Path) -> Verdict {
    let file = format!("{name}.png");
    let end = |status: Status, message: String, differing, flip| {
        if status != Status::Pass {
            std::fs::write(candidates.join(&file), png).unwrap();
        }
        let message = format!("{name}: {message}");
        Verdict {
            status,
            message,
            differing,
            flip,
        }
    };
    let fail = |message: String| end(Status::Fail, format!("[E-GOLDEN] {message}"), None, None);

    let shot = match decode(png) {
        Ok(shot) => shot,
        Err(e) => return fail(format!("the shot does not decode: {e}")),
    };
    if (shot.width, shot.height) != SIZE {
        let (width, height) = (shot.width, shot.height);
        return fail(format!("the shot is {width}x{height}, not 320x240"));
    }
    if shot.rgba8.chunks(4).any(|p| p[3] != 255) {
        return fail("the shot is not opaque".into());
    }
    let golden_path = refs.join(&file);
    if !golden_path.exists() {
        let candidate = candidates.join(&file).display().to_string();
        let message = format!("[W-NO-GOLDEN] no golden yet; the candidate is {candidate}");
        return end(Status::NoGolden, message, None, None);
    }
    let read = std::fs::read(&golden_path).map_err(|e| e.to_string());
    let golden = match read.and_then(|bytes| decode(&bytes)) {
        Ok(golden) => golden,
        Err(e) => return fail(format!("the golden does not decode: {e}")),
    };
    if (golden.width, golden.height) != SIZE {
        let (width, height) = (golden.width, golden.height);
        return fail(format!("the golden is {width}x{height}, not 320x240"));
    }
    let pairs = golden.rgba8.chunks(4).zip(shot.rgba8.chunks(4));
    let differing = pairs.filter(|(g, s)| g != s).count();
    let total = (SIZE.0 * SIZE.1) as usize;
    if mode == Mode::Exact {
        if differing == 0 {
            return end(Status::Pass, "exact".into(), Some(0), None);
        }
        let message = format!("[E-GOLDEN] {differing} of {total} pixels differ from the golden");
        return end(Status::Fail, message, Some(differing), None);
    }
    let (stats, map) = flip(&golden, &shot);
    let maps = candidates.join("flip");
    std::fs::create_dir_all(&maps).unwrap();
    let map = encode(SIZE.0, SIZE.1, png::ColorType::Rgb, &map);
    std::fs::write(maps.join(&file), map).unwrap();
    let measured = format!(
        "FLIP mean {:.6}, p99 {:.6}, p999 {:.6}, max {:.6}; {differing} of {total} pixels differ",
        stats.mean, stats.p99, stats.p999, stats.max
    );
    let (status, message) = match read_thresholds(refs) {
        Err(e) => (Status::Fail, format!("{e}; {measured}")),
        Ok(None) => {
            let message = format!("[W-NO-THRESHOLDS] {measured}; no thresholds yet");
            (Status::NoThresholds, message)
        }
        Ok(Some(limits)) if stats.within(&limits) => (Status::Pass, measured),
        Ok(Some(limits)) => {
            let (mean, p99, p999) = (limits.mean, limits.p99, limits.p999);
            let limits = format!("the limits are mean {mean}, p99 {p99} and p999 {p999}");
            (Status::Fail, format!("[E-GOLDEN] {measured}; {limits}"))
        }
    };
    end(status, message, Some(differing), Some(stats))
}
