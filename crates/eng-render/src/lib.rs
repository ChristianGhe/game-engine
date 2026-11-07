//! The render passes (spec 6.7, 6.13): rung 1b's top-down view (kept later
//! as the automap and debug view), tone mapping, the present blit, and
//! per-frame counters, on eng-gpu's fixed graph, and rung 2's first-person
//! view on its sorted graph (below). Nothing here feeds back into the
//! simulation: a game's extraction builds a [`TopDownScene`] (or a
//! [`FirstPersonScene`]) from the simulation's state, interpolating if it
//! wants, and the renderer reads only that.
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
//!   nothing in view skips the top-down draw. The probe's `shot`, `video`
//!   and `play` reports carry them; a budgets file's `[gpu]` table, when it
//!   has one, limits `shot` and `video` frames (`budgets.toml` has one;
//!   acceptance feature `gpu-report-budgets`).
//!
//! Rung 2's first-person pipeline (`render-retro`, ADR-0020), beside the
//! top-down view and sharing nothing with it but the blit shader:
//! - **[`FirstPersonRenderer`]** (tasks T9 to T11) draws a [`GpuWorld`]
//!   as a [`FirstPersonScene`] sees it, through eng-gpu's sorted graph:
//!   `opaque` draws the world's [`Chunk`]s, each placed by one instance
//!   record of its origin minus the eye taken in f64, into linear albedo,
//!   reversed-Z depth (`near / distance ahead`, 0 where nothing was drawn)
//!   and `info` ([`Info`]: region, [`SurfaceClass`] and object id in two
//!   `u32`s); `sprites` draws each thing ([`Sprite`]) on top as a
//!   billboard standing on its feet, turned about Z only to face the
//!   camera, showing the frame of its [`RotationSheet`] that its rotation
//!   toward the eye picks, alpha-tested and depth-tested, with class
//!   [`SurfaceClass::Sprite`], its id and its region in `info`; `light`
//!   lights the albedo by its region's light in lux
//!   ([`RegionLook`], [`LIGHT_STEP_LUX`]) into luminance in nits; `sky`
//!   draws the [`Sky`] where the depth is 0; `fog` moves each pixel toward
//!   its region's fog with distance; `tonemap` exposes by the scene's
//!   EV100 ([`exposure`]) and maps by Khronos PBR Neutral (plan decision
//!   7); `palette` maps the display colours through a [`PaletteLut`]
//!   (decision 8) into the output; and `present` blits it, k×k into a
//!   window for eng-gpu's upscaled targets. Each look feature switches off
//!   alone ([`Look`]), fog and palette by bypassing their passes; with all
//!   off the image is the plain pipeline's
//!   ([`FirstPersonRenderer::render_plain`]). Its
//!   [`FirstPersonRenderer::read_depth`], [`FirstPersonRenderer::read_info`],
//!   [`FirstPersonRenderer::read_albedo`], [`FirstPersonRenderer::read_lit`]
//!   and [`FirstPersonRenderer::read_fogged`] read those targets back.
//! - **[`FirstPersonCamera`]**: an f64 eye, [`BAngle`](eng_math::BAngle)
//!   yaw and pitch, a vertical field of view and a near plane
//!   ([`NEAR_PLANE`], 0.1 m), made ready for an aspect ratio as a
//!   [`ViewProj`]: positions subtracted from the eye in f64, then a
//!   reversed-Z perspective with an infinite far plane.
//! - **[`sprite_rotation`]**: which of a thing's 8 rotations the viewer
//!   sees, by plan decision 4's integer rule, and [`RotationSheet`]'s
//!   mirror flags for sheets that draw only 5; the `sprites` pass applies
//!   them each frame ([`Sprite::frame_toward`]).
//! - **[`PaletteLut`]**: the palette look's 32x32x32 table, baked from
//!   sRGB colours by OKLab distance, and the [`BAYER_4X4`] dither matrix;
//!   **[`GplPalette`]** reads the colours from GIMP's plain-text `.gpl`
//!   format, strictly.
//!
//! Their floats go through eng-math only, as the simulation's do, so the
//! numbers do not depend on the target's own maths library.
#![forbid(unsafe_code)]

mod camera;
mod error;
mod first_person;
mod gpl;
mod palette;
mod renderer;
mod scene;
mod sprite_rot;

pub use camera::{Clip, FirstPersonCamera, Mat4, NEAR_PLANE, ViewBasis, ViewProj};
pub use eng_gpu::{ColourTag, FrameCounters, Material, SceneTexture, Texels, TextureSlot};
pub use error::RenderError;
pub use first_person::{
    CONTRAST_STEPS, Chunk, DITHER_CODES, FirstPersonRenderer, FirstPersonScene, GpuWorld, Info,
    LIGHT_FALLOFF, LIGHT_STEP_LUX, LIGHT_STEPS, LOW_RESOLUTION, Look, MAX_REGIONS, RegionLook, Sky,
    Sprite, SurfaceClass, WorldMesh, WorldVertex, exposure,
};
pub use gpl::GplPalette;
pub use palette::{
    BAYER_4X4, LUT_CELLS, LUT_SIDE, MAX_PALETTE_COLOURS, Oklab, PaletteError, PaletteLut, Srgb8,
    bayer, bayer4_offset, lut_cell_centre, srgb_to_linear,
};
pub use renderer::Renderer;
pub use scene::{LinearRgb, Quad, Shape, TopDownCamera, TopDownScene};
pub use sprite_rot::{
    RotationSheet, SPRITE_ROTATIONS, SheetFrame, direction_to, sprite_rotation,
    sprite_rotation_toward,
};
