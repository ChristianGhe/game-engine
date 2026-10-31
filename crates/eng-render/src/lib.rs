//! The render passes (spec 6.7, 6.13): rung 1b's top-down view (kept later
//! as the automap and debug view), tone mapping, the present blit, and
//! per-frame counters, on eng-gpu's fixed graph. Nothing here feeds back
//! into the simulation: a game's extraction builds a [`TopDownScene`] from
//! the simulation's state, interpolating if it wants, and the renderer reads
//! only that.
//!
//! - **[`TopDownScene`]**: a [`TopDownCamera`], a background colour and
//!   [`Shape`]s, rectangles and discs in world space, drawn in order. The
//!   view looks straight down: +X to the right of the image, +Y up it,
//!   heights ignored, square pixels at the camera's pixels per meter.
//!   Colours are [`LinearRgb`]: linear and scene-referred, where 1 shows as
//!   full intensity once clamped. Light in nits and exposure in EV100 come
//!   with lights and the tone-mapping curve in rung 2.
//! - **Camera-relative.** [`TopDownScene::quads`] subtracts the camera from
//!   each position in f64, scales it to clip space and only then rounds to
//!   f32 (AGENTS.md conventions), so a world far from its origin keeps its
//!   precision. Rectangles are clipped to the image in f64 first, so one far
//!   larger than the image keeps its edges. Shapes that cover none of the
//!   image are left out, so they cost nothing to upload. A scene that cannot
//!   be drawn (a value that is not finite, a rectangle inside out, a zoom
//!   that is not above 0) is an `[E-SCENE]` [`RenderError`], before any GPU
//!   work.
//! - **[`Renderer`]** compiles the passes once; [`Renderer::render`] draws
//!   a frame through eng-gpu's fixed graph:
//!   - draw (`top_down.wgsl`): the background as the clear colour, then
//!     every [`Quad`] in one instanced draw into the scene (`Rgba16Float`);
//!   - tonemap (`tonemap.wgsl`): rung 1b's clamp to [0, 1] (plan decision 2)
//!     into the output (`Rgba8UnormSrgb`), with alpha 1. `shot`, `video` and
//!     the golden images read the output;
//!   - present (`present.wgsl`), when the targets have a surface: the output
//!     drawn texel for texel into the surface's format.
//!
//!   [`Renderer::render_to`] draws the same three passes into a window's
//!   image (eng-gpu's `SurfaceFrame`), which `play` then presents.
//! - **[`FrameCounters`]** come back from every frame: draws, pipeline and
//!   bind-group switches, and uploaded bytes, counted by eng-gpu from what
//!   it encodes, so they are exact and the same on every adapter. Offscreen
//!   a frame costs 2 draws, 2 pipeline switches, 1 bind-group switch and 32
//!   bytes for each shape in view; presenting adds 1, 1 and 1. A scene with
//!   nothing in view skips the top-down draw. The probe's `shot` and
//!   `video` reports carry them, and `budgets.toml [gpu]` holds them per
//!   frame (acceptance feature `gpu-report-budgets`).
#![forbid(unsafe_code)]

mod error;
mod renderer;
mod scene;

pub use eng_gpu::FrameCounters;
pub use error::RenderError;
pub use renderer::Renderer;
pub use scene::{LinearRgb, Quad, Shape, TopDownCamera, TopDownScene};
