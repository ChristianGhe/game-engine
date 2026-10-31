//! Opening the GPU: the instance, the adapter (plan decision 4), a Tier 0
//! device, and what every fullscreen pass shares.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use crate::error::GpuError;
use crate::pass::{FullscreenDesc, FullscreenPipeline, Shared};
use crate::report::{self, GpuReport};
use crate::targets;

/// One backend for each OS, the ones spec 8.3's adapters use: Metal on
/// macOS, DX12 on Windows, Vulkan elsewhere. wgpu tries Vulkan first and
/// sorts adapters only by device type, so with all three a Windows PC with a
/// Vulkan driver would render through Vulkan, and machines would differ.
const BACKENDS: wgpu::Backends = if cfg!(target_os = "macos") {
    wgpu::Backends::METAL
} else if cfg!(windows) {
    wgpu::Backends::DX12
} else {
    wgpu::Backends::VULKAN
};

/// How [`Gpu::new`] picks its adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GpuOptions {
    /// The first adapter whose name contains this text, ignoring case. With
    /// `None`, wgpu prefers a discrete GPU, then an integrated one, and
    /// software rendering last.
    pub adapter: Option<String>,
}

impl GpuOptions {
    /// `adapter` from `ENG_GPU_ADAPTER`; an empty value counts as unset.
    pub fn from_env() -> Self {
        Self {
            adapter: std::env::var("ENG_GPU_ADAPTER")
                .ok()
                .filter(|name| !name.is_empty()),
        }
    }
}

/// Whether a missing adapter fails rather than skips: `ENG_REQUIRE_GPU=1`,
/// which CI sets (plan decision 4).
pub fn gpu_required() -> bool {
    std::env::var("ENG_REQUIRE_GPU").is_ok_and(|value| value == "1")
}

/// A device on one adapter, at Tier 0: WebGPU's default features and limits
/// (spec 6.7), whatever the adapter could do beyond them.
///
/// Errors that no call returns, which should not happen through this API,
/// panic with `[E-GPU]`: validation errors are fatal (spec 6.7).
pub struct Gpu {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) shared: Shared,
    report: GpuReport,
}

impl Gpu {
    /// Opens a device on the adapter `options` names, or on wgpu's choice.
    pub fn new(options: &GpuOptions) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: BACKENDS,
            // wgpu's release flags in every build, so tests run as the game
            // does; `WGPU_DEBUG=1` and `WGPU_VALIDATION=1` add debug labels
            // and the backend's validation layer when debugging. Debug builds
            // used to add them by default, and Debian 13's Vulkan loader then
            // hung or crashed now and then when several threads created and
            // dropped devices at once (rung 1b, T7).
            flags: wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL.with_env(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions {
                // FXC ships with Windows, so no DLL goes next to the binary
                // (plan decision 10). wgpu's default would take DXC when one
                // is on the PATH, so machines would differ.
                dx12: wgpu::Dx12BackendOptions {
                    shader_compiler: wgpu::Dx12Compiler::Fxc,
                    ..wgpu::Dx12BackendOptions::default()
                },
                ..wgpu::BackendOptions::default()
            },
            display: None,
        });
        let adapters = block_on(instance.enumerate_adapters(BACKENDS));
        let adapter = match &options.adapter {
            Some(wanted) => {
                let wanted = wanted.to_lowercase();
                adapters
                    .iter()
                    .find(|a| a.get_info().name.to_lowercase().contains(&wanted))
                    .cloned()
            }
            None => block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..wgpu::RequestAdapterOptions::default()
            }))
            .ok(),
        };
        let Some(adapter) = adapter else {
            return Err(GpuError::NoAdapter {
                wanted: options.adapter.clone(),
                found: adapters
                    .iter()
                    .map(|a| report::describe(&a.get_info()))
                    .collect(),
            });
        };
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("eng-gpu"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..wgpu::DeviceDescriptor::default()
        }))
        .map_err(|e| GpuError::Device(e.to_string()))?;
        device.on_uncaptured_error(Arc::new(|error| {
            panic!("[E-GPU] wgpu error outside any call that returns one: {error}")
        }));
        let report = report::report(&adapter.get_info(), adapter.features());
        let shared = Shared::new(&device);
        Ok(Self {
            device,
            queue,
            shared,
            report,
        })
    }

    /// The adapter and what it supports.
    pub fn report(&self) -> &GpuReport {
        &self.report
    }

    /// Compiles a fullscreen pass's fragment stage (`desc.wgsl`) behind
    /// eng-gpu's vertex stage, for drawing into `desc.target`.
    pub fn fullscreen_pipeline(
        &self,
        desc: &FullscreenDesc<'_>,
    ) -> Result<FullscreenPipeline, GpuError> {
        let shader_error = |message: String| GpuError::Shader {
            label: desc.label.to_string(),
            message,
        };
        let format = targets::format(desc.target).ok_or_else(|| {
            shader_error(format!(
                "eng-gpu has no `{}` to draw into yet",
                desc.target.name()
            ))
        })?;
        self.scoped(|| self.shared.pipeline(&self.device, desc, format))
            .map_err(shader_error)
    }

    /// Runs `work`, then returns the first error wgpu reported meanwhile:
    /// validation, internal or out of memory.
    pub(crate) fn scoped<T>(&self, work: impl FnOnce() -> T) -> Result<T, String> {
        let memory = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let internal = self.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let validation = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let value = work();
        // Scopes pop in reverse order, each as soon as `pop` is called.
        let popped = [validation.pop(), internal.pop(), memory.pop()];
        let mut first = None;
        for scope in popped {
            if let Some(error) = block_on(scope) {
                first.get_or_insert_with(|| error.to_string());
            }
        }
        first.map_or(Ok(value), Err)
    }
}

/// Drives a wgpu future to its end without an async runtime (plan decision
/// 6). The native backends finish these at once; GPU work that takes time is
/// waited for with `Device::poll` instead.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}
