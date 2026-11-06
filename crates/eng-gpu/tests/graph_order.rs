//! Ordering and culling in [`Graph::sorted`] (rung 2, queue item Q1; ADR-0019,
//! proposed): passes sorted by what they read, write and continue, names
//! breaking ties, whatever order they are declared in; `[E-GRAPH]` for a
//! cycle or for two writers of one resource; and the passes whose writes
//! reach neither the output nor the surface dropped. No GPU is needed.

use eng_gpu::{
    DRAW, Format, Graph, GraphError, PRESENT, PassDecl, PassNode, Resource, TONEMAP, Target,
    Transient,
};

const LIT: Transient = Transient::new("lit", Format::Rgba16Float);
const FOGGED: Transient = Transient::new("fogged", Format::Rgba16Float);
const DEBUG: Transient = Transient::new("debug", Format::Rgba16Float);

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
    }
}

/// Something like the first-person graph of the rung 2 plan, cut to single
/// writes: two passes on the scene, light, a sky drawn on top of it, fog,
/// the tone map and present, and a debug view that nothing reads.
const FIRST_PERSON: [PassNode; 8] = [
    node("opaque", &[], &[Resource::Scene], None),
    node("sprites", &[], &[Resource::Scene], Some("opaque")),
    node(
        "light",
        &[Resource::Scene],
        &[Resource::Transient(LIT)],
        None,
    ),
    node("sky", &[], &[Resource::Transient(LIT)], Some("light")),
    node(
        "fog",
        &[Resource::Transient(LIT)],
        &[Resource::Transient(FOGGED)],
        None,
    ),
    node(
        "tonemap",
        &[Resource::Transient(FOGGED)],
        &[Resource::Output],
        None,
    ),
    node("present", &[Resource::Output], &[Resource::Surface], None),
    node(
        "debug",
        &[Resource::Scene],
        &[Resource::Transient(DEBUG)],
        None,
    ),
];

fn names(graph: &Graph) -> Vec<&'static str> {
    graph.passes().iter().map(|p| p.name).collect()
}

/// Every order of `items` (Heap's algorithm), the first as given.
fn permutations<T: Copy>(items: &[T]) -> Vec<Vec<T>> {
    fn heap<T: Copy>(k: usize, items: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if k <= 1 {
            out.push(items.clone());
            return;
        }
        heap(k - 1, items, out);
        for i in 0..k - 1 {
            if k.is_multiple_of(2) {
                items.swap(i, k - 1);
            } else {
                items.swap(0, k - 1);
            }
            heap(k - 1, items, out);
        }
    }
    let mut out = Vec::new();
    heap(items.len(), &mut items.to_vec(), &mut out);
    out
}

fn sorted(nodes: &[PassNode], target: Target) -> Graph {
    Graph::sorted(nodes, target).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_permutations_are_all_there() {
    let all = permutations(&[1, 2, 3, 4]);
    assert_eq!(all.len(), 24);
    let mut unique = all.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 24);
}

#[test]
fn any_order_of_declaration_gives_one_order() {
    let reference = sorted(&FIRST_PERSON, Target::Surface);
    assert_eq!(
        names(&reference),
        [
            "opaque", "sprites", "light", "sky", "fog", "tonemap", "present"
        ]
    );
    assert_eq!(reference.culled(), ["debug"]);
    let all = permutations(&FIRST_PERSON);
    assert_eq!(all.len(), 40_320);
    for nodes in all {
        let graph = sorted(&nodes, Target::Surface);
        assert_eq!(graph.passes(), reference.passes());
        assert_eq!(graph.culled(), reference.culled());
        assert_eq!(graph.pool(), reference.pool());
    }
}

#[test]
fn rung_1b_passes_in_any_order_sort_to_the_fixed_order() {
    for nodes in permutations(&[DRAW.node(), TONEMAP.node(), PRESENT.node()]) {
        let graph = sorted(&nodes, Target::Surface);
        assert_eq!(graph.passes(), Graph::fixed(Target::Surface).passes());
        assert!(graph.culled().is_empty());
        assert!(graph.pool().is_empty());
    }
    for nodes in permutations(&[DRAW.node(), TONEMAP.node()]) {
        let graph = sorted(&nodes, Target::Offscreen);
        assert_eq!(graph.passes(), Graph::fixed(Target::Offscreen).passes());
    }
    // Declared in 1b's order, it is the fixed graph, down to the order
    // `render` takes the passes in.
    assert_eq!(
        Graph::sorted(
            &[DRAW.node(), TONEMAP.node(), PRESENT.node()],
            Target::Surface
        ),
        Ok(Graph::fixed(Target::Surface))
    );
    assert_eq!(
        Graph::sorted(&[DRAW.node(), TONEMAP.node()], Target::Offscreen),
        Ok(Graph::fixed(Target::Offscreen))
    );
}

#[test]
fn names_break_ties_between_passes_free_to_run() {
    const A: Transient = Transient::new("a", Format::Rgba16Float);
    const B: Transient = Transient::new("b", Format::Rgba16Float);
    let nodes = [
        node(
            "join",
            &[Resource::Transient(B), Resource::Transient(A)],
            &[Resource::Output],
            None,
        ),
        node("zebra", &[], &[Resource::Transient(A)], None),
        node("apple", &[], &[Resource::Transient(B)], None),
    ];
    assert_eq!(
        names(&sorted(&nodes, Target::Offscreen)),
        ["apple", "zebra", "join"]
    );
}

#[test]
fn a_continuing_pass_runs_after_the_one_it_continues_and_before_readers() {
    // "a_decals" sorts first by name, but continues "z_walls"; "m_sprites"
    // continues the decals, and the tone map reads the scene after all
    // three.
    let nodes = [
        node("tonemap", &[Resource::Scene], &[Resource::Output], None),
        node("a_decals", &[], &[Resource::Scene], Some("z_walls")),
        node("m_sprites", &[], &[Resource::Scene], Some("a_decals")),
        node("z_walls", &[], &[Resource::Scene], None),
    ];
    for nodes in permutations(&nodes) {
        assert_eq!(
            names(&sorted(&nodes, Target::Offscreen)),
            ["z_walls", "a_decals", "m_sprites", "tonemap"]
        );
    }
}

#[test]
fn a_cycle_names_its_passes() {
    const A: Transient = Transient::new("a", Format::Rgba16Float);
    const B: Transient = Transient::new("b", Format::Rgba16Float);
    let nodes = [
        node(
            "tonemap",
            &[Resource::Transient(A)],
            &[Resource::Output],
            None,
        ),
        node(
            "one",
            &[Resource::Transient(B)],
            &[Resource::Transient(A)],
            None,
        ),
        node(
            "two",
            &[Resource::Transient(A)],
            &[Resource::Transient(B)],
            None,
        ),
    ];
    for nodes in permutations(&nodes) {
        let e = Graph::sorted(&nodes, Target::Offscreen).unwrap_err();
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
}

#[test]
fn a_longer_cycle_reads_in_running_order() {
    const A: Transient = Transient::new("a", Format::Rgba16Float);
    const B: Transient = Transient::new("b", Format::Rgba16Float);
    const C: Transient = Transient::new("c", Format::Rgba16Float);
    let nodes = [
        node(
            "x",
            &[Resource::Transient(C)],
            &[Resource::Transient(A)],
            None,
        ),
        node(
            "y",
            &[Resource::Transient(A)],
            &[Resource::Transient(B)],
            None,
        ),
        node(
            "w",
            &[Resource::Transient(B)],
            &[Resource::Transient(C)],
            None,
        ),
        node("out", &[Resource::Transient(B)], &[Resource::Output], None),
    ];
    // x before y before w before x again, from the smallest name.
    assert_eq!(
        Graph::sorted(&nodes, Target::Offscreen),
        Err(GraphError::Cycle {
            passes: vec!["w", "x", "y"]
        })
    );
}

#[test]
fn a_pass_cannot_read_or_continue_what_it_writes() {
    let feedback = node("feedback", &[Resource::Scene], &[Resource::Scene], None);
    let e = Graph::sorted(&[feedback, TONEMAP.node()], Target::Offscreen).unwrap_err();
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
        Graph::sorted(&[itself, TONEMAP.node()], Target::Offscreen),
        Err(GraphError::Cycle {
            passes: vec!["itself"]
        })
    );
    // Two passes that continue each other, with nothing to start the scene.
    let one = node("one", &[], &[Resource::Scene], Some("two"));
    let two = node("two", &[], &[Resource::Scene], Some("one"));
    assert_eq!(
        Graph::sorted(&[TONEMAP.node(), one, two], Target::Offscreen),
        Err(GraphError::Cycle {
            passes: vec!["one", "two"]
        })
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
    for nodes in permutations(&[DRAW.node(), TONEMAP.node(), AGAIN.node()]) {
        let e = Graph::sorted(&nodes, Target::Offscreen).unwrap_err();
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
    }
    // Two passes continuing one: the chain forks.
    let nodes = [
        DRAW.node(),
        node("left", &[], &[Resource::Scene], Some("draw")),
        node("right", &[], &[Resource::Scene], Some("draw")),
        TONEMAP.node(),
    ];
    assert_eq!(
        Graph::sorted(&nodes, Target::Offscreen),
        Err(GraphError::TwoWriters {
            resource: Resource::Scene,
            first: "left",
            second: "right",
        })
    );
}

#[test]
fn a_continued_pass_exists_and_writes_the_same_resource() {
    let lost = node("lost", &[], &[Resource::Scene], Some("nowhere"));
    let e = Graph::sorted(&[lost, TONEMAP.node()], Target::Offscreen).unwrap_err();
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
    let e = Graph::sorted(&[DRAW.node(), over], Target::Offscreen).unwrap_err();
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `over` writes `output` but continues `draw`, which writes `scene`; \
         a pass continues one that writes the same resource"
    );
}

#[test]
fn the_declarations_are_checked_whatever_their_order() {
    let fail = |nodes: &[PassNode], target| Graph::sorted(nodes, target).unwrap_err();
    assert_eq!(
        fail(&[TONEMAP.node()], Target::Offscreen),
        GraphError::Unwritten {
            pass: "tonemap",
            resource: Resource::Scene,
        }
    );
    assert_eq!(
        fail(&[TONEMAP.node()], Target::Offscreen).to_string(),
        "[E-GRAPH] pass `tonemap` reads `scene`, which no pass writes"
    );
    assert_eq!(
        fail(
            &[DRAW.node(), DRAW.node(), TONEMAP.node()],
            Target::Offscreen
        ),
        GraphError::DuplicateName { pass: "draw" }
    );
    let nothing = node("nothing", &[Resource::Scene], &[], None);
    assert_eq!(
        fail(&[DRAW.node(), nothing, TONEMAP.node()], Target::Offscreen),
        GraphError::Writes {
            pass: "nothing",
            count: 0,
        }
    );
    assert_eq!(
        fail(
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
        fail(&[DRAW.node(), direct], Target::Surface),
        GraphError::OutputIsSurface { pass: "tonemap" }
    );
    assert_eq!(
        fail(&[DRAW.node()], Target::Offscreen),
        GraphError::OutputNotWritten
    );
    assert_eq!(
        fail(&[DRAW.node(), TONEMAP.node()], Target::Surface),
        GraphError::SurfaceNotWritten
    );
}

#[test]
fn a_pass_whose_writes_reach_neither_output_nor_surface_is_culled() {
    let graph = sorted(&FIRST_PERSON, Target::Surface);
    assert_eq!(graph.culled(), ["debug"]);
    assert!(!names(&graph).contains(&"debug"));
    // A chain of passes that leads nowhere goes whole, in running order.
    const SEEN: Transient = Transient::new("seen", Format::Rgba16Float);
    const HEARD: Transient = Transient::new("heard", Format::Rgba16Float);
    let nodes = [
        node(
            "b_listen",
            &[Resource::Transient(SEEN)],
            &[Resource::Transient(HEARD)],
            None,
        ),
        node(
            "a_look",
            &[Resource::Scene],
            &[Resource::Transient(SEEN)],
            None,
        ),
        DRAW.node(),
        TONEMAP.node(),
    ];
    let graph = sorted(&nodes, Target::Offscreen);
    assert_eq!(names(&graph), ["draw", "tonemap"]);
    assert_eq!(graph.culled(), ["a_look", "b_listen"]);
    // Writing the output keeps a pass, and what it reads, even when nothing
    // reads the output after it; in a surface graph presenting does too.
    let graph = sorted(&[DRAW.node(), TONEMAP.node()], Target::Offscreen);
    assert!(graph.culled().is_empty());
    let graph = sorted(
        &[DRAW.node(), TONEMAP.node(), PRESENT.node()],
        Target::Surface,
    );
    assert!(graph.culled().is_empty());
}

#[test]
fn a_culled_pass_is_still_checked() {
    let broken = node(
        "broken",
        &[Resource::Transient(LIT)],
        &[Resource::Transient(DEBUG)],
        None,
    );
    assert_eq!(
        Graph::sorted(&[DRAW.node(), TONEMAP.node(), broken], Target::Offscreen),
        Err(GraphError::Unwritten {
            pass: "broken",
            resource: Resource::Transient(LIT),
        })
    );
}

#[test]
fn graph_new_culls_nothing() {
    const UNREAD: PassDecl = PassDecl {
        name: "unread",
        reads: &[Resource::Scene],
        writes: &[Resource::Transient(DEBUG)],
    };
    let graph =
        Graph::new(&[DRAW, UNREAD, TONEMAP], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(names(&graph), ["draw", "unread", "tonemap"]);
    assert!(graph.culled().is_empty());
    assert!(Graph::fixed(Target::Surface).culled().is_empty());
}
