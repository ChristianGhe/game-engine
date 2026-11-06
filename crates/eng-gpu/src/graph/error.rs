//! What can be wrong with a graph's declarations, or with the passes given to
//! it. Every message starts with `[E-GRAPH]` and names the passes.

use std::fmt;

use super::{PassDecl, Resource};

/// A graph whose declarations do not fit together, or passes that do not
/// match their graph. Rung 2 added variants, and more may come, so a match
/// outside eng-gpu needs a wildcard arm.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
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
    /// [`crate::Graph::render`] was given another number of passes.
    PassCount {
        expected: usize,
        found: usize,
    },
    /// A pass given to [`crate::Graph::render`] declares other reads or
    /// writes than the graph has at its position.
    PassMismatch {
        index: usize,
        expected: PassDecl,
        found: PassDecl,
    },
    /// Two passes of [`crate::Graph::sorted`] share a name.
    DuplicateName {
        pass: &'static str,
    },
    /// A pass continues one that is not declared.
    UnknownContinued {
        pass: &'static str,
        continues: &'static str,
    },
    /// A pass of [`crate::Graph::sorted`] writes nothing.
    WritesNothing {
        pass: &'static str,
    },
    /// A pass of [`crate::Graph::sorted`] lists one write twice.
    WritesTwice {
        pass: &'static str,
        resource: Resource,
    },
    /// A pass writes `resource`, which the pass it continues does not.
    ContinuesOther {
        pass: &'static str,
        resource: Resource,
        continues: &'static str,
    },
    /// A pass continues one that may be bypassed.
    ContinuesBypassable {
        pass: &'static str,
        continues: &'static str,
    },
    /// A pass marked bypassable that cannot be: `reason` says why.
    Unbypassable {
        pass: &'static str,
        reason: &'static str,
    },
    /// A bypassable pass whose write differs from its input in format or
    /// size, so it could not take the input's texture.
    BypassMismatch {
        pass: &'static str,
        input: Resource,
        output: Resource,
    },
    /// [`crate::Graph::sorted_without`] switches off a pass that is not
    /// declared.
    UnknownSwitchedOff {
        pass: &'static str,
    },
    /// [`crate::Graph::sorted_without`] switches off a pass that may not be
    /// bypassed.
    NotBypassable {
        pass: &'static str,
    },
    /// A pass of [`crate::Graph::sorted`] reads what no pass writes.
    Unwritten {
        pass: &'static str,
        resource: Resource,
    },
    /// Two passes write one resource and neither continues the other;
    /// `first` and `second` in name order.
    TwoWriters {
        resource: Resource,
        first: &'static str,
        second: &'static str,
    },
    /// Passes that must each run after the one before, the first after the
    /// last: from the smallest name, in that order.
    Cycle {
        passes: Vec<&'static str>,
    },
    /// Two transients of one name differ in format or size.
    TransientMismatch {
        name: &'static str,
    },
    /// A transient whose divisor is 0.
    Divisor {
        name: &'static str,
    },
    /// A transient named `scene`, `output` or `surface`.
    TransientName {
        name: &'static str,
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
            Self::DuplicateName { pass } => {
                write!(f, "two passes are named `{pass}`; each needs its own name")
            }
            Self::UnknownContinued { pass, continues } => write!(
                f,
                "pass `{pass}` continues `{continues}`, but no pass has that name"
            ),
            Self::WritesNothing { pass } => write!(
                f,
                "pass `{pass}` writes nothing; a pass writes at least one resource"
            ),
            Self::WritesTwice { pass, resource } => write!(
                f,
                "pass `{pass}` lists `{}` among its writes twice",
                resource.name()
            ),
            Self::ContinuesOther {
                pass,
                resource,
                continues,
            } => write!(
                f,
                "pass `{pass}` writes `{}` but continues `{continues}`, which does not; \
                 a pass writes only what the pass it continues writes",
                resource.name()
            ),
            Self::ContinuesBypassable { pass, continues } => write!(
                f,
                "pass `{pass}` continues `{continues}`, which may be bypassed; \
                 a pass that may be bypassed is never continued"
            ),
            Self::Unbypassable { pass, reason } => {
                write!(f, "pass `{pass}` may be bypassed, but {reason}")
            }
            Self::BypassMismatch {
                pass,
                input,
                output,
            } => write!(
                f,
                "pass `{pass}` may be bypassed, but `{}` differs from its input `{}` in format \
                 or size, so it cannot take the input's texture",
                output.name(),
                input.name()
            ),
            Self::UnknownSwitchedOff { pass } => {
                write!(
                    f,
                    "pass `{pass}` is switched off, but no pass has that name"
                )
            }
            Self::NotBypassable { pass } => write!(
                f,
                "pass `{pass}` is switched off, but it may not be bypassed"
            ),
            Self::Unwritten { pass, resource } => write!(
                f,
                "pass `{pass}` reads `{}`, which no pass writes",
                resource.name()
            ),
            Self::TwoWriters {
                resource,
                first,
                second,
            } => write!(
                f,
                "passes `{first}` and `{second}` both write `{}`, and neither continues the other",
                resource.name()
            ),
            Self::Cycle { passes } => match passes.as_slice() {
                [pass] => write!(
                    f,
                    "pass `{pass}` would have to run after itself: it reads or continues what it writes"
                ),
                _ => {
                    let quoted: Vec<String> = passes.iter().map(|p| format!("`{p}`")).collect();
                    write!(
                        f,
                        "passes {} form a cycle: each must run after the one before it, and {} after {}",
                        quoted.join(", "),
                        quoted[0],
                        quoted[quoted.len() - 1]
                    )
                }
            },
            Self::TransientMismatch { name } => write!(
                f,
                "two transients are named `{name}` but differ in format or size"
            ),
            Self::Divisor { name } => write!(
                f,
                "transient `{name}` divides the targets' size by 0; 1 is full size"
            ),
            Self::TransientName { name } => write!(
                f,
                "a transient is named `{name}`, like a built-in resource; give it another name"
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
