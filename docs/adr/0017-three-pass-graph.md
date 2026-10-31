# ADR-0017: Three passes, not two

- **Status:** Proposed (for the owner's approval)
- **Date:** 2026-11-01
- **Decision:** amends ADR-0003 (D7, spec 6.7) on rung 1b's render graph
- **Who:** Claude's proposal (rung 1b plan, difference 1 and decision 1;
  built in tasks T7 and T8). The owner decides.

## Context

The spec says two things about rung 1b's graph that cannot both hold:

- 6.7 and ADR-0003: "Rung 1b uses a fixed two-pass graph: draw, then
  present". 9.2: "the fixed two-pass graph (draw, then a blit into the
  surface's preferred format)".
- 6.7 and ADR-0003 also make tone mapping the last graph pass, writing
  into an engine-owned texture that screenshots and golden tests read,
  and add a separate fullscreen blit into the surface, because swapchain
  textures cannot be relied on for copies.

With two passes, tone mapping would have to happen inside the draw pass,
which would then write 8-bit colour instead of the `Rgba16Float` HDR
scene that ADR-0003 asks for, or inside the present blit, whose target is
the surface, which shots cannot read. Three passes meet every rule. Rung
1b's plan said so (difference 1), and T7 and T8 built them.

## Decision

- **Rung 1b's fixed graph is three passes**, each declaring what it reads
  and writes (eng-gpu's `PassDecl`, ADR-0003's fixed rule):

  | Pass | Reads | Writes | In eng-render |
  |---|---|---|---|
  | `draw` | nothing | `scene`: `Rgba16Float`, linear, scene-referred | a clear to the background, then `top_down.wgsl`: the shapes in one instanced draw |
  | `tonemap` | `scene` | `output`: `Rgba8UnormSrgb`, engine-owned | `tonemap.wgsl`: rung 1b's clamp to [0, 1] (plan decision 2) |
  | `present` | `output` | `surface`: the window's image | `present.wgsl`: the output, blitted texel for texel |

- **Offscreen there is no `present`.** `Graph::fixed(Target::Offscreen)`
  is `draw`, `tonemap`; `Graph::fixed(Target::Surface)` adds `present`.
  `shot`, `video` and the golden tests render offscreen and read `output`;
  `play` renders the surface graph into its window.
- **`Graph::new` checks the declarations:** each read was written by an
  earlier pass; each pass writes exactly one resource; `output` is
  written, and never by the pass that writes the surface; an offscreen
  graph never touches the surface, and a surface graph writes it.
- **"Tone mapping is the last graph pass"** (ADR-0003) means the last pass
  that shapes the image. Only the present blit follows it, and the blit
  reproduces the output's bytes, so a window shows what a shot reads.
- **The surface's format is fixed:** `Bgra8UnormSrgb`, or `Bgra8Unorm`
  drawn through an sRGB view; a surface that offers neither fails with
  `[E-GPU]`. ADR-0003 says "the surface's preferred format"; a fixed sRGB
  format is what lets the blit reproduce the output (T8 decision 7, T11
  decision 6).

## Consequences

- A frame offscreen costs 2 draws, 2 pipeline switches and 1 bind-group
  switch, plus 32 bytes per shape in view; presenting adds 1, 1 and 1
  (eng-gpu's `FrameCounters`). The proposed `budgets.toml [gpu]` limits
  are sized from these numbers (rung 1b, amendment 40).
- Shots and goldens test the tone-mapped image that every window
  presents, not a swapchain image, so they need no window.
- Rung 2's tone-mapping curve and EV100 exposure change the tone-map
  pass (its shader, and what it binds: exposure is a per-frame input,
  which fullscreen passes cannot bind today); the graph and its
  declarations stay. Rung 2's ordering, culling and pooling (ADR-0003)
  work on the same declarations.
- The spec's 6.7 and 9.2 still say two passes. If the owner accepts this
  ADR, they and ADR-0003's rung 1b bullet should say three.
- A surface that offers neither BGRA format cannot present. If a runner
  shows one, the choice is between failing and another format with its
  own present pipeline (T8, open question 1).

## Alternatives considered

- **Tone map inside the draw pass**, then present. The draw pass would
  write the 8-bit output directly, and no HDR scene would exist for the
  passes ADR-0003 adds later (fog, a palette post-process, exposure).
- **Tone map inside the present blit.** The engine-owned texture would
  then be the HDR scene, and shots would have to tone-map on the CPU or
  read the surface, which swapchains do not reliably allow. Shots would
  no longer read what the window shows.
- **Copy the output into the surface** instead of a blit: spec 6.7 rules
  it out, and the formats differ (RGBA against BGRA).

## Revisit when

Rung 2 replaces the fixed list with a graph that orders and culls passes;
HDR display output arrives; or a runner's surface offers no BGRA format.
