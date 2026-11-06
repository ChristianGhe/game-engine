//! Ordering and culling in [`Graph::sorted`] (rung 2, queue item Q1; ADR-0019,
//! proposed): passes sorted by what they read, write and continue, names
//! breaking ties, whatever order they are declared in; and the passes whose
//! writes reach neither the output nor the surface dropped. The errors are
//! in `graph_errors.rs` and bypassing in `bypass.rs`. No GPU is needed.

mod support;

use eng_gpu::{
    DRAW, Format, Graph, PRESENT, PassDecl, PassNode, Resource, TONEMAP, Target, Transient,
};
use support::{in_every_order, permutations};

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

const LIT: Resource = transient("lit");
const FOGGED: Resource = transient("fogged");
const DEBUG: Resource = transient("debug");

/// Something like the first-person graph of the rung 2 plan, cut to single
/// writes: two passes on the scene, light, a sky drawn on top of it, fog,
/// the tone map and present, and a debug view that nothing reads.
const FIRST_PERSON: [PassNode; 8] = [
    node("opaque", &[], &[Resource::Scene], None),
    node("sprites", &[], &[Resource::Scene], Some("opaque")),
    node("light", &[Resource::Scene], &[LIT], None),
    node("sky", &[], &[LIT], Some("light")),
    node("fog", &[LIT], &[FOGGED], None),
    node("tonemap", &[FOGGED], &[Resource::Output], None),
    node("present", &[Resource::Output], &[Resource::Surface], None),
    node("debug", &[Resource::Scene], &[DEBUG], None),
];

const A: Resource = transient("a");
const B: Resource = transient("b");
const C: Resource = transient("c");
const M: Resource = transient("m");
const Z: Resource = transient("z");

/// A graph whose passes tie often, none of them culled: after `root`,
/// `a1`, `b1` and `m1` are free to run together; `a1` frees `z2` and `b1`
/// frees `c2`, which then tie with what is left.
const TIES: [PassNode; 8] = [
    node("root", &[], &[Resource::Scene], None),
    node("a1", &[Resource::Scene], &[A], None),
    node("b1", &[Resource::Scene], &[B], None),
    node("m1", &[Resource::Scene], &[M], None),
    node("z2", &[A], &[Z], None),
    node("c2", &[B], &[C], None),
    node("join", &[Z, C, M], &[Resource::Output], None),
    node("present", &[Resource::Output], &[Resource::Surface], None),
];

fn names(graph: &Graph) -> Vec<&'static str> {
    graph.passes().iter().map(|p| p.name).collect()
}

#[test]
fn the_permutations_are_all_there() {
    let all = permutations(&[1, 2, 3, 4]);
    assert_eq!(all.len(), 24);
    let mut unique = all.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 24);
    assert_eq!(permutations(&TIES).len(), 40_320);
}

#[test]
fn the_smallest_name_free_to_run_goes_first_in_every_order() {
    // root; then a1, b1 and m1 are free: a1. Then b1, m1 and z2: b1. Then
    // c2, m1 and z2: c2. A queue in the order passes became free would give
    // root, a1, b1, m1, z2, c2; a tie broken by place of declaration would
    // change with the order of declaration.
    let graph = in_every_order(&TIES, &[], Target::Surface);
    assert_eq!(
        names(&graph),
        ["root", "a1", "b1", "c2", "m1", "z2", "join", "present"]
    );
    assert!(graph.culled().is_empty());
}

#[test]
fn the_first_person_graph_sorts_alike_in_every_order() {
    let graph = in_every_order(&FIRST_PERSON, &[], Target::Surface);
    assert_eq!(
        names(&graph),
        [
            "opaque", "sprites", "light", "sky", "fog", "tonemap", "present"
        ]
    );
    assert_eq!(graph.culled(), ["debug"]);
}

#[test]
fn rung_1b_passes_in_any_order_sort_to_the_fixed_order() {
    for nodes in permutations(&[DRAW.node(), TONEMAP.node(), PRESENT.node()]) {
        let graph = Graph::sorted(&nodes, Target::Surface).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(graph.passes(), Graph::fixed(Target::Surface).passes());
        assert!(graph.culled().is_empty());
        assert!(graph.pool().is_empty());
    }
    let graph = in_every_order(&[DRAW.node(), TONEMAP.node()], &[], Target::Offscreen);
    assert_eq!(graph.passes(), Graph::fixed(Target::Offscreen).passes());
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
    assert_eq!(
        names(&in_every_order(&nodes, &[], Target::Offscreen)),
        ["z_walls", "a_decals", "m_sprites", "tonemap"]
    );
}

#[test]
fn a_continuing_pass_may_write_part_of_what_it_continues() {
    // T8's case: opaque writes three targets, sprites continue all three,
    // and decals continue only the albedo. Light reads each after the last
    // pass that writes it, and ambient occlusion, which reads only the
    // depth, is free to run before the decals.
    const ALBEDO: Resource = transient("albedo");
    const INFO: Resource = transient("info");
    const DEPTH: Resource = transient("depth");
    let nodes = [
        node("opaque", &[], &[ALBEDO, INFO, DEPTH], None),
        node("sprites", &[], &[ALBEDO, INFO, DEPTH], Some("opaque")),
        node("decals", &[], &[ALBEDO], Some("sprites")),
        node("ao", &[DEPTH], &[LIT], None),
        node("light", &[ALBEDO, INFO, LIT], &[Resource::Output], None),
    ];
    assert_eq!(
        names(&in_every_order(&nodes, &[], Target::Offscreen)),
        ["opaque", "sprites", "ao", "decals", "light"]
    );
    // Two passes may continue one, if they write different resources.
    let nodes = [
        node("opaque", &[], &[ALBEDO, INFO], None),
        node("paint", &[], &[ALBEDO], Some("opaque")),
        node("tag", &[], &[INFO], Some("opaque")),
        node("light", &[ALBEDO, INFO], &[Resource::Output], None),
    ];
    assert_eq!(
        names(&in_every_order(&nodes, &[], Target::Offscreen)),
        ["opaque", "paint", "tag", "light"]
    );
}

#[test]
fn a_pass_whose_writes_reach_neither_output_nor_surface_is_culled() {
    let graph = in_every_order(&FIRST_PERSON, &[], Target::Surface);
    assert_eq!(graph.culled(), ["debug"]);
    assert!(!names(&graph).contains(&"debug"));
    // A chain of passes that leads nowhere goes whole, in running order.
    const SEEN: Resource = transient("seen");
    const HEARD: Resource = transient("heard");
    let nodes = [
        node("b_listen", &[SEEN], &[HEARD], None),
        node("a_look", &[Resource::Scene], &[SEEN], None),
        DRAW.node(),
        TONEMAP.node(),
    ];
    let graph = in_every_order(&nodes, &[], Target::Offscreen);
    assert_eq!(names(&graph), ["draw", "tonemap"]);
    assert_eq!(graph.culled(), ["a_look", "b_listen"]);
    // A pass that writes the output among other things stays, and so do
    // the passes before it.
    let nodes = [
        node("both", &[Resource::Scene], &[SEEN, Resource::Output], None),
        DRAW.node(),
    ];
    let graph = in_every_order(&nodes, &[], Target::Offscreen);
    assert_eq!(names(&graph), ["draw", "both"]);
    // Writing the output keeps a pass, and what it reads, even when nothing
    // reads the output after it; in a surface graph presenting does too.
    let graph = Graph::sorted(&[DRAW.node(), TONEMAP.node()], Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(graph.culled().is_empty());
    let graph = Graph::sorted(
        &[DRAW.node(), TONEMAP.node(), PRESENT.node()],
        Target::Surface,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(graph.culled().is_empty());
}

#[test]
fn graph_new_culls_nothing() {
    const UNREAD: PassDecl = PassDecl {
        name: "unread",
        reads: &[Resource::Scene],
        writes: &[DEBUG],
    };
    let graph =
        Graph::new(&[DRAW, UNREAD, TONEMAP], Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(names(&graph), ["draw", "unread", "tonemap"]);
    assert!(graph.culled().is_empty());
    assert!(graph.bypassed().is_empty());
    assert!(Graph::fixed(Target::Surface).culled().is_empty());
}
