//! The fixed render graph (spec 6.3, 6.7; rung 1b plan, decision 1): passes
//! declare the virtual resources they read and write, and building a graph
//! checks the declarations. Ordering, culling and pooling come in rung 2.

use std::fmt;

/// A virtual resource that passes read and write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resource {
    /// The scene: linear, scene-referred HDR colour (`Rgba16Float`).
    Scene,
    /// The tone-mapped image (`Rgba8UnormSrgb`). It is always an
    /// engine-owned texture, never the surface: screenshots and golden tests
    /// read it (spec 6.7).
    Output,
    /// The window's swapchain image, in the surface's preferred format. Only
    /// a [`Target::Surface`] graph has one.
    Surface,
}

impl Resource {
    /// The name used in messages and reports.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Output => "output",
            Self::Surface => "surface",
        }
    }
}

/// What a pass reads and writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassDecl {
    pub name: &'static str,
    /// Each must be written by an earlier pass. A fullscreen pass draws from
    /// its one read ([`crate::PassRecorder::fullscreen`]).
    pub reads: &'static [Resource],
    /// Exactly one: the pass's colour target.
    pub writes: &'static [Resource],
}

/// Draws the scene. In 1b eng-render's top-down pass goes here (task T8).
pub const DRAW: PassDecl = PassDecl {
    name: "draw",
    reads: &[],
    writes: &[Resource::Scene],
};

/// Maps the scene into the output. In 1b the map is a clamp to [0, 1]
/// (plan decision 2); the output's sRGB format does the encoding.
pub const TONEMAP: PassDecl = PassDecl {
    name: "tonemap",
    reads: &[Resource::Scene],
    writes: &[Resource::Output],
};

/// Blits the output into the surface's format, rather than copying, because
/// swapchain textures cannot be relied on for copies (spec 6.7).
pub const PRESENT: PassDecl = PassDecl {
    name: "present",
    reads: &[Resource::Output],
    writes: &[Resource::Surface],
};

/// Where a graph's frames go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Engine-owned textures only: `shot`, `video` and tests.
    Offscreen,
    /// A window: the output is also presented.
    Surface,
}

/// A checked list of passes, run in order by [`Graph::render`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    pub(crate) passes: Vec<PassDecl>,
    target: Target,
}

impl Graph {
    /// Checks `passes`, in the order they will run:
    /// - each read was written by an earlier pass;
    /// - each pass writes exactly one resource;
    /// - the output is written, never by the pass that writes the surface;
    /// - an offscreen graph never uses the surface, and a surface graph
    ///   writes it.
    pub fn new(passes: &[PassDecl], target: Target) -> Result<Self, GraphError> {
        let mut written: Vec<Resource> = Vec::new();
        for pass in passes {
            let uses_surface = pass
                .reads
                .iter()
                .chain(pass.writes)
                .any(|r| *r == Resource::Surface);
            if uses_surface && target == Target::Offscreen {
                return Err(GraphError::SurfaceOffscreen { pass: pass.name });
            }
            if let Some(&resource) = pass.reads.iter().find(|r| !written.contains(r)) {
                return Err(GraphError::ReadBeforeWrite {
                    pass: pass.name,
                    resource,
                });
            }
            // Before the count, so that the error names this rule: only a pass
            // with two writes can break it while each pass writes one.
            if pass.writes.contains(&Resource::Output) && pass.writes.contains(&Resource::Surface) {
                return Err(GraphError::OutputIsSurface { pass: pass.name });
            }
            if pass.writes.len() != 1 {
                return Err(GraphError::Writes {
                    pass: pass.name,
                    count: pass.writes.len(),
                });
            }
            written.extend_from_slice(pass.writes);
        }
        if !written.contains(&Resource::Output) {
            return Err(GraphError::OutputNotWritten);
        }
        if target == Target::Surface && !written.contains(&Resource::Surface) {
            return Err(GraphError::SurfaceNotWritten);
        }
        Ok(Self {
            passes: passes.to_vec(),
            target,
        })
    }

    /// Rung 1b's graph: [`DRAW`], [`TONEMAP`], then [`PRESENT`] for a surface.
    pub fn fixed(target: Target) -> Self {
        let passes: &[PassDecl] = match target {
            Target::Offscreen => &[DRAW, TONEMAP],
            Target::Surface => &[DRAW, TONEMAP, PRESENT],
        };
        Self::new(passes, target).expect("the fixed graph passes its own checks")
    }

    pub fn passes(&self) -> &[PassDecl] {
        &self.passes
    }

    pub fn target(&self) -> Target {
        self.target
    }
}

/// A graph whose declarations do not fit together, or passes that do not
/// match their graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    ReadBeforeWrite {
        pass: &'static str,
        resource: Resource,
    },
    Writes {
        pass: &'static str,
        count: usize,
    },
    OutputIsSurface {
        pass: &'static str,
    },
    SurfaceOffscreen {
        pass: &'static str,
    },
    OutputNotWritten,
    SurfaceNotWritten,
    /// [`Graph::render`] was given another number of passes.
    PassCount {
        expected: usize,
        found: usize,
    },
    /// A pass given to [`Graph::render`] declares other reads or writes than
    /// the graph has at its position.
    PassMismatch {
        index: usize,
        expected: PassDecl,
        found: PassDecl,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[E-GRAPH] ")?;
        match self {
            Self::ReadBeforeWrite { pass, resource } => write!(
                f,
                "pass `{pass}` reads `{}` before any pass writes it",
                resource.name()
            ),
            Self::Writes { pass, count } => write!(
                f,
                "pass `{pass}` writes {count} resources; a pass writes exactly one"
            ),
            Self::OutputIsSurface { pass } => write!(
                f,
                "pass `{pass}` writes the output into the surface; the output stays an engine-owned texture"
            ),
            Self::SurfaceOffscreen { pass } => write!(
                f,
                "pass `{pass}` uses the surface, but the graph renders offscreen"
            ),
            Self::OutputNotWritten => {
                f.write_str("no pass writes `output`, which screenshots read")
            }
            Self::SurfaceNotWritten => {
                f.write_str("no pass writes `surface`, so nothing would be presented")
            }
            Self::PassCount { expected, found } => write!(
                f,
                "the graph declares {expected} passes, but {found} were given"
            ),
            Self::PassMismatch {
                index,
                expected,
                found,
            } if expected.name != found.name => write!(
                f,
                "pass {index} is `{}`, but the graph declares `{}` there",
                found.name, expected.name
            ),
            Self::PassMismatch {
                index,
                expected,
                found,
            } => write!(
                f,
                "pass {index}, `{}`, reads {} and writes {}, but the graph declares it reading {} and writing {}",
                found.name,
                names(found.reads),
                names(found.writes),
                names(expected.reads),
                names(expected.writes)
            ),
        }
    }
}

impl std::error::Error for GraphError {}

/// Resources as `[scene, output]`.
fn names(resources: &[Resource]) -> String {
    let names: Vec<&str> = resources.iter().map(|r| r.name()).collect();
    format!("[{}]", names.join(", "))
}
