//! The frame record's strict reader ([`FrameRecord::parse`]): JSON text
//! (through `json.rs`) into a [`FrameRecord`], refusing anything version 1
//! does not hold.

use std::collections::BTreeSet;

use eng_render::SurfaceClass;

use super::json::{
    At, Json, angle, array, boolean, f32_of, f64_of, fields, nullable, parse_json, string, u32_of,
    uint, within, wrong,
};
use super::{
    BANDS, CameraRecord, ClassBands, FRAME_RECORD_VERSION, FrameEntry, FrameRecord, HISTOGRAM_BINS,
    ObjectKey, ObjectRecord, Probe, RecordView, Unlisted, WorldBox, class_name, class_named,
    probe_points,
};

fn class(value: &Json, at: At<'_>) -> Result<SurfaceClass, String> {
    let name = string(value, at)?;
    class_named(name).map_or_else(|| wrong(at, &format!("{name:?} is no class")), Ok)
}

fn triple(value: &Json, at: At<'_>) -> Result<[f64; 3], String> {
    let items = array(value, at)?;
    if items.len() != 3 {
        return wrong(at, "not three numbers");
    }
    let mut out = [0.0; 3];
    for (i, item) in items.iter().enumerate() {
        out[i] = f64_of(item, &format!("{at}/{i}"))?;
    }
    Ok(out)
}

fn world_box(value: &Json, at: At<'_>) -> Result<WorldBox, String> {
    let [max, min] = fields(value, at, ["max", "min"])?;
    let b = WorldBox {
        min: triple(min, &format!("{at}/min"))?,
        max: triple(max, &format!("{at}/max"))?,
    };
    if (0..3).any(|axis| b.min[axis] > b.max[axis]) {
        return wrong(at, "min is above max");
    }
    Ok(b)
}

fn camera(value: &Json, at: At<'_>) -> Result<CameraRecord, String> {
    let keys = ["eye", "fov_y_deg", "near", "pitch_deg", "yaw_deg"];
    let [eye, fov, near, pitch, yaw] = fields(value, at, keys)?;
    let near = f32_of(near, &format!("{at}/near"))?;
    if near <= 0.0 {
        return wrong(&format!("{at}/near"), "not above 0");
    }
    Ok(CameraRecord {
        eye: triple(eye, &format!("{at}/eye"))?,
        yaw_deg: angle(yaw, &format!("{at}/yaw_deg"), 0.0)?,
        pitch_deg: angle(pitch, &format!("{at}/pitch_deg"), -180.0)?,
        fov_y_deg: angle(fov, &format!("{at}/fov_y_deg"), 0.0)?,
        near,
    })
}

fn object(value: &Json, at: At<'_>, size: (u32, u32)) -> Result<ObjectRecord, String> {
    let keys = [
        "class",
        "contrast",
        "id",
        "luminance",
        "name",
        "pixels",
        "screen_box",
        "surround_luminance",
        "world_box",
    ];
    let [
        class_v,
        contrast,
        id,
        lum,
        name,
        pixels,
        screen,
        around,
        wbox,
    ] = fields(value, at, keys)?;
    let key = ObjectKey {
        class: class(class_v, &format!("{at}/class"))?,
        id: u32_of(id, &format!("{at}/id"))?,
    };
    if key.class == SurfaceClass::None {
        return wrong(at, "an object of class none: the sky is no object");
    }
    let total = u64::from(size.0) * u64::from(size.1);
    let pixels = uint(pixels, &format!("{at}/pixels"), total)?;
    let unit = |v: &Json, key: &str| nullable(v, |v| within(v, &format!("{at}/{key}"), 0.0, 1.0));
    let record = ObjectRecord {
        key,
        name: nullable(name, |v| {
            string(v, &format!("{at}/name")).map(str::to_owned)
        })?,
        world_box: world_box(wbox, &format!("{at}/world_box"))?,
        pixels,
        screen_box: nullable(screen, |v| screen_box(v, &format!("{at}/screen_box"), size))?,
        luminance: unit(lum, "luminance")?,
        surround_luminance: unit(around, "surround_luminance")?,
        contrast: unit(contrast, "contrast")?,
    };
    let seen = [record.screen_box.is_some(), record.luminance.is_some()];
    if seen != [pixels > 0; 2] {
        return wrong(
            at,
            "screen_box and luminance are null exactly when pixels is 0",
        );
    }
    if record.surround_luminance.is_some() != record.contrast.is_some()
        || (pixels == 0 && record.contrast.is_some())
    {
        return wrong(
            at,
            "surround_luminance and contrast go together, and need pixels",
        );
    }
    if let Some([x0, y0, x1, y1]) = record.screen_box {
        let area = u64::from(x1 - x0 + 1) * u64::from(y1 - y0 + 1);
        if pixels > area {
            return wrong(at, "more pixels than its screen box holds");
        }
    }
    Ok(record)
}

fn screen_box(value: &Json, at: At<'_>, (width, height): (u32, u32)) -> Result<[u32; 4], String> {
    let items = array(value, at)?;
    if items.len() != 4 {
        return wrong(at, "not four numbers");
    }
    let mut b = [0; 4];
    for (i, item) in items.iter().enumerate() {
        b[i] = u32_of(item, &format!("{at}/{i}"))?;
    }
    let [x0, y0, x1, y1] = b;
    if x0 > x1 || y0 > y1 || x1 >= width || y1 >= height {
        return wrong(
            at,
            &format!("{b:?} is not a box in a {width}x{height} image"),
        );
    }
    Ok(b)
}

fn probe(value: &Json, at: At<'_>, want: (u32, u32)) -> Result<Probe, String> {
    let keys = [
        "class", "depth", "distance", "edge", "id", "slope", "x", "y",
    ];
    let [class_v, depth, distance, edge, id, slope, x, y] = fields(value, at, keys)?;
    let p = Probe {
        x: u32_of(x, &format!("{at}/x"))?,
        y: u32_of(y, &format!("{at}/y"))?,
        key: ObjectKey {
            class: class(class_v, &format!("{at}/class"))?,
            id: u32_of(id, &format!("{at}/id"))?,
        },
        depth: f32_of(depth, &format!("{at}/depth"))?,
        distance: nullable(distance, |v| f64_of(v, &format!("{at}/distance")))?,
        edge: boolean(edge, &format!("{at}/edge"))?,
        slope: f32_of(slope, &format!("{at}/slope"))?,
    };
    if (p.x, p.y) != want {
        return wrong(at, &format!("a probe at ({}, {}), not {want:?}", p.x, p.y));
    }
    if p.depth < 0.0 || p.slope < 0.0 {
        return wrong(at, "a negative depth or slope");
    }
    if p.distance.is_some() != (p.depth > 0.0) || p.distance.is_some_and(|d| d <= 0.0) {
        return wrong(
            at,
            "distance is above 0 where depth is, and null where it is 0",
        );
    }
    Ok(p)
}

/// `N` counts, each at most `max`.
fn counts<const N: usize>(value: &Json, at: At<'_>, max: u64) -> Result<[u64; N], String> {
    let items = array(value, at)?;
    if items.len() != N {
        return wrong(at, &format!("{} numbers, not {N}", items.len()));
    }
    let mut out = [0; N];
    for (i, item) in items.iter().enumerate() {
        out[i] = uint(item, &format!("{at}/{i}"), max)?;
    }
    Ok(out)
}

/// A sum of counts each at most the image's pixels, which cannot wrap.
fn total_of(counts: impl Iterator<Item = u64>) -> u128 {
    counts.map(u128::from).sum()
}

fn classes(value: &Json, at: At<'_>, total: u64) -> Result<ClassBands, String> {
    let Json::Object(pairs) = value else {
        return wrong(at, "not an object");
    };
    let mut want: Vec<&str> = SurfaceClass::ALL.iter().map(|c| class_name(*c)).collect();
    want.sort_unstable();
    let names: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
    if names != want {
        return wrong(at, &format!("keys {names:?}, not {want:?}"));
    }
    let mut bands: ClassBands = [[0; BANDS]; SurfaceClass::ALL.len()];
    for (name, counted) in pairs {
        let class = class_named(name).expect("checked above");
        bands[usize::from(class.code())] =
            counts::<BANDS>(counted, &format!("{at}/{name}"), total)?;
    }
    Ok(bands)
}

fn unlisted(value: &Json, at: At<'_>, total: u64) -> Result<Unlisted, String> {
    let [class_v, id, pixels] = fields(value, at, ["class", "id", "pixels"])?;
    let u = Unlisted {
        key: ObjectKey {
            class: class(class_v, &format!("{at}/class"))?,
            id: u32_of(id, &format!("{at}/id"))?,
        },
        pixels: uint(pixels, &format!("{at}/pixels"), total)?,
    };
    if u.key.class == SurfaceClass::None || u.pixels == 0 {
        return wrong(at, "an unlisted key is never the sky, and has pixels");
    }
    Ok(u)
}

/// Reads one frame of a record of `view` at `size`.
fn frame(
    value: &Json,
    at: At<'_>,
    view: RecordView,
    size: (u32, u32),
) -> Result<FrameEntry, String> {
    let keys = [
        "camera",
        "classes",
        "histogram",
        "invalid_info",
        "luminance",
        "objects",
        "probes",
        "tick",
        "unlisted",
    ];
    let [cam, cls, hist, invalid, lum, objs, probes_v, tick, unl] = fields(value, at, keys)?;
    let total = u64::from(size.0) * u64::from(size.1);
    let read_objects = array(objs, &format!("{at}/objects"))?.iter().enumerate();
    let read_probes = array(probes_v, &format!("{at}/probes"))?;
    let read_unlisted = array(unl, &format!("{at}/unlisted"))?.iter().enumerate();
    let entry = FrameEntry {
        tick: uint(tick, &format!("{at}/tick"), u64::MAX)?,
        camera: nullable(cam, |v| camera(v, &format!("{at}/camera")))?,
        objects: read_objects
            .map(|(i, v)| object(v, &format!("{at}/objects/{i}"), size))
            .collect::<Result<_, _>>()?,
        unlisted: read_unlisted
            .map(|(i, v)| unlisted(v, &format!("{at}/unlisted/{i}"), total))
            .collect::<Result<_, _>>()?,
        invalid_info: uint(invalid, &format!("{at}/invalid_info"), total)?,
        classes: nullable(cls, |v| classes(v, &format!("{at}/classes"), total))?,
        probes: Vec::new(),
        luminance: within(lum, &format!("{at}/luminance"), 0.0, 1.0)?,
        histogram: counts::<HISTOGRAM_BINS>(hist, &format!("{at}/histogram"), total)?,
    };
    let points = probe_points(size.0, size.1);
    let probes = match view {
        RecordView::FirstPerson if read_probes.len() != points.len() => {
            return wrong(
                &format!("{at}/probes"),
                &format!("not {} probes", points.len()),
            );
        }
        RecordView::FirstPerson => read_probes
            .iter()
            .zip(points)
            .enumerate()
            .map(|(i, (v, want))| probe(v, &format!("{at}/probes/{i}"), want))
            .collect::<Result<_, _>>()?,
        RecordView::TopDown => Vec::new(),
    };
    let entry = FrameEntry { probes, ..entry };
    if total_of(entry.histogram.iter().copied()) != u128::from(total) {
        return wrong(
            &format!("{at}/histogram"),
            &format!("does not count {total} pixels"),
        );
    }
    let keys_in_order = |keys: &[ObjectKey]| keys.windows(2).all(|w| w[0] < w[1]);
    let objects: Vec<ObjectKey> = entry.objects.iter().map(|o| o.key).collect();
    let others: Vec<ObjectKey> = entry.unlisted.iter().map(|u| u.key).collect();
    let both: BTreeSet<ObjectKey> = objects.iter().chain(&others).copied().collect();
    if !keys_in_order(&objects) || !keys_in_order(&others) {
        return wrong(
            at,
            "objects and unlisted each in (class, id) order, each key once",
        );
    }
    if both.len() != objects.len() + others.len() {
        return wrong(at, "a key both asked for and unlisted");
    }
    match view {
        RecordView::TopDown => {
            let reduced = entry.camera.is_none()
                && entry.classes.is_none()
                && entry.objects.is_empty()
                && entry.unlisted.is_empty()
                && read_probes.is_empty()
                && entry.invalid_info == 0;
            if !reduced {
                return wrong(
                    at,
                    "a top-down frame has no camera, classes, objects or probes",
                );
            }
        }
        RecordView::FirstPerson => {
            let (Some(_), Some(bands)) = (entry.camera, entry.classes) else {
                return wrong(at, "a first-person frame has a camera and classes");
            };
            add_up(&entry, &bands, total).map_err(|what| format!("{at}: {what}"))?;
            let seen = |p: &Probe| p.key.class == SurfaceClass::None || both.contains(&p.key);
            if let Some(i) = entry.probes.iter().position(|p| !seen(p)) {
                return wrong(
                    &format!("{at}/probes/{i}"),
                    "a key neither asked for, unlisted nor the sky",
                );
            }
        }
    }
    Ok(entry)
}

/// Whether a first-person frame's counts add up: each class's thirds to
/// its objects' and unlisted keys' pixels, and every class's with
/// `invalid_info` to the image (sums in u128, so they cannot wrap).
fn add_up(entry: &FrameEntry, bands: &ClassBands, total: u64) -> Result<(), String> {
    for class in SurfaceClass::ALL {
        let counted = total_of(bands[usize::from(class.code())].iter().copied());
        let objects = entry.objects.iter().filter(|o| o.key.class == class);
        let unlisted = entry.unlisted.iter().filter(|u| u.key.class == class);
        let drawn = total_of(objects.map(|o| o.pixels).chain(unlisted.map(|u| u.pixels)));
        if class != SurfaceClass::None && drawn != counted {
            return Err(format!(
                "class {} has {counted} pixels in its thirds, but its objects and unlisted \
                 keys have {drawn}: they do not add up",
                class_name(class)
            ));
        }
    }
    let all = total_of(bands.iter().flatten().copied());
    if all + u128::from(entry.invalid_info) != u128::from(total) {
        return Err("classes and invalid_info do not add up to the image".into());
    }
    Ok(())
}

impl FrameRecord {
    /// Reads a `<out>.frame.json` strictly: version 1 only, every key and
    /// no other, in sorted order, every value of its type and range, the
    /// objects and unlisted keys in (class, id) order, the probes at their
    /// grid's pixels on keys the frame lists or the sky, each class's
    /// counts adding up to its objects' and all to the image, a top-down
    /// frame reduced, the ticks rising, and the text in the one layout
    /// [`FrameRecord::to_text`] writes. The error names the JSON pointer.
    pub fn parse(text: &str) -> Result<Self, String> {
        let json = parse_json(text)?;
        let [frames, height, version, view, width] =
            fields(&json, "", ["frames", "height", "version", "view", "width"])?;
        let version = uint(version, "/version", u64::MAX)?;
        if version != FRAME_RECORD_VERSION {
            return wrong(
                "/version",
                &format!("{version}, not {FRAME_RECORD_VERSION}"),
            );
        }
        let view = match string(view, "/view")? {
            "first-person" => RecordView::FirstPerson,
            "top-down" => RecordView::TopDown,
            other => return wrong("/view", &format!("{other:?} is no view")),
        };
        let size = (u32_of(width, "/width")?, u32_of(height, "/height")?);
        if size.0 == 0 || size.1 == 0 {
            return wrong("", "an image without pixels");
        }
        let frames = array(frames, "/frames")?
            .iter()
            .enumerate()
            .map(|(i, v)| frame(v, &format!("/frames/{i}"), view, size))
            .collect::<Result<Vec<_>, _>>()?;
        if frames.is_empty() {
            return wrong("/frames", "no frame");
        }
        if frames.windows(2).any(|w| w[0].tick >= w[1].tick) {
            return wrong("/frames", "ticks that do not rise");
        }
        let record = Self {
            view,
            width: size.0,
            height: size.1,
            frames,
        };
        // The one layout: the same spacing and number spellings, so two
        // records of one frame are the same bytes or not the same record.
        if record.to_text() != text {
            return wrong(
                "",
                "the text is not in the record's one layout (FrameRecord::to_text): other \
                 spacing, or a number spelled otherwise",
            );
        }
        Ok(record)
    }
}
