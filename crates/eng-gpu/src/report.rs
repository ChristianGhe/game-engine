//! The capability report (spec 6.7, 6.13): which adapter renders, and what it
//! could do beyond Tier 0. Plain data; reports carry it as their `gpu` object.

/// The adapter a [`crate::Gpu`] runs on, and what it supports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuReport {
    /// The adapter's name, such as `Apple M2 Pro` or
    /// `llvmpipe (LLVM 19.1.7, 256 bits)`.
    pub adapter: String,
    pub backend: Backend,
    pub device_type: DeviceType,
    /// The backend's vendor and device ids: PCI ids where it has them, else
    /// 0 (Metal).
    pub vendor: u32,
    pub device: u32,
    /// The driver's name and its version text, as the backend gives them;
    /// empty where it gives none (Metal).
    pub driver: String,
    pub driver_info: String,
    /// The capability tier the device runs at. Level 1 uses only Tier 0,
    /// WebGPU's default features and limits (spec 6.7), whatever the adapter
    /// supports.
    pub tier: u8,
    /// What the adapter supports. Tier 0 enables none of it.
    pub capabilities: Capabilities,
}

/// Optional adapter features that later tiers or pass timing would use
/// (spec 6.7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// GPU timestamp queries. Pass timing reports `null` without them.
    pub timestamp_queries: bool,
    /// Arrays of textures in one binding (bindless).
    pub binding_arrays: bool,
    /// Indirect draws whose count comes from a buffer.
    pub multi_draw_indirect_count: bool,
    /// Ray queries in shaders.
    pub ray_query: bool,
    /// Task and mesh shaders.
    pub mesh_shaders: bool,
}

/// The graphics API under the adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Vulkan,
    Metal,
    Dx12,
    /// Anything else wgpu might report. Only the three above are built.
    Other,
}

impl Backend {
    /// `vulkan`, `metal`, `dx12` or `other`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::Dx12 => "dx12",
            Self::Other => "other",
        }
    }
}

/// What kind of device the adapter is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceType {
    IntegratedGpu,
    DiscreteGpu,
    /// A virtual or hosted GPU, such as a CI runner's paravirtual device.
    VirtualGpu,
    /// Software rendering, such as lavapipe or WARP.
    Cpu,
    Other,
}

impl DeviceType {
    /// `integrated-gpu`, `discrete-gpu`, `virtual-gpu`, `cpu` or `other`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IntegratedGpu => "integrated-gpu",
            Self::DiscreteGpu => "discrete-gpu",
            Self::VirtualGpu => "virtual-gpu",
            Self::Cpu => "cpu",
            Self::Other => "other",
        }
    }
}

pub(crate) fn backend(backend: wgpu::Backend) -> Backend {
    match backend {
        wgpu::Backend::Vulkan => Backend::Vulkan,
        wgpu::Backend::Metal => Backend::Metal,
        wgpu::Backend::Dx12 => Backend::Dx12,
        _ => Backend::Other,
    }
}

pub(crate) fn device_type(device_type: wgpu::DeviceType) -> DeviceType {
    match device_type {
        wgpu::DeviceType::IntegratedGpu => DeviceType::IntegratedGpu,
        wgpu::DeviceType::DiscreteGpu => DeviceType::DiscreteGpu,
        wgpu::DeviceType::VirtualGpu => DeviceType::VirtualGpu,
        wgpu::DeviceType::Cpu => DeviceType::Cpu,
        wgpu::DeviceType::Other => DeviceType::Other,
    }
}

/// One adapter in a message: `name (backend, device type)`.
pub(crate) fn describe(info: &wgpu::AdapterInfo) -> String {
    format!(
        "{} ({}, {})",
        info.name,
        backend(info.backend).as_str(),
        device_type(info.device_type).as_str()
    )
}

pub(crate) fn report(info: &wgpu::AdapterInfo, features: wgpu::Features) -> GpuReport {
    GpuReport {
        adapter: info.name.clone(),
        backend: backend(info.backend),
        device_type: device_type(info.device_type),
        vendor: info.vendor,
        device: info.device,
        driver: info.driver.clone(),
        driver_info: info.driver_info.clone(),
        tier: 0,
        capabilities: Capabilities {
            timestamp_queries: features.contains(wgpu::Features::TIMESTAMP_QUERY),
            binding_arrays: features.contains(wgpu::Features::TEXTURE_BINDING_ARRAY),
            multi_draw_indirect_count: features.contains(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT),
            ray_query: features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY),
            mesh_shaders: features.contains(wgpu::Features::EXPERIMENTAL_MESH_SHADER),
        },
    }
}
