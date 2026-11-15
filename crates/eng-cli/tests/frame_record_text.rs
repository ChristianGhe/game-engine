//! The frame record's writer against its strict reader (rung 2, plan
//! addition A1): whatever targets a frame reads back (any size, any mix
//! of objects asked for and not, sky, bad `info`, edges, depth and colour)
//! and whatever names and boxes the extraction gives, the text the writer
//! makes is one the reader takes, and it reads back to the same record and
//! the same bytes. Top-down records and several frames too.

use std::collections::BTreeMap;

use eng_cli::frame_record::{
    AskedObject, FrameEntry, FrameRecord, ObjectKey, RecordView, WorldBox, first_person_entry,
    top_down_entry,
};
use eng_gpu::{Format, Image, Texels};
use eng_math::{BAngle, WorldPos};
use eng_render::{FirstPersonCamera, Info, SurfaceClass};
use proptest::prelude::*;

/// One pixel: which of the frame's keys it shows (`None` for bad `info`),
/// its depth where it is not sky, and its colour.
#[derive(Clone, Debug)]
struct Pixel {
    key: Option<usize>,
    depth: f32,
    rgb: [u8; 3],
}

/// An object the extraction asks for: the key's index, its name and box.
#[derive(Clone, Debug)]
struct Asked {
    key: usize,
    name: Option<String>,
    corners: [f64; 6],
}

#[derive(Clone, Debug)]
struct Case {
    width: u32,
    height: u32,
    /// (class code, id); class code 0 is the sky.
    keys: Vec<(u8, u32)>,
    pixels: Vec<Pixel>,
    asked: Vec<Asked>,
    eye: [f64; 3],
    angles: [u32; 3],
    near: f32,
    ticks: Vec<u64>,
}

fn pixel(keys: usize) -> impl Strategy<Value = Pixel> {
    let key = prop_oneof![1 => Just(None), 12 => (0..keys).prop_map(Some)];
    (key, 0.0f32..=1.0, any::<[u8; 3]>()).prop_map(|(key, depth, rgb)| Pixel { key, depth, rgb })
}

fn asked(keys: usize) -> impl Strategy<Value = Asked> {
    // Names with quotes, escapes, control characters and wide ones.
    let name = proptest::option::of("[a-z\"\\\\\\x00-\\x1f é😀]{0,8}");
    let corner = prop_oneof![-1.0e9f64..1.0e9, Just(0.0), Just(-0.0), Just(1.0e-7)];
    let corners = [(); 6].map(|()| corner.clone());
    (0..keys, name, corners).prop_map(|(key, name, corners)| Asked { key, name, corners })
}

fn case() -> impl Strategy<Value = Case> {
    let keys = prop::collection::vec((0u8..7, 0u32..4), 1..8);
    (1u32..24, 1u32..18, keys).prop_flat_map(|(width, height, keys)| {
        let n = keys.len();
        let pixels = prop::collection::vec(pixel(n), (width * height) as usize);
        let asked = prop::collection::vec(asked(n), 0..10);
        let eye = [(); 3].map(|()| -1.0e6f64..1.0e6);
        let angles = any::<[u32; 3]>();
        let near = 0.001f32..10.0;
        let ticks = prop::collection::btree_set(any::<u64>(), 1..4);
        (pixels, asked, eye, angles, near, ticks).prop_map(
            move |(pixels, asked, eye, angles, near, ticks)| Case {
                width,
                height,
                keys: keys.clone(),
                pixels,
                asked,
                eye,
                angles,
                near,
                ticks: ticks.into_iter().collect(),
            },
        )
    })
}

fn class(code: u8) -> SurfaceClass {
    SurfaceClass::from_code(code).expect("a class code")
}

impl Case {
    fn key(&self, index: usize) -> ObjectKey {
        let (code, id) = self.keys[index];
        ObjectKey {
            class: class(code),
            id,
        }
    }

    /// The `info`, depth and output a frame would read back.
    fn targets(&self) -> (Texels, Texels, Image) {
        let (mut info, mut depth, mut rgba8) = (Vec::new(), Vec::new(), Vec::new());
        for p in &self.pixels {
            let (words, d) = match p.key.map(|k| self.key(k)) {
                None => ([Info::RESERVED, 0], p.depth),
                Some(key) if key.class == SurfaceClass::None => (Info::NONE.pack(), 0.0),
                Some(key) => {
                    let info = Info {
                        region: 0,
                        class: key.class,
                        id: key.id,
                    };
                    (info.pack(), p.depth)
                }
            };
            info.extend(words.iter().flat_map(|w| w.to_le_bytes()));
            depth.extend(d.to_le_bytes());
            rgba8.extend([p.rgb[0], p.rgb[1], p.rgb[2], 255]);
        }
        let texels = |format, bytes| Texels {
            width: self.width,
            height: self.height,
            format,
            bytes,
        };
        let image = Image {
            width: self.width,
            height: self.height,
            rgba8,
        };
        (
            texels(Format::Rg32Uint, info),
            texels(Format::Depth32Float, depth),
            image,
        )
    }

    /// The objects asked for, in key order, never the sky.
    fn asked(&self) -> Vec<AskedObject> {
        let mut asked: BTreeMap<ObjectKey, AskedObject> = BTreeMap::new();
        for a in &self.asked {
            let key = self.key(a.key);
            if key.class == SurfaceClass::None {
                continue;
            }
            let [x0, y0, z0, x1, y1, z1] = a.corners;
            let lo = |a: f64, b: f64| if a <= b { a } else { b };
            let hi = |a: f64, b: f64| if a <= b { b } else { a };
            let world_box = WorldBox {
                min: [lo(x0, x1), lo(y0, y1), lo(z0, z1)],
                max: [hi(x0, x1), hi(y0, y1), hi(z0, z1)],
            };
            asked.entry(key).or_insert(AskedObject {
                key,
                name: a.name.clone(),
                world_box,
            });
        }
        asked.into_values().collect()
    }

    fn camera(&self) -> FirstPersonCamera {
        let [x, y, z] = self.eye;
        let [yaw, pitch, fov_y] = self.angles.map(BAngle);
        FirstPersonCamera {
            eye: WorldPos::new(x, y, z),
            yaw,
            pitch,
            fov_y,
            near: self.near,
        }
    }

    fn record(&self, view: RecordView) -> FrameRecord {
        let (info, depth, image) = self.targets();
        let (camera, asked) = (self.camera(), self.asked());
        let frames: Vec<FrameEntry> = (self.ticks.iter())
            .map(|&tick| match view {
                RecordView::FirstPerson => {
                    first_person_entry(tick, &camera, &asked, &info, &depth, &image)
                        .unwrap_or_else(|e| panic!("{e}"))
                }
                RecordView::TopDown => top_down_entry(tick, &image),
            })
            .collect();
        FrameRecord {
            view,
            width: self.width,
            height: self.height,
            frames,
        }
    }
}

proptest! {
    // No regressions file: a failure prints its case to put in a test.
    #![proptest_config(ProptestConfig {
        cases: ProptestConfig::default().cases.max(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn every_record_the_writer_makes_the_reader_takes_back(case in case()) {
        for view in [RecordView::FirstPerson, RecordView::TopDown] {
            let record = case.record(view);
            let text = record.to_text();
            let read = FrameRecord::parse(&text);
            prop_assert!(read.is_ok(), "{:?}\n{text}", read.err());
            let read = read.unwrap();
            prop_assert_eq!(&read, &record);
            prop_assert_eq!(read.to_text(), text);
        }
    }
}
