//! The errors of [`Graph::sorted`] (rung 2, queue item Q1; ADR-0019,
//! proposed): each `[E-GRAPH]`, naming the passes, and the same in every
//! order of declaration, also when the declarations hold two errors or two
//! cycles. No GPU is needed.

mod support;

use eng_gpu::{
    DRAW, Format, Graph, GraphError, PRESENT, PassDecl, PassNode, Resource, TONEMAP, Target,
    Transient,
};
use support::error_in_every_order;

const fn transient(name: &'static str) -> Resource {
    Resource::Transient(Transient::new(name, Format::Rgba16Float))
}

const fn node(
    name: &'static str,
    reads: &'static [Resource],
    writes: &'static [Resource],
    continues: Option<&'static str>,
) -> PassNode {
    PassNode {
        name,
        reads,
        writes,
        continues,
        bypassable: false,
    }
}

const A: Resource = transient("a");
const B: Resource = transient("b");
const C: Resource = transient("c");
const D: Resource = transient("d");

fn fails(nodes: &[PassNode], target: Target) -> GraphError {
    error_in_every_order(nodes, &[], target)
}

#[test]
fn a_cycle_names_its_passes() {
    let nodes = [
        node("tonemap", &[A], &[Resource::Output], None),
        node("one", &[B], &[A], None),
        node("two", &[A], &[B], None),
    ];
    let e = fails(&nodes, Target::Offscreen);
    assert_eq!(
        e,
        GraphError::Cycle {
            passes: vec!["one", "two"]
        }
    );
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] passes `one`, `two` form a cycle: each must run after the one before it, \
         and `one` after `two`"
    );
}

#[test]
fn a_longer_cycle_reads_in_running_order() {
    let nodes = [
        node("x", &[C], &[A], None),
        node("y", &[A], &[B], None),
        node("w", &[B], &[C], None),
        node("out", &[B], &[Resource::Output], None),
    ];
    // x before y before w before x again, from the smallest name.
    assert_eq!(
        fails(&nodes, Target::Offscreen),
        GraphError::Cycle {
            passes: vec!["w", "x", "y"]
        }
    );
}

#[test]
fn of_two_cycles_the_one_with_the_smallest_name_is_reported() {
    let nodes = [
        node("p", &[B], &[A], None),
        node("q", &[A], &[B], None),
        node("e", &[D], &[C], None),
        node("f", &[C], &[D], None),
        node("out", &[A, C], &[Resource::Output], None),
    ];
    assert_eq!(
        fails(&nodes, Target::Offscreen),
        GraphError::Cycle {
            passes: vec!["e", "f"]
        }
    );
}

#[test]
fn a_pass_cannot_read_or_continue_what_it_writes() {
    let feedback = node("feedback", &[Resource::Scene], &[Resource::Scene], None);
    let e = fails(&[feedback, TONEMAP.node()], Target::Offscreen);
    assert_eq!(
        e,
        GraphError::Cycle {
            passes: vec!["feedback"]
        }
    );
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `feedback` would have to run after itself: it reads or continues what it writes"
    );
    let itself = node("itself", &[], &[Resource::Scene], Some("itself"));
    assert_eq!(
        fails(&[itself, TONEMAP.node()], Target::Offscreen),
        GraphError::Cycle {
            passes: vec!["itself"]
        }
    );
    // Two passes that continue each other, with nothing to start the scene.
    let one = node("one", &[], &[Resource::Scene], Some("two"));
    let two = node("two", &[], &[Resource::Scene], Some("one"));
    assert_eq!(
        fails(&[TONEMAP.node(), one, two], Target::Offscreen),
        GraphError::Cycle {
            passes: vec!["one", "two"]
        }
    );
}

#[test]
fn two_writers_of_one_resource_need_one_to_continue_the_other() {
    // Graph::new runs both, in order, as rung 1b did (counters.rs relies on
    // it); Graph::sorted could not tell which goes first.
    const AGAIN: PassDecl = PassDecl {
        name: "again",
        reads: &[Resource::Scene],
        writes: &[Resource::Output],
    };
    assert!(Graph::new(&[DRAW, TONEMAP, AGAIN], Target::Offscreen).is_ok());
    let e = fails(
        &[DRAW.node(), TONEMAP.node(), AGAIN.node()],
        Target::Offscreen,
    );
    assert_eq!(
        e,
        GraphError::TwoWriters {
            resource: Resource::Output,
            first: "again",
            second: "tonemap",
        }
    );
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] passes `again` and `tonemap` both write `output`, and neither continues the other"
    );
    // Two passes continuing one, writing the same resource: the chain forks.
    let nodes = [
        DRAW.node(),
        node("left", &[], &[Resource::Scene], Some("draw")),
        node("right", &[], &[Resource::Scene], Some("draw")),
        TONEMAP.node(),
    ];
    assert_eq!(
        fails(&nodes, Target::Offscreen),
        GraphError::TwoWriters {
            resource: Resource::Scene,
            first: "left",
            second: "right",
        }
    );
}

#[test]
fn a_continued_pass_exists_and_writes_all_its_continuer_writes() {
    let lost = node("lost", &[], &[Resource::Scene], Some("nowhere"));
    let e = fails(&[lost, TONEMAP.node()], Target::Offscreen);
    assert_eq!(
        e,
        GraphError::UnknownContinued {
            pass: "lost",
            continues: "nowhere",
        }
    );
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `lost` continues `nowhere`, but no pass has that name"
    );
    let over = node("over", &[], &[Resource::Output], Some("draw"));
    let e = fails(&[DRAW.node(), over], Target::Offscreen);
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `over` writes `output` but continues `draw`, which does not; \
         a pass writes only what the pass it continues writes"
    );
    // More than the continued pass writes, even with all of it.
    let more = node("more", &[], &[Resource::Scene, A], Some("draw"));
    let out = node("out", &[A], &[Resource::Output], None);
    assert_eq!(
        fails(&[DRAW.node(), more, out], Target::Offscreen),
        GraphError::ContinuesOther {
            pass: "more",
            resource: A,
            continues: "draw",
        }
    );
}

#[test]
fn each_declaration_is_checked_in_every_order() {
    let unwritten = fails(&[TONEMAP.node()], Target::Offscreen);
    assert_eq!(
        unwritten,
        GraphError::Unwritten {
            pass: "tonemap",
            resource: Resource::Scene,
        }
    );
    assert_eq!(
        unwritten.to_string(),
        "[E-GRAPH] pass `tonemap` reads `scene`, which no pass writes"
    );
    assert_eq!(
        fails(
            &[DRAW.node(), DRAW.node(), TONEMAP.node()],
            Target::Offscreen
        ),
        GraphError::DuplicateName { pass: "draw" }
    );
    let nothing = node("nothing", &[Resource::Scene], &[], None);
    assert_eq!(
        fails(&[DRAW.node(), nothing, TONEMAP.node()], Target::Offscreen),
        GraphError::WritesNothing { pass: "nothing" }
    );
    let twice = node("twice", &[], &[Resource::Scene, Resource::Scene], None);
    assert_eq!(
        fails(&[twice, TONEMAP.node()], Target::Offscreen),
        GraphError::WritesTwice {
            pass: "twice",
            resource: Resource::Scene,
        }
    );
    assert_eq!(
        fails(
            &[DRAW.node(), TONEMAP.node(), PRESENT.node()],
            Target::Offscreen
        ),
        GraphError::SurfaceOffscreen { pass: "present" }
    );
    let direct = node(
        "tonemap",
        &[Resource::Scene],
        &[Resource::Output, Resource::Surface],
        None,
    );
    assert_eq!(
        fails(&[DRAW.node(), direct], Target::Surface),
        GraphError::OutputIsSurface { pass: "tonemap" }
    );
    assert_eq!(
        fails(&[DRAW.node()], Target::Offscreen),
        GraphError::OutputNotWritten
    );
    assert_eq!(
        fails(&[DRAW.node(), TONEMAP.node()], Target::Surface),
        GraphError::SurfaceNotWritten
    );
    // A culled pass is checked too.
    let broken = node("broken", &[B], &[A], None);
    assert_eq!(
        fails(&[DRAW.node(), TONEMAP.node(), broken], Target::Offscreen),
        GraphError::Unwritten {
            pass: "broken",
            resource: B,
        }
    );
}

#[test]
fn of_two_errors_the_same_is_reported_in_every_order() {
    // Two reads of nothing: the smaller name.
    let x = node("x", &[A], &[B], None);
    let y = node("y", &[C], &[D], None);
    assert_eq!(
        fails(&[y, x, DRAW.node(), TONEMAP.node()], Target::Offscreen),
        GraphError::Unwritten {
            pass: "x",
            resource: A,
        }
    );
    // A read of nothing and two writers: the read, which is checked first.
    let z = node("z", &[], &[Resource::Scene], None);
    assert_eq!(
        fails(&[y, z, DRAW.node(), TONEMAP.node()], Target::Offscreen),
        GraphError::Unwritten {
            pass: "y",
            resource: C,
        }
    );
    // Two writers and a cycle: the writers.
    let p = node("p", &[B], &[A], None);
    let q = node("q", &[A], &[B], None);
    assert_eq!(
        fails(&[p, q, z, DRAW.node(), TONEMAP.node()], Target::Offscreen),
        GraphError::TwoWriters {
            resource: Resource::Scene,
            first: "draw",
            second: "z",
        }
    );
}

#[test]
fn transients_are_checked_in_every_order() {
    const ODD: Resource = Resource::Transient(Transient::new("a", Format::Rgba8UnormSrgb));
    let make = node("make", &[], &[A], None);
    let read = node("read", &[ODD], &[Resource::Output], None);
    let e = fails(&[make, read], Target::Offscreen);
    assert_eq!(e, GraphError::TransientMismatch { name: "a" });
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] two transients are named `a` but differ in format or size"
    );
    const SCENE: Resource = transient("scene");
    let e = fails(
        &[
            node("make", &[], &[SCENE], None),
            node("read", &[SCENE], &[Resource::Output], None),
        ],
        Target::Offscreen,
    );
    assert_eq!(e, GraphError::TransientName { name: "scene" });
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] a transient is named `scene`, like a built-in resource; give it another name"
    );
}

#[test]
fn graph_new_reports_a_transient_of_another_format_as_such() {
    // Read with another format, the transient is another resource, which
    // no earlier pass wrote; the mismatch is the error that explains it.
    const ODD: Resource = Resource::Transient(Transient::new("a", Format::Rgba8UnormSrgb));
    const MAKE: PassDecl = PassDecl {
        name: "make",
        reads: &[],
        writes: &[A],
    };
    const READ: PassDecl = PassDecl {
        name: "read",
        reads: &[ODD],
        writes: &[Resource::Output],
    };
    assert_eq!(
        Graph::new(&[MAKE, READ], Target::Offscreen),
        Err(GraphError::TransientMismatch { name: "a" })
    );
    const SURFACE: PassDecl = PassDecl {
        name: "make",
        reads: &[],
        writes: &[transient("surface")],
    };
    assert_eq!(
        Graph::new(&[SURFACE, DRAW, TONEMAP], Target::Offscreen),
        Err(GraphError::TransientName { name: "surface" })
    );
}
