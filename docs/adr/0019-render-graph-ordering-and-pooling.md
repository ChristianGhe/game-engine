# ADR-0019: The render graph orders, culls, bypasses, pools and labels

- **Status:** Proposed (for the owner's approval)
- **Date:** 2026-11-06
- **Decision:** carries out ADR-0003 (D7, spec 6.7) on rung 2's render
  graph; keeps ADR-0017's fixed graph
- **Who:** Claude's proposal (rung 2 plan, decision 5; built in queue item
  Q1). The owner decides.

## Context

ADR-0003 and spec 6.7 say that rung 2 adds ordering, culling of unused
passes, transient-texture pooling and labels to the render graph. The rung
2 plan (decision 5) adds bypassing: a switched-off post pass is removed and
its readers read its input. Rung 1b built a fixed list (ADR-0017):
`Graph::new` checks passes in the order given, and `Graph::fixed` is draw,
tonemap and present.

Three things constrain the change:

- **The product path stays.** eng-render draws through `Graph::fixed`, and
  the four testbed shots must keep their bytes.
- **The protected `counters.rs` stays.** It builds
  `Graph::new(&[DRAW, TONEMAP, AGAIN])`, two writers of `output` run in the
  order given, and a `PassDecl { name, reads, writes }` literal. A
  sorted graph cannot accept two unordered writers, so `Graph::new` cannot
  become the sorted graph.
- **`Resource` was a closed enum** of `scene`, `output` and `surface`, and
  pipelines name the resource they draw into, which sets their format.

**Built ahead of T8.** The plan gives running a planned graph on the GPU
to task T8: the pool of textures in `Targets`, continuing passes that load
their targets, and drawing transients. Q1 built them anyway, because its
brief asked for a GPU test that pooled textures really are reused across
frames. This ADR is where the owner first sees that design. T8 may change
it.

## Decision

- **A second constructor.** `Graph::sorted(&[PassNode], Target)` sorts,
  culls and pools. `Graph::sorted_without(nodes, off, target)` does the
  same with the passes named in `off` switched off. `PassNode` is
  `PassDecl` plus `continues` and `bypassable`. `Graph::new`,
  `Graph::fixed` and `PassDecl` keep 1b's behaviour, checks and order.
  `PassDecl::node()` and `PassNode::decl()` convert. 1b's three passes,
  declared in any order, sort to `Graph::fixed`'s order. Declared in 1b's
  order they are equal to `Graph::fixed`.
- **Order.** A pass runs after the pass it continues, and after the last
  writer of each resource it reads. Among the passes free to run, the
  smallest name goes first: Kahn's sort, as eng-sim orders systems. Every
  check also walks the passes in name order. So any permutation of the
  same nodes gives the same `passes()`, `culled()`, `bypassed()` and
  `pool()`, or the same error. The tests run every order of eight-pass
  graphs whose passes tie often. A queue in the order passes become free,
  or ties broken by place of declaration, each fail them.
- **Continues names a pass, not a resource.**
  - A pass that continues another writes some or all of what that pass
    writes, and nothing else. It runs after it and loads those targets
    instead of clearing them. Clearing in such a pass fails with `[E-GPU]`.
  - Of the passes that write one resource, only one may continue a given
    pass, so each resource's writers form one chain. Two passes may
    continue one pass if they write different resources.
  - A resource's readers run after the end of its chain. A read always
    sees a resource after every pass that writes it, so no pass overwrites
    what another has yet to read.
  - The plan's decals case holds: opaque writes albedo, info and depth,
    sprites continue all three, decals continue only the albedo.
- **Several writes are planned, not drawn.** A `PassNode` writes at least
  one resource, each once, and the plan orders, culls and pools passes
  with several writes. `Graph::render` refuses a running pass with more
  than one write with `[E-GPU]` until several colour targets exist (T8).
  `Graph::new` still requires exactly one write.
- **Bypass.** A pass marked `bypassable` reads at least one resource,
  writes exactly one, and neither continues nor is continued. Its write
  must have the format and size of its input, its first read. This is
  checked whether or not the pass is switched off, so switching it off
  cannot fail later.
  - Switched off, the pass is removed. Its write takes its input's texture,
    so its readers read the input. A built-in resource keeps its own
    texture: with the palette switched off, the tone map's `toned` is drawn
    into `output`.
  - The rest is ordered and culled as if the pass had never been declared,
    and `Graph::bypassed()` names it.
  - Switching off a pass that may not be bypassed, or one that does not
    exist, fails with `[E-GRAPH]`.
  - `render` still takes the bypassed pass, checks it and does not record
    it.
- **Errors.** Each starts with `[E-GRAPH]` and names the passes:
  - a duplicate name;
  - a pass that writes nothing, or one resource twice;
  - a continued pass that does not exist, does not write all its continuer
    writes, or may be bypassed;
  - the bypass rules above;
  - transients of one name that differ, one named `scene`, `output` or
    `surface`, or one divided by 0;
  - a read that no pass writes;
  - two writers of one resource with neither continuing the other;
  - a cycle, given as the passes on it, from the smallest name in running
    order. A pass that reads or continues what it writes is a cycle of one.
  - 1b's rules: the surface only in a surface graph, the output never
    written into the surface by the same pass, the output written, and the
    surface written.
- **Culling.** A pass whose writes reach neither `output` nor `surface`,
  through the passes that read or continue them, is dropped.
  `Graph::culled()` names the dropped passes in the order they would have
  run. They are checked like the others.
- **Transients are a fourth `Resource` variant.**
  `Resource::Transient(Transient { name, format, divisor })` is `Copy` and
  const, so it fits `PassDecl`'s `&'static [Resource]` slices and a
  pipeline's `target`.
  - `Format` is eng-gpu's own enum, `Rgba16Float` or `Rgba8UnormSrgb`
    (no wgpu type in the API).
  - `divisor` divides the targets' size, rounding up, for low internal
    resolution.
  - The name identifies a transient. `Graph::new` checks transients first,
    so a read in another format is reported as the mismatch it is.
- **Public API.** `Resource` and `GraphError` are now `#[non_exhaustive]`.
  - Both gained variants: `Resource::Transient`, and 16 for `GraphError`.
  - A match outside eng-gpu needs a wildcard arm. There is none today;
    `counters.rs`, `shaders.rs` and eng-render only name variants.
  - New public types: `PassNode`, `Transient`, `Format`, `PooledTexture`.
  - New public methods:
    - `Graph::sorted`, `Graph::sorted_without`, `Graph::culled`,
      `Graph::bypassed` and `Graph::pool`;
    - `Targets::transient_textures`;
    - `PassDecl::node` and `PassNode::decl`.
- **Pooling.** In running order, a transient lives from the first pass
  that uses it to the last. Its kind is (format, divisor, usage), and the
  usage is "drawn" plus "sampled" if any pass reads it. Transients are
  taken in the order they start. Each gets the lowest-numbered texture of
  its kind that is free by then, or a new one. For intervals this is
  optimal: a kind gets as many textures as its busiest pass needs, so a
  chain of ping-pong passes needs two. A bypassed write has no texture of
  its own. `Graph::pool()` gives the plan on the CPU.
- **The targets own the pooled textures** (ahead of T8).
  - The n-th texture of a kind in a graph's plan is the n-th of that kind
    in the `Targets`. It is made on the first frame that needs it and kept
    from then on, so frames of one size reuse it, and so do other graphs on
    the same targets.
  - A resize makes new targets, as eng-app already does, so the textures
    are made again at the new size.
  - `Graph::render` takes the targets by shared reference, so the pool
    sits behind a mutex, and `render`'s signature is unchanged.
    `Targets::transient_textures()` counts them.
- **`Graph::render` takes passes in the order they were declared**,
  culled and bypassed ones included. It checks the passes that run and
  has them record in running order. Then it checks the others, which do
  not record. For a `Graph::new` graph the two orders are the same, so the
  checks and recordings happen as in 1b.
- **Labels.** Every pass, target, pooled texture, view and pipeline has a
  wgpu label. Each pass's input bind group is named after the pass. With
  `WGPU_DEBUG=1` each pass is also wrapped in a debug group of its name; a
  unit test checks one group opens per pass and none without the flag.
- **Out of scope:**
  - Per-pass GPU times: `TIMESTAMP_QUERY` is not a Tier 0 feature, and the
    device asks for default features only.
  - Drawing several targets or a depth target (T8).
  - The report's pool count.

## Consequences

- eng-render and the probe are unchanged. The four testbed shots are
  byte-identical to f3d8513's on Metal (Apple M2 Pro). `counters.rs` and
  `shaders.rs` pass unchanged.
- The first-person graph of the rung 2 plan can be declared as nodes in
  any order:
  - `sprites` continues `opaque`, and decals can continue part of it;
  - `sky` continues `light`;
  - fog and palette can be switched off;
  - a debug view that nothing reads is culled.
- `Graph::sorted` refuses `counters.rs`'s two writers of `output`, which
  `Graph::new` accepts. That is the point of the rule. A pass that draws
  over another says so with `continues`.
- `Graph` holds more: declaration order, running order, loads, culled and
  bypassed names, aliases and the plan. A `Graph::new` graph still compares
  equal to `Graph::fixed` as before.
- Pooled textures live as long as their targets. A pool grows to the
  largest need of the graphs drawn on it and never shrinks.

## Alternatives considered

- **Continues per resource** (`continues: &[Resource]`, as the plan's
  table reads). Two passes continuing one resource would then be ordered by
  their names, and the draw order (decals before or after sprites) would
  hang on spelling. Naming the pass states it.
- **Continues only with the same writes.** That blocks decals, which
  continue only the albedo of a pass that writes three targets.
- **Make `Graph::new` sort.** That breaks `counters.rs`, whose two writers
  of `output` have no order but the list's.
- **Bypass by keeping the pass and copying.** A copy costs a pass and a
  texture; taking the input's texture costs nothing.
- **Bypass by order of the full graph.** Removing the pass after sorting
  would leave its readers waiting where it stood; sorting the rest again
  orders them as if it had never been declared.
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

- T8 draws several targets or a depth target, and runs the planned graph
  in eng-render.
- A pass's writes come from two different chains: it would continue two
  passes at once (say albedo from sprites and lighting from sky), which one
  `continues` cannot say.
- Timestamp queries arrive (spec 6.7: used where available, `null`
  otherwise).
- Pooled memory needs a limit.
