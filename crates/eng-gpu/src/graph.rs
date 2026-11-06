//! The render graph (spec 6.3, 6.7; ADR-0003, ADR-0017 and ADR-0019, both
//! proposed): passes declare the virtual resources they read and write, and
//! building a graph checks the declarations.
//!
//! - [`Graph::new`] and [`Graph::fixed`] are rung 1b's: the passes run in
//!   the order given, each read after an earlier write.
//! - [`Graph::sorted`] is rung 2's: it orders [`PassNode`]s itself
//!   (`order.rs`), drops the passes whose writes reach neither the output nor
//!   the surface (`cull.rs`), and plans the transient textures (`pool.rs`).
//!
//! Both kinds of graph render the same way ([`Graph::render`]).

mod cull;
mod error;
mod order;
mod pool;

pub use error::GraphError;
pub(crate) use pool::Plan;
pub use pool::PooledTexture;

/// A virtual resource that passes read and write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resource {
    /// The scene: linear, scene-referred HDR colour (`Rgba16Float`).
    Scene,
    /// The tone-mapped image (`Rgba8UnormSrgb`). It is always an
    /// engine-owned texture, never the surface: screenshots and golden tests
    /// read it (spec 6.7).
    Output,
    /// The window's swapchain image, or an offscreen texture standing in for
    /// it ([`crate::Targets::with_offscreen_surface`]), in `Bgra8UnormSrgb`.
    /// Only a [`Target::Surface`] graph has one.
    Surface,
    /// A texture that lives within one frame, from the pass that first writes
    /// it to the last pass that reads it. The graph plans where it lives
    /// ([`Graph::pool`]), and the targets keep those textures from frame to
    /// frame ([`crate::Targets::transient_textures`]).
    Transient(Transient),
}

impl Resource {
    /// The name used in messages and reports.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Output => "output",
            Self::Surface => "surface",
            Self::Transient(transient) => transient.name,
        }
    }
}

/// The formats a [`Transient`] can have: the scene's and the output's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    /// Linear, scene-referred HDR colour, as the scene.
    Rgba16Float,
    /// 8-bit colour, sRGB-encoded on write, as the output.
    Rgba8UnormSrgb,
}

/// A transient texture, declared where passes read and write it as
/// [`Resource::Transient`]. Its name identifies it: a graph refuses two
/// transients of one name that differ in format or size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Transient {
    pub name: &'static str,
    pub format: Format,
    /// The targets' width and height are divided by this, rounding up: 1 is
    /// full size, 2 half. At least 1.
    pub divisor: u32,
}

impl Transient {
    /// A transient of the targets' size.
    pub const fn new(name: &'static str, format: Format) -> Self {
        Self {
            name,
            format,
            divisor: 1,
        }
    }

    /// The same transient at 1/`divisor` of the targets' size.
    pub const fn divided(self, divisor: u32) -> Self {
        Self {
            name: self.name,
            format: self.format,
            divisor,
        }
    }
}

/// What a pass reads and writes, in the order [`Graph::new`] runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassDecl {
    pub name: &'static str,
    /// Each must be written by an earlier pass. A fullscreen pass draws from
    /// its one read ([`crate::PassRecorder::fullscreen`]).
    pub reads: &'static [Resource],
    /// Exactly one: the pass's colour target.
    pub writes: &'static [Resource],
}

impl PassDecl {
    /// The same pass for [`Graph::sorted`], continuing no other.
    pub const fn node(self) -> PassNode {
        PassNode {
            name: self.name,
            reads: self.reads,
            writes: self.writes,
            continues: None,
        }
    }
}

/// A pass as [`Graph::sorted`] takes it: what it reads and writes, and the
/// earlier pass whose target it draws on top of, if any. Its order comes
/// from these, not from where it is declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassNode {
    /// Unique within a graph; it breaks ties in the order.
    pub name: &'static str,
    /// Each is read after every pass that writes it. A fullscreen pass draws
    /// from its one read ([`crate::PassRecorder::fullscreen`]).
    pub reads: &'static [Resource],
    /// Exactly one: the pass's colour target. Only one pass may start a
    /// resource; any other pass that writes it continues the one before.
    pub writes: &'static [Resource],
    /// The pass whose target this one draws on top of. It must write the
    /// same resource; this pass runs after it, loads the target instead of
    /// clearing it, and the resource's readers run after both. A pass can be
    /// continued by one pass only, so the passes that write a resource form
    /// one chain. With `None` the pass clears its target.
    pub continues: Option<&'static str>,
}

impl PassNode {
    /// Its name, reads and writes: what its [`crate::Pass::decl`] returns.
    pub const fn decl(&self) -> PassDecl {
        PassDecl {
            name: self.name,
            reads: self.reads,
            writes: self.writes,
        }
    }
}

/// Draws the scene. In 1b it is eng-render's top-down pass.
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
    /// The passes that run, in order.
    pub(crate) passes: Vec<PassDecl>,
    target: Target,
    /// Every pass as declared, culled ones too: what [`Graph::render`] takes,
    /// in this order.
    pub(crate) declared: Vec<PassDecl>,
    /// For each pass that runs, its place in `declared`.
    pub(crate) slots: Vec<usize>,
    /// For each pass that runs, whether it continues an earlier pass, and so
    /// loads its target rather than clearing it.
    pub(crate) loads: Vec<bool>,
    culled: Vec<&'static str>,
    pub(crate) plan: Plan,
}

impl Graph {
    /// Checks `passes`, in the order they will run:
    /// - each read was written by an earlier pass;
    /// - each pass writes exactly one resource;
    /// - the output is written, never by the pass that writes the surface;
    /// - an offscreen graph never uses the surface, and a surface graph
    ///   writes it;
    /// - transients of one name agree, and none is divided by 0.
    ///
    /// Each pass clears its target, and none is culled. Transients are
    /// pooled as in [`Graph::sorted`].
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
            declared: passes.to_vec(),
            slots: (0..passes.len()).collect(),
            loads: vec![false; passes.len()],
            culled: Vec::new(),
            plan: Plan::new(passes)?,
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

    /// Orders `nodes` by what they read, write and continue, whatever order
    /// they are declared in, then culls and pools (ADR-0019, proposed):
    /// - **Order:** a pass runs after the pass it continues, and after every
    ///   pass that writes what it reads. Among passes free to run, the
    ///   smallest name goes first, as eng-sim orders systems, so any
    ///   permutation of the same nodes gives the same order.
    /// - **Checks**, each failing with `[E-GRAPH]` and naming the passes:
    ///   unique names; one write each; a continued pass that exists and
    ///   writes the same resource; every read written by some pass; one
    ///   chain of writers per resource (two passes that write it with
    ///   neither continuing the other fail); no cycle; and [`Graph::new`]'s
    ///   rules on the output and the surface.
    /// - **Culling:** a pass whose writes reach neither the output nor the
    ///   surface is dropped, and [`Graph::culled`] names it.
    /// - **Pooling:** transients share a texture when their format, size
    ///   and usage match and their lifetimes within the frame do not overlap
    ///   ([`Graph::pool`]).
    pub fn sorted(nodes: &[PassNode], target: Target) -> Result<Self, GraphError> {
        let sorted = order::sort(nodes, target)?;
        let keep = cull::kept(nodes, &sorted.after);
        let (run, culled): (Vec<usize>, Vec<usize>) =
            sorted.order.iter().partition(|&&index| keep[index]);
        let passes: Vec<PassDecl> = run.iter().map(|&index| nodes[index].decl()).collect();
        let plan = Plan::new(&passes)?;
        Ok(Self {
            passes,
            target,
            declared: nodes.iter().map(PassNode::decl).collect(),
            loads: run
                .iter()
                .map(|&index| nodes[index].continues.is_some())
                .collect(),
            slots: run,
            culled: culled.iter().map(|&index| nodes[index].name).collect(),
            plan,
        })
    }

    /// The passes that run, in order.
    pub fn passes(&self) -> &[PassDecl] {
        &self.passes
    }

    pub fn target(&self) -> Target {
        self.target
    }

    /// The declared passes that do not run, because their writes reach
    /// neither the output nor the surface, in the order they would have run.
    /// Empty for a [`Graph::new`] graph.
    pub fn culled(&self) -> &[&'static str] {
        &self.culled
    }

    /// The transient textures a frame needs, in the order it first uses
    /// them, and the transients drawn into each.
    pub fn pool(&self) -> &[PooledTexture] {
        self.plan.textures()
    }
}
