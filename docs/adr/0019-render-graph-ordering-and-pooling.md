# ADR-0019: The render graph orders, culls, pools and labels

- **Status:** Proposed (for the owner's approval)
- **Date:** 2026-11-06
- **Decision:** carries out ADR-0003 (D7, spec 6.7) on rung 2's render
  graph; keeps ADR-0017's fixed graph
- **Who:** Claude's proposal (rung 2 plan, decision 5; built in queue item
  Q1). The owner decides.

## Context

ADR-0003 and spec 6.7 say that rung 2 adds ordering, culling of unused
passes, transient-texture pooling and labels to the render graph. Rung 1b
built a fixed list (ADR-0017): `Graph::new` checks passes in the order
given, and `Graph::fixed` is draw, tonemap and present.

Three things constrain the change:

- **The product path stays.** eng-render draws through `Graph::fixed`, and
  the four testbed shots must keep their bytes.
- **The protected `counters.rs` stays.** It builds
  `Graph::new(&[DRAW, TONEMAP, AGAIN])`, two writers of `output` run in the
  order given, and a `PassDecl { name, reads, writes }` literal. A
  sorted graph cannot accept two unordered writers, so `Graph::new` cannot
  become the sorted graph.
- **`Resource` is a closed enum** of `scene`, `output` and `surface`, and
  pipelines name the resource they draw into, which sets their format.

## Decision

- **A second constructor.** `Graph::sorted(&[PassNode], Target)` sorts,
  culls and pools. `PassNode` is `PassDecl` plus `continues`.
  `Graph::new`, `Graph::fixed` and `PassDecl` keep 1b's behaviour, checks
  and order. `PassDecl::node()` and `PassNode::decl()` convert. 1b's
  three passes, declared in any order, sort to `Graph::fixed`'s order.
  Declared in 1b's order they are equal to `Graph::fixed`.
- **Order.** A pass runs after the pass it continues, and after the last
  writer of each resource it reads. Among the passes free to run, the
  smallest name goes first: Kahn's sort, as eng-sim orders systems. Every
  check also walks the passes in name order. So any permutation of the
  same nodes gives the same graph, and the same error.
- **Continues names a pass, not a resource.** A pass that continues
  another writes the same resource, runs after it and loads its target
  instead of clearing it. Clearing in such a pass fails with `[E-GPU]`.
  Only one pass may continue a given pass, so the writers of a resource
  form one chain. Its readers run after the end of the chain. A read
  always sees a resource after every pass that writes it, so no pass
  overwrites what another has yet to read.
- **Errors.** Each starts with `[E-GRAPH]` and names the passes:
  - a duplicate name;
  - a continued pass that does not exist, or writes another resource;
  - a read that no pass writes;
  - two writers of one resource with neither continuing the other;
  - a cycle, given as the passes on it, from the smallest name in running
    order. A pass that reads or continues what it writes is a cycle of one.
  - 1b's rules: one write per pass, the surface only in a surface graph,
    the output written and never as the surface, and the surface written.
- **Culling.** A pass whose writes reach neither `output` nor `surface`,
  through the passes that read or continue them, is dropped.
  `Graph::culled()` names the dropped passes in the order they would have
  run. They are checked like the others.
- **Transients are a fourth `Resource` variant.**
  `Resource::Transient(Transient { name, format, divisor })` is `Copy` and
  const, so it fits `PassDecl`'s `&'static [Resource]` slices and a
  pipeline's `target`. Nothing outside eng-gpu matches `Resource`
  exhaustively, so `counters.rs` and eng-render compile unchanged.
  - `Format` is eng-gpu's own enum, `Rgba16Float` or `Rgba8UnormSrgb`
    (no wgpu type in the API).
  - `divisor` divides the targets' size, rounding up, for low internal
    resolution.
  - The name identifies a transient: two of one name that differ fail with
    `[E-GRAPH]`, and so does a divisor of 0.
- **Pooling.** In running order, a transient lives from the first pass
  that uses it to the last. Its kind is (format, divisor, usage), and the
  usage is "drawn" plus "sampled" if any pass reads it. Transients are
  taken in the order they start. Each gets the lowest-numbered texture of
  its kind that is free by then, or a new one. For intervals this is
  optimal: a kind gets as many textures as its busiest pass needs, so a
  chain of ping-pong passes needs two. `Graph::pool()` gives the plan on
  the CPU.
- **The targets own the pooled textures.** The n-th texture of a kind in a
  graph's plan is the n-th of that kind in the `Targets`, made on the first
  frame that needs it and kept from then on. So frames of one size, and
  other graphs on the same targets, reuse them. A resize makes new
  targets, as eng-app already does, so they are made again at the new
  size. `Graph::render` takes the targets by shared reference, so the pool
  sits behind a mutex, and `render`'s signature is unchanged.
  `Targets::transient_textures()` counts them.
- **`Graph::render` takes passes in the order they were declared**, culled
  ones included. A culled pass is checked against its declaration but does
  not record. The rest record and encode in running order. For a
  `Graph::new` graph the two orders are the same, as in 1b.
- **Labels.** Every pass, target, pooled texture, view and pipeline has a
  wgpu label. Each pass's input bind group is named after the pass. With
  `WGPU_DEBUG=1` each pass is also wrapped in a debug group of its name.
- **Out of scope:**
  - Per-pass GPU times: `TIMESTAMP_QUERY` is not a Tier 0 feature, and the
    device asks for default features only.
  - More than one write per pass: one colour target, as in 1b.
  - A bypassed pass whose write takes its read's texture.
  - The report's pool count.

## Consequences

- eng-render and the probe are unchanged. The four testbed shots are
  byte-identical to f3d8513's on Metal (Apple M2 Pro). `counters.rs` and
  `shaders.rs` pass unchanged.
- The first-person graph of the rung 2 plan can be declared as nodes in
  any order:
  - `sprites` continues `opaque`;
  - `sky` continues `light`;
  - a debug view that nothing reads is culled.
- `Graph::sorted` refuses `counters.rs`'s two writers of `output`, which
  `Graph::new` accepts. That is the point of the rule. A pass that draws
  over another says so with `continues`.
- `Graph` holds more: declaration order, running order, loads, culled
  names and the plan. A `Graph::new` graph still compares equal to
  `Graph::fixed` as before.
- Adding a `Resource` variant would break an exhaustive match outside
  eng-gpu. There is none today.
- Pooled textures live as long as their targets. A pool grows to the
  largest need of the graphs drawn on it and never shrinks.

## Alternatives considered

- **Continues per resource** (`continues: &[Resource]`, as the plan's
  table reads). Two passes continuing one resource would then be ordered by
  their names, and the draw order (decals before or after sprites) would
  hang on spelling. Naming the pass states it.
- **Make `Graph::new` sort.** That breaks `counters.rs`, whose two writers
  of `output` have no order but the list's.
- **`render` takes passes in running order.** Callers would have to learn
  the sort's result to call it, which defeats the sort.
- **A separate pool object passed to `render`.** That needs a new render
  call beside 1b's, and the targets already are "the textures of one
  size".
- **The pool inside `Graph`.** A graph is CPU-only, `Clone` and `Eq`, and
  built without a GPU in tests.
- **Transients as their own type beside `Resource`.** Declarations and
  pipelines would need two resource types, and `PassDecl` would change.
- **Aliasing memory rather than textures.** wgpu has no placed resources at
  Tier 0. Whole-texture reuse is what it allows.

## Revisit when

- A pass writes several targets or a depth target.
- Timestamp queries arrive (spec 6.7: used where available, `null`
  otherwise).
- A switched-off pass is bypassed.
- Pooled memory needs a limit.
