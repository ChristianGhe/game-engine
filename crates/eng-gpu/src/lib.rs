//! The GPU layer (spec 6.7): adapter choice, the capability report, the
//! render graph, its passes' draws and per-frame counters, offscreen
//! targets, the transient pool and readback. It is the only crate that may import wgpu
//! (`[floor]` in `architecture.toml`), and no wgpu type appears in its
//! public API. GPU work never feeds back into the simulation. eng-render
//! builds rung 1b's passes on it.
//!
//! - **[`Gpu`]** opens a device at Tier 0, WebGPU's default features and
//!   limits, whatever the adapter could do beyond them, except that it asks
//!   for timestamp queries when the adapter has them, to time passes (rung
//!   2, O11); the tier stays 0. It refuses an adapter without storage
//!   buffers in the vertex stage, which the scene layout needs. [`GpuOptions`] picks
//!   the adapter: `ENG_GPU_ADAPTER` names it, else wgpu chooses. Without one,
//!   [`Gpu::new`] fails with `[E-NO-ADAPTER]`; tests then skip, unless
//!   `ENG_REQUIRE_GPU=1` ([`gpu_required`]) says CI needs a GPU (plan
//!   decision 4). Each OS has one backend: Metal on macOS, Vulkan on Linux,
//!   and DX12 on Windows, with the FXC shader compiler, which ships with
//!   Windows (decision 10). Debug builds run as release builds do;
//!   `WGPU_DEBUG=1` and `WGPU_VALIDATION=1` add debug labels and the
//!   backend's validation layer.
//! - **[`GpuReport`]**, the capability report: the adapter, its backend,
//!   device type and driver, the tier, and what it supports beyond Tier 0
//!   ([`Capabilities`]: timestamp queries and the later tiers' features).
//! - **[`Graph`]**, the render graph. Passes declare what they read and
//!   write ([`PassDecl`]).
//!   - Rung 1b's fixed graph (decision 1) has three passes, not the spec's
//!     two (ADR-0017): [`DRAW`] writes the [`Resource::Scene`],
//!     [`TONEMAP`] reads it and writes the [`Resource::Output`], and
//!     [`PRESENT`] reads the output and writes the [`Resource::Surface`], in
//!     a [`Target::Surface`] graph only. [`Graph::new`] checks a list that
//!     runs in the order given; [`Graph::fixed`] is rung 1b's list, and the
//!     one eng-render draws through.
//!   - Rung 2's graph (ADR-0019): [`Graph::sorted`] orders
//!     [`PassNode`]s by what they read, write and continue (a pass that
//!     draws on top of an earlier pass's targets), names breaking ties;
//!     refuses a cycle and two writers of one resource with `[E-GRAPH]`;
//!     with [`Graph::sorted_without`], bypasses switched-off passes, whose
//!     readers read their input ([`Graph::bypassed`]); drops the passes
//!     whose writes reach neither the output nor the surface
//!     ([`Graph::culled`]); and plans the frame's [`Resource::Transient`]s
//!     ([`Transient`]), which share a texture when their format, size and
//!     usage match and their lifetimes do not overlap ([`Graph::pool`]).
//! - **[`Pass`]**: each frame, [`Graph::render`] takes one pass per
//!   declaration, in the order declared, has every pass that runs record
//!   into a [`PassRecorder`] in running order, checks each recording against
//!   its declaration, then uploads the frame's instances, and encodes and
//!   submits the frame. A pass draws into up to four colour targets and one
//!   depth target ([`Format::Depth32Float`], reversed: it clears to 0, the
//!   far plane). It can clear them (unless it continues another, whose
//!   targets it loads) and draw, in order, rung 1b's pipelines into a pass
//!   of one write:
//!   - [`FullscreenPipeline`]s, which take the pass's one read as input;
//!     the caller writes only their fragment stage;
//!   - [`InstancedPipeline`]s: a triangle strip per instance, each instance
//!     reading its [`Attribute`]s from bytes the pass records, into any
//!     target but the surface;
//!
//!   and rung 2's (ADR-0020), made for a pass's reads and writes, which
//!   [`Graph::render_frame`] gives the frame's [`FrameInput`]: a per-frame
//!   uniform and [`InstanceRecord`]s, and a [`Scene`]:
//!   - [`MeshPipeline`]s: indexed triangles from the scene's vertex and
//!     `u32` index megabuffers ([`MeshDraw`]), each instance a record of the
//!     chunk's origin minus the eye, taken in f64 on the CPU;
//!   - [`ScreenPipeline`]s: fullscreen passes that read every read of their
//!     pass, colour, integer or depth, and bind the uniform.
//! - **[`Scene`]** ([`Gpu::scene`]): the GPU scene layout of spec 6.7, fixed
//!   at rung 2 (ADR-0020), written once when a scene loads: the
//!   megabuffers, a [`Material`] table in storage, and `texture_2d_array`s
//!   of [`SceneTexture`]s bucketed by size (64, 128 and 256 px), one per
//!   bucket and [`ColourTag`], sampled at the nearest texel, no mipmaps.
//! - **[`FrameCounters`]**: [`Graph::render`] returns what the frame asked
//!   of the GPU (spec 6.13): draws, pipeline switches, bind-group switches
//!   and uploaded bytes. They are counted from the commands it encodes, so
//!   they are exact and the same on every adapter. A pass binds a pipeline
//!   only when a draw needs another, and each bind group once. They keep
//!   rung 1b's four fields; what rung 2 measures beyond them, the GPU time
//!   of each pass and the pool's size, is [`FrameStats`], from
//!   [`Targets::frame_stats`]. Pass times are opt-in
//!   ([`Targets::time_passes`]): `None` with timing off or without
//!   timestamp queries, and a frame with timing off has no timestamps.
//! - **Labels**: every pass, target, pooled texture and pipeline is named
//!   in wgpu's labels, and with `WGPU_DEBUG=1` each pass is also wrapped in
//!   a debug group of its name.
//! - **[`Targets`]**: the engine-owned scene (`Rgba16Float`, linear HDR) and
//!   output (`Rgba8UnormSrgb`) of one size, and the transient textures that
//!   graphs' pools draw into, made on the first frame that needs them and
//!   reused by every later one ([`Targets::transient_textures`]). [`Targets::read_output`] reads
//!   the output back as an [`Image`], RGBA8 as stored, rows top first, and
//!   [`Targets::read_transient`] what a graph's last frame left in one of
//!   its transients, such as rung 2's depth and `info`, as [`Texels`].
//!   [`Targets::with_offscreen_surface`] adds a texture in the surface's
//!   format, `Bgra8UnormSrgb`, standing in for a window's: a
//!   [`Target::Surface`] graph then renders whole, and
//!   [`Targets::read_surface`] reads back what it presented, in RGBA order.
//!   [`Targets::upscaled`] makes targets for rung 2's low internal
//!   resolution (ADR-0020): the window's size over an integer k, rounded
//!   down, whose present pass draws each texel as k×k pixels of the
//!   window's image ([`Targets::surface_size`]), centred, the remainder
//!   black.
//! - **[`WindowSurface`]**: a window's own surface, from [`Gpu::with_window`]
//!   on an adapter that can present to it, in `Bgra8UnormSrgb`, or
//!   `Bgra8Unorm` drawn through an sRGB view (`[E-GPU]` if it offers
//!   neither), presented with Fifo (plan decision 7). Each frame,
//!   [`WindowSurface::acquire`] gives a [`SurfaceFrame`] (or `None`: skip
//!   the frame), [`Graph::render_to`] draws into it, and
//!   [`WindowSurface::present`] shows it after the submit. The window comes
//!   through the window seam (ADR-0016): `raw-window-handle`'s two traits,
//!   which `eng-platform-api`'s `Arc<dyn Window>` implements.
//!
//! Errors come back as [`GpuError`] values whose messages start with a log
//! tag (`[E-NO-ADAPTER]`, `[E-GPU]`, `[E-SHADER]`, `[E-GRAPH]`). wgpu's own
//! errors, validation included, are caught around each call that returns a
//! `Result`; any other one panics, so validation errors are fatal in tests
//! (spec 6.7).
//!
//! `tests/shaders.rs` validates every WGSL file in the repo with naga and
//! translates it to SPIR-V, MSL and HLSL (decision 11, acceptance feature
//! `shaders`). So every shader lives in a `.wgsl` file, loaded with
//! `include_str!`: WGSL written inline in Rust escapes that check.
#![forbid(unsafe_code)]

mod counters;
mod encode;
mod error;
mod fullscreen;
mod gpu;
mod graph;
mod instanced;
mod layout;
mod mesh;
mod pass;
mod readback;
mod report;
mod scene;
mod screen;
mod surface;
mod targets;
mod timing;

pub use counters::FrameCounters;
pub use error::GpuError;
pub use fullscreen::{FullscreenDesc, FullscreenPipeline};
pub use gpu::{Gpu, GpuOptions, gpu_required};
pub use graph::{
    DRAW, Format, Graph, GraphError, PRESENT, PassDecl, PassNode, PooledTexture, Resource, TONEMAP,
    Target, Transient,
};
pub use instanced::{Attribute, InstancedDesc, InstancedPipeline};
pub use layout::MAX_COLOUR_TARGETS;
pub use mesh::{MeshDesc, MeshDraw, MeshPipeline};
pub use pass::{FrameInput, LinearRgba, Pass, PassRecorder};
pub use readback::{Image, Texels};
pub use report::{Backend, Capabilities, DeviceType, GpuReport};
pub use scene::{
    ColourTag, InstanceRecord, MAX_TEXTURE_LAYERS, Material, Scene, SceneDesc, SceneTexture,
    TEXTURE_SIDES, TextureSlot,
};
pub use screen::{ScreenDesc, ScreenPipeline};
pub use surface::{SurfaceFrame, WindowSurface};
pub use targets::{READBACK_LIMIT, Targets};
pub use timing::{FrameStats, PassTime};
