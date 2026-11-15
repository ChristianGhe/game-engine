//! Each frame's entry, computed from its targets read back: the image's
//! luminance and histogram, and for a first-person frame each object's
//! pixels, screen box, luminance and surroundings, the classes' thirds and
//! the depth probes (see `frame_record.rs` for what each means).

use std::collections::BTreeMap;

use eng_gpu::{Format, Image, Texels};
use eng_render::{FirstPersonCamera, Info, SurfaceClass, srgb_to_linear};

use super::{
    AskedObject, BANDS, CameraRecord, ClassBands, FrameEntry, HISTOGRAM_BINS, ObjectKey,
    ObjectRecord, PROBE_COLUMNS, PROBE_ROWS, Probe, REC709, SURROUND, Unlisted, round6,
};

/// Linear light of each 8-bit sRGB level, by eng-render's curve.
fn linear_levels() -> [f64; 256] {
    core::array::from_fn(|level| f64::from(srgb_to_linear(level as f32 / 255.0)))
}

/// The luminance of an 8-bit sRGB pixel: Rec. 709's weights of its linear
/// red, green and blue ([`REC709`], eng-render's `srgb_to_linear`), 0 to 1.
pub fn luminance(rgb: [u8; 3]) -> f64 {
    let levels = linear_levels();
    pixel_luminance(&levels, rgb)
}

fn pixel_luminance(levels: &[f64; 256], [r, g, b]: [u8; 3]) -> f64 {
    REC709[0] * levels[usize::from(r)]
        + REC709[1] * levels[usize::from(g)]
        + REC709[2] * levels[usize::from(b)]
}

/// The histogram's 15 inner edges: `srgb_to_linear(k / 16)`, k = 1 to 15.
fn histogram_edges() -> [f64; HISTOGRAM_BINS - 1] {
    core::array::from_fn(|k| f64::from(srgb_to_linear((k + 1) as f32 / HISTOGRAM_BINS as f32)))
}

/// What the whole image says: each pixel's luminance, the mean and the
/// histogram.
struct ImageStats {
    luminance: Vec<f64>,
    mean: f64,
    histogram: [u64; HISTOGRAM_BINS],
}

fn image_stats(image: &Image) -> ImageStats {
    let levels = linear_levels();
    let edges = histogram_edges();
    let mut histogram = [0; HISTOGRAM_BINS];
    let luminance: Vec<f64> = image
        .rgba8
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            let y = pixel_luminance(&levels, [p[0], p[1], p[2]]);
            histogram[edges.iter().filter(|edge| **edge <= y).count()] += 1;
            y
        })
        .collect();
    let sum: f64 = luminance.iter().sum();
    let mean = if luminance.is_empty() {
        0.0
    } else {
        sum / luminance.len() as f64
    };
    ImageStats {
        luminance,
        mean: round6(mean),
        histogram,
    }
}

/// A top-down frame's reduced entry: the image's luminance and histogram.
pub fn top_down_entry(tick: u64, image: &Image) -> FrameEntry {
    let stats = image_stats(image);
    FrameEntry {
        tick,
        camera: None,
        objects: Vec::new(),
        unlisted: Vec::new(),
        invalid_info: 0,
        classes: None,
        probes: Vec::new(),
        luminance: stats.mean,
        histogram: stats.histogram,
    }
}

/// The probes' pixels, row by row from the top left.
pub fn probe_points(width: u32, height: u32) -> Vec<(u32, u32)> {
    let at = |i: u32, n: u32, side: u32| ((2 * i + 1) as u64 * side as u64 / (2 * n) as u64) as u32;
    (0..PROBE_ROWS)
        .flat_map(|j| (0..PROBE_COLUMNS).map(move |i| (i, j)))
        .map(|(i, j)| (at(i, PROBE_COLUMNS, width), at(j, PROBE_ROWS, height)))
        .collect()
}

/// An object's running sums over the image.
#[derive(Clone, Copy)]
struct Seen {
    pixels: u64,
    sum: f64,
    min: (u32, u32),
    max: (u32, u32),
}

/// A first-person frame's entry, from the frame's camera, the objects
/// asked for ([`super::asked_objects`], in key order) and its targets read
/// back: `info` (`Rg32Uint`), depth (`Depth32Float`) and the output, all
/// of one size. An error, a bug in the caller, if they are not, or if a
/// probed depth is not finite.
pub fn first_person_entry(
    tick: u64,
    camera: &FirstPersonCamera,
    asked: &[AskedObject],
    info: &Texels,
    depth: &Texels,
    image: &Image,
) -> Result<FrameEntry, String> {
    let (width, height) = (image.width, image.height);
    let sizes = [(info.width, info.height), (depth.width, depth.height)];
    if sizes.iter().any(|size| *size != (width, height)) {
        return Err(format!(
            "the frame record needs info, depth and output of one size: {:?} {:?} {:?}",
            sizes[0],
            sizes[1],
            (width, height)
        ));
    }
    if info.format != Format::Rg32Uint || depth.format != Format::Depth32Float {
        return Err(format!(
            "the frame record reads info as Rg32Uint and depth as Depth32Float, not {:?} and {:?}",
            info.format, depth.format
        ));
    }
    if asked.windows(2).any(|pair| pair[0].key >= pair[1].key) {
        return Err("the objects asked for must be in key order, each once".into());
    }
    if asked.iter().any(|o| o.key.class == SurfaceClass::None) {
        return Err("an object asked for has class none: the sky is no object".into());
    }
    let stats = image_stats(image);
    let pixel = |x: u32, y: u32| (y as usize) * (width as usize) + x as usize;
    let keys: Vec<Option<ObjectKey>> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            Info::unpack(info.uint2(x, y)).map(|i| ObjectKey {
                class: i.class,
                id: i.id,
            })
        })
        .collect();
    let mut seen: Vec<Option<Seen>> = vec![None; asked.len()];
    let mut unlisted: BTreeMap<ObjectKey, u64> = BTreeMap::new();
    let mut classes: ClassBands = [[0; BANDS]; SurfaceClass::ALL.len()];
    let mut invalid_info = 0;
    for y in 0..height {
        let band = (y as u64 * BANDS as u64 / height as u64) as usize;
        for x in 0..width {
            let at = pixel(x, y);
            let Some(key) = keys[at] else {
                invalid_info += 1;
                continue;
            };
            classes[usize::from(key.class.code())][band] += 1;
            if key.class == SurfaceClass::None {
                continue;
            }
            match asked.binary_search_by(|o| o.key.cmp(&key)) {
                Ok(index) => {
                    let lum = stats.luminance[at];
                    let s = seen[index].get_or_insert(Seen {
                        pixels: 0,
                        sum: 0.0,
                        min: (x, y),
                        max: (x, y),
                    });
                    s.pixels += 1;
                    s.sum += lum;
                    s.min = (s.min.0.min(x), s.min.1.min(y));
                    s.max = (s.max.0.max(x), s.max.1.max(y));
                }
                Err(_) => *unlisted.entry(key).or_insert(0) += 1,
            }
        }
    }
    let objects = asked
        .iter()
        .zip(&seen)
        .map(|(object, seen)| {
            let mut record = ObjectRecord {
                key: object.key,
                name: object.name.clone(),
                world_box: object.world_box,
                pixels: 0,
                screen_box: None,
                luminance: None,
                surround_luminance: None,
                contrast: None,
            };
            let Some(s) = seen else {
                return record;
            };
            let mean = s.sum / s.pixels as f64;
            record.pixels = s.pixels;
            record.screen_box = Some([s.min.0, s.min.1, s.max.0, s.max.1]);
            record.luminance = Some(round6(mean));
            // The box grown by SURROUND, clipped, less the object's own.
            let (x0, y0) = (
                s.min.0.saturating_sub(SURROUND),
                s.min.1.saturating_sub(SURROUND),
            );
            let x1 = s.max.0.saturating_add(SURROUND).min(width - 1);
            let y1 = s.max.1.saturating_add(SURROUND).min(height - 1);
            let (mut count, mut sum) = (0_u64, 0.0);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let at = pixel(x, y);
                    if keys[at] != Some(object.key) {
                        count += 1;
                        sum += stats.luminance[at];
                    }
                }
            }
            if count > 0 {
                let around = sum / count as f64;
                let total = mean + around;
                let contrast = if total > 0.0 {
                    (mean - around).abs() / total
                } else {
                    0.0
                };
                record.surround_luminance = Some(round6(around));
                record.contrast = Some(round6(contrast));
            }
            record
        })
        .collect();
    let near = f64::from(camera.near);
    let mut probes = Vec::with_capacity((PROBE_COLUMNS * PROBE_ROWS) as usize);
    for (x, y) in probe_points(width, height) {
        let d = depth.depth(x, y);
        if !d.is_finite() {
            return Err(format!(
                "[E-GPU] the depth at probe ({x}, {y}) is {d}, not finite"
            ));
        }
        let key = keys[pixel(x, y)].unwrap_or(ObjectKey {
            class: SurfaceClass::None,
            id: 0,
        });
        let mut edge = false;
        let mut slope: f32 = 0.0;
        let neighbours = [
            (x.checked_sub(1), Some(y)),
            (x.checked_add(1).filter(|x| *x < width), Some(y)),
            (Some(x), y.checked_sub(1)),
            (Some(x), y.checked_add(1).filter(|y| *y < height)),
        ];
        for (nx, ny) in neighbours {
            let (Some(nx), Some(ny)) = (nx, ny) else {
                continue;
            };
            if keys[pixel(nx, ny)] == Some(key) {
                let step = (depth.depth(nx, ny) - d).abs();
                if step.is_finite() && step > slope {
                    slope = step;
                }
            } else {
                edge = true;
            }
        }
        probes.push(Probe {
            x,
            y,
            key,
            depth: d,
            distance: (d > 0.0).then(|| round6(near / f64::from(d))),
            edge,
            slope,
        });
    }
    Ok(FrameEntry {
        tick,
        camera: Some(CameraRecord::of(camera)),
        objects,
        unlisted: unlisted
            .into_iter()
            .map(|(key, pixels)| Unlisted { key, pixels })
            .collect(),
        invalid_info,
        classes: Some(classes),
        probes,
        luminance: stats.mean,
        histogram: stats.histogram,
    })
}
