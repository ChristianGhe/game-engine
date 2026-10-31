//! The top-down scene: what a game's extraction hands the renderer each
//! frame, and its projection into clip space on the CPU.

use eng_math::{Vec2, WorldPos, clamp_f64};

use crate::RenderError;

/// A colour in linear, scene-referred units (AGENTS.md conventions). Rung
/// 1b's tone map clamps each channel to [0, 1], so 1 shows as the display's
/// full intensity, and more is allowed in the scene. Light in nits and EV100
/// exposure arrive with lights in rung 2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearRgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl LinearRgb {
    fn is_finite(self) -> bool {
        self.r.is_finite() && self.g.is_finite() && self.b.is_finite()
    }
}

/// How the top-down view looks at the world: straight down the Z axis, with
/// +X to the right of the image and +Y up it (right-handed, Z up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopDownCamera {
    /// The world point at the image's centre. Its z is ignored.
    pub centre: WorldPos,
    /// The zoom: pixels per meter, the same across and down, so pixels stay
    /// square. Finite and above 0.
    pub pixels_per_meter: f64,
}

/// One thing the top-down view draws, filled with one colour. Heights (z)
/// are ignored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// An axis-aligned rectangle from `min` to `max`.
    Rect {
        min: WorldPos,
        max: WorldPos,
        colour: LinearRgb,
    },
    /// A filled circle around `centre`, `radius` meters across from it.
    Disc {
        centre: WorldPos,
        radius: f32,
        colour: LinearRgb,
    },
}

/// What the top-down view shows in one frame. A game's extraction builds
/// it from the simulation's state, interpolated if it wants; the renderer
/// reads nothing else and never writes back.
#[derive(Clone, Debug, PartialEq)]
pub struct TopDownScene {
    pub camera: TopDownCamera,
    /// What shows where no shape is.
    pub background: LinearRgb,
    /// Drawn in order, each over the ones before it.
    pub shapes: Vec<Shape>,
}

/// A shape as the top-down pass draws it: a rectangle in clip space, where
/// x runs from -1 at the image's left edge to 1 at its right, and y from -1
/// at the bottom to 1 at the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub centre: Vec2,
    pub half_extents: Vec2,
    pub colour: LinearRgb,
    /// Whether only the ellipse inside the rectangle is filled, a disc in
    /// square pixels.
    pub disc: bool,
}

impl TopDownScene {
    /// The quads the top-down pass draws for an image of `width` by
    /// `height` pixels, in the shapes' order. The camera's centre maps to
    /// the image's centre.
    ///
    /// Each position is subtracted from the camera's in f64, scaled to clip
    /// space, and only then rounded to f32 (AGENTS.md conventions), so
    /// large worlds keep their precision. Rectangles are clipped to the
    /// image first, in f64, so one far larger than the image keeps its
    /// edges exact; discs are not. A shape that covers none of the image,
    /// being beyond an edge or empty, is left out, so it costs nothing to
    /// upload. A scene that cannot be drawn is an `[E-SCENE]` error.
    pub fn quads(&self, width: u32, height: u32) -> Result<Vec<Quad>, RenderError> {
        let view = View::new(&self.camera, width, height)?;
        if !self.background.is_finite() {
            return Err(RenderError::Scene {
                shape: None,
                problem: "the background colour must be finite".into(),
            });
        }
        let mut quads = Vec::with_capacity(self.shapes.len());
        for (index, shape) in self.shapes.iter().enumerate() {
            let quad = view.quad(shape).map_err(|problem| RenderError::Scene {
                shape: Some(index),
                problem,
            })?;
            quads.extend(quad);
        }
        Ok(quads)
    }
}

/// The camera and image size, as a map from world to clip space.
struct View {
    centre: (f64, f64),
    /// Clip units per meter, across and down.
    scale: (f64, f64),
}

impl View {
    fn new(camera: &TopDownCamera, width: u32, height: u32) -> Result<Self, RenderError> {
        let fail = |problem: String| RenderError::Scene {
            shape: None,
            problem,
        };
        let zoom = camera.pixels_per_meter;
        if !(zoom.is_finite() && zoom > 0.0) {
            return Err(fail(format!(
                "the camera's pixels_per_meter is {zoom}; it must be finite and above 0"
            )));
        }
        let centre = camera.centre;
        if !(centre.x.is_finite() && centre.y.is_finite()) {
            return Err(fail("the camera's centre must be finite".into()));
        }
        if width == 0 || height == 0 {
            return Err(fail(format!(
                "a {width}x{height} view; each side must be at least 1 pixel"
            )));
        }
        Ok(Self {
            centre: (centre.x, centre.y),
            scale: (
                2.0 * zoom / f64::from(width),
                2.0 * zoom / f64::from(height),
            ),
        })
    }

    /// Where a world position lands in clip space, still in f64.
    fn clip(&self, x: f64, y: f64) -> (f64, f64) {
        (
            (x - self.centre.0) * self.scale.0,
            (y - self.centre.1) * self.scale.1,
        )
    }

    /// The quad for `shape`, or `None` if it covers none of the view; a
    /// problem with the shape as the error.
    fn quad(&self, shape: &Shape) -> Result<Option<Quad>, String> {
        // The centre and half extents in clip space, in f64.
        let (centre, half, colour, disc) = match *shape {
            Shape::Rect { min, max, colour } => {
                if ![min.x, min.y, max.x, max.y].iter().all(|v| v.is_finite()) {
                    return Err("a rectangle's corners must be finite".into());
                }
                if max.x < min.x || max.y < min.y {
                    return Err("a rectangle's max is below its min".into());
                }
                // Clipped to the view: the same pixels, and a quad small
                // enough for f32 to keep the edges where they are.
                let view = |v: f64| clamp_f64(v, -1.0, 1.0);
                let (low, high) = (self.clip(min.x, min.y), self.clip(max.x, max.y));
                let (low, high) = ((view(low.0), view(low.1)), (view(high.0), view(high.1)));
                let centre = ((low.0 + high.0) / 2.0, (low.1 + high.1) / 2.0);
                let half = ((high.0 - low.0) / 2.0, (high.1 - low.1) / 2.0);
                (centre, half, colour, false)
            }
            Shape::Disc {
                centre,
                radius,
                colour,
            } => {
                if !(centre.x.is_finite() && centre.y.is_finite()) {
                    return Err("a disc's centre must be finite".into());
                }
                if !(radius.is_finite() && radius >= 0.0) {
                    return Err(format!(
                        "a disc's radius is {radius}; it must be finite and not negative"
                    ));
                }
                let radius = f64::from(radius);
                let half = (radius * self.scale.0, radius * self.scale.1);
                (self.clip(centre.x, centre.y), half, colour, true)
            }
        };
        if !colour.is_finite() {
            return Err("its colour must be finite".into());
        }
        // Some of the image lies strictly inside the quad on both axes.
        let covers =
            |centre: f64, half: f64| half > 0.0 && centre + half > -1.0 && centre - half < 1.0;
        if !(covers(centre.0, half.0) && covers(centre.1, half.1)) {
            return Ok(None);
        }
        let quad = Quad {
            centre: Vec2::new(centre.0 as f32, centre.1 as f32),
            half_extents: Vec2::new(half.0 as f32, half.1 as f32),
            colour,
            disc,
        };
        let values = [
            quad.centre.x,
            quad.centre.y,
            quad.half_extents.x,
            quad.half_extents.y,
        ];
        if !values.iter().all(|v| v.is_finite()) {
            return Err("it is too large to draw".into());
        }
        Ok(Some(quad))
    }
}
