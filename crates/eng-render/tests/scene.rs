//! The top-down scene and its projection (rung 1b, task T8): shapes in world
//! space become quads in clip space on the CPU, the camera subtracted in f64
//! first (AGENTS.md conventions). No GPU is needed.
//!
//! The views here are 320x240 at 30 pixels per meter, so a meter is 0.1875
//! of clip space across and 0.25 down, both exact in binary: each expected
//! value is worked out by hand.

use eng_math::{Vec2, WorldPos};
use eng_render::{LinearRgb, Quad, RenderError, Shape, TopDownCamera, TopDownScene};

const GREY: LinearRgb = LinearRgb {
    r: 0.5,
    g: 0.5,
    b: 0.5,
};
const RED: LinearRgb = LinearRgb {
    r: 4.0,
    g: 0.0,
    b: 0.0,
};

fn camera(x: f64, y: f64) -> TopDownCamera {
    TopDownCamera {
        centre: WorldPos::new(x, y, 0.0),
        pixels_per_meter: 30.0,
    }
}

fn scene(camera: TopDownCamera, shapes: Vec<Shape>) -> TopDownScene {
    TopDownScene {
        camera,
        background: GREY,
        shapes,
    }
}

fn rect(min: (f64, f64), max: (f64, f64)) -> Shape {
    Shape::Rect {
        min: WorldPos::new(min.0, min.1, 0.0),
        max: WorldPos::new(max.0, max.1, 0.0),
        colour: GREY,
    }
}

fn disc(centre: (f64, f64), radius: f32) -> Shape {
    Shape::Disc {
        centre: WorldPos::new(centre.0, centre.1, 0.0),
        radius,
        colour: RED,
    }
}

fn quad(centre: (f32, f32), half: (f32, f32), colour: LinearRgb, disc: bool) -> Quad {
    Quad {
        centre: Vec2::new(centre.0, centre.1),
        half_extents: Vec2::new(half.0, half.1),
        colour,
        disc,
    }
}

/// Three shapes around the origin and the quads they make.
fn shapes_at(x: f64, y: f64) -> (Vec<Shape>, Vec<Quad>) {
    let shapes = vec![
        // 4 by 4 meters, its centre 1 meter above the camera's.
        rect((x - 2.0, y - 1.0), (x + 2.0, y + 3.0)),
        // 4 meters right and 2 down, 30 pixels in radius either way.
        disc((x + 4.0, y - 2.0), 1.0),
        // Up and to the right: positive clip coordinates.
        rect((x + 1.0, y + 1.0), (x + 3.0, y + 2.0)),
    ];
    let quads = vec![
        quad((0.0, 0.25), (0.375, 0.5), GREY, false),
        quad((0.75, -0.5), (0.1875, 0.25), RED, true),
        quad((0.375, 0.375), (0.1875, 0.125), GREY, false),
    ];
    (shapes, quads)
}

#[test]
fn shapes_become_quads_with_x_to_the_right_and_y_up() {
    let (shapes, quads) = shapes_at(0.0, 0.0);
    assert_eq!(scene(camera(0.0, 0.0), shapes).quads(320, 240), Ok(quads));
}

#[test]
fn the_camera_is_subtracted_in_f64_before_rounding_to_f32() {
    // f32 cannot tell 1e9 + 0.5 from 1e9 (its step there is 64 m), so
    // rounding before subtracting would put every shape at the camera.
    let (x, y) = (1.0e9 + 0.5, -3.0e9);
    let (mut shapes, quads) = shapes_at(x, y);
    // Heights do not matter from above, not even a height that is NaN.
    if let Shape::Rect { min, .. } = &mut shapes[0] {
        min.z = 1.0e300;
    }
    if let Shape::Disc { centre, .. } = &mut shapes[1] {
        centre.z = f64::NAN;
    }
    let mut camera = camera(x, y);
    camera.centre.z = -7.0;
    assert_eq!(scene(camera, shapes).quads(320, 240), Ok(quads));
}

#[test]
fn pixels_stay_square_whatever_the_view() {
    // Half as wide at the same zoom: a meter takes twice the clip space
    // across, and the same down.
    let shapes = vec![disc((2.0, -2.0), 1.0)];
    assert_eq!(
        scene(camera(0.0, 0.0), shapes).quads(160, 240),
        Ok(vec![quad((0.75, -0.5), (0.375, 0.25), RED, true)])
    );
}

#[test]
fn a_rectangle_far_larger_than_the_view_keeps_its_edges() {
    // Clipped to the view in f64, its quad is small enough for f32 to hold
    // the edge at 0.5 m exactly; unclipped, the centre and half extents
    // would be near 10^6 and cancel, moving the edge 5 pixels.
    let shapes = vec![rect((-1.0e7, -1.0e7), (0.5, 0.5))];
    assert_eq!(
        scene(camera(0.0, 0.0), shapes).quads(320, 240),
        Ok(vec![quad(
            (-0.453125, -0.4375),
            (0.546875, 0.5625),
            GREY,
            false
        )])
    );
}

#[test]
fn shapes_outside_the_view_are_left_out() {
    // The view reaches 4 meters up and down from the camera.
    let shapes = vec![
        // Left of the view.
        rect((-10.0, 0.0), (-6.0, 1.0)),
        // Above it, touching its top edge.
        rect((0.0, 4.0), (1.0, 5.0)),
        // Crossing the top edge: clipped to the view.
        rect((0.0, 3.5), (1.0, 5.0)),
        // Below it, touching its bottom edge.
        disc((0.0, -5.0), 1.0),
        // Crossing the bottom edge.
        disc((0.0, -4.5), 1.0),
        // Nothing to fill.
        rect((1.0, 1.0), (1.0, 2.0)),
        disc((1.0, 1.0), 0.0),
    ];
    assert_eq!(
        scene(camera(0.0, 0.0), shapes).quads(320, 240),
        Ok(vec![
            quad((0.09375, 0.9375), (0.09375, 0.0625), GREY, false),
            quad((0.0, -1.125), (0.1875, 0.25), RED, true),
        ])
    );
}

fn error(scene: &TopDownScene, width: u32, height: u32) -> (Option<usize>, String) {
    match scene.quads(width, height) {
        Err(e @ RenderError::Scene { shape, .. }) => {
            let text = e.to_string();
            assert!(text.starts_with("[E-SCENE] "), "{text}");
            (shape, text)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_camera_or_view_that_shows_nothing_is_an_error() {
    for zoom in [0.0, -30.0, f64::NAN, f64::INFINITY] {
        let mut camera = camera(0.0, 0.0);
        camera.pixels_per_meter = zoom;
        let (shape, text) = error(&scene(camera, vec![]), 320, 240);
        assert_eq!(shape, None);
        assert_eq!(
            text,
            format!(
                "[E-SCENE] the camera's pixels_per_meter is {zoom}; it must be finite and above 0"
            )
        );
    }
    let lost = scene(camera(f64::NAN, 0.0), vec![]);
    assert_eq!(
        error(&lost, 320, 240).1,
        "[E-SCENE] the camera's centre must be finite"
    );
    for (width, height) in [(0, 240), (320, 0)] {
        assert_eq!(
            error(&scene(camera(0.0, 0.0), vec![]), width, height).1,
            format!("[E-SCENE] a {width}x{height} view; each side must be at least 1 pixel")
        );
    }
    let mut dark = scene(camera(0.0, 0.0), vec![]);
    dark.background.g = f32::NAN;
    assert_eq!(
        error(&dark, 320, 240).1,
        "[E-SCENE] the background colour must be finite"
    );
}

#[test]
fn a_shape_that_cannot_be_drawn_is_an_error_naming_it() {
    let fine = rect((0.0, 0.0), (1.0, 1.0));
    let mut grey_nan = GREY;
    grey_nan.b = f32::INFINITY;
    let cases = [
        (
            rect((1.0, 0.0), (0.0, 1.0)),
            "a rectangle's max is below its min",
        ),
        (
            rect((0.0, 1.0), (1.0, 0.0)),
            "a rectangle's max is below its min",
        ),
        (
            rect((0.0, f64::NAN), (1.0, 1.0)),
            "a rectangle's corners must be finite",
        ),
        (
            rect((0.0, 0.0), (f64::INFINITY, 1.0)),
            "a rectangle's corners must be finite",
        ),
        (disc((f64::NAN, 0.0), 1.0), "a disc's centre must be finite"),
        (
            disc((0.0, 0.0), -1.0),
            "a disc's radius is -1; it must be finite and not negative",
        ),
        (
            disc((0.0, 0.0), f32::NAN),
            "a disc's radius is NaN; it must be finite and not negative",
        ),
        (
            Shape::Rect {
                min: WorldPos::new(0.0, 0.0, 0.0),
                max: WorldPos::new(1.0, 1.0, 0.0),
                colour: grey_nan,
            },
            "its colour must be finite",
        ),
    ];
    for (bad, problem) in cases {
        let (shape, text) = error(&scene(camera(0.0, 0.0), vec![fine, bad]), 320, 240);
        assert_eq!(shape, Some(1), "{text}");
        assert_eq!(text, format!("[E-SCENE] shape 1: {problem}"));
    }
    // A disc is not clipped: at 10^10 pixels per meter, one of 10^31 m has
    // half extents beyond f32 in clip space.
    let mut close = camera(0.0, 0.0);
    close.pixels_per_meter = 1.0e10;
    assert_eq!(
        error(
            &scene(close, vec![fine, disc((0.0, 0.0), 1.0e31)]),
            320,
            240
        )
        .1,
        "[E-SCENE] shape 1: it is too large to draw"
    );
}
