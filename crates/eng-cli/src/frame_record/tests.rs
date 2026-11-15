//! The frame record without a GPU: a synthetic frame's targets, the text's
//! layout and strict reader (`tests/text.rs`), and the comparison's
//! tolerances. The GPU's side, every shot and the planted bugs, is the
//! testbed's `tests/frame_record.rs` and `tests/frame_bugs.rs`.

use std::path::Path;

use eng_gpu::{Format, Image, Texels};
use eng_math::{BAngle, WorldPos};
use eng_render::{
    Chunk, FirstPersonCamera, Info, RotationSheet, Sprite, SurfaceClass, srgb_to_linear,
};

use super::{
    AskedObject, CameraRecord, FrameRecord, ObjectKey, PROBE_COLUMNS, PROBE_ROWS, RecordView,
    WorldBox, asked_objects, class_name, class_named, first_person_entry, luminance, probe_points,
    record_path, round6, world_objects,
};
use crate::view::FirstPersonWorld;

mod text;

const W: u32 = 16;
const H: u32 = 12;

fn key(class: SurfaceClass, id: u32) -> ObjectKey {
    ObjectKey { class, id }
}

const FLOOR: ObjectKey = ObjectKey {
    class: SurfaceClass::Floor,
    id: 7,
};
const THING: ObjectKey = ObjectKey {
    class: SurfaceClass::Sprite,
    id: 3,
};
/// Asked for and never drawn.
const HIDDEN: ObjectKey = ObjectKey {
    class: SurfaceClass::Sprite,
    id: 4,
};
/// Drawn and never asked for.
const STRAY: ObjectKey = ObjectKey {
    class: SurfaceClass::Wall,
    id: 99,
};

const SKY_RGB: [u8; 3] = [120, 140, 200];
const FLOOR_RGB: [u8; 3] = [60, 60, 60];
const THING_RGB: [u8; 3] = [250, 30, 30];

/// What the synthetic frame shows at a pixel: `None` for bad `info`.
fn shows(x: u32, y: u32) -> Option<ObjectKey> {
    match (x, y) {
        (0, 0) => None,
        (15, 0) => Some(STRAY),
        (5..9, 2..5) => Some(THING),
        (_, 8..) => Some(FLOOR),
        _ => Some(key(SurfaceClass::None, 0)),
    }
}

fn rgb_at(x: u32, y: u32) -> [u8; 3] {
    match shows(x, y) {
        Some(FLOOR) => FLOOR_RGB,
        Some(THING) => THING_RGB,
        _ => SKY_RGB,
    }
}

/// The floor's depth falls by 0.01 a row toward the top; the thing is flat
/// at 0.5; the sky is 0.
fn depth_at(x: u32, y: u32) -> f32 {
    match shows(x, y) {
        Some(FLOOR) => 0.2 + 0.01 * y as f32,
        Some(THING) => 0.5,
        Some(STRAY) => 0.25,
        _ => 0.0,
    }
}

struct Targets {
    info: Texels,
    depth: Texels,
    image: Image,
}

fn targets() -> Targets {
    let (mut info, mut depth, mut rgba8) = (Vec::new(), Vec::new(), Vec::new());
    for y in 0..H {
        for x in 0..W {
            let words = match shows(x, y) {
                None => [Info::RESERVED, 0],
                Some(k) => Info {
                    region: 1,
                    class: k.class,
                    id: k.id,
                }
                .pack(),
            };
            info.extend(words.iter().flat_map(|w| w.to_le_bytes()));
            depth.extend(depth_at(x, y).to_le_bytes());
            let rgb = rgb_at(x, y);
            rgba8.extend([rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    Targets {
        info: Texels {
            width: W,
            height: H,
            format: Format::Rg32Uint,
            bytes: info,
        },
        depth: Texels {
            width: W,
            height: H,
            format: Format::Depth32Float,
            bytes: depth,
        },
        image: Image {
            width: W,
            height: H,
            rgba8,
        },
    }
}

fn camera() -> FirstPersonCamera {
    FirstPersonCamera {
        eye: WorldPos::new(1.0, 2.0, 1.6),
        yaw: BAngle(1 << 30),
        pitch: BAngle((-(1_i32 << 28)) as u32),
        fov_y: BAngle(879_750_026),
        near: 0.1,
    }
}

fn asked() -> Vec<AskedObject> {
    let unit = |x: f64| WorldBox {
        min: [x, 0.0, 0.0],
        max: [x + 1.0, 1.0, 1.0],
    };
    [
        (FLOOR, None, 0.0),
        (THING, Some("lamp"), 2.0),
        (HIDDEN, None, 4.0),
    ]
    .into_iter()
    .map(|(key, name, x)| AskedObject {
        key,
        name: name.map(str::to_owned),
        world_box: unit(x),
    })
    .collect()
}

/// The mean luminance over `grown` (x0, y0, x1, y1) of the pixels not
/// `own`, summed row by row as the record sums it.
fn surround(own: ObjectKey, [x0, y0, x1, y1]: [u32; 4]) -> f64 {
    let (mut sum, mut count) = (0.0, 0.0);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if shows(x, y) != Some(own) {
                sum += luminance(rgb_at(x, y));
                count += 1.0;
            }
        }
    }
    sum / count
}

fn record() -> FrameRecord {
    let t = targets();
    let entry = first_person_entry(5, &camera(), &asked(), &t.info, &t.depth, &t.image).unwrap();
    FrameRecord {
        view: RecordView::FirstPerson,
        width: W,
        height: H,
        frames: vec![entry],
    }
}

#[test]
fn the_record_goes_beside_the_picture() {
    assert_eq!(
        record_path(Path::new("out/s.png")),
        Path::new("out/s.png.frame.json")
    );
    assert_eq!(
        record_path(Path::new("v.apng")),
        Path::new("v.apng.frame.json")
    );
    for class in SurfaceClass::ALL {
        assert_eq!(class_named(class_name(class)), Some(class));
    }
    assert_eq!(class_named("sky"), None);
}

#[test]
fn luminance_is_rec_709_of_linear_light() {
    assert_eq!(luminance([0, 0, 0]), 0.0);
    assert!((luminance([255, 255, 255]) - 1.0).abs() < 1e-12);
    let lin = |v: u8| f64::from(srgb_to_linear(f32::from(v) / 255.0));
    let want = 0.2126 * lin(250) + 0.7152 * lin(30) + 0.0722 * lin(30);
    assert_eq!(luminance(THING_RGB), want);
    assert_eq!(round6(0.123_456_5), 0.123_457);
    assert_eq!(round6(2.0), 2.0);
}

#[test]
fn probes_sit_in_the_middle_of_each_cell() {
    let points = probe_points(320, 240);
    assert_eq!(points.len(), (PROBE_COLUMNS * PROBE_ROWS) as usize);
    assert_eq!(points[0], (20, 20));
    assert_eq!(points[7], (300, 20));
    assert_eq!(points[8], (20, 60));
    assert_eq!(points[47], (300, 220));
    let small = probe_points(W, H);
    assert_eq!(small[0], (1, 1));
    assert_eq!(small[47], (15, 11));
}

#[test]
fn a_frame_counts_each_objects_pixels_box_and_light() {
    let record = record();
    let frame = &record.frames[0];
    assert_eq!(frame.tick, 5);
    assert_eq!(frame.camera, Some(CameraRecord::of(&camera())));
    let camera = frame.camera.unwrap();
    assert_eq!((camera.yaw_deg, camera.pitch_deg), (90.0, -22.5));
    let [floor, thing, hidden] = &frame.objects[..] else {
        panic!("{:?}", frame.objects);
    };
    assert_eq!((floor.key, floor.pixels), (FLOOR, 16 * 4));
    assert_eq!(floor.screen_box, Some([0, 8, 15, 11]));
    assert_eq!(floor.luminance, Some(round6(luminance(FLOOR_RGB))));
    // Its surroundings are the four rows above it: sky, and the thing's
    // bottom row.
    let around = surround(FLOOR, [0, 4, 15, 11]);
    assert_eq!(floor.surround_luminance, Some(round6(around)));
    assert_eq!((thing.key, thing.name.as_deref()), (THING, Some("lamp")));
    assert_eq!((thing.pixels, thing.screen_box), (12, Some([5, 2, 8, 4])));
    // Around it, 4 px out and clipped: columns 1-12, rows 0-8, less its own
    // 12: 84 pixels of sky and 12 of floor.
    let around = surround(THING, [1, 0, 12, 8]);
    let (sky, ground) = (luminance(SKY_RGB), luminance(FLOOR_RGB));
    assert!((around - (84.0 * sky + 12.0 * ground) / 96.0).abs() < 1e-12);
    assert_eq!(thing.surround_luminance, Some(round6(around)));
    let l = luminance(THING_RGB);
    assert_eq!(
        thing.contrast,
        Some(round6((l - around).abs() / (l + around)))
    );
    assert_eq!(
        (hidden.key, hidden.pixels, hidden.screen_box),
        (HIDDEN, 0, None)
    );
    assert_eq!(
        (hidden.luminance, hidden.surround_luminance, hidden.contrast),
        (None, None, None)
    );
    assert_eq!(frame.unlisted.len(), 1);
    assert_eq!(
        (frame.unlisted[0].key, frame.unlisted[0].pixels),
        (STRAY, 1)
    );
    assert_eq!(frame.invalid_info, 1);
    let bands = frame.classes.unwrap();
    assert_eq!(bands[usize::from(SurfaceClass::Floor.code())], [0, 0, 64]);
    assert_eq!(bands[usize::from(SurfaceClass::Sprite.code())], [8, 4, 0]);
    assert_eq!(
        bands[usize::from(SurfaceClass::None.code())],
        [64 - 8 - 2, 64 - 4, 0]
    );
    assert_eq!(frame.histogram.iter().sum::<u64>(), u64::from(W * H));
    // Probe (3, 1) is at (7, 3): the thing, flat, on no edge; (2, 1) at
    // (5, 3) its left edge; (0, 4) at (1, 9) is floor, sloped by 0.01 a
    // row; (2, 0) at (5, 1) is sky above the thing's top edge.
    let probe = |i: u32, j: u32| frame.probes[(j * PROBE_COLUMNS + i) as usize];
    let thing = probe(3, 1);
    assert_eq!(
        ((thing.x, thing.y), thing.key, thing.depth),
        ((7, 3), THING, 0.5)
    );
    assert!(probe(2, 1).edge && probe(2, 1).key == THING);
    assert_eq!(
        (thing.distance, thing.edge, thing.slope),
        (Some(0.2), false, 0.0)
    );
    let floor = probe(0, 4);
    assert_eq!(((floor.x, floor.y), floor.key), ((1, 9), FLOOR));
    assert!((floor.slope - 0.01).abs() < 1e-6, "{floor:?}");
    let sky = probe(2, 0);
    assert_eq!(
        (sky.key.class, sky.depth, sky.distance),
        (SurfaceClass::None, 0.0, None)
    );
    assert!(sky.edge, "the thing is below it");
}

#[test]
fn the_world_asks_for_each_face_and_a_frame_for_each_sprite() {
    let mut chunk = Chunk::new(WorldPos::new(10.0, 20.0, 0.0));
    let floor = Info {
        region: 0,
        class: SurfaceClass::Floor,
        id: 1,
    };
    let wall = Info {
        class: SurfaceClass::WallFacingX,
        id: 2,
        ..floor
    };
    let at = |x: f64, y: f64, z: f64| WorldPos::new(10.0 + x, 20.0 + y, z);
    let uv = [[0.0; 2]; 4];
    let quads = [
        (
            [
                at(0., 0., 0.),
                at(2., 0., 0.),
                at(2., 1., 0.),
                at(0., 1., 0.),
            ],
            floor,
        ),
        (
            [
                at(3., 0., 0.),
                at(3., 1., 0.),
                at(3., 1., 2.),
                at(3., 0., 2.),
            ],
            wall,
        ),
        (
            [
                at(0., 1., 0.),
                at(1., 1., 0.),
                at(1., 4., 0.),
                at(0., 4., 0.),
            ],
            floor,
        ),
    ];
    for (corners, info) in quads {
        chunk.quad(corners, uv, 0, info);
    }
    let world = FirstPersonWorld {
        label: "test",
        chunks: vec![chunk],
        materials: Vec::new(),
        textures: Vec::new(),
        palette: None,
    };
    let objects = world_objects(&world);
    let boxes: Vec<(ObjectKey, WorldBox)> = objects.iter().map(|o| (o.key, o.world_box)).collect();
    let floor_box = WorldBox {
        min: [10.0, 20.0, 0.0],
        max: [12.0, 24.0, 0.0],
    };
    let wall_box = WorldBox {
        min: [13.0, 20.0, 0.0],
        max: [13.0, 21.0, 2.0],
    };
    assert_eq!(
        boxes,
        [
            (key(SurfaceClass::Floor, 1), floor_box),
            (key(SurfaceClass::WallFacingX, 2), wall_box),
        ]
    );
    let sprite = Sprite {
        id: 1,
        region: 0,
        feet: WorldPos::new(5.0, 6.0, 0.5),
        facing: BAngle::ZERO,
        width: 1.0,
        height: 1.5,
        sheet: RotationSheet::Eight,
        materials: [0; 8],
    };
    let names = [
        (key(SurfaceClass::Sprite, 1), "barrel".to_owned()),
        (key(SurfaceClass::Sprite, 1), "second".to_owned()),
        (key(SurfaceClass::Sprite, 9), "nobody".to_owned()),
    ];
    let all = asked_objects(&objects, &[sprite], &names);
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].key, key(SurfaceClass::Sprite, 1));
    assert_eq!(all[2].name.as_deref(), Some("barrel"), "the first name");
    assert_eq!(
        all[2].world_box,
        WorldBox {
            min: [4.5, 5.5, 0.5],
            max: [5.5, 6.5, 2.0]
        }
    );
    assert_eq!(all[0].name, None);
}

#[test]
fn targets_of_another_size_or_format_are_refused() {
    let mut t = targets();
    t.depth.format = Format::Rgba8UnormSrgb;
    let err = first_person_entry(0, &camera(), &asked(), &t.info, &t.depth, &t.image).unwrap_err();
    assert!(err.contains("Depth32Float"), "{err}");
    let mut t = targets();
    t.image.width = 8;
    let err = first_person_entry(0, &camera(), &asked(), &t.info, &t.depth, &t.image).unwrap_err();
    assert!(err.contains("one size"), "{err}");
    let mut unsorted = asked();
    unsorted.swap(0, 1);
    let t = targets();
    let err = first_person_entry(0, &camera(), &unsorted, &t.info, &t.depth, &t.image).unwrap_err();
    assert!(err.contains("key order"), "{err}");
}
