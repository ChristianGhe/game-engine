//! Rendering on a real adapter (rung 1b, task T8: "a known scene reads back
//! as expected; counters exact"). The renderer draws a top-down scene
//! through the fixed graph, tone maps it and, with a surface, presents it.
//! Without an adapter each test skips, unless `ENG_REQUIRE_GPU=1` (CI) makes
//! it fail.

use eng_gpu::{Gpu, GpuError, GpuOptions, Image, Targets};
use eng_math::WorldPos;
use eng_render::{
    FrameCounters, LinearRgb, RenderError, Renderer, Shape, TopDownCamera, TopDownScene,
};

fn gpu() -> Option<Gpu> {
    match Gpu::new(&GpuOptions::from_env()) {
        Ok(gpu) => Some(gpu),
        Err(e @ GpuError::NoAdapter { .. }) if !eng_gpu::gpu_required() => {
            eprintln!("skipped: {e}");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

/// sRGB 128 decoded, half a step from either neighbour: it reads back as
/// 128 wherever the GPU rounds.
const MID: f32 = 0.215_861;

/// Far from the origin: f32 cannot hold these positions to the meter (its
/// step there is 64 m or more), so only a camera subtracted in f64 draws
/// the scene in place.
const X0: f64 = 1.0e9;
const Y0: f64 = -2.0e9;

fn at(x: f64, y: f64) -> WorldPos {
    WorldPos::new(X0 + x, Y0 + y, 0.0)
}

/// The scene: a grey room, a red dot in it, and a green rectangle over the
/// room's top right corner, on a dark blue background, seen from (4, 3) at
/// 40 pixels per meter. On the 320x240 shot size (plan decision 3) the view
/// spans 8 by 6 meters, the room's edges fall on pixel boundaries, and no
/// pixel centre lies on the dot's edge.
fn known_scene() -> TopDownScene {
    TopDownScene {
        camera: TopDownCamera {
            centre: at(4.0, 3.0),
            pixels_per_meter: 40.0,
        },
        background: LinearRgb {
            r: 0.0,
            g: 0.0,
            b: MID,
        },
        shapes: vec![
            Shape::Rect {
                min: at(1.0, 1.0),
                max: at(7.0, 5.0),
                colour: LinearRgb {
                    r: MID,
                    g: MID,
                    b: MID,
                },
            },
            // Above 1, so the tone map clamps it.
            Shape::Disc {
                centre: at(2.5, 2.0),
                radius: 0.5,
                colour: LinearRgb {
                    r: 4.0,
                    g: 0.0,
                    b: 0.0,
                },
            },
            // Below 0 in red, clamped too.
            Shape::Rect {
                min: at(5.0, 3.5),
                max: at(7.5, 5.5),
                colour: LinearRgb {
                    r: -1.0,
                    g: 1.0,
                    b: 0.0,
                },
            },
            // Off the view: neither drawn nor uploaded.
            Shape::Rect {
                min: at(20.0, 20.0),
                max: at(21.0, 21.0),
                colour: LinearRgb {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                },
            },
        ],
    }
}

const BACKGROUND: [u8; 4] = [0, 0, 128, 255];
const GREY: [u8; 4] = [128, 128, 128, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];

/// The known scene's pixel at column `x` and row `y`, worked out by hand:
/// a meter is 40 pixels, the camera's (4, 3) is pixel (160, 120), and +Y
/// is up the image.
fn expected(x: u32, y: u32) -> [u8; 4] {
    let inside =
        |xs: (u32, u32), ys: (u32, u32)| (xs.0..xs.1).contains(&x) && (ys.0..ys.1).contains(&y);
    // The dot: centre (2.5, 2) is pixel (100, 160), its radius 20 pixels.
    let (dx, dy) = (f64::from(x) + 0.5 - 100.0, f64::from(y) + 0.5 - 160.0);
    if inside((200, 300), (20, 100)) {
        GREEN
    } else if dx * dx + dy * dy <= 400.0 {
        RED
    } else if inside((40, 280), (40, 200)) {
        GREY
    } else {
        BACKGROUND
    }
}

fn assert_image(image: &Image, expected: impl Fn(u32, u32) -> [u8; 4]) {
    assert_eq!((image.width, image.height), (320, 240));
    let mut wrong = Vec::new();
    for y in 0..image.height {
        for x in 0..image.width {
            let (found, want) = (image.pixel(x, y), expected(x, y));
            if found != want {
                wrong.push(format!("({x}, {y}): {found:?}, not {want:?}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} pixels differ, first {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(8)]
    );
}

fn renderer(gpu: &Gpu) -> Renderer {
    Renderer::new(gpu).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_known_scene_reads_back_as_expected() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::new(&gpu, 320, 240).unwrap_or_else(|e| panic!("{e}"));
    renderer(&gpu)
        .render(&gpu, &targets, &known_scene())
        .unwrap_or_else(|e| panic!("{e}"));
    let image = targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}"));
    assert_image(&image, expected);
}

#[test]
fn a_window_frame_presents_what_the_output_holds() {
    let Some(gpu) = gpu() else { return };
    let targets = Targets::with_offscreen_surface(&gpu, 320, 240).unwrap_or_else(|e| panic!("{e}"));
    renderer(&gpu)
        .render(&gpu, &targets, &known_scene())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_image(
        &targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}")),
        expected,
    );
    assert_image(
        &targets.read_surface(&gpu).unwrap_or_else(|e| panic!("{e}")),
        expected,
    );
}

#[test]
fn counters_are_exact_for_each_frame() {
    let Some(gpu) = gpu() else { return };
    let renderer = renderer(&gpu);
    let offscreen = Targets::new(&gpu, 320, 240).unwrap_or_else(|e| panic!("{e}"));
    let window = Targets::with_offscreen_surface(&gpu, 320, 240).unwrap_or_else(|e| panic!("{e}"));
    let render = |targets: &Targets, scene: &TopDownScene| {
        renderer
            .render(&gpu, targets, scene)
            .unwrap_or_else(|e| panic!("{e}"))
    };
    // The top-down pass draws the three visible shapes at once, 32 bytes
    // each: centre and half extents (two f32 each), colour (three f32) and
    // whether it is a disc (a u32). The tone map draws once with its input.
    assert_eq!(
        render(&offscreen, &known_scene()),
        FrameCounters {
            draws: 2,
            pipeline_switches: 2,
            bind_group_switches: 1,
            upload_bytes: 96,
        }
    );
    // A window adds the present blit, which binds its own pipeline and input.
    assert_eq!(
        render(&window, &known_scene()),
        FrameCounters {
            draws: 3,
            pipeline_switches: 3,
            bind_group_switches: 2,
            upload_bytes: 96,
        }
    );
    // Nothing to draw but the background: only the tone map draws.
    let mut empty = known_scene();
    empty.shapes.retain(|shape| match shape {
        Shape::Rect { min, .. } => min.x > X0 + 10.0,
        Shape::Disc { .. } => false,
    });
    assert_eq!(empty.shapes.len(), 1, "the shape off the view stays");
    assert_eq!(
        render(&offscreen, &empty),
        FrameCounters {
            draws: 1,
            pipeline_switches: 1,
            bind_group_switches: 1,
            upload_bytes: 0,
        }
    );
    assert_image(
        &offscreen
            .read_output(&gpu)
            .unwrap_or_else(|e| panic!("{e}")),
        |_, _| BACKGROUND,
    );
}

#[test]
fn a_scene_that_cannot_be_drawn_renders_nothing() {
    let Some(gpu) = gpu() else { return };
    let renderer = renderer(&gpu);
    let targets = Targets::new(&gpu, 320, 240).unwrap_or_else(|e| panic!("{e}"));
    renderer
        .render(&gpu, &targets, &known_scene())
        .unwrap_or_else(|e| panic!("{e}"));
    let mut broken = known_scene();
    broken.shapes.push(Shape::Disc {
        centre: at(2.0, 2.0),
        radius: -1.0,
        colour: LinearRgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        },
    });
    match renderer.render(&gpu, &targets, &broken) {
        Err(RenderError::Scene { shape: Some(4), .. }) => {}
        other => panic!("{other:?}"),
    }
    // The last frame stays in the output.
    assert_image(
        &targets.read_output(&gpu).unwrap_or_else(|e| panic!("{e}")),
        expected,
    );
}
