//! The fixed render graph's declarations (spec 6.3, 6.7; rung 1b plan,
//! decision 1): what each pass reads and writes, and the checks that run when
//! a graph is built. No GPU is needed.

use eng_gpu::{DRAW, Graph, GraphError, PRESENT, PassDecl, Resource, TONEMAP, Target};

fn names(graph: &Graph) -> Vec<&'static str> {
    graph.passes().iter().map(|p| p.name).collect()
}

#[test]
fn the_three_passes_declare_what_they_read_and_write() {
    assert_eq!(DRAW.name, "draw");
    assert_eq!(DRAW.reads, &[] as &[Resource]);
    assert_eq!(DRAW.writes, &[Resource::Scene]);
    assert_eq!(TONEMAP.name, "tonemap");
    assert_eq!(TONEMAP.reads, &[Resource::Scene]);
    assert_eq!(TONEMAP.writes, &[Resource::Output]);
    assert_eq!(PRESENT.name, "present");
    assert_eq!(PRESENT.reads, &[Resource::Output]);
    assert_eq!(PRESENT.writes, &[Resource::Surface]);
}

#[test]
fn the_fixed_graph_presents_only_to_a_surface() {
    assert_eq!(
        names(&Graph::fixed(Target::Surface)),
        ["draw", "tonemap", "present"]
    );
    assert_eq!(names(&Graph::fixed(Target::Offscreen)), ["draw", "tonemap"]);
    assert_eq!(Graph::fixed(Target::Offscreen).target(), Target::Offscreen);
    assert_eq!(
        Graph::new(&[DRAW, TONEMAP, PRESENT], Target::Surface),
        Ok(Graph::fixed(Target::Surface))
    );
}

#[test]
fn a_read_must_follow_its_write() {
    assert_eq!(
        Graph::new(&[TONEMAP, DRAW], Target::Offscreen),
        Err(GraphError::ReadBeforeWrite {
            pass: "tonemap",
            resource: Resource::Scene,
        })
    );
    assert_eq!(
        Graph::new(&[DRAW, PRESENT, TONEMAP], Target::Surface),
        Err(GraphError::ReadBeforeWrite {
            pass: "present",
            resource: Resource::Output,
        })
    );
}

#[test]
fn a_pass_writes_exactly_one_resource() {
    let nothing = PassDecl {
        name: "nothing",
        reads: &[Resource::Scene],
        writes: &[],
    };
    assert_eq!(
        Graph::new(&[DRAW, nothing, TONEMAP], Target::Offscreen),
        Err(GraphError::Writes {
            pass: "nothing",
            count: 0,
        })
    );
    let both = PassDecl {
        name: "both",
        reads: &[],
        writes: &[Resource::Scene, Resource::Output],
    };
    assert_eq!(
        Graph::new(&[both], Target::Offscreen),
        Err(GraphError::Writes {
            pass: "both",
            count: 2,
        })
    );
}

#[test]
fn an_offscreen_graph_never_touches_the_surface() {
    assert_eq!(
        Graph::new(&[DRAW, TONEMAP, PRESENT], Target::Offscreen),
        Err(GraphError::SurfaceOffscreen { pass: "present" })
    );
    // Reading it is as wrong as writing it.
    let peek = PassDecl {
        name: "peek",
        reads: &[Resource::Surface],
        writes: &[Resource::Output],
    };
    assert_eq!(
        Graph::new(&[DRAW, peek], Target::Offscreen),
        Err(GraphError::SurfaceOffscreen { pass: "peek" })
    );
}

#[test]
fn output_is_never_the_surface() {
    // Tone mapping straight into the swapchain would leave screenshots
    // nothing engine-owned to read (spec 6.7).
    let direct = PassDecl {
        name: "tonemap",
        reads: &[Resource::Scene],
        writes: &[Resource::Output, Resource::Surface],
    };
    assert_eq!(
        Graph::new(&[DRAW, direct], Target::Surface),
        Err(GraphError::OutputIsSurface { pass: "tonemap" })
    );
}

#[test]
fn output_must_be_written_and_a_surface_presented() {
    assert_eq!(
        Graph::new(&[DRAW], Target::Offscreen),
        Err(GraphError::OutputNotWritten)
    );
    assert_eq!(
        Graph::new(&[DRAW, TONEMAP], Target::Surface),
        Err(GraphError::SurfaceNotWritten)
    );
}

#[test]
fn errors_name_the_pass_and_carry_the_tag() {
    let e = Graph::new(&[TONEMAP, DRAW], Target::Offscreen).unwrap_err();
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `tonemap` reads `scene` before any pass writes it"
    );
    let e = GraphError::PassMismatch {
        index: 1,
        expected: TONEMAP,
        found: DRAW,
    };
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass 1 is `draw`, but the graph declares `tonemap` there"
    );
    // The same name with other reads or writes.
    let impostor = PassDecl {
        name: "tonemap",
        reads: &[Resource::Scene],
        writes: &[Resource::Scene],
    };
    let e = GraphError::PassMismatch {
        index: 1,
        expected: TONEMAP,
        found: impostor,
    };
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass 1, `tonemap`, reads [scene] and writes [scene], \
         but the graph declares it reading [scene] and writing [output]"
    );
}
