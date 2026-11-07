//! The crate's error type.

use std::fmt;

use eng_gpu::GpuError;

/// Why a frame was not rendered. Each message starts with a log tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The GPU side failed (`[E-GPU]`, `[E-SHADER]`, `[E-GRAPH]`).
    Gpu(GpuError),
    /// The scene cannot be drawn: a value that is not finite, a rectangle
    /// inside out, a camera that shows nothing, or a disc too large for
    /// f32 clip space; or a first-person world or scene that cannot be
    /// drawn, its message naming the chunk at fault. `shape` is the shape's
    /// index in [`crate::TopDownScene::shapes`], if a shape is at fault.
    /// `[E-SCENE]`
    Scene {
        shape: Option<usize>,
        problem: String,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gpu(error) => error.fmt(f),
            Self::Scene {
                shape: Some(index),
                problem,
            } => write!(f, "[E-SCENE] shape {index}: {problem}"),
            Self::Scene {
                shape: None,
                problem,
            } => write!(f, "[E-SCENE] {problem}"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<GpuError> for RenderError {
    fn from(error: GpuError) -> Self {
        Self::Gpu(error)
    }
}
