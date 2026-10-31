//! Properties of the top-down projection (rung 1b, task T8), for any scene.
//! No GPU is needed.
//!
//! The first two use positions on a 1/16 m grid, zooms and image sizes that
//! are powers of two, and offsets that are multiples of 2^20 m, so every
//! value involved is exact in binary and the results can be compared
//! exactly: against a model worked out in pixels, and against the same
//! scene moved far away.

use eng_math::WorldPos;
use eng_render::{LinearRgb, Quad, Shape, TopDownCamera, TopDownScene};
use proptest::prelude::*;

/// A multiple of 1/16 within `limit` sixteenths of 0.
fn grid(limit: i64) -> impl Strategy<Value = f64> {
    (-limit..=limit).prop_map(|n| n as f64 / 16.0)
}

fn colour() -> impl Strategy<Value = LinearRgb> {
    (-4.0f32..4.0, -4.0f32..4.0, -4.0f32..4.0).prop_map(|(r, g, b)| LinearRgb { r, g, b })
}

fn at(x: f64, y: f64) -> WorldPos {
    WorldPos::new(x, y, 0.0)
}

/// A size on the grid up to 4 m, nothing one time in three.
fn size() -> impl Strategy<Value = f64> {
    prop_oneof![
        1 => Just(0.0),
        2 => (1i64..=64).prop_map(|n| n as f64 / 16.0),
    ]
}

/// A shape within 16 m of (0, 0), on the grid. The image's edges lie a
/// power of two of meters from its centre, so shapes often end exactly on
/// one, or straddle it.
fn shape() -> impl Strategy<Value = Shape> {
    let corner = || (grid(1 << 8), grid(1 << 8));
    prop_oneof![
        (corner(), size(), size(), colour()).prop_map(|((x, y), w, h, colour)| {
            Shape::Rect {
                min: at(x, y),
                max: at(x + w, y + h),
                colour,
            }
        }),
        (corner(), size(), colour()).prop_map(|((x, y), r, colour)| Shape::Disc {
            centre: at(x, y),
            radius: r as f32,
            colour,
        }),
    ]
}

/// A scene on the grid: a camera within 1,024 m of (0, 0) at 1 to 64
/// pixels per meter, and up to 12 shapes within 16 m of it.
fn scene() -> impl Strategy<Value = TopDownScene> {
    (
        grid(1 << 14),
        grid(1 << 14),
        0u32..=6,
        prop::collection::vec(shape(), 0..12),
    )
        .prop_map(|(x, y, zoom, shapes)| {
            let scene = TopDownScene {
                camera: TopDownCamera {
                    centre: at(0.0, 0.0),
                    pixels_per_meter: f64::from(1u32 << zoom),
                },
                background: LinearRgb {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                },
                shapes,
            };
            moved(&scene, x, y)
        })
}

/// A power of two from 1 to 1,024 pixels.
fn side() -> impl Strategy<Value = u32> {
    (0u32..=10).prop_map(|k| 1 << k)
}

/// The model: the quad for `shape` worked out in pixels, x from the left
/// and y from the top, then turned into clip space; `None` if it misses
/// the image.
fn model(scene: &TopDownScene, shape: &Shape, width: u32, height: u32) -> Option<Quad> {
    let (w, h) = (f64::from(width), f64::from(height));
    let zoom = scene.camera.pixels_per_meter;
    let px = |x: f64| w / 2.0 + (x - scene.camera.centre.x) * zoom;
    let py = |y: f64| h / 2.0 - (y - scene.camera.centre.y) * zoom;
    let quad = |centre: (f64, f64), half: (f64, f64), colour, disc| Quad {
        centre: eng_math::Vec2::new(centre.0 as f32, centre.1 as f32),
        half_extents: eng_math::Vec2::new(half.0 as f32, half.1 as f32),
        colour,
        disc,
    };
    match *shape {
        Shape::Rect { min, max, colour } => {
            // The pixels it spans, cut to the image.
            let cut = |v: f64, side: f64| eng_math::clamp_f64(v, 0.0, side);
            let (left, right) = (cut(px(min.x), w), cut(px(max.x), w));
            let (top, bottom) = (cut(py(max.y), h), cut(py(min.y), h));
            if right <= left || bottom <= top {
                return None;
            }
            Some(quad(
                ((left + right) / w - 1.0, 1.0 - (top + bottom) / h),
                ((right - left) / w, (bottom - top) / h),
                colour,
                false,
            ))
        }
        Shape::Disc {
            centre,
            radius,
            colour,
        } => {
            let (x, y, r) = (px(centre.x), py(centre.y), f64::from(radius) * zoom);
            if r <= 0.0 || x + r <= 0.0 || x - r >= w || y + r <= 0.0 || y - r >= h {
                return None;
            }
            Some(quad(
                (x * 2.0 / w - 1.0, 1.0 - y * 2.0 / h),
                (r * 2.0 / w, r * 2.0 / h),
                colour,
                true,
            ))
        }
    }
}

/// `scene` moved by (`dx`, `dy`), camera and shapes alike.
fn moved(scene: &TopDownScene, dx: f64, dy: f64) -> TopDownScene {
    let shift = |p: WorldPos| WorldPos::new(p.x + dx, p.y + dy, p.z);
    let mut moved = scene.clone();
    moved.camera.centre = shift(moved.camera.centre);
    for shape in &mut moved.shapes {
        match shape {
            Shape::Rect { min, max, .. } => {
                *min = shift(*min);
                *max = shift(*max);
            }
            Shape::Disc { centre, .. } => *centre = shift(*centre),
        }
    }
    moved
}

proptest! {
    #[test]
    fn quads_match_a_model_worked_out_in_pixels(
        scene in scene(),
        width in side(),
        height in side(),
    ) {
        let expected: Vec<Quad> = scene
            .shapes
            .iter()
            .filter_map(|shape| model(&scene, shape, width, height))
            .collect();
        prop_assert_eq!(scene.quads(width, height), Ok(expected));
    }

    #[test]
    fn moving_everything_far_away_changes_nothing(
        scene in scene(),
        width in side(),
        height in side(),
        dx in -(1i64 << 16)..=1 << 16,
        dy in -(1i64 << 16)..=1 << 16,
    ) {
        // Up to 6.9e10 m away, where f32's step is 4,096 m or more.
        let far = moved(&scene, dx as f64 * 1048576.0, dy as f64 * 1048576.0);
        prop_assert_eq!(far.quads(width, height), scene.quads(width, height));
    }

    #[test]
    fn any_finite_scene_gives_finite_quads_in_order(
        cx in -1.0e12f64..1.0e12,
        cy in -1.0e12f64..1.0e12,
        zoom in 1.0e-3f64..1.0e3,
        width in 1u32..=8192,
        height in 1u32..=8192,
        shapes in prop::collection::vec(
            (
                (-1.0e12f64..1.0e12, -1.0e12f64..1.0e12),
                (0.0f64..1.0e6, 0.0f64..1.0e6),
                0.0f32..1.0e6,
                any::<bool>(),
                colour(),
            ),
            0..12,
        ),
    ) {
        let shapes: Vec<Shape> = shapes
            .into_iter()
            .map(|((x, y), (w, h), radius, disc, colour)| if disc {
                Shape::Disc { centre: at(x, y), radius, colour }
            } else {
                Shape::Rect { min: at(x, y), max: at(x + w, y + h), colour }
            })
            .collect();
        let scene = TopDownScene {
            camera: TopDownCamera { centre: at(cx, cy), pixels_per_meter: zoom },
            background: LinearRgb { r: 1.0, g: 1.0, b: 1.0 },
            shapes,
        };
        let quads = scene.quads(width, height);
        prop_assert!(quads.is_ok(), "{:?}", quads);
        let quads = quads.unwrap_or_default();
        for quad in &quads {
            let values = [quad.centre.x, quad.centre.y, quad.half_extents.x, quad.half_extents.y];
            prop_assert!(values.iter().all(|v| v.is_finite()), "{:?}", quad);
            prop_assert!(
                quad.half_extents.x >= 0.0 && quad.half_extents.y >= 0.0,
                "{:?}",
                quad
            );
        }
        // The quads keep the shapes' order: each matches a later shape.
        let mut rest = scene.shapes.iter();
        for quad in &quads {
            let found = rest.any(|shape| match *shape {
                Shape::Rect { colour, .. } => !quad.disc && colour == quad.colour,
                Shape::Disc { colour, .. } => quad.disc && colour == quad.colour,
            });
            prop_assert!(found, "{:?} is out of order", quad);
        }
    }
}
