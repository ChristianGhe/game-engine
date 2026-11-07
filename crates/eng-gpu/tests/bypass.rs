//! Bypassing in [`Graph::sorted_without`] (rung 2 plan, decision 5 and Q1
//! done-when 4; ADR-0019): a switched-off pass that may be
//! bypassed is removed and its readers read its input; formats that differ,
//! or switching off a pass that may not be bypassed, are `[E-GRAPH]`. No
//! GPU is needed; `transients.rs` renders a bypassed graph.

mod support;

use eng_gpu::{
    DRAW, Format, Graph, GraphError, PassNode, PooledTexture, Resource, Target, Transient,
};
use support::{error_in_every_order, in_every_order};

const fn transient(name: &'static str) -> Resource {
    Resource::Transient(Transient::new(name, Format::Rgba16Float))
}

const fn node(
    name: &'static str,
    reads: &'static [Resource],
    writes: &'static [Resource],
    bypassable: bool,
) -> PassNode {
    PassNode {
        name,
        reads,
        writes,
        continues: None,
        bypassable,
    }
}

const LIT: Resource = transient("lit");
const FOGGED: Resource = transient("fogged");
const TONED: Resource = Resource::Transient(Transient::new("toned", Format::Rgba8UnormSrgb));

/// The first-person graph's post passes: fog and palette may be switched
/// off.
const POST: [PassNode; 5] = [
    node("draw", &[], &[Resource::Scene], false),
    node("light", &[Resource::Scene], &[LIT], false),
    node("fog", &[LIT], &[FOGGED], true),
    node("tonemap", &[FOGGED], &[TONED], false),
    node("palette", &[TONED], &[Resource::Output], true),
];

fn names(graph: &Graph) -> Vec<&'static str> {
    graph.passes().iter().map(|p| p.name).collect()
}

fn texture(format: Format, transients: &[&'static str]) -> PooledTexture {
    PooledTexture {
        format,
        divisor: 1,
        sampled: true,
        transients: transients.to_vec(),
    }
}

#[test]
fn with_every_pass_on_nothing_is_bypassed() {
    let graph = in_every_order(&POST, &[], Target::Offscreen);
    assert_eq!(
        names(&graph),
        ["draw", "light", "fog", "tonemap", "palette"]
    );
    assert!(graph.bypassed().is_empty());
    assert_eq!(
        graph.pool(),
        [
            texture(Format::Rgba16Float, &["lit"]),
            texture(Format::Rgba16Float, &["fogged"]),
            texture(Format::Rgba8UnormSrgb, &["toned"]),
        ]
    );
}

#[test]
fn a_bypassed_pass_is_removed_and_its_readers_read_its_input() {
    let graph = in_every_order(&POST, &["fog"], Target::Offscreen);
    assert_eq!(names(&graph), ["draw", "light", "tonemap", "palette"]);
    assert_eq!(graph.bypassed(), ["fog"]);
    assert!(graph.culled().is_empty());
    // The tone map reads `fogged`, which is drawn where `lit` is.
    assert_eq!(
        graph.pool(),
        [
            texture(Format::Rgba16Float, &["lit"]),
            texture(Format::Rgba8UnormSrgb, &["toned"]),
        ]
    );
}

#[test]
fn bypassing_the_pass_into_the_output_draws_its_input_there() {
    // The plan's palette switched off: the tone map writes the output.
    let graph = in_every_order(&POST, &["palette"], Target::Offscreen);
    assert_eq!(names(&graph), ["draw", "light", "fog", "tonemap"]);
    assert_eq!(
        graph.pool(),
        [
            texture(Format::Rgba16Float, &["lit"]),
            texture(Format::Rgba16Float, &["fogged"]),
        ]
    );
    let both = in_every_order(&POST, &["palette", "fog"], Target::Offscreen);
    assert_eq!(names(&both), ["draw", "light", "tonemap"]);
    assert_eq!(both.bypassed(), ["fog", "palette"]);
    assert_eq!(both.pool(), [texture(Format::Rgba16Float, &["lit"])]);
    // The order of `off` does not matter, nor does naming a pass twice.
    let again = Graph::sorted_without(&POST, &["fog", "palette", "fog"], Target::Offscreen);
    assert_eq!(again, Ok(both));
}

#[test]
fn two_bypassed_passes_in_a_row_hand_on_the_first_input() {
    const HAZE: Resource = transient("haze");
    let nodes = [
        node("draw", &[], &[Resource::Scene], false),
        node("light", &[Resource::Scene], &[LIT], false),
        node("fog", &[LIT], &[FOGGED], true),
        node("haze", &[FOGGED], &[HAZE], true),
        node("tonemap", &[HAZE], &[Resource::Output], false),
    ];
    let graph = in_every_order(&nodes, &["haze", "fog"], Target::Offscreen);
    assert_eq!(names(&graph), ["draw", "light", "tonemap"]);
    assert_eq!(graph.pool(), [texture(Format::Rgba16Float, &["lit"])]);
}

#[test]
fn the_rest_is_ordered_as_if_the_bypassed_pass_were_never_declared() {
    // With `y` on, `a` waits for it, and `m` goes before both. Bypassed, `a`
    // reads the scene, and is free as soon as `m` is: `a` first.
    const W: Resource = transient("w");
    const A2: Resource = transient("a2");
    const M: Resource = transient("m");
    let nodes = [
        node("x", &[], &[Resource::Scene], false),
        node("y", &[Resource::Scene], &[W], true),
        node("a", &[W], &[A2], false),
        node("m", &[Resource::Scene], &[M], false),
        node("out", &[A2, M], &[Resource::Output], false),
    ];
    assert_eq!(
        names(&in_every_order(&nodes, &[], Target::Offscreen)),
        ["x", "m", "y", "a", "out"]
    );
    // W takes the scene's texture, so no transient of W is pooled.
    let graph = in_every_order(&nodes, &["y"], Target::Offscreen);
    assert_eq!(names(&graph), ["x", "a", "m", "out"]);
    assert_eq!(
        graph.pool(),
        [
            texture(Format::Rgba16Float, &["a2"]),
            texture(Format::Rgba16Float, &["m"]),
        ]
    );
}

#[test]
fn switching_off_a_pass_that_may_not_be_bypassed_fails() {
    let e = error_in_every_order(&POST, &["light"], Target::Offscreen);
    assert_eq!(e, GraphError::NotBypassable { pass: "light" });
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `light` is switched off, but it may not be bypassed"
    );
    let e = error_in_every_order(&POST, &["bloom"], Target::Offscreen);
    assert_eq!(e, GraphError::UnknownSwitchedOff { pass: "bloom" });
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `bloom` is switched off, but no pass has that name"
    );
    // Of two, the smaller name.
    assert_eq!(
        error_in_every_order(&POST, &["tonemap", "light"], Target::Offscreen),
        GraphError::NotBypassable { pass: "light" }
    );
}

#[test]
fn a_bypass_between_formats_or_sizes_that_differ_fails() {
    // Checked whether or not the pass is switched off, so that switching it
    // off cannot fail later.
    let fog = node("fog", &[LIT], &[TONED], true);
    let nodes = [
        POST[0],
        POST[1],
        fog,
        node("tonemap", &[TONED], &[Resource::Output], false),
    ];
    let e = error_in_every_order(&nodes, &[], Target::Offscreen);
    assert_eq!(
        e,
        GraphError::BypassMismatch {
            pass: "fog",
            input: LIT,
            output: TONED,
        }
    );
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `fog` may be bypassed, but `toned` differs from its input `lit` in \
         format or size, so it cannot take the input's texture"
    );
    const HALF: Resource =
        Resource::Transient(Transient::new("half", Format::Rgba16Float).divided(2));
    let nodes = [
        POST[0],
        POST[1],
        node("shrink", &[LIT], &[HALF], true),
        node("tonemap", &[HALF], &[Resource::Output], false),
    ];
    assert_eq!(
        error_in_every_order(&nodes, &["shrink"], Target::Offscreen),
        GraphError::BypassMismatch {
            pass: "shrink",
            input: LIT,
            output: HALF,
        }
    );
    // The present blit cannot be bypassed: the surface is BGRA.
    let nodes = [
        DRAW.node(),
        node("tonemap", &[Resource::Scene], &[Resource::Output], false),
        node("present", &[Resource::Output], &[Resource::Surface], true),
    ];
    assert_eq!(
        error_in_every_order(&nodes, &[], Target::Surface),
        GraphError::BypassMismatch {
            pass: "present",
            input: Resource::Output,
            output: Resource::Surface,
        }
    );
}

#[test]
fn a_bypassable_pass_has_one_input_one_write_and_no_continues() {
    let nodes = [
        node("make", &[], &[Resource::Scene], true),
        POST[1],
        POST[2],
        POST[3],
        POST[4],
    ];
    let e = error_in_every_order(&nodes, &[], Target::Offscreen);
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `make` may be bypassed, but it reads nothing, so it has no input to pass on"
    );
    const MORE: Resource = transient("more");
    let two = node("two", &[LIT], &[FOGGED, MORE], true);
    let nodes = [POST[0], POST[1], two, POST[3], POST[4]];
    assert!(matches!(
        error_in_every_order(&nodes, &[], Target::Offscreen),
        GraphError::Unbypassable { pass: "two", .. }
    ));
    let over = PassNode {
        continues: Some("light"),
        ..node("over", &[Resource::Scene], &[LIT], true)
    };
    let nodes = [POST[0], POST[1], over, POST[2], POST[3], POST[4]];
    assert!(matches!(
        error_in_every_order(&nodes, &[], Target::Offscreen),
        GraphError::Unbypassable { pass: "over", .. }
    ));
    // Nor may another pass continue it.
    let after_fog = PassNode {
        continues: Some("fog"),
        ..node("after-fog", &[], &[FOGGED], false)
    };
    let nodes = [POST[0], POST[1], POST[2], after_fog, POST[3], POST[4]];
    let e = error_in_every_order(&nodes, &[], Target::Offscreen);
    assert_eq!(
        e,
        GraphError::ContinuesBypassable {
            pass: "after-fog",
            continues: "fog",
        }
    );
    assert_eq!(
        e.to_string(),
        "[E-GRAPH] pass `after-fog` continues `fog`, which may be bypassed; \
         a pass that may be bypassed is never continued"
    );
}
