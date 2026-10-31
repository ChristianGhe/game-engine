//! The GPU layer (spec 6.7): adapter choice, the capability report, the
//! fixed render graph, offscreen targets and readback. It is the only crate
//! that may import wgpu (`[floor]` in `architecture.toml`), and no wgpu type
//! appears in its public API. GPU work never feeds back into the simulation.
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
//!   encodes and submits the frame. A pass can clear its target and draw
//!   [`FullscreenPipeline`]s, which take the pass's one read as input; the
//!   caller writes only their fragment stage.
//! - **[`Targets`]**: the engine-owned scene (`Rgba16Float`, linear HDR) and
//!   output (`Rgba8UnormSrgb`) of one size. [`Targets::read_output`] reads
//!   the output back as an [`Image`], RGBA8 as stored, rows top first.
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

mod error;
mod gpu;
mod graph;
mod pass;
mod report;
mod targets;

pub use error::GpuError;
pub use gpu::{Gpu, GpuOptions, gpu_required};
pub use graph::{DRAW, Graph, GraphError, PRESENT, PassDecl, Resource, TONEMAP, Target};
pub use pass::{FullscreenDesc, FullscreenPipeline, LinearRgba, Pass, PassRecorder};
pub use report::{Backend, Capabilities, DeviceType, GpuReport};
pub use targets::{Image, Targets};
