//! The frame record (rung 2, plan addition A1): what `shot` and `video`
//! drew, as text an agent can check without looking at the picture. Each
//! writes `<out>.frame.json` beside its PNG or APNG (the whole file name
//! with `.frame.json` added, so `s.png` gives `s.png.frame.json`), with
//! one entry per frame: a shot's one, or each of a video's, in order. It
//! is written when the picture is, and a run stopped early writes neither.
//!
//! **The format, version 2** ([`FRAME_RECORD_VERSION`]). One JSON object,
//! keys sorted at every level, written by [`FrameRecord::to_text`] in one
//! fixed layout (an object, probe or unlisted key on a line of its own),
//! read back strictly by [`FrameRecord::parse`]:
//! - `version`; `view`, `"first-person"` or `"top-down"`; the image's
//!   `width` and `height`; `frames`, each entry:
//! - `tick`: the state drawn, at alpha 1.
//! - `camera` (first person): the eye in meters, absolute; `yaw_deg` (0 up
//!   to 360), `pitch_deg` (-180 up to 180) and `fov_y_deg`, each exactly
//!   the binary angle drawn; `near` in meters. `null` for a top-down view.
//! - `objects` (first person): every object the game's extraction asked
//!   to draw, in (class, id) order, drawn or not: each face of the world's
//!   chunks, by the class and id its vertices give `info`, and each
//!   sprite, by [`SurfaceClass::Sprite`] and its id. Each has `class` (as
//!   [`class_name`] writes it), `id`, `name` (the game's, from
//!   [`crate::FirstPersonView::names`], or `null`), `world_box` (`min` and
//!   `max`, absolute meters: the face's vertices, or the box a sprite's
//!   billboard sweeps as it turns, its feet ± half its width across X and
//!   Y and its height up Z), then from `info` and the output:
//!   - `pixels`: how many pixels `info` gives its (class, id);
//!   - `screen_box`: `[x0, y0, x1, y1]`, the first and last column and row
//!     holding them, inclusive, (0, 0) the top left;
//!   - `colour` (version 2): their mean linear colour, `[R, G, B]`: the
//!     output's 8-bit sRGB channels decoded by eng-render's
//!     `srgb_to_linear`, each 0 to 1;
//!   - `luminance`: their mean luminance, a pixel's being linear: Rec.
//!     709's `0.2126 R + 0.7152 G + 0.0722 B` of those channels
//!     ([`luminance`]), 0 to 1;
//!   - `surround_colour` (version 2) and `surround_luminance`: the mean
//!     colour and luminance of its surroundings, the pixels within
//!     [`SURROUND`] pixels of its screen box (the box grown by that on
//!     every side, clipped to the image) that are not its own;
//!   - `contrast`: Michelson's, `|L - S| / (L + S)` of the two mean
//!     luminances, 0 to 1, and 0 when both are 0.
//!
//!   Without pixels the six are `null`; without surroundings (its box
//!   grown is all its own) the last three are. The means are rounded to
//!   six decimals ([`round6`]), the contrast taken before rounding.
//! - `unlisted` (first person): each (class, id) that `info` shows but no
//!   object asked for, with its `pixels`: an extraction or shader bug, so
//!   it is normally empty. The sky, class `none`, is never unlisted.
//! - `invalid_info`: pixels whose `info` does not unpack
//!   ([`eng_render::Info::unpack`]).
//! - `classes` (first person): for each [`SurfaceClass`] by name, its
//!   pixels in the image's top, middle and bottom thirds (row `y` is in
//!   third `3y / height`). `null` for a top-down view.
//! - `void` (first person; version 2): the pixels where nothing was drawn
//!   (`info` is class `none`) whose view ray points below the horizon:
//!   through the pixel's centre, as eng-render's `ViewProj::ray` builds it
//!   for the image's aspect ratio, with its Z below 0. The sky pass paints
//!   every pixel no surface covers, so the clear colour never reaches the
//!   output: a hole in the world shows the sky instead, and below the
//!   horizon, where the ground stands between the eye and the sky, that
//!   is a hole (plan addition A2's rule 2). `null` for a top-down view.
//! - `probes` (first person): depth at a fixed grid of
//!   [`PROBE_COLUMNS`] by [`PROBE_ROWS`] pixels, row by row from the top
//!   left; probe (i, j) is at `x = (2i + 1) width / 16`, `y = (2j + 1)
//!   height / 12`, rounded down: the middle of each of 8x6 equal cells.
//!   Each has its `x` and `y`, the `class` and `id` there, `depth` as the
//!   target holds it (`near / distance ahead`, 0 where the sky shows),
//!   `distance` ahead in meters (`near / depth`, six decimals, `null` for
//!   the sky), `edge` (one of its four neighbours in the image shows
//!   another (class, id)) and `slope` (the largest depth step to a
//!   neighbour of its own (class, id), 0 without one), which
//!   [`Tolerances`] needs. Empty for a top-down view.
//! - `luminance`: the whole image's mean luminance, six decimals.
//! - `surface_luminance` (first person; version 2): the mean luminance of
//!   the pixels showing a surface (`info` of any class but `none`), six
//!   decimals: the scene's exposure without its sky, which A2's rule 1
//!   holds an object's light to. `null` for a top-down view or a frame of
//!   sky alone.
//! - `histogram`: the image's pixels in [`HISTOGRAM_BINS`] bins of
//!   luminance, bin `k` holding luminance from `srgb_to_linear(k / 16)` up
//!   to `srgb_to_linear((k + 1) / 16)`: 16 equal steps of the luminance's
//!   sRGB encoding, so the dark end is not one bin.
//!
//! **A top-down view** has no `info` or depth target, so its record is
//! reduced: `camera`, `classes`, `void` and `surface_luminance` `null`,
//! `objects`, `unlisted` and `probes` empty, and the image's `luminance`
//! and `histogram`.
//!
//! **Version 1** (rung 2's TA1) had neither `colour`, `surround_colour`,
//! `void` nor `surface_luminance`; TA2 added them for A2's rules. No version 1 record was ever
//! committed as a reference (the owner commits the first at G6), so the
//! reader takes version 2 alone, and tells the reader of an older file to
//! render it again.
//!
//! **Determinism.** Everything comes from the frame's own targets and the
//! extraction, summed in a fixed order in f64 through eng-math and
//! eng-render's `srgb_to_linear`, so the same targets give the same bytes
//! on every machine: exact on the pinned lavapipe. Another adapter
//! rasterizes edges and rounds colours a little otherwise, so records from
//! two adapters are compared within [`Tolerances`]
//! ([`FrameRecord::compare`]).

mod compare;
mod entry;
mod json;
mod read;
mod write;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eng_math::{max_f64, min_f64};
use eng_render::{FirstPersonCamera, Sprite, SurfaceClass};

pub use compare::{Comparison, Tolerances, Worst, contrast_tolerance, depth_tolerance};
pub use entry::{first_person_entry, luminance, probe_points, top_down_entry};

use crate::shot_view::bangle_degrees;
use crate::view::FirstPersonWorld;

/// The record's format, its `version`.
pub const FRAME_RECORD_VERSION: u64 = 2;

/// The depth probes' grid: columns across, rows down.
pub const PROBE_COLUMNS: u32 = 8;
pub const PROBE_ROWS: u32 = 6;

/// The luminance histogram's bins.
pub const HISTOGRAM_BINS: usize = 16;

/// How far around an object's screen box its surroundings reach (pixels).
pub const SURROUND: u32 = 4;

/// The image's horizontal thirds, top to bottom, that `classes` counts.
pub const BANDS: usize = 3;

/// Rec. 709's weights of linear red, green and blue in luminance.
pub const REC709: [f64; 3] = [0.2126, 0.7152, 0.0722];

/// `<out>.frame.json`: the file name with `.frame.json` added.
pub fn record_path(out: &Path) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".frame.json");
    PathBuf::from(name)
}

/// `x` rounded to six decimals, half away from zero: how the record writes
/// means and distances.
pub fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// An object as `info` names it: its surface class and id. Sorted by the
/// class's code, then the id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey {
    pub class: SurfaceClass,
    pub id: u32,
}

/// The class as the record writes it.
pub const fn class_name(class: SurfaceClass) -> &'static str {
    match class {
        SurfaceClass::None => "none",
        SurfaceClass::Floor => "floor",
        SurfaceClass::Ceiling => "ceiling",
        SurfaceClass::Wall => "wall",
        SurfaceClass::WallFacingX => "wall-facing-x",
        SurfaceClass::WallFacingY => "wall-facing-y",
        SurfaceClass::Sprite => "sprite",
    }
}

/// The class [`class_name`] writes as `name`.
pub fn class_named(name: &str) -> Option<SurfaceClass> {
    SurfaceClass::ALL
        .into_iter()
        .find(|class| class_name(*class) == name)
}

/// An axis-aligned box in absolute world coordinates (meters).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl WorldBox {
    fn point(p: [f64; 3]) -> Self {
        Self { min: p, max: p }
    }

    fn take(&mut self, other: &Self) {
        for axis in 0..3 {
            self.min[axis] = min_f64(self.min[axis], other.min[axis]);
            self.max[axis] = max_f64(self.max[axis], other.max[axis]);
        }
    }

    /// The box a sprite's billboard sweeps as it turns about Z.
    pub fn of_sprite(sprite: &Sprite) -> Self {
        let (feet, half) = (sprite.feet, f64::from(sprite.width) / 2.0);
        Self {
            min: [feet.x - half, feet.y - half, feet.z],
            max: [
                feet.x + half,
                feet.y + half,
                feet.z + f64::from(sprite.height),
            ],
        }
    }
}

/// An object the extraction asked to draw: its key, the game's name for
/// it, and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct AskedObject {
    pub key: ObjectKey,
    pub name: Option<String>,
    pub world_box: WorldBox,
}

/// The faces of `world`'s chunks, one object per (class, id) their
/// triangles give `info`, boxed around those triangles' corners (each the
/// chunk's origin plus the vertex, in f64), in key order. A key on several
/// triangles or chunks is one object.
pub fn world_objects(world: &FirstPersonWorld) -> Vec<AskedObject> {
    let mut boxes: BTreeMap<ObjectKey, WorldBox> = BTreeMap::new();
    for chunk in &world.chunks {
        let o = chunk.origin;
        for &index in &chunk.indices {
            let Some(vertex) = chunk.vertices.get(index as usize) else {
                continue;
            };
            let [x, y, z] = vertex.position.map(f64::from);
            let corner = WorldBox::point([o.x + x, o.y + y, o.z + z]);
            let key = ObjectKey {
                class: vertex.info.class,
                id: vertex.info.id,
            };
            boxes
                .entry(key)
                .and_modify(|b| b.take(&corner))
                .or_insert(corner);
        }
    }
    boxes
        .into_iter()
        .map(|(key, world_box)| AskedObject {
            key,
            name: None,
            world_box,
        })
        .collect()
}

/// Every object a frame asks for: `world`'s ([`world_objects`]) and one per
/// sprite id ([`WorldBox::of_sprite`], several sprites of one id boxed
/// together), named from `names` (the first name a key has; names for keys
/// nobody asked for are dropped), in key order.
pub fn asked_objects(
    world: &[AskedObject],
    sprites: &[Sprite],
    names: &[(ObjectKey, String)],
) -> Vec<AskedObject> {
    let mut asked: BTreeMap<ObjectKey, AskedObject> = BTreeMap::new();
    for object in world {
        asked
            .entry(object.key)
            .and_modify(|o| o.world_box.take(&object.world_box))
            .or_insert_with(|| object.clone());
    }
    for sprite in sprites {
        let key = ObjectKey {
            class: SurfaceClass::Sprite,
            id: sprite.id,
        };
        let world_box = WorldBox::of_sprite(sprite);
        asked
            .entry(key)
            .and_modify(|o| o.world_box.take(&world_box))
            .or_insert(AskedObject {
                key,
                name: None,
                world_box,
            });
    }
    for (key, name) in names.iter().rev() {
        if let Some(object) = asked.get_mut(key) {
            object.name = Some(name.clone());
        }
    }
    asked.into_values().collect()
}

/// A frame's camera as the record writes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRecord {
    pub eye: [f64; 3],
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub fov_y_deg: f64,
    pub near: f32,
}

impl CameraRecord {
    pub fn of(camera: &FirstPersonCamera) -> Self {
        let eye = camera.eye;
        Self {
            eye: [eye.x, eye.y, eye.z],
            yaw_deg: bangle_degrees(camera.yaw, false),
            pitch_deg: bangle_degrees(camera.pitch, true),
            fov_y_deg: bangle_degrees(camera.fov_y, false),
            near: camera.near,
        }
    }
}

/// One object's line of a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectRecord {
    pub key: ObjectKey,
    pub name: Option<String>,
    pub world_box: WorldBox,
    pub pixels: u64,
    /// `[x0, y0, x1, y1]`, inclusive.
    pub screen_box: Option<[u32; 4]>,
    /// Mean linear red, green and blue.
    pub colour: Option<[f64; 3]>,
    pub luminance: Option<f64>,
    pub surround_colour: Option<[f64; 3]>,
    pub surround_luminance: Option<f64>,
    pub contrast: Option<f64>,
}

/// A (class, id) that `info` shows and no object asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unlisted {
    pub key: ObjectKey,
    pub pixels: u64,
}

/// One depth probe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    pub x: u32,
    pub y: u32,
    /// What `info` shows there; class `none` for the sky (or bad `info`).
    pub key: ObjectKey,
    pub depth: f32,
    pub distance: Option<f64>,
    pub edge: bool,
    pub slope: f32,
}

/// The pixels of each class, by [`SurfaceClass::code`], in each third.
pub type ClassBands = [[u64; BANDS]; SurfaceClass::ALL.len()];

/// One frame of a record.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameEntry {
    pub tick: u64,
    pub camera: Option<CameraRecord>,
    pub objects: Vec<ObjectRecord>,
    pub unlisted: Vec<Unlisted>,
    pub invalid_info: u64,
    pub classes: Option<ClassBands>,
    /// Pixels showing nothing below the horizon; `None` for a top-down
    /// view.
    pub void: Option<u64>,
    /// The mean luminance of the pixels showing a surface (any class but
    /// none); `None` for a top-down view or a frame of sky alone.
    pub surface_luminance: Option<f64>,
    pub probes: Vec<Probe>,
    pub luminance: f64,
    pub histogram: [u64; HISTOGRAM_BINS],
}

/// Which view a record's frames are of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordView {
    FirstPerson,
    TopDown,
}

impl RecordView {
    pub const fn name(self) -> &'static str {
        match self {
            Self::FirstPerson => "first-person",
            Self::TopDown => "top-down",
        }
    }
}

/// A `<out>.frame.json`: its view, the image's size and every frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameRecord {
    pub view: RecordView,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<FrameEntry>,
}

impl FrameRecord {
    /// An empty record of `view` for `width` x `height` frames.
    pub fn new(view: RecordView, width: u32, height: u32) -> Self {
        Self {
            view,
            width,
            height,
            frames: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
