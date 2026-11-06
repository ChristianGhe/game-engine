//! The transient pool's plan (rung 2, queue item Q1; ADR-0019, proposed):
//! transients share a texture when their format, size and usage match and
//! their lifetimes within the frame do not overlap. No GPU is needed;
//! `transients.rs` checks that the targets really reuse the textures.

use eng_gpu::{
    DRAW, Format, Graph, GraphError, PassDecl, PassNode, PooledTexture, Resource, TONEMAP, Target,
    Transient,
};

const fn transient(name: &'static str) -> Resource {
    Resource::Transient(Transient::new(name, Format::Rgba16Float))
}

/// A pass that writes `write`, clearing it.
const fn node(
    name: &'static str,
    reads: &'static [Resource],
    writes: &'static [Resource],
) -> PassNode {
    PassNode {
        name,
        reads,
        writes,
        continues: None,
    }
}

const PING: Resource = transient("ping");
const PONG: Resource = transient("pong");
const PING_AGAIN: Resource = transient("ping-again");

/// A chain of four passes, each reading the one before: a start, two steps
/// over ping-pong transients, and the pass into the output.
const CHAIN: [PassNode; 4] = [
    PassNode {
        name: "1-start",
        reads: &[],
        writes: &[PING],
        continues: None,
    },
    PassNode {
        name: "2-step",
        reads: &[PING],
        writes: &[PONG],
        continues: None,
    },
    PassNode {
        name: "3-step",
        reads: &[PONG],
        writes: &[PING_AGAIN],
        continues: None,
    },
    node("4-out", &[PING_AGAIN], &[Resource::Output]),
];

fn pool(nodes: &[PassNode]) -> Vec<PooledTexture> {
    Graph::sorted(nodes, Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"))
        .pool()
        .to_vec()
}

fn full(transients: &[&'static str]) -> PooledTexture {
    PooledTexture {
        format: Format::Rgba16Float,
        divisor: 1,
        sampled: true,
        transients: transients.to_vec(),
    }
}

#[test]
fn a_chain_of_four_passes_over_ping_pong_transients_needs_two_textures() {
    // ping lives in passes 1 and 2, pong in 2 and 3, ping-again in 3 and 4:
    // ping is done before ping-again starts.
    assert_eq!(
        pool(&CHAIN),
        [full(&["ping", "ping-again"]), full(&["pong"])]
    );
    // Declared backwards, it is the same plan.
    let mut backwards = CHAIN;
    backwards.reverse();
    assert_eq!(pool(&backwards), pool(&CHAIN));
}

#[test]
fn a_longer_chain_still_needs_two() {
    const T: [Resource; 8] = [
        transient("t0"),
        transient("t1"),
        transient("t2"),
        transient("t3"),
        transient("t4"),
        transient("t5"),
        transient("t6"),
        transient("t7"),
    ];
    const NODES: [PassNode; 9] = [
        PassNode {
            name: "p0",
            reads: &[],
            writes: &[T[0]],
            continues: None,
        },
        PassNode {
            name: "p1",
            reads: &[T[0]],
            writes: &[T[1]],
            continues: None,
        },
        PassNode {
            name: "p2",
            reads: &[T[1]],
            writes: &[T[2]],
            continues: None,
        },
        PassNode {
            name: "p3",
            reads: &[T[2]],
            writes: &[T[3]],
            continues: None,
        },
        PassNode {
            name: "p4",
            reads: &[T[3]],
            writes: &[T[4]],
            continues: None,
        },
        PassNode {
            name: "p5",
            reads: &[T[4]],
            writes: &[T[5]],
            continues: None,
        },
        PassNode {
            name: "p6",
            reads: &[T[5]],
            writes: &[T[6]],
            continues: None,
        },
        PassNode {
            name: "p7",
            reads: &[T[6]],
            writes: &[T[7]],
            continues: None,
        },
        node("p8", &[T[7]], &[Resource::Output]),
    ];
    assert_eq!(
        pool(&NODES),
        [
            full(&["t0", "t2", "t4", "t6"]),
            full(&["t1", "t3", "t5", "t7"])
        ]
    );
}

#[test]
fn a_transient_read_late_keeps_its_texture() {
    // ping is read again by the last pass, so it overlaps ping-again.
    let late = node("4-out", &[PING_AGAIN, PING], &[Resource::Output]);
    let nodes = [CHAIN[0], CHAIN[1], CHAIN[2], late];
    assert_eq!(
        pool(&nodes),
        [full(&["ping"]), full(&["pong"]), full(&["ping-again"])]
    );
}

/// [`CHAIN`] with its third pass writing `other` instead of ping-again.
fn chain_ending_in(other: &'static [Resource]) -> [PassNode; 4] {
    [
        CHAIN[0],
        CHAIN[1],
        node("3-step", &[PONG], other),
        node("4-out", other, &[Resource::Output]),
    ]
}

#[test]
fn only_transients_of_one_format_and_size_share() {
    const SRGB: Resource = Resource::Transient(Transient::new("srgb", Format::Rgba8UnormSrgb));
    const HALF: Resource =
        Resource::Transient(Transient::new("half", Format::Rgba16Float).divided(2));
    assert_eq!(
        pool(&chain_ending_in(&[SRGB])),
        [
            full(&["ping"]),
            full(&["pong"]),
            PooledTexture {
                format: Format::Rgba8UnormSrgb,
                ..full(&["srgb"])
            }
        ]
    );
    assert_eq!(
        pool(&chain_ending_in(&[HALF])),
        [
            full(&["ping"]),
            full(&["pong"]),
            PooledTexture {
                divisor: 2,
                ..full(&["half"])
            }
        ]
    );
    // The same kind as ping shares with it, whatever its name.
    const OTHER: Resource = transient("other");
    assert_eq!(
        pool(&chain_ending_in(&[OTHER])),
        [full(&["ping", "other"]), full(&["pong"])]
    );
}

#[test]
fn a_transient_no_pass_reads_has_its_own_usage() {
    // Graph::new does not cull, so a transient can be drawn and never read:
    // its texture is not bound, so it does not share with a read one.
    const UNREAD: PassDecl = PassDecl {
        name: "unread",
        reads: &[],
        writes: &[transient("unread")],
    };
    const START: PassDecl = PassDecl {
        name: "start",
        reads: &[],
        writes: &[PING],
    };
    const STEP: PassDecl = PassDecl {
        name: "step",
        reads: &[PING],
        writes: &[Resource::Scene],
    };
    let graph = Graph::new(&[START, STEP, UNREAD, TONEMAP], Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        graph.pool(),
        [
            full(&["ping"]),
            PooledTexture {
                sampled: false,
                ..full(&["unread"])
            }
        ]
    );
}

#[test]
fn culled_passes_take_no_texture() {
    const UNREAD: PassNode = PassNode {
        name: "unread",
        reads: &[Resource::Scene],
        writes: &[transient("unread")],
        continues: None,
    };
    let graph = Graph::sorted(&[DRAW.node(), UNREAD, TONEMAP.node()], Target::Offscreen)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(graph.culled(), ["unread"]);
    assert!(graph.pool().is_empty());
}

#[test]
fn a_continued_transient_keeps_one_texture_through_its_chain() {
    let nodes = [
        CHAIN[0],
        PassNode {
            name: "0-over",
            reads: &[],
            writes: &[PING],
            continues: Some("1-start"),
        },
        CHAIN[1],
        CHAIN[2],
        CHAIN[3],
    ];
    let graph = Graph::sorted(&nodes, Target::Offscreen).unwrap_or_else(|e| panic!("{e}"));
    let names: Vec<&str> = graph.passes().iter().map(|p| p.name).collect();
    assert_eq!(names, ["1-start", "0-over", "2-step", "3-step", "4-out"]);
    assert_eq!(
        graph.pool(),
        [full(&["ping", "ping-again"]), full(&["pong"])]
    );
}

#[test]
fn rung_1b_graphs_have_no_pool() {
    assert!(Graph::fixed(Target::Offscreen).pool().is_empty());
    assert!(Graph::fixed(Target::Surface).pool().is_empty());
}

#[test]
fn transients_of_one_name_must_agree() {
    const OTHER: Resource = Resource::Transient(Transient::new("ping", Format::Rgba8UnormSrgb));
    let nodes = [
        CHAIN[0],
        PassNode {
            name: "2-step",
            reads: &[PING],
            writes: &[Resource::Scene],
            continues: None,
        },
        PassNode {
            name: "3-odd",
            reads: &[OTHER],
            writes: &[Resource::Output],
            continues: None,
        },
    ];
    let e = Graph::sorted(&nodes, Target::Offscreen).unwrap_err();
    assert_eq!(e, GraphError::TransientMismatch { name: "ping" });
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] two transients are named `ping` but differ in format or size"
    );
    const ZERO: Resource =
        Resource::Transient(Transient::new("zero", Format::Rgba16Float).divided(0));
    const MAKE: PassDecl = PassDecl {
        name: "make",
        reads: &[],
        writes: &[ZERO],
    };
    let e = Graph::new(&[DRAW, MAKE, TONEMAP], Target::Offscreen).unwrap_err();
    assert_eq!(e, GraphError::Divisor { name: "zero" });
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] transient `zero` divides the targets' size by 0; 1 is full size"
    );
}
