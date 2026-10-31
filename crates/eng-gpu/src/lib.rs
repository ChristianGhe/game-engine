//! The GPU layer (spec 6.7): adapter choice, the capability report, the
//! fixed render graph, its passes' draws and per-frame counters, offscreen
//! targets and readback. It is the only crate that may import wgpu
//! (`[floor]` in `architecture.toml`), and no wgpu type appears in its
//! public API. GPU work never feeds back into the simulation. eng-render
//! builds rung 1b's passes on it.
//!
//! - **[`Gpu`]** opens a device at Tier 0, WebGPU's default features and
//!   limits, whatever the adapter could do beyond them. [`GpuOptions`] picks
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
//! - **[`Graph`]**, the fixed render graph (decision 1). Passes declare what
//!   they read and write ([`PassDecl`]): [`DRAW`] writes the
//!   [`Resource::Scene`], [`TONEMAP`] reads it and writes the
//!   [`Resource::Output`], and [`PRESENT`] reads the output and writes the
//!   [`Resource::Surface`], in a [`Target::Surface`] graph only.
//!   [`Graph::new`] checks the declarations; [`Graph::fixed`] is rung 1b's
//!   list. Ordering, culling and pooling come in rung 2.
//! - **[`Pass`]**: each frame, [`Graph::render`] has every pass record into a
//!   [`PassRecorder`], checks each recording against its declaration, then
//!   uploads the frame's instances, and encodes and submits the frame. A
//!   pass can clear its target and draw, in order:
//!   - [`FullscreenPipeline`]s, which take the pass's one read as input; the
//!     caller writes only their fragment stage;
//!   - [`InstancedPipeline`]s: a triangle strip per instance, each instance
//!     reading its [`Attribute`]s from bytes the pass records, into the
//!     scene or the output.
//! - **[`FrameCounters`]**: [`Graph::render`] returns what the frame asked
//!   of the GPU (spec 6.13): draws, pipeline switches, bind-group switches
//!   and uploaded bytes. They are counted from the commands it encodes, so
//!   they are exact and the same on every adapter. A pass binds a pipeline
//!   only when a draw needs another, and its input once.
//! - **[`Targets`]**: the engine-owned scene (`Rgba16Float`, linear HDR) and
//!   output (`Rgba8UnormSrgb`) of one size. [`Targets::read_output`] reads
//!   the output back as an [`Image`], RGBA8 as stored, rows top first.
//!   [`Targets::with_offscreen_surface`] adds a texture in the surface's
//!   format, `Bgra8UnormSrgb`, standing in for a window's: a
//!   [`Target::Surface`] graph then renders whole, and
//!   [`Targets::read_surface`] reads back what it presented, in RGBA order.
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
mod error;
mod fullscreen;
mod gpu;
mod graph;
mod instanced;
mod pass;
mod report;
mod surface;
mod targets;

pub use counters::FrameCounters;
pub use error::GpuError;
pub use fullscreen::{FullscreenDesc, FullscreenPipeline};
pub use gpu::{Gpu, GpuOptions, gpu_required};
pub use graph::{DRAW, Graph, GraphError, PRESENT, PassDecl, Resource, TONEMAP, Target};
pub use instanced::{Attribute, InstancedDesc, InstancedPipeline};
pub use pass::{LinearRgba, Pass, PassRecorder};
pub use report::{Backend, Capabilities, DeviceType, GpuReport};
pub use surface::{SurfaceFrame, WindowSurface};
pub use targets::{Image, Targets};
