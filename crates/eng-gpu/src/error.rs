//! The crate's error type.

use std::fmt;

use crate::GraphError;
use crate::targets::MAX_SIDE;

/// What went wrong on the GPU side. Each message starts with a log tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuError {
    /// No adapter, or none whose name contains `wanted` (`ENG_GPU_ADAPTER`).
    /// `found` lists each adapter seen as `name (backend, device type)`.
    /// `[E-NO-ADAPTER]`; the probe exits 64 with it (plan decision 4).
    NoAdapter {
        wanted: Option<String>,
        found: Vec<String>,
    },
    /// The adapter could not open a Tier 0 device. `[E-GPU]`
    Device(String),
    /// A WGSL module or the pipeline made from it did not compile.
    /// `[E-SHADER]`
    Shader { label: String, message: String },
    /// wgpu reported an error while rendering or reading back: validation,
    /// out of memory or internal. `[E-GPU]`
    Wgpu(String),
    /// The graph, or the passes given to it. `[E-GRAPH]`
    Graph(GraphError),
    /// A pass recorded something its declaration does not allow. `[E-GPU]`
    Pass { pass: &'static str, message: String },
    /// A target side outside 1 to 8,192, Tier 0's largest texture. `[E-GPU]`
    Size { width: u32, height: u32 },
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter { wanted, found } => {
                match wanted {
                    Some(name) => write!(
                        f,
                        "[E-NO-ADAPTER] no GPU adapter's name contains \"{name}\" (ENG_GPU_ADAPTER)"
                    )?,
                    None => f.write_str("[E-NO-ADAPTER] no GPU adapter")?,
                }
                if found.is_empty() {
                    f.write_str("; none found")
                } else {
                    write!(f, "; found: {}", found.join("; "))
                }
            }
            Self::Device(message) => write!(f, "[E-GPU] cannot open a Tier 0 device: {message}"),
            Self::Shader { label, message } => write!(f, "[E-SHADER] {label}: {message}"),
            Self::Wgpu(message) => write!(f, "[E-GPU] {message}"),
            Self::Graph(error) => error.fmt(f),
            Self::Pass { pass, message } => write!(f, "[E-GPU] pass `{pass}`: {message}"),
            Self::Size { width, height } => write!(
                f,
                "[E-GPU] target size {width}x{height}: each side must be 1 to {MAX_SIDE} (Tier 0)"
            ),
        }
    }
}

impl std::error::Error for GpuError {}

impl From<GraphError> for GpuError {
    fn from(error: GraphError) -> Self {
        Self::Graph(error)
    }
}
