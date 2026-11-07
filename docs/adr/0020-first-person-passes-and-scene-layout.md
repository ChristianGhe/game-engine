# ADR-0020: The first-person passes and the GPU scene layout

- **Status:** Proposed (for the owner's approval)
- **Date:** 2026-11-07
- **Decision:** amends ADR-0003 (D7, spec 6.7) and ADR-0017: a palette
  pass follows the tone map, and the integer upscale is in `present`
  (rung 2 plan, difference 1). Fixes 6.7's GPU scene layout for rung 2
  (plan, decision 6). Uses ADR-0019's graph rules unchanged
- **Who:** Claude's proposal (rung 2 plan, decision 6 and difference 1;
  task T0a). The owner decides.

## Context

Rung 2 draws a room in first person through `Graph::sorted` (ADR-0019).
Two parts of spec 6.7 shape it.

- **The look stack ends in a palette.** 6.7 makes the palette
  post-process a pass, with optional low internal resolution and integer
  upscaling. 6.7 and ADR-0003 also say "tone mapping is the last graph
  pass". ADR-0017 reads that as "the last pass that shapes the image. Only
  the present blit follows it, and the blit reproduces the output's
  bytes". The palette's lookup table is indexed by 8-bit sRGB codes
  (`code >> 3`, decision 8), so it maps display colours and must follow
  the tone map. Rung 2 therefore departs from that text (plan, difference
  1): the tone map is the last *scene-referred* pass, and `present` may
  scale `output` rather than copy it.
- **The scene layout is "fixed at rung 2"** (6.7, ADR-0003): shared
  vertex and index megabuffers, per-instance data in storage buffers,
  materials as indices into tables, textures as indices into texture
  arrays bucketed by size.

What eng-gpu has at HEAD:
- Per-instance data as vertex attributes, in one vertex buffer made each
  frame (`InstancedDesc`; `pass.rs`, `upload`); fullscreen passes that
  bind one texture and a sampler (`fullscreen.rs`). No index, storage or
  uniform buffer, no depth, no texture but the graph's targets. `Format`
  has two values (`graph.rs:63`).
- A Tier 0 device: no features, `wgpu::Limits::default()`
  (`gpu.rs:116-117`). In wgpu-types 30.0.1: 4 bind groups; 16 sampled
  textures and 8 storage buffers per stage; 256 array layers; binding
  arrays of 0 elements; 8 colour targets, 32 bytes per sample in all.
- The protected `counters.rs` pins `Graph::new`, `Graph::fixed`,
  `PassDecl`, the `FullscreenDesc` and `InstancedDesc` literals,
  `Targets::new`, `Graph::render`, the `Pass` trait and
  `PassRecorder::{clear, fullscreen, instanced}`. It builds
  `FrameCounters` by literal six times (lines 140, 166, 199, 223, 256,
  286), with `upload_bytes` of 224 and 32 in the last two, so that struct
  cannot gain a field. `shot_video.rs` pins `gpu.per_frame` (96 bytes),
  `tier: 0` and the capability names; `shaders.rs` validates every
  `.wgsl` file.
- `Graph::render_to` requires the window's image to be the targets' size
  (`pass.rs:187`). ADR-0019 gives transients a `divisor` "for low
  internal resolution"; built-in resources such as `output` have the
  targets' size, and a bypassed pass's write must have its input's size.

## Decision

- **The first-person graph** is eight `PassNode`s for `Graph::sorted`.
  The top-down view stays on `Graph::fixed` with 1b's clamp.

  | Pass | Reads | Writes | Switched off |
  |---|---|---|---|
  | `opaque` | none | `albedo` (`Rgba16Float`), `info` (`Rg32Uint`: region, surface class, object id), `depth` (`Depth32Float`, reversed-Z, infinite far) | never |
  | `sprites` | none | continues `opaque`: all three | never |
  | `light` | `albedo`, `info`, `depth` | `lit` (`Rgba16Float`) | falloff, banding and fake contrast each off |
  | `sky` | `depth` | continues `light`: `lit`, where depth is the far plane | a flat sky colour |
  | `fog` | `lit`, `info`, `depth` | `fogged` (`Rgba16Float`, as `lit`, for the bypass) | bypassed |
  | `tonemap` | `fogged` | `toned` (`Rgba8UnormSrgb`) | never |
  | `palette` | `toned` | `output` | bypassed |
  | `present` | `output` | `surface` | never; low resolution turns its k×k on |

- **The tail.**
  - `tonemap` is the last *scene-referred* pass: Khronos PBR Neutral with
    EV100 exposure (decision 7, O10), into `toned`.
  - `palette` follows it and is display-referred: `toned` through the 32³
    table (decision 8) into `output`. Switched off, it is bypassed
    (ADR-0019) and the tone map draws into `output`.
  - Shots, video and goldens read `output`, as in 1b.
  - `present` blits `output` into the surface. With low resolution off,
    the targets have the window's size and the blit is texel for texel, as
    in 1b. With it on, eng-render makes the first-person `Targets` at the
    window's size divided by an integer k, so every pass, `output`
    included, runs at 1/k with divisor 1, and `present` draws each texel
    as k×k pixels. T10 relaxes `render_to`'s size rule for that case, and
    decides how a window size that k does not divide is filled.
  - **The targets' size wins over ADR-0019's divisor** for low resolution.
    `output` is built in, so no divisor can shrink it, and shots read it.
    A 1/k `toned` could not be bypassed into a full-size `output`, so the
    upscale would move into the tone map. The divisor stays for a pass
    that runs below the targets' size; T22 rewords ADR-0019's phrase.
- **Meshes and instances.** One vertex and one `u32` index megabuffer,
  written when a scene loads, with f32 positions relative to each chunk's
  `WorldPos` origin. Each frame the CPU writes one record per drawn chunk
  or sprite into a read-only storage buffer; a chunk's holds its origin
  minus the eye, subtracted in f64, then rounded to f32 (6.7). Static
  meshes so stay camera-relative without being uploaded again.
- **Materials.** A storage buffer of material records. A draw names a
  material by index; a record names its texture's bucket, colour tag and
  layer, and its flags (alpha test, for sprites).
- **Textures.** `texture_2d_array`s of square textures, bucketed by size
  (64, 128 and 256 px), one array per bucket and colour tag (sRGB or
  linear): at most six arrays. Tier 0 has no binding arrays, so a texture
  is an array layer, not an index into an array of textures. Nearest
  sampling, no mipmaps.
- **A per-frame uniform**: the camera's projection and rotation (the eye
  is the origin), EV100 exposure and the look switches. Fullscreen passes
  bind it too, which ADR-0017 said they could not yet do.
- **Bind groups**, by how often they change: the frame (uniform, instance
  records), the scene (materials, texture arrays, a nearest sampler), the
  pass's reads. Three of Tier 0's four; T8 may change the split.
- **1b's API stays as `counters.rs` uses it.** New pipeline descriptors
  and recorder calls go beside the old ones; a new `Pass` method has a
  default body. New numbers (bytes written when a scene loads, per-pass
  GPU times under O11, the pool's texture count) go in new types and new
  report keys, never in `FrameCounters` or `per_frame`. A frame's
  `upload_bytes` counts what that frame writes: its instance records and
  its uniform.
- **The uniform is written only in a frame whose passes bind it.** 1b's
  passes bind none, so `counters.rs`'s 224 and 32 bytes and
  `shot_video.rs`'s 96 stay as they are.

## Consequences

- If the owner accepts this, 6.7's and ADR-0003's "tone mapping is the
  last graph pass" reads "the last scene-referred pass". ADR-0017's "the
  blit reproduces the output's bytes" then holds for the top-down graph,
  and for the first-person graph with low resolution off. T22 updates
  ADR-0003 and ADR-0017; the spec text is the owner's.
- With low resolution on, a window shows a shot's pixels scaled k×k, not
  one to one. A shot is `output` at its own size; only the window scales.
- 1b's pictures do not move: the top-down graph, its four shots and
  `counters.rs` are untouched. First-person frames get their own limits,
  `[gpu.first_person]` in `budgets.toml` (G5, from T10's counts); `[gpu]`
  stays as `shot_video.rs` feeds it.
- Limits: at most 256 textures per bucket and tag (array layers), six of
  16 sampled textures, squares of 64-256 px only (a larger one is a new
  bucket and an amendment). `opaque` uses 16 of 32 bytes per sample.
- `info` (`Rg32Uint`) cannot be filtered, so its readers bind it as an
  unsigned-integer texture, not through 1b's fullscreen layout.
- Storage buffers in the vertex stage are WebGPU core; wgpu marks adapters
  without them with the downlevel flag `VERTEX_STORAGE`, which `Gpu::new`
  does not check today. T8 checks it, with an error that names it.

## Alternatives considered

- **The palette before the tone map.** It indexes display codes, so it
  would quantize HDR values it cannot read.
- **The palette outside the graph**, after `output`. The tone map would
  stay literally last, but 6.7 makes each look feature a pass that can be
  switched off, and shots would no longer read what the window shows.
- **The upscale in its own pass, or in `palette`.** Its own pass costs a
  full-size target and a pass that `present` already is. Inside `palette`,
  `output` would be k² larger for every shot, and bypassing the palette
  would drop the upscale.
- **Camera-relative vertices uploaded each frame**, as 1b uploads its
  instances: every vertex, every frame, and not 6.7's layout.
- **Per-instance data as vertex attributes** (1b's `InstancedPipeline`):
  it works at Tier 0, but 6.7 fixes storage buffers, which later levels'
  indirect draws index by instance.
- **Arrays of textures** (`TEXTURE_BINDING_ARRAY`), or f64 positions in
  shaders (`SHADER_F64`): both beyond Tier 0.
- **One texture atlas.** World-anchored UVs (6.11) repeat, which an atlas
  cannot do with the sampler's addressing; an array layer repeats.

## Owner decisions, gates and protected files

- No O-number decides this ADR alone. O10 (the curve) and O11
  (timestamps) change passes, not the layout; A1 (O6) would read `info`.
- If the owner refuses difference 1, rung 2 drops the palette pass and the
  upscale, or keeps them outside the graph. The scene layout stands.
- G4 accepts this before T8-T11. G5 adds `[gpu.first_person]` to
  `budgets.toml` (protected) and protects `tests/look.rs`. `counters.rs`,
  `shaders.rs` and `shot_video.rs` stay unchanged.

## Revisit when

A level needs binding arrays or indirect draws (a higher tier); a texture
above 256 px, or more than 256 in one bucket; mipmaps; or HDR display
output, which changes the tail.
