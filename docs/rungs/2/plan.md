# Rung 2: Inside the room

> **DRAFT, NOT APPROVED.** Drafted 2026-11-06 by Claude (Opus 5.5) while
> the owner was away, and revised twice after fresh reviews; nothing here
> binds until the owner approves it at gate G1. `docs/rungs/current.txt`
> stays `1b` until G5 (O1), and `docs/rungs/2/acceptance.json` does not
> exist: the owner writes it at G1 from "Proposed acceptance criteria". Every proposal below stands
> until the owner changes it.

- **Status:** draft for the owner's first sitting. The four pre-approval
  queue items have landed on `main` (commits in `docs/rungs/2/PROGRESS.md`).
  Sitting 1 needed three planning tasks first ("Before the first sitting"):
  all three are done. T0a's ADR-0020 to ADR-0024 are in `docs/adr/`
  (Proposed, reviewed twice), and T0's and T0b's patches are in
  `docs/rungs/2/patches/` (reviewed). They write only ADR drafts and patch
  files.
- **Spec:** the rung 2 row of section 9 of
  `docs/superpowers/specs/2026-10-20-ai-built-engine-design.md`, with 6.1-6.3
  (crates, rules, the fixed spine), 6.4 (simulation), 6.6 (frame loop), 6.7
  (rendering), 6.10 (input), 6.11 (collision and movement, sim data), 6.13
  (benchmarks, counters), 6.14 (accessibility), 7.2 (test layers), 7.4
  (docs-lint), 7.5 (skills, subagents), 7.7 (rhythm), 8.2-8.4 (CI, GPU tests,
  notices) and 11 (look, sprites).
- **Goal** *[spec 9]*: a first-person, Doom-style view of a simple room, with
  movement and collision through `kit-fps` and `eng-collision-grid`, the
  retro pipeline and look stack, billboard sprites, and a render graph that
  orders, culls and pools; the GCMouse shim; goldens on every adapter;
  gungraun benchmarks, docs-lint, cargo-about and dist, mutation testing.
  *[proposal]*: the player looks with the mouse as well as walking with the
  keys, and "dist" is read as a build folder that CI makes, not a release
  (difference 5).
- **Where each item comes from.** As in rung 1b, every item carries a tag:
  *[spec X]* (the spec asks for it, in section X), *[spec X + proposal]*
  (the spec asks and this plan fills in details) or *[proposal]* (Claude's
  own call). Departures are listed in "Where this differs from the spec".

## Sitting 1: what you decide

Each item links to its decision; the recommended answer comes first.
1. **G1, the plan:** approve it, the 25 criteria, 14 days and 43 sessions.
   Write `docs/rungs/2/acceptance.json`, but keep `current.txt` at `1b`
   until G5 (O1: the native-steps test reads the current rung's checks).
2. **1b:** commit its goldens (G6, commands in `PROGRESS.md`), play it (G7),
   and accept ADR-0017.
3. **O2** a second map with sim data, not a new dot room. **O3** the GCMouse
   shim as its own crate with `unsafe`. **O4** a brute-force collision
   oracle, not parry3d. **O5** `cargo xtask dist`, not cargo-dist.
4. **O6** agent seeing: A1, A3, A4 small and A2's three rules; A5 at rung 3.
5. **O7** benchmarks against the push's base. **O8** a missed mutant fails.
   **O9** fullscreen for first-person maps. **O10** Khronos PBR Neutral, to
   inform. **O11** ask for timestamp queries when there. **O12** leave
   `pmove`'s angled-wall fix to rung 3. **O13** gravity only in the air.
6. **G2 and G3**, from `docs/rungs/2/patches/`, by your O3 answer. Apply
   `g2-crates.patch` first (three crates), then one of:
   - the shim as a crate: `g2-gcmouse-crate.patch`, then
     `g3-crate-batch-3.patch`;
   - the shim as a module: `g2-gcmouse-module.patch`, then
     `g3-crate-batch-3-shim-module.patch`;
   - no shim: `g3-crate-batch-3-no-shim.patch`.

   If O4 picks parry3d, add `g3-parry3d.patch` after any of them. Batch 3
   is 17 names, and each path passes `quick` and the lint job. The
   patches do nothing for O5: its tool comes with T20.
7. **G4:** accept, change or refuse ADR-0019 to ADR-0024.

Sitting 2 is G5 (the patches, then a push); sitting 3 is G6 and G7 for rung
2 ("Owner gates").

## Where rung 1b stands

Approved 2026-10-31; CI green on all five native jobs on 2026-11-01 (run
41302087332), 16 of 19 features passing; 31 sessions against a budget of 30.
Still open: G6 (the owner commits the four golden candidates, ready with
commands in `docs/rungs/2/PROGRESS.md`), G7 (the owner plays), and ADR-0017
(three passes), still "Proposed". Rung 2 keeps 1b's top-down pictures, five
hash streams and assertions byte-for-byte as they are (decision 1), so G6
and G7 can come before rung 2's first sitting or in it.

**Protected tests that pin today's APIs.** Several protected tests build
1b's types by struct literal, implement 1b's traits, or pin output exactly.
Rung 2 keeps these as they are, so those tests compile and pass unchanged;
new behaviour comes through new types, functions and keys beside them.

| Kept as in 1b | Pinned by |
|---|---|
| `SimConfig`'s fields, `SimBuilder::new` | `eng-session/tests/hash_equality.rs:48`, `game-testbed/tests/twin.rs:213` |
| `GameDef`'s fields, `PlayRun::start`, `eng_app::play_on` | `twin.rs:33` (a `GameDef` literal), `twin.rs:76,84` |
| `Welcome`'s fields (no wire change in rung 2) | `eng-net-wire/tests/never_panic.rs:44`, `tests/golden.rs` |
| `Control`'s methods; new ones have default bodies | `twin.rs:354` (`LateClock`, a test platform's own `Control`) and the wrappers in `eng-platform-headless/tests/conformance.rs:227,258`; the unprotected `eng-platform-api/tests/window.rs` implements `Control` and `Window` too |
| `--version --json`: its top-level and `sim` keys, `sim.protocol` equal to dot-square's stream, and `formats` exactly (`recording: 1`, `hash_stream: 1`, no new file) | `game-testbed/tests/version.rs:292`, `:321`, `:323-325`, `:373`, `:379` |
| `Graph::new`, `Graph::fixed`, `PassDecl { name, reads, writes }`; the `FullscreenDesc` and `InstancedDesc` literals; `Targets::new(gpu, 16, 8)`; `Graph::render(gpu, &targets, &mut [&mut dyn Pass])`; the `Pass` trait as an outside `impl` sees it; `PassRecorder::{clear, fullscreen, instanced}`; `FrameCounters`' fields, built by literal | `eng-gpu/tests/counters.rs:11-15`, `:32-58`, `:83-108`, `:140, 166, 199, 223`, `:181-187` (`Graph::new(&[DRAW, TONEMAP, AGAIN])`: two writers of `Output`) |
| A shot report's `gpu.per_frame` exactly, `tier: 0`, the five capability names; the budgets parser accepting the test's own `[gpu]` literal | `game-testbed/tests/shot_video.rs:22-25` (its `BUDGETS`), `:301`, `:305-313`, `:323-324` |

## What already exists (do not rebuild)

| Crate | What rung 2 starts from (1b, then the queue) |
|---|---|
| `eng-gpu` | 1b: `Graph::fixed` (draw, tonemap, present) and `Graph::new` over `PassDecl`; instanced and fullscreen pipelines; `Targets` with readback; `FrameCounters`; `GpuReport`; `WindowSurface` (Fifo); `tests/shaders.rs`. **Q1:** `Graph::sorted(&[PassNode], Target)` and `Graph::sorted_without(nodes, off, target)` in `src/graph.rs` and `src/graph/{order,cull,bypass,pool,error}.rs`: Kahn's order with names breaking ties, culling, bypass, continues, `culled()`, `bypassed()`, `pool()`. `Resource::Transient` (a name, a `Format` of `Rgba16Float` or `Rgba8UnormSrgb`, a divisor). Built ahead of T8: the texture pool in `Targets` and `LoadOp::Load` for continuing passes. `Resource` and `GraphError` are `#[non_exhaustive]`, with 16 new error variants. Tests `graph_order`, `graph_errors`, `pool`, `bypass`, `bypass_render`, `transients`. ADR-0019, Proposed. Not yet: a running pass with several writes (`[E-GPU]`), depth, textures, uniform, storage or index buffers, timestamps |
| `eng-render` | 1b: the top-down view, the clamp tone map, the present blit, `[E-SCENE]`. **Q4**, CPU only: `FirstPersonCamera` (`src/camera.rs`: f64 eye subtracted first, reversed-Z, infinite far; no near distance chosen yet); `sprite_rotation`, `sprite_rotation_toward` and five-rotation sheets (`src/sprite_rot.rs`); `PaletteLut::bake` (a 32³ OKLab LUT of up to 256 colours) and `BAYER_4X4` (`src/palette.rs`). Tests `camera`, `sprite_rotation`, `palette` (Ottosson's OKLab values and a pinned FNV hash of a bake) |
| `kit-fps` | 1b: `pmove` (flat floor). **Q2:** `ground_move`, `GroundConfig` v1 (`DEFAULT`), `GroundOutcome` and `GROUND_PROBE` (a 1/1024 m trace down) in `src/ground.rs`: gravity only in the air, steps only from the ground and only onto floor; on flat ground `pmove`'s result bit for bit (proptest). Tests `ground`, `slopes`, `boxes`, `tests/support/boxes.rs` |
| `eng-input` | 1b: keyboard `LiveInput`, strict bindings, scripts with `side`, `yaw_deg`, `pitch_deg`. **Q3:** `LookSettings` (`sensitivity_microdeg`, default 66_000; `invert_pitch`; `pitch_limit`, default 14_563) and `MouseLook` (`motion`, `focus_lost`, `set_view`, `sample`) in `src/look.rs`; look is off by default. Oracle: `mouse_look.rs`, `mouse_look_exact.rs`, `mouse_look_model.rs`, `look_rig/mod.rs` |
| `eng-collision-api` | `CollisionWorld` (`trace`, `overlaps`), `BoxHull`, `Trace`, `BoxRoom`, and `conformance::check` (not protected) |
| `eng-platform-api` | Events for raw `MouseMotion`, buttons, wheel and gamepads; `Control` has `now` and `exit` only; `Window` has `size()` only (`window.rs:22`); the protected conformance suite |
| `eng-platform-winit` | Keys and window events only; no mouse. 1b's T10 left notes (T13 carries them) |
| `eng-app`, `eng-cli` | `TickClock`, one frame per tick, occlusion, a lost surface panics; `run`, `replay`, `dump-world`, `shot`, `video`, `play`; report v2 with `gpu`; budgets `[sim]` and `[gpu]` |
| `game-testbed(-sim)` | The 8 m x 6 m x 2 m dot room (`BoxRoom`), `top_down.rs`, scenarios `dot-square` and `dot-corner`, five hash streams, four shots, `golden/thresholds.toml` (mean 0.01, p99 0.2, p999 0.5), no goldens yet |
| `xtask` and CI | Jobs lint, test, riscv, wasm, selftest, native (12 steps), summary. `docs-check` is not the spec's docs-lint. No schedule, no benchmarks, no mutation testing. AGENTS.md is at 150 of 150 lines |

## What gets built

### New crates (4)

Layers are `architecture.toml`'s. The exact entries are gate G2.

| Crate | Proposed layer and edges | What it does in rung 2 | From |
|---|---|---|---|
| `eng-collision-grid` | `L2`, core, `same_layer = ["eng-collision-api"]`; `game-testbed-sim` gains it in its `same_layer` | Level 1's collision: a uniform grid over static primitives (vertical wall segments with a z range, horizontal convex slabs for floors, ceilings and steps), deterministic sweeps of a `BoxHull` that end in contact (ADR-0024: exactly on axis faces, within `CONTACT_GAP` off the axes). It implements `CollisionWorld` and passes `eng-collision-api`'s conformance suite. Built once from sim data (decision 1) | spec 6.1, 6.11, 9 |
| `game-testbed-client` | `game`; `game-testbed` gains `same_layer = ["game-testbed-client"]` | The testbed's presentation: the top-down extraction (moved, unchanged), the first-person extraction, region looks, textures and sprites made by code, the plain-text palette | spec 6.1 (games row); rung 1b difference 6 |
| `eng-platform-gcmouse` | `L1`, std; `eng-platform-winit` gains it in its `same_layer` | macOS only: raw mouse deltas from GCMouse, summed for the winit platform to drain each frame, and a count of its callbacks. Empty elsewhere. The engine's only `unsafe` code (decision 11, O3). `[floor]` entries make it and `objc2-game-controller` single-user crates: only `eng-platform-winit` may depend on the shim, and only the shim on `objc2-game-controller` | spec 6.10 (the shim); its own crate: proposal |
| `eng-bench` | `tool` | gungraun benchmarks: the dot and room sim steps, grid traces, `ground_move`. gungraun is a Linux-only dev-dependency; elsewhere each bench is an empty `main` | spec 6.1 (tools row), 6.13 |

### Changed crates

| Crate | Change in rung 2 | From |
|---|---|---|
| `eng-gpu` | A running pass with several colour targets and a depth target; `Format` widened (decision 5); textures, a per-frame uniform, the scene layout (decision 6); per-pass GPU times (O11). `Graph::new`, `Graph::fixed` and `PassDecl` stay | spec 6.7 + proposal |
| `eng-render` | First-person opaque, sprite, light, sky, fog, tone-map and palette passes (the `render-retro` pipeline, as modules of this crate); the top-down view unchanged | spec 6.1, 6.7 |
| `eng-collision-api` | The contract promises that a blocked trace ends in contact: exactly on an axis face, within `CONTACT_GAP` off the axes; the conformance suite checks it (ADR-0024) | spec 6.11; rule 6 |
| `eng-platform-api`, `-headless`, `-winit` | Cursor lock and borderless fullscreen as new `Control` methods, and `Window::refresh_rate`, all with default bodies, and a separate conformance suite; winit maps mouse motion, buttons and wheel; macOS motion from the shim | spec 6.10; rule 6 (ADR-0022) |
| `eng-ecs`, `eng-sim` | Constant resources outside the per-tick state and snapshots, and `Sim::sim_data_hash` (decision 1) | spec 6.11 + proposal |
| `eng-cli`, `eng-app` | `GameMaps` and new entry points beside `GameDef`; `--map`, `--view`, first-person `shot`, `video` and `play`; the map and sim data in recordings, hash-stream headers and reports; display-paced frames; cursor and fullscreen | spec 6.6, 6.10, 6.11, 7.1 |
| `game-testbed-sim` | A second map, `room`; the dot map unchanged | proposal |
| `xtask` (patches only) | docs-lint, bench, mutants, notices, dist, replay reading `map`, new CI steps and selftests | spec 7.4, 6.13, 7.2, 9 |

### One frame, first person

```
OS events ─eng-platform-winit─▶ keys, raw mouse (macOS: eng-platform-gcmouse) ─▶ eng-app
  LiveInput: held keys + MouseLook ─sample(tick)─▶ PlayerCommand (forward, side, view_yaw, view_pitch)
  eng-session over Memory ─▶ Sim::step: ground_move against the grid, things turn
  previous, current, alpha; plus the live view angles (presentation only)
  game-testbed-client ─▶ FirstPersonScene (camera WorldPos f64, things, region looks)
  eng-render, Graph::sorted:
    opaque ▶ sprites ▶ light ▶ sky ▶ fog ▶ tonemap ▶ palette ▶ output ─present (k×k)▶ surface
                                                           │
                                                 shot, video, goldens
```

### The first-person graph *[spec 6.7 + proposal]*

A pass that continues another names that **pass**, writes some or all of
what it writes, and loads those targets instead of clearing them
(ADR-0019).

| Pass | Reads | Writes | Switched off |
|---|---|---|---|
| `opaque` | none | `albedo` (`Rgba16Float`), `info` (`Rg32Uint`: region, surface class, object id), `depth` (`Depth32Float`, reversed-Z, infinite far) | never |
| `sprites` | none | continues `opaque`: `albedo`, `info`, `depth` | never |
| `light` | `albedo`, `info`, `depth` | `lit` (`Rgba16Float`) | falloff and 32-step banding off: no distance falloff; fake contrast off: no wall-direction term |
| `sky` | `depth` | continues `light`: `lit`, where depth is the far plane | a flat sky colour |
| `fog` | `lit`, `info`, `depth` | `fogged` | bypassed |
| `tonemap` | `fogged` | `toned` (`Rgba8UnormSrgb`) | never: the curve and EV100 exposure (decision 7) |
| `palette` | `toned` | `output` | bypassed: its readers read `toned` |
| `present` | `output` | `surface` | low resolution off: `output` at the window's size; on: at 1/k, drawn k×k |

- **Light** *[spec 6.7 + proposal]*: region illuminance in lux, Lambertian
  albedo, luminance in nits, a fixed EV100 per camera. Doom's 0-255 light
  levels map to lux by a table, so falloff and banding can use its 32 steps.
- **`info`** carries each pixel's region for light and fog, and an object id
  that addition A1 can read almost for free.
- **The top-down graph** stays on `Graph::fixed` with 1b's clamp, so its
  pictures, 1b's goldens and `counters.rs` are untouched.

### The room *[proposal]*

A 12 m x 9 m hall with a 4 m ceiling; a 0.4 m platform reached by one step,
and a 0.6 m ledge that must block; a square 1 m pillar; an alcove with a
2.5 m ceiling and green fog; a skylight showing the sky. **Every wall is on
an axis**, so `pmove`'s angled-wall stop (O12) cannot arise in rung 2. Four
regions, fixed spawn points and three things: a "marker" that turns once
every 8 s, so each of its 8 rotations shows from a fixed view, and one behind
the pillar as seen from the spawn. Exact sizes are T7's call.

### Probe additions

```
<game> run|dump-world|shot|video|play ... [--map dots|room]
<game> shot|video ... [--view top-down|first-person] [--look look.toml]
<game> play [--windowed] [--fov DEG] [--sensitivity DEG_PER_COUNT] [--invert-pitch] [--look look.toml]
```

`--map` defaults to `dots`, so every 1b command line means what it did; an
unknown map exits 64 with `[E-MAP]`. `--view` defaults to the map's own
(top-down for `dots`, first person for `room`). `--look` switches look
passes, all on by default. No new exit code.

## Decisions (*[proposal]* unless tagged; the owner can overrule any)

1. **Maps and sim data** *[spec 6.11 + proposal; ADR-0023]*.
   - **Maps.** The testbed gets a second map, `room`. Each map is a build
     function in a new `GameMaps` static beside `GameDef`, so `SimConfig` and
     `GameDef` keep their fields. `GameDef::build` is the first map, `dots`,
     and registers exactly what it does today.
   - **Sim data.** The room's geometry and grid are constant resources:
     registered with their schema, outside the per-tick state, the state hash
     and snapshots. A snapshot must fit 1,200 bytes (`[E-SNAPSHOT-SIZE]`); the
     dot's is 466, and a wall list would not fit.
   - **`Sim::sim_data_hash`**, FNV-1a-64 over their canonical bytes (0 with
     none), fills the recording header's existing `sim_data_hash` (0 today),
     so recordings stay v1. `replay verify` picks the map whose hash matches,
     or exits 64 with `[E-RECORDING-BUILD]`. Only the first map may lack sim
     data.
   - **Hash streams.** A map with sim data adds `map=<name>
     sim_data=0x<16 hex>` to the header's second line, so `compare-hashes`
     holds sim data equal across architectures (6.11). `hash_stream` stays
     v1; its first line (`version.rs:379`) and the dot streams do not change.
   - **What the stamp shows.** `--version --json` describes the first map
     only. `version.rs` pins its keys, `sim.protocol` against dot-square's
     stream and `formats` exactly, so neither the room's protocol and sim
     data nor the versions of `look.toml` and `shots.toml` can appear there.
     Instead a run's report names its `map`, `sim_protocol`, `sim_data_hash`
     and the version of each file the probe read (new keys within report
     v2), and each file's reader checks its own `version` field strictly.
   - **The wire.** `Welcome` does not change. Over Memory the probe builds
     server and client from one map. A client built from another map has
     another simulation protocol, and the handshake denies it first
     (`DenyReason::SIM_PROTOCOL`, `eng-session/src/server.rs:182`). Sim data
     between machines is rung 5's handshake (6.8). The
     alternative, replacing the dot room, is O2.
2. **Collision** *[spec 6.11 + proposal; ADR-0024]*. 1 m grid cells,
   visited in cell order, then primitive index; ties between hits go to the
   lower index. Sweeps are deterministic (f64, IEEE basic operations only): a
   box against a wall segment is a 2D separating-axis sweep plus the
   segment's z range, against a slab a z sweep inside its polygon.
   - **Contact** (ADR-0024). `ground_move` decides "on the ground" with a
     `GROUND_PROBE` (1/1024 m) trace down, so a blocked trace must end in
     contact, which the contract does not promise today. f64 can hold
     contact exactly only against a face square to an axis.
     - **On an axis**, the trace ends exactly on the boundary that the
       world's own `overlaps` uses.
     - **Off the axes** (the grid supports such walls), it ends within a
       new contract constant, `CONTACT_GAP` (2^-20 m proposed, a thousandth
       of `GROUND_PROBE`).
     - `conformance::check` gains the rule, with migration notes: `BoxRoom`
       already meets it, and the grid must.
   - `BoxRoom` and the grid are the contract's two implementations. A
     brute-force oracle in the tests (every primitive, no grid) must give the
     same `Trace` bits (O4: parry3d instead).
3. **Ground movement** *[spec 6.11 + proposal; built in Q2; O13]*:
   `GroundConfig` v1 (gravity 9.81 m/s², step 0.4 m, snap 0.4 m, steepest
   walkable normal) and `ground_move`. On the ground: `pmove`'s friction and
   acceleration, `vel.z = 0`, a flat slide, then a stepped try (up, slide,
   down) onto floor only, which wins only if its horizontal travel is
   strictly greater (squared distances, compared exactly), then a snap down.
   In the air: gravity only (no air control, no step-up; O13),
   `vel.z ← vel.z − g·dt`, then the sweep by `vel·dt`. Vertical speed is
   `MoveState.vel.z`, and "on the ground" is recomputed each tick, so
   `MoveState`'s encoding does not change. No jumping. The room's hull is
   0.5 x 0.5 x 1.75 m, the eye 1.6 m above the feet.
4. **Things and sprites** *[spec 6.7, 11 + proposal; rule built in Q4]*. A
   thing is an entity with a position and a `BAngle` facing.
   - **The rotation, from the geometry:** the one whose 45° sector, centred on
     the facing, holds the direction from the thing to the viewer, counting
     counter-clockwise. A thing facing the viewer shows its front, rotation
     0; a quarter turn counter-clockwise gives 2; behind gives 4. In binary
     angles, `((to_viewer − facing) + 2^28) >> 29`, wrapping; a boundary goes
     to the counter-clockwise sector.
   - A five-rotation sheet shows rotation k above 4 as `8 − k` mirrored.
     Billboards turn only about Z, are alpha-tested and write depth.
   - The client renders each sprite's 8 rotations on the CPU at start-up from
     a small 3D primitive with a marked front, so sprites "are rendered from
     3D sources" (spec 11) and no image file enters the repo.
5. **The graph grows** *[spec 6.7, ADR-0003 + proposal; ADR-0019; built in
   Q1]*.
   - **Kept as 1b:** `Graph::new`, `Graph::fixed` and `PassDecl`, which the
     protected `counters.rs` uses for two writers of `Output`.
   - **`Graph::sorted`** over `PassNode` (`PassDecl` plus `continues`, the
     name of a pass, and `bypassable`) does the rest. **Ordering:** Kahn's
     sort, a pass after the pass it continues and after the last writer of
     each resource it reads, names breaking ties; every permutation gives the
     same graph or the same error. **Culling:** a pass whose writes reach
     neither `output` nor `surface` is dropped and listed. **Bypass:** a
     switched-off post pass is removed and its readers read its input, of the
     same format and size. **Pooling:** transients pooled by (format,
     divisor, sampled), reused across frames of one size. **Labels** name
     every pass, with debug groups under `WGPU_DEBUG=1`.
   - **T8 adds:** a running pass with up to four colour targets and one depth
     target (`Graph::render` refuses several writes with `[E-GPU]` today);
     `Format` widened from its two values (`graph.rs:63`) to at least
     `Rg32Uint` and `Depth32Float`; and GPU time per pass. The new pipeline
     descriptors and recorder calls go **beside** the old ones, which stay as
     `counters.rs` uses them, and any new `Pass` method has a default body. 6.7 wants
     timestamp queries "when available, with null otherwise", but 1b's device
     asks for WebGPU's defaults only (ARCHITECTURE.md, "Tier 0"), and
     `TIMESTAMP_QUERY` is beyond them: O11.
6. **The GPU scene layout** *[spec 6.7 "fixed at rung 2" + proposal;
   ADR-0020, which also records the first-person passes of difference 1]*.
   - One vertex and one index megabuffer (`u32`), vertices relative to their
     chunk's origin. Each frame the CPU uploads every chunk's origin minus the
     camera, subtracted in f64, into a per-instance storage buffer, so static
     meshes stay camera-relative without re-upload.
   - A storage-buffer material table. `texture_2d_array`s bucketed by size
     (64, 128, 256), one per bucket and colour tag, as Tier 0 has no binding
     arrays. Nearest sampling, no mipmaps.
   - A per-frame uniform for camera, exposure and look switches, which
     ADR-0017 said fullscreen passes could not yet bind.
7. **Tone map** *[spec 6.7 + proposal; spec 15 leaves the curve to the
   builder; O10]*: Khronos PBR Neutral. It is analytic (no LUT file) and
   keeps base colours, which suits the palette after it; EV100 exposure. The
   top-down graph keeps 1b's clamp.
8. **Palette** *[spec 6.7 + proposal; LUT built in Q4]*.
   - A 32³ LUT baked on the CPU by OKLab distance, ties to the lowest index,
     indexed by `code >> 3` of each 8-bit sRGB code; optional 4x4 Bayer
     dither.
   - The palette is a plain-text file, as 6.7 says: GIMP's open `.gpl`
     format, game data at `game-testbed-client/palettes/room.gpl`.
   - A script in the repo generates it. It is not Doom's palette, which is id
     Software's work; spec 11 asks for public tropes, not protected works.
   - As the first non-code asset, it gets 6.11's provenance sidecar (source,
     maker, tool, licence, status).
9. **Mouse look** *[spec 6.10, 6.14 + proposal; built in Q3]*.
   - **Settings:** sensitivity in integer millionths of a degree per count
     (default 66_000, Quake's 0.022° × 3), so the value a player writes is
     the value used; invert pitch; a pitch limit in binary-angle units
     (default 14_563, 80° rounded down).
   - **Signs:** positive yaw turns toward +Y, so `dx > 0` (right) lowers yaw.
     `dy > 0` is toward the user (`event.rs:19`) and pitch is positive up
     (`script.rs:35`), so `dy > 0` lowers pitch unless inverted.
   - **Rounding:** each delta is rounded to the nearest 1/256 count, ties away
     from zero, and summed exactly in `i64`. The turn is `n·s / 1_406_250`
     units in `i128`, half away from zero, from the whole sum since the last
     re-base, so any split of the same motion into ticks gives the same yaw.
   - **The pitch limit:** at the limit the sum re-bases, dropping motion past
     it (as Quake does); only that case depends on tick boundaries.
   - Losing focus drops unsampled motion. Buttons stay 0 until doors (rung 3).
10. **Window and frames** *[spec 6.6, 6.10 + proposal; O9]*.
    - First-person maps open borderless fullscreen, with `--windowed` for
      tests. `dots` keeps 1b's window, which the protected `play.rs` runs.
      The cursor is locked (Confined where Locked is missing, which in winit
      0.30.13 is X11 only) and hidden while focused, and released on focus
      loss.
    - **The contract change** (ADR-0022, rule 6) adds cursor and fullscreen
      requests to `Control`, and `refresh_rate()` to `Window`. Its
      **migration notes**:
      - each new method has a default body (doing nothing, or `None` for the
        refresh rate), so every `impl Control` and `impl Window` compiles
        unchanged: `twin.rs`'s `LateClock`, the headless conformance test's
        wrappers, and the unprotected `tests/window.rs`;
      - winit and headless override them; headless records the requests, and
        winit reads the refresh rate from the window's monitor;
      - a wrapper that does not forward a request drops it, so the new rules
        are a separate suite, and `check` and `check_scripted` keep their
        expectations;
      - `PlatformEvent` and `App` do not change.
    - Frames come at the refresh rate when the window reports one, else once
      per tick; ticks still come from the timer. Xvfb has no vsync (1b's T11),
      and that it reports no refresh rate is a guess T13 checks. The
      camera's position is interpolated at alpha, and its angles come from
      live input between ticks (presentation only, contract 4).
11. **The GCMouse shim** *[spec 6.10 + proposal; ADR-0021; O3]*.
    - The methods it needs in `objc2-game-controller` 0.3.2 (`mice`,
      `current`, `mouseInput`, `setMouseMovedHandler`) are all `unsafe fn`.
    - The workspace lint `unsafe_code = "deny"` refuses `unsafe` wherever the
      workspace lints apply. The shim's crate allows it in one module, with a
      `SAFETY` comment on each block.
    - Its handlers run on the main queue, add into a sum the winit platform
      drains in `about_to_wait`, and count their calls. The drain follows the
      frames by design, so the callback count is what shows the mouse's rate.
    - **The rule:** no other crate may `allow(unsafe_code)`. `arch-check` (G5
      patch) fails on one, and the selftest `unsafe-outside-shim` proves it.
      Crates with `#![forbid(unsafe_code)]` keep it; the core set must have
      it, and `game-testbed` and `xtask` never did.
12. **Shots** *[spec 8.3 + proposal]*. First-person shots take player 0's
    camera at the tick, at alpha 1. The list moves from `shots.rs`'s
    constant into a protected `crates/game-testbed/shots.toml` (names,
    ticks, inputs; poses with A3). Per-shot FLIP limits live with the
    goldens, in the owner's `golden/thresholds.toml`, as `[flip.<shot>]`
    tables beside today's `[flip]` (spec 15: "per test"). Proposed shots:
    `room-spawn`, `-step`, `-alcove`, `-sky`, `-sprites`, `-ledge` and
    `-look-off` (every switch off).
13. **Structured look checks** *[spec 8.3 + proposal]*, exact on every
    adapter because they compare a machine with itself or test a property:
    every switch off gives the plain pipeline's image; with the palette on,
    every pixel is a palette colour; on a flat material, banding gives at
    most 32 levels per region; fog moves monotonically toward its colour with
    depth along a probe row; the tone curve is within one 8-bit level of a
    CPU reference on a ramp.
14. **Benchmarks** *[spec 6.13 + proposal; O7]*: instruction counts on
    `ubuntu-24.04` x64. `cargo xtask bench` builds the pushed head and the
    push's base on one runner. The base is the push event's `before`, or a
    new agent branch's merge base with `main`; the parent would see only the
    last commit. Over 5% fails unless the push adds a performance ADR naming
    the benchmark. There is no stored baseline to go stale.
15. **Mutation testing** *[spec 7.2 + proposal; O8]*.
    - On a push that changes a core crate, `cargo mutants --in-diff` runs on
      the diff from the push's base, in 4 shards of at most 20 minutes. Not
      `origin/main`: an owner push of commits an agent branch already
      carried (ADR-0018) would give an empty diff.
    - A push that changes no core crate writes a **pending** `in-diff` step,
      as 1b's X2 defines it: not a pass, failing no job. (A job with no report
      fails, `acceptance.rs:48-52`.)
    - **The rung's run.** The push after G5 carries only G5's files, so
      `in-diff` may stay pending. So the weekly schedule, and a manual
      dispatch the owner can start, also run a `rung-diff` step: the diff
      from the commit that added the current rung's `acceptance.json` (G1),
      which holds every core change of T5-T22. Criterion 19 binds to it.
    - A missed mutant fails unless `.cargo/mutants.toml` (protected) excludes
      it. A weekly full run is reported, not failed.
16. **docs-lint** *[spec 7.4 + proposal]*: `docs-check` also checks that
    every `cargo xtask` command, probe command, path, `ENG_*` variable and
    `[E-*]` tag named in `AGENTS.md`, `docs/ARCHITECTURE.md` and
    `.claude/skills/*/SKILL.md` exists.
17. **Notices and dist** *[spec 9 + proposal; 8.4 describes rung 7's
    builds]*.
    - `cargo xtask notices` runs cargo-about over the testbed binary's graph,
      with deny.toml's licence list.
    - `cargo xtask dist` makes `target/dist/<target>/`: the release binary,
      its debug info apart, `version.json` and the notices. Stripping and
      split debug info come from `--config profile.release.strip=...` and
      `--config profile.release.split-debuginfo=...` on xtask's cargo
      command line, so no `[profile]` table changes; T20 picks the values per
      target.
    - The archive is a `.tar.gz` that xtask writes itself: tar headers in a
      few lines of code, gzip through `flate2`, already allowed (batch 2,
      through png). Windows 10 and later unpack it with the built-in `tar`.
    - CI makes one on `ubuntu-24.04`, `windows-2025` and `macos-26` as an
      artifact. No tags, release or signing.

## Where this differs from the spec, and why

1. **The graph's shape.** ADR-0019 records the mechanism; ADR-0020 records
   the first-person passes and their tail below, amending ADR-0003 and
   ADR-0017.
   - Passes write several targets and continue earlier passes.
   - A display-referred palette pass follows the tone map, so "tone mapping
     is the last graph pass" becomes "the last scene-referred pass". The
     palette maps display colours and cannot run before it.
   - The integer upscale is in `present`, so a window shows a shot's pixels
     scaled k×k rather than one to one.
2. **Sim data and a map selector arrive in rung 2.** 6.11 brings sim data
   with the level compiler (rung 3). The room's walls cannot ride in a
   1,200-byte snapshot, and 1b's oracles would otherwise change (decision 1).
   The handshake's sim-data hash (6.8) still waits for rung 5.
3. **The collision oracle is brute force, not parry3d** (6.1). For boxes
   against wall segments at any angle and flat slabs, an exhaustive loop
   over every primitive is a complete oracle. `parry3d-f64` 0.31.1
   resolves to 39 crates, among them glam, simba, wide and spade (O4).
4. **The GCMouse shim is a crate of its own with `unsafe`.** 6.10 says "a
   small shim"; the binding forces `unsafe` (decision 11).
5. **"dist" is `cargo xtask dist`, not the `dist` tool.** cargo-dist
   (0.34.0) generates a release workflow that writes to the repository, and
   its `init` adds `[profile.dist]`, a protected table. That is from its docs
   as known before this draft, not re-checked against 0.34.0. 8.2 wants
   xtask as the only entry point, pinned actions and `permissions: {}`; xtask
   reaches the same binary with `--config` and an archive of its own
   (decision 17). The tool stays an option at rung 7 (O5).
6. **Benchmarks run on every push**, not only nightly (8.2), against the
   push's base rather than a stored baseline: with direct commits to `main`
   (ADR-0014) no PR holds one. T17 measures the job's time. Likewise
   "required only when core-set crates change" (7.2) means on a push.
7. **The owner's Mac is part of "goldens on every adapter"** (8.3, fourth
   row), as an owner-checked criterion; CI cannot see that GPU.
8. **Docs-lint also reads `ARCHITECTURE.md` and the skills**, not only the
   AGENTS.md files (7.4).
9. **Fullscreen by default only for first-person maps** (6.10 says by
   default). The dot map is a debug view and keeps 1b's window, which the
   protected `play.rs` runs.
10. **Deferred:**
    - `eng-assets`, beyond the palette's sidecar;
    - hot reload (rung 3, as 6.11 says);
    - mipmaps; HUD and text (`eng-ui`);
    - gilrs (rung 6); the App Nap assertion (rung 5);
    - Miri and live control;
    - the coverage floor and the nightly tier (8.2 names no rung; proposed
      for D1);
    - `bench --scene` (7.1): its wall-clock and GPU numbers need real
      hardware (6.13, P7, P8), and `eng-bench` gives instruction counts;
    - cargo-public-api snapshots of contract crates (7.4): they need a
      pinned nightly (spec 4) and protected snapshots. Rung 2's contract
      changes are covered by ADR-0022's and ADR-0024's migration notes.
      Proposed for D1.

## Agent seeing: five additions for the owner to take or drop

None of these is in the rung unless the owner takes it at G1.

**Why they are here.**
- The owner's aim (2026-11-01): "changing the industry would mean ai agents
  can develop games without human feedback". A review that day, run at
  commit 353872d (ADR-0018's push commit), with its notes kept outside the
  repo, asked whether agents can see enough to check a generated scene.
  Its verdict was no: enough to guard the fixed 1b testbed against code bugs,
  with the owner playing at G7, but not a scene nobody prepared, and not
  rung 2.
- The review's findings, from one Mac and one 2D scene, are not a detection
  rate. Its scratch files are gone and nothing below was re-measured:
  - Of 14 planted render bugs, `quick` caught 14, 10 of them only through
    tests hard-coded to the testbed; `shot` flagged 1; replays and
    assertions flagged 0.
  - A dot skipped when touching the east wall passed everything.
  - A vision model given scene text caught 9 of 12, and 12 of 12 given the
    world state. A script with no model also caught 12 of 12. The state did
    the work, not the eyes.
  - FLIP passed a missing 16 px orange object. That fits the limits in
    `thresholds.toml`: p999 is the 77th-worst of 76,800 pixels, so fewer
    than 77 wrong pixels cannot move it.
- A related experiment: on 2026-11-03 the owner had a hand-weighted reader
  that turns top-down frames into text built outside the repo. It was not
  re-run here, and a first-person view would need new numbers.
- The spec's limits still hold. 2.1 and 2.2 make the owner the tester for
  feel and fun, and 2.3 and 7.2 keep vision models advisory. 8.3's
  "object-ID buffer, depth probes, luminance histograms" is one sentence with
  no rung.

**What the base plan already gives:** the `info` target holds every pixel's
object id, there are per-shot FLIP limits, and the exact look checks of
decision 13.

| | Addition | What it adds | Acceptance it adds (job: step) | Task, units | Owner time |
|---|---|---|---|---|---|
| A1 | **Frame record** | `shot` and `video` write `<out>.frame.json`: the camera; every object the extraction asked for (id, kind, world box); then, from `info` and `output`, each object's visible pixels, screen box, mean luminance and contrast against its surroundings; depth at a fixed 8x6 grid of probes; a luminance histogram | `frame-record`: a record for every shot; exact on the pinned lavapipe, within per-shot tolerances elsewhere; a planted "not drawn" bug and a "drawn black" bug each change it. native x5: frame-record | TA1, 2 | none |
| A2 | **Rules for "proper"** | Protected `<scene>.rules.toml`, like assertions, checked by a script over frame records. Three starter rules: an object the manifest marks `must_see` has at least 50 visible pixels and contrast at least 0.1; no first-person pixel shows the clear colour; floor shows in the lower third and ceiling or sky in the upper third | `scene-rules`: every shot passes; each rule has a selftest that breaks it. native x5: scene-rules; selftest: violation:rule-1 to rule-3 | TA2, 1 (needs A1) | about 20 min to approve the rules |
| A3 | **First-person poses** | `shot --pose x,y,z,yaw,pitch`, a camera independent of movement; `shots.toml` names poses; the report lists the views checked | `fps-poses`: every named pose renders on every runner, and the report's `views` equals the manifest. native x5: poses | TA3, 1 | none |
| A4 | **What each check misses** | 12 planted render bugs, written by an agent that did not build the renderer, as patch files that `cargo xtask selftest render-bug-<n>` applies to a workspace copy, as selftests do now; one deletion run that switches each check off in turn; a bug-by-check table in `PROGRESS.md` | `check-misses`: every planted bug fails a check that is not hard-coded to one scene, or is a known miss the owner accepted. selftest: render-bug-1 to render-bug-12 | TA4, 2 | about 10 min |
| A5 | **Two texts per frame** (the owner's idea) | One text from game state (what should be seen, by a CPU ray cast of the map from the camera, independent of the GPU), and one from the pixels (A1's record). A script compares them line by line and fails on a difference; a model may explain a difference, never decide (7.2) | `two-texts`: no difference on every pose and over a 600-frame walk; each A4 bug that changes what is seen shows one. native x5: two-texts | TA5, 2 (needs A1) | none |

**What I recommend, and why.**
- **Take A3 and A1.** First-person goldens need poses anyway. The id target
  exists for fog, so the record is cheapest while the passes are being
  written, and a missed 16 px object cannot hide from it.
- **Take A4 small**: it is the only way to know what the others buy, and the
  selftest machinery exists.
- **Take A2's three rules** if A1 is taken: a record without rules decides
  nothing.
- **Leave A5 for rung 3**, whose fuzz maps are the first scenes nobody
  prepared. One hand-built room gives it little that goldens and A1-A2 miss,
  and A1 builds its pixel side now.
- **Cost**, scaled by 1b's 1.37 sessions per unit like the base rung: this
  set is 6 units, 8.2 sessions rounded up to 9 (52 in all); all five are 8
  units, 10.96 rounded up to 11 (54). The time-box is unchanged.

## Proposed acceptance criteria

The draft for `docs/rungs/2/acceptance.json`, which the owner writes (G1).
Job and step names are proposals; the new steps and jobs come from the G5
patches. "native x5" means the five runners of spec 8.2. The rung 2 row has
no numbered done-when list, so its items are numbered here: (1) the
first-person room, (2) movement and collision, (3) the graph, (4) the retro
pipeline and look stack, (5) sprites, (6) the GCMouse shim, (7) goldens on
every adapter, (8) benchmarks, (9) docs-lint, (10) notices and dist, (11)
mutation testing, (12) the owner plays (touchpoint 1).

| # | Feature | Criterion | Checks (job: step) | Item |
|---|---|---|---|---|
| 1 | `room-window` | `play --map room --windowed --ticks 600 --seed 1 --input room-walk.input.toml --hashes h.txt` presents first-person frames in a real window, and `h.txt` equals `hashes/room-walk.seed1.txt` | native x5: window-room | 1 |
| 2 | `replay-cross-platform` | As 1b's criterion 7, now with the room scenarios. The five dot streams are unchanged (unless O12's fix-now answer changes them, regenerated with the owner's approval); the room streams are identical on every target, sim-data hash included | test x2, riscv, native x5: replay; summary: compare-hashes | 1, 2 |
| 3 | `collision-grid` | The grid passes the collision conformance suite with ADR-0024's contact rule (exact on axis faces, within `CONTACT_GAP` off the axes), and gives the brute-force oracle's `Trace` bits on proptest maps and sweeps | test x64, arm64: collision; riscv: core-tests | 2 |
| 4 | `ground-movement` | Q2's tests (steps up to 0.4 m climb and 0.41 m blocks, snapping, gravity only in the air, no slope climbing, never inside solid), and its flat-ground proptest over the grid | test x64, arm64: movement; riscv: core-tests | 2 |
| 5 | `sim-data` | The room's snapshot fits 1,200 bytes; sim data is outside the state, its hash and snapshots; `sim_data_hash` is in recordings, reports and the room's hash-stream header and changes with the room's data only; the dot map's is 0 | test x64, arm64: sim-data | 1, 2 |
| 6 | `render-graph` | Q1's oracles: any declaration order gives one graph or one error; culling, bypass, continues and pooling as ADR-0019 says; labels set. `Graph::new`, `Graph::fixed` and 1b's counters unchanged | test x64: render-graph (`graph_order`, `graph_errors`, `pool`, `bypass`); native x5: tests (`bypass_render`, `transients`) | 3 |
| 7 | `look-stack` | Decision 13's exact checks, and each look pass switches off alone | native x5: look | 4 |
| 8 | `sprites` | Q4's rotation tests (an independent f64 `atan2` oracle, the hand-checked cases, the tie rule, mirror flags); Z-only billboards; alpha test with depth (the thing behind the pillar is hidden) | test x64, arm64: sprites; native x5: look | 5 |
| 9 | `mouse-look` | Q3's oracle (decision 9's rules); the same motion timeline gives the same commands on every target (proptest) | test x64, arm64: mouse-look | 6 |
| 10 | `cursor-fullscreen` | The headless platform records cursor and fullscreen requests (the new suite); `play --report` names the grab mode, fullscreen and refresh rate; focus loss releases the cursor | native x5: window-room, tests | 1, 6 |
| 11 | `gcmouse` | On macos-26 the shim builds and its tests with a fake source pass. The runner has no mouse, so nothing here reads a real GCMouse | native macos-26: tests | 6 |
| 12 | `shots-exact` | Every manifest shot, 1b's four and rung 2's first-person ones, equals its golden exactly on the pinned lavapipe | native ubuntu-24.04: shot-exact | 7 |
| 13 | `shots-flip` | Within each shot's FLIP limits on the other four runners | native (other 4): shot-flip | 7 |
| 14 | `owner-mac-shots` | The shot harness on the owner's M2 Pro is within the limits; `shots.json` records the macOS and Metal driver versions. An agent runs it on the Mac and the owner confirms | the owner | 7 |
| 15 | `benchmarks` | Instruction counts for both sim steps, grid traces and `ground_move`; over 5% against the push's base fails without a performance ADR | bench: bench | 8 |
| 16 | `docs-lint` | Decision 16; a stale path fails | lint: docs-check; selftest: violation:docs-stale-path | 9 |
| 17 | `notices` | Notices list every third-party crate in the testbed binary for the three playable targets, with licence texts; a licence off the list fails | lint: notices | 10 |
| 18 | `dist` | The archive holds the stripped binary, its debug info, `version.json` and notices; the unpacked binary runs `--version --json` and `run --headless --ticks 60` | native ubuntu-24.04, windows-2025, macos-26: dist | 10 |
| 19 | `mutants` | Over the rung's core changes (the diff from the G1 commit), no missed mutant outside the exclusions (decision 15) | mutants: rung-diff | 11 |
| 20 | `violations-2` | Each fails, with a selftest | selftest: violation:unsafe-outside-shim, violation:gcmouse-outside-shim, violation:docs-stale-path, violation:mutant-missed | 6, 9, 11 |
| 21 | `gpu-report-budgets` | As 1b's criterion 9, plus a first-person room shot within a new `[gpu.first_person]` table (the step renders one from G5), and per-pass GPU times (or `null`) and the pool's texture count in new `gpu` keys. `per_frame`, `tier: 0` and the capability names stay as `shot_video.rs` pins them | native x5: gpu-budgets | 3 |
| 22 | `proof-video` | As 1b's, plus `room.apng`, the 600-tick room walk | native ubuntu-24.04: proof-video | 12 |
| 23 | `owner-play` | On the owner's Mac: fullscreen, mouse look, WASD, the step, turning sprites, `--look` switches, Esc quits; the owner plays and accepts | the owner | 12 |
| 24 | carried from 1b | `native-build-test`, `native-window` (the dot window), `esc-quits`, `violations-1b`, `crash-json`, `version-json`, `shaders`, `wire-session` (unchanged: no wire change), `live-input`, `lint-tier`, `tests-and-targets` (the core set adds `eng-collision-grid`), `agents-md-length`, `native-duration` | as in 1b's `acceptance.json` | all |
| 25 | `gcmouse-rate` | On the owner's Mac, while the mouse moves fast, the shim's callbacks per second exceed the display's refresh rate (`play --report`). An agent runs it there and the owner confirms; `owner: true` with no checks, as `acceptance.rs` requires | the owner | 6 |

A taken addition adds its row from the agent-seeing table.

## The pre-approval queue, as built

Four items, built 2026-11-06 before G1, each in a pinned worktree, reviewed
and cherry-picked onto `main`; `docs/rungs/2/PROGRESS.md` has their commits,
reviews and costs. None touched a protected path or added a crate or a
dependency; 1b's five hash streams and four shots stayed byte-identical (the
shots' SHA-256s still begin `c6cb0547eb48a67e`, `bd264452cd2f5ef4`,
`d1726b1ecac42ebc` and `8bb9845b5e6aa2c2`, as in run 41302087332).

| Item | Built | Picked up by |
|---|---|---|
| Q1 (T1), `eng-gpu` | `Graph::sorted` and the rest of decision 5, in "What already exists"; ADR-0019 | T8 (several writes, `Format`, timestamps); G4 (ADR-0019); G5 (protect its tests) |
| Q2 (T2), `kit-fps` | `ground_move` and decision 3. Its review found a slope-climbing blocker (air control plus an unchecked stepped move), fixed to the decision as written | T5 (contact, flat-ground proptest over the grid), T7, O12, O13 |
| Q3 (T3), `eng-input` | `LookSettings` and `MouseLook`, decision 9 | T15 (live angles, `--sensitivity`, pitch limit); G5 |
| Q4 (T4), `eng-render` | Camera, sprite rotations, palette LUT and Bayer matrix | T9 (near distance), T10 (LUT indexing), T11; G5 |

## Before the first sitting

Three planning tasks, so that every gate of the first sitting has its inputs
ready; sitting 1 waits until all three are done. Like the queue, they change
no protected path on `main`: they commit ADR drafts (`docs/adr/` is not
protected) and patch files under `docs/rungs/2/patches/`, tested in a
scratch clone. **State now:** T0a is drafted (ADR-0020 to ADR-0024 in
`docs/adr/`, Proposed, reviewed twice); T0 and T0b are done (seven patches
in `docs/rungs/2/patches/`, reviewed).

| Task | Builds | Done when |
|---|---|---|
| T0 | The G2 patch: skeletons of the four crates (crate roots with the required attributes, `Cargo.toml` with `[lints] workspace = true`), their `architecture.toml` entries and new edges, and `overflow-checks` for `eng-collision-grid`, in one commit's worth, since `arch-check` fails on a listed crate that does not exist as on one not listed (1b's T0). The shim crate's lines are a separate patch, so O3's other answers apply without it | `quick` green in a scratch clone with the patch applied, with and without the shim patch |
| T0a (drafted, under review) | ADR drafts, all "Proposed": ADR-0020 (the first-person passes and the GPU scene layout, decision 6; it also records difference 1, the palette after the tone map and the k×k upscale in `present`, amending ADR-0003 and ADR-0017); ADR-0021 (`unsafe` in the shim, decision 11); ADR-0022 (the platform contract: `Control`'s requests and `Window::refresh_rate`, with decision 10's migration notes); ADR-0023 (maps and sim data, decision 1); ADR-0024 (traces stop at contact: exactly on axis faces, within `CONTACT_GAP` off them; decision 2, with migration notes); and the ADR index | `docs-check` and `typos` pass; each names what it amends and how it migrates; a fresh `reviewer` reads them against this plan |
| T0b | The G3 patch: crate batch 3 in `deny.toml`'s allow-list, the wrapper ban for `objc2-game-controller` (naming the shim crate, or `eng-platform-winit` if O3 picks a module), a `clarify` entry if `bincode-next`'s licence needs one, `_typos.toml` if needed. Also a separate parry3d patch, resolved against the real lock, so O4's other answer can be applied at the same sitting | `cargo deny check` passes in a scratch clone that adds batch 3's crates to `eng-bench` and the shim crate with T0's patch and this one applied, and again with the parry3d patch |

**If the owner answers O3-O5 otherwise at sitting 1:**
- **O3 as a module:** G2 applies T0 without the shim patch; the wrapper ban
  names `eng-platform-winit`; ADR-0021 changes its crate. **No shim:** the
  same, without the ban; criteria 11 and 25 drop.
- **O4 parry3d:** G3 also applies the parry3d patch; T5 builds both oracles.
- **O5 cargo-dist:** it needs a `[profile.dist]` table in the root
  `Cargo.toml` (protected) and the workflow it generates. T20 prepares both
  as a G5 patch, the G5 workflow row gains them, and difference 5 drops.

## Proposed time-box and AI budget (for the owner to set)

**How the numbers were reached, from 1b's actuals.** 1b planned 17 tasks and
two reviews (19 units) and used 31 sessions, five of them planning. So each
planned unit cost 26 / 19 = 1.37 sessions, counting follow-ups, review fixes,
patches and the first-run fix.

- **Used so far** (`docs/rungs/2/PROGRESS.md`): 2 planning sessions and 4
  queue sessions (Q1 to Q4, each with its review and fixes), 6 in all. The
  second and third revisions are two more planning sessions: 8.
- **Still to come,** counted as 1b's were, so nothing counts twice: T0, T0a,
  T0b and T5-T22, one unit each (21); T23, the two rung reviews, as 1b
  planned them (2; their fixes are in the 1.37); T24, collecting candidates,
  as 1b's T16 (1; first-run fixes are in the 1.37); T25, as 1b's T17 (1).
  25 units × 1.37 = 34.25, rounded up to 35.
- **Total: 8 + 35 = 43.**

**The proposal.**
- **AI budget:** the owner's current Claude plan and nothing else (P1
  default). At most **43 agent sessions** for the base rung, **52** with the
  recommended additions, or **54** with all five, logged in
  `docs/rungs/2/PROGRESS.md`. As in 1b, a task with its reviewer is one
  session.
- **Time-box:** **14 calendar days from G1**, as in 1a and 1b.
  - **Agent time:** 35 sessions at 4 to 6 per five-hour window is 6 to 9
    windows. The 2026-11-06 window ran 2 planning sessions, a plan review and
    the 4 queue items with their reviews (2.5M to 2.9M subagent tokens by
    PROGRESS.md's costs table, which counts the planner's revision "in all")
    before the limit stopped it after about 2 hours 15 minutes; in 1b, four
    builders and their reviewers used one in under three hours. At two or
    three windows a day that is 2 to 4.5 days.
  - The rest is the owner's three sittings and the new jobs' first runs. In
    1b the owner's gates were the long pole: G6 and G7 are still open six
    days after green CI.
  - The pivot rule (7.7) compares rungs 1a-3 with twice their boxes; 1a took
    one day of 14.
- **Usage limit:** at most two builders at once, a large task paired with a
  small one. A builder stopped by the limit keeps its context and resumes
  after the reset (1b lesson).
- **Owner time** (estimate): three sittings of about an hour each ("Owner
  gates").
- **CI:** free public runners. `bench` and `mutants` run beside the PR tier,
  timed by `budgets.toml [ci]` (G5).

**After rung 2: deload D1** *[spec 7.7]*. D1 follows at once and adds no
features, so rung 2 must not push its work into it. It takes: duplicate code
between the top-down and first-person paths; an audit of AGENTS.md, which is
full; refreshing ARCHITECTURE.md and the ADR index; the performance debt the
first benchmarks show; non-floor patch bumps; reviewing the time-boxes of 1a,
1b and 2 against the pivot rule; the owner's call on a nightly tier and the
coverage floor (8.2); and cargo-public-api snapshots.

## Tasks

All *[proposal]*. Each task ends with `cargo xtask quick` green and one
commit, which the orchestrator reviews and cherry-picks onto `main`; crate
tasks get a `reviewer` pass, and for core code a reviewer that re-implements
the rule independently (Q2's f64 port found the queue's one blocker). The
orchestrator writes the `PROGRESS.md` entry from the builder's report.
Builders work in worktrees pinned with `git worktree add --detach`, which
have their own `target/`, so replay runs the worktree's own binary. A brief
for a planted-bug or mutation run says that proptest writes
`*.proptest-regressions`, a protected path, to be deleted in the worktree.
Protected changes are patch files in `docs/rungs/2/patches/`, made with
`--full-index`, tested in a scratch clone, and applied by the owner.
**Carry:** notes are hand-offs from 1b or from an earlier task; they live
here, because a note left only in an entry gets lost (1b lesson).

| Task | Needs | Builds | Done when |
|---|---|---|---|
| T0, T0a, T0b | none | "Before the first sitting" | as there |
| T1-T4 | none | the queue, built | as built |
| T5 | G2, G4 (ADR-0024); G3 if O4 picks parry3d | `eng-collision-grid` (decision 2); ADR-0024's contact rule and `CONTACT_GAP` in `eng-collision-api` and its conformance suite; the brute-force oracle test (to be protected); RISC-V. **Carry:** `BoxRoom`'s clamp lands a hull exactly on a wall (`dot-corner` asserts `x == 7.75`); the grid does the same on axis faces. **Carry (Q2):** with contact exact, both outcomes of decision 3's step-move tie give the same bits on flat ground in any world, so a flat-ground run cannot guard the tie rule. It matters only where a downward trace's end depends on where it starts: floors off the axes, under `CONTACT_GAP` | criterion 3's tests pass; `arch-check` passes; `kit-fps`'s `flat_ground_gives_pmoves_result_bit_for_bit` (through `same_as_pmove`) passes over the grid too; if the grid has floors off the axes, a test there checks the tie rule |
| T6 | G4 (ADR-0023) | Maps and sim data (decision 1): constant resources and `Sim::sim_data_hash`; `GameMaps` and the new entry points; `--map`; the map and sim data in recordings, hash-stream headers and reports; `replay verify` choosing the map by its hash. Patch: xtask replay reads `map` from `scenarios.toml`. **Carry:** "Protected tests that pin today's APIs" | the dot streams are byte-identical; a sim with a constant resource snapshots and hashes its state without it; `hash_equality.rs`, `never_panic.rs`, `twin.rs`, `version.rs` and `exit_codes.rs` pass unchanged |
| T7 | T2, T5, T6 | The `room` map in `game-testbed-sim` (the room, spawns, things, `ground_move`); the room's input scripts, **committed directly** (they are not protected, and replay rejects only `.assert.toml` files no scenario names, `replay.rs:92-109`), so T15, T16 and A3 can use them; and one patch, `room-scenarios.patch`, with the three `scenarios.toml` entries (`room-walk`, `room-ledge`, `room-corner`), their `*.assert.toml` and their hash streams. **The streams wait in the patch:** committed before their scenarios, `xtask replay` reports `[E-HASH] … in committed but not in <target>` and `quick` goes red. The owner applies it at G5 with T6's replay patch; an agent then runs `quick` and commits. **Carry:** `[sim]` budgets are 4,096 state bytes and 16 entities. **Carry (Q2, O12):** the room has no angled wall; if O12 fixes `slide` anyway, the dot streams may change; regenerate only if they do, with the owner's approval | criterion 5's tests; in a scratch clone with both patches applied, `quick` is green and the room streams match on the targets this Mac can run (arm64, Docker x64, QEMU riscv) |
| T8 | T1, G4 (ADR-0020), O11 | `eng-gpu`: a running pass with several colour targets and a depth target; `Format` widened (decision 5); textures; the per-frame uniform; the scene layout (decision 6); timestamps per pass if O11 says so, asking for `TIMESTAMP_QUERY` only when the adapter has it. **Carry:** Q1 already built the pool in `Targets` and `LoadOp::Load`; 1b's one instance buffer per frame and one input bind group per pass are what megabuffers replace; exposure needs the uniform (ADR-0017); the surface format stays `Bgra8UnormSrgb`; new report data goes in new `gpu` keys, never in `per_frame` or `capabilities`, and `tier` stays 0 (`shot_video.rs`); `FrameCounters` cannot gain fields (`counters.rs` builds it by literal at lines 140, 166, 199 and 223), so per-pass times and the pool's count come back in a new type beside it | offscreen depth, several-target and texture-array tests on Metal and Docker lavapipe; `counters.rs` and Q1's tests unchanged |
| T9 | T4, T8 | `eng-render`: the `opaque` and `sky` passes, camera-relative chunk draws, `info`. **Carry (Q4):** the plan sets no near distance; Q4's tests use one under 1 m, and the 0.25 m hull half-width bounds it from above, so pick it here (0.1 m suggested) | a known room renders with depth probes as computed on the CPU |
| T10 | T9 | `eng-render`: the `light`, `fog`, `tonemap` (decision 7) and `palette` passes, the `.gpl` reader, the k×k present, the switches. **Carry (Q4):** sampling `toned` (`Rgba8UnormSrgb`) returns linear values, so the palette pass reads it through an `Rgba8Unorm` view or re-encodes first | decision 13's checks pass on Metal and lavapipe |
| T11 | T9 | `eng-render`: the `sprites` pass (decision 4), continuing `opaque`. Built beside T10 in separate modules; T10 lands first if both touch `renderer.rs` | criterion 8's tests |
| T12 | G2, T7, T9, T11 | `game-testbed-client`: the top-down extraction moved unchanged; the first-person extraction; region looks; textures and sprites made by code; `palettes/room.gpl`, its generator script and its provenance sidecar. **Carry:** top-down colours are sRGB-level centres at 40 px/m; `view_yaw` 0x4000 is for the dot map only; `GameDef` keeps its fields (`twin.rs:33`) | the four 1b shots are pixel-identical; a first-person shot renders |
| T13 | G4 (ADR-0022) | The platform contract change (decision 10): `Control`'s requests and `Window::refresh_rate` with default bodies; headless records requests; winit maps mouse motion, buttons and wheel and reads the refresh rate. Patch: the new suite (a file under `src/conformance/` and its `mod` line) and new headless and winit tests for it, to be protected. **Carry (1b T10):** the contract's wheel `y` keeps winit's sign and its `x` is winit's negated; under Xvfb a notch came twice; winit hides macOS's natural scrolling and acceleration; raw motion comes only while focused | the new suite passes headless and on winit where a display exists; the existing suite, `eng-platform-headless/tests/conformance.rs`, `eng-platform-winit/tests/conformance.rs` and `twin.rs` pass unchanged |
| T14 | G2, G3, G4 (ADR-0021), T13 | `eng-platform-gcmouse` (decision 11) with its callback count; `examples/events.rs` prints its deltas. **Carry:** GCMouse's sign of `deltaY` and its units are unverified; check them on the owner's Mac with the screen unlocked | builds on macos-26; unit tests with a fake source; on this Mac, callbacks arrive per event |
| T15 | T3, T10 (`--look`), T12, T13 | `eng-app`: first-person `play` (decisions 9, 10), the flags, the live view angles, through new functions; `play_on` and `PlayRun::start` keep their signatures (`twin.rs:76,84`). **Carry (1b T11):** waking at once after each present spins under Xvfb (no vsync); a lost surface is a panic and should now be recreated from the kept window; a locked Mac screen counts as occluded. **Carry (Q3):** `MouseLook` has no reader for the angles between ticks (`sample` consumes the pending motion), so the live view needs one; sensitivity is integer millionths, so `--sensitivity 0.066` needs an exact decimal parse (at most six decimals, refused beyond); the default pitch limit is 14_563 units, and `degrees_to_angle(80.0)` gives 14_564, so a `--pitch-limit` in degrees must round down | `play.rs` and `twin.rs` pass unchanged; a windowed room run's hash stream equals `run --headless` with the same script and seed (the committed stream lands at G5) |
| T16 | T10, T12 | Shots: `shots.toml`, the first-person shots, `--view`, the look harness `tests/look.rs`, and `shots.rs` reading the manifest and per-shot limits, all as patches. In `eng-cli`, the probe's strict budgets parser (`budgets.rs:13,33,43`) gains an optional `[gpu.first_person]` table, applied to first-person frames, before G5 adds it to `budgets.toml`. **Carry:** pass `LP_NATIVE_VECTOR_WIDTH=256` only; candidates go to `target/golden-candidates/`; p999 cannot see under about 77 px | candidates from Metal and lavapipe |
| T17 | G2, G3, T5, T7 | `eng-bench`; `cargo xtask bench` and `ci --job bench` (patch); the workflow job with valgrind and `gungraun-runner` =0.20.0; comparison with the push's base (decision 14). **Carry:** gungraun only on Linux, so `cargo test --all-targets` still builds on the other four native runners | a local Docker run on Linux x64 reports counts for a head and its base; `eng-bench` builds on this Mac |
| T18 | G1 | Docs-lint in `docs-check` (patch) and its selftest | a stale path in a scratch copy of AGENTS.md fails |
| T19 | G1, O8 | `cargo xtask mutants` and `ci --job mutants` (shards, time-box; `in-diff` from the push's base, pending when no core crate changed; `rung-diff` from the commit that added the current rung's `acceptance.json`), the weekly schedule and a manual dispatch, `.cargo/mutants.toml`, the `test-auditor` subagent (spec 7.5), all patches | an in-diff run on T2's change reports its mutants; a push with no core change writes a pending step; `rung-diff` in a scratch clone covers Q1-Q4's core changes |
| T20 | G1, O5 | `cargo xtask notices` and `dist` (decision 17), `about.toml` and its template (to be protected), patches | notices and an archive on this Mac; Linux in Docker; the unpacked binary runs |
| TA1-TA5 | only if taken at G1 | A1: T10, T16. A2: TA1. A3: with T16. A4: T16, before T21 (its selftests join the G5 package). A5: TA1 | as in the agent-seeing table |
| T21 | T15-T20 (and TA rows taken) | The G5 package: new `test` and `native` steps, `gpu-budgets` also rendering a room shot under `[gpu.first_person]` (today it renders dot-square only, `ci_native.rs:384-415`), the `selftest` cases, the summary and acceptance for the new jobs, workflows, `budgets.toml`, protection, and `current.txt` set to `2` (the G5 rows). **Carry (1b):** build26's Mesa has no window surface, so Ubuntu's lavapipe goes first in `VK_DRIVER_FILES` for windows; find windows by `--pid`; on Windows a running `xtask.exe` cannot be replaced and `canonicalize` gives `\\?\`; Xcode's git defaults to `main` | each job's steps pass where they can run locally; in a scratch clone with the package applied and `current.txt` at `2`, `quick` is green, `ci_native_tests.rs`'s check of every native step included |
| T22 | T15-T20 | Docs, beside T21: `AGENTS.md` (150 lines already: every new line replaces one), `docs/ARCHITECTURE.md`, ADR-0019 to ADR-0024 brought up to what was built, the `perf-investigate` skill (patch) | `docs-check` passes, docs-lint included |
| T23 | T21, T22 | Two fresh-context `reviewer` passes over the rung, then fixes | no open blockers |
| T24 | G5 | The first CI runs after G5: fixes, golden candidates and FLIP maps (`golden-update` skill), `owner-mac-shots` on this Mac. Fixes to xtask or the workflows go to the owner as one patch (G5b), as 1b's `ci-first-run.patch` did (6443ea5) | the owner has every candidate for G6, from a green run |
| T25 | T23, T24 | `room.apng` and a short scripted session for the owner | the owner accepts (G7) |

**Order, with at most two builders at a time:**
1. done: T1 with T2, then T3 with T4;
2. before sitting 1: T0 with T0a, then T0b;
3. **sitting 1** (G1-G4);
4. T5 with T6, T7 with T8, T9 with T13, T10 with T11, T12 with T14, T15
   with T16 (and TA3), T17 with T18, T19 with T20, then the TA rows taken;
5. T21 with T22, then T23;
6. **sitting 2** (G5); T24, with G5b if the first run needs it; then T25;
7. **sitting 3** (G6, G7), after a green run.

## Owner decisions this rung needs

- **O1. Approve** the plan, acceptance, time-box and budget (G1), and write
  `docs/rungs/2/acceptance.json`. **`current.txt` moves to `2` at G5, not
  G1.** From the moment it says `2`, `quick` runs
  `ci_native_tests.rs:42-70`, which reads the current rung's
  `acceptance.json` and fails unless every native check names a step that
  `ci_native.rs` runs. `window-room`, `look` and `dist` exist only from G5's
  patch, so setting it at G1 would leave `quick` red through T5-T22. The
  other readers are harmless: `acceptance::compute` (CI's summary) reads only
  the current rung's file, so it keeps reporting 1b until G5, and the
  session-start hook only prints the name. Briefs point builders at rung 2's
  plan meanwhile. (The other fix, pending stub steps in xtask at sitting 1,
  would add an xtask patch to that sitting; this one needs none.) Close 1b
  first or in the same sitting: its G6 (ready, with commands in
  `docs/rungs/2/PROGRESS.md`), its G7 and ADR-0017.
- **O2. Maps** (decision 1). A second map with sim data (recommended: 1b's
  oracles stay byte-identical, no protected test changes, and rung 3 needs
  both), or replace the dot room, which regenerates the dot streams and
  changes top-down goldens and dot assertions.
- **O3. The GCMouse shim** (decision 11). Recommended: a separate crate with
  the engine's first `unsafe`. Or a module inside `eng-platform-winit`
  (`forbid` relaxed there), or no shim, keeping winit's coalesced macOS
  deltas, which spec 6.10 warns mislead playtests.
- **O4. Collision oracle** (decision 2). Brute force (recommended), or
  `parry3d-f64` 0.31.1 as a test-only dependency, through T0b's separate
  parry3d patch at G3.
- **O5. dist** (decision 17). `cargo xtask dist` (recommended), or the
  `dist` tool (cargo-dist 0.34.0), whose `[profile.dist]` and workflow then
  join G5 ("Before the first sitting").
- **O6. Agent seeing.** Take or drop A1-A5 (recommended: A1, A3, A4 small,
  A2's three rules).
- **O7. Benchmark gate** (decision 14). The push's base plus a performance
  ADR (recommended), or committed baselines (protected).
- **O8. Mutants** (decision 15). Fail on a missed mutant in the diff
  (recommended), or report only until D1.
- **O9. Fullscreen by default** for first-person maps (decision 10), with
  `--windowed` for tests. Confirm.
- **O10. Tone curve** (decision 7; spec 15 leaves it to the builder): Khronos
  PBR Neutral. Only to inform.
- **O11. GPU timestamps** (decision 5, T8). Ask for `TIMESTAMP_QUERY` when
  the adapter has it, for per-pass times only (recommended: 6.7 asks for them
  "when available"), or keep the default-features-only device and report
  `null`. Either way `tier` stays 0 and the capability names stay;
  ARCHITECTURE.md's "Tier 0" text is updated.
- **O12. `pmove` on angled walls** (found in Q2). f32 rounding can make the
  shared `slide` hit one off-axis plane twice and stop the move. The room has
  no angled wall ("The room"), so the recommendation is to fix it with level
  format v1 in rung 3. Fixing it now changes `pmove`; the dot streams may
  change, and are regenerated only if they do.
- **O13. Movement in the air** (decision 3). As built: gravity only, no air
  control and no step-up while falling. Either is a small change if the
  owner wants that feel.
- **Pending spec decisions:**
  - P1: sessions on the current plan.
  - P4: textures and sprites come from code and the palette from a script,
    option A, the default; nothing in rung 2 changes if P4 does.
  - P5: three sittings.
  - P7: agents run overnight here; T14's and G7's mouse checks need the
    owner's unlocked screen.
  - P9: the grid is core, so it runs under RISC-V QEMU.
  - P6 does not bear on rung 2: no weapons, and buttons stay 0.
- **Not raised:** the parked OASIS idea; spec 12 revisits it at level starts,
  and rung 2 is not one.

## Owner gates

Every change below is to a path `protected.toml` lists, except G1's plan,
`current.txt`, and G4's ADRs (rule decisions). Agents prepare each change and
the owner applies it. **Three sittings, each with its inputs ready:**
1. **Sitting 1, at the start,** once T0, T0a and T0b are done: G1, G2 (T0's
   patch), G3 (T0b's patch) and G4 (T0a's ADRs);
2. **Sitting 2,** when T21's package is ready: G5, ending with a push (and
   G5b after it if the first run needs a fix);
3. **Sitting 3,** after a green run's candidates: G6 and G7.

| Gate | Sitting | Path | Change | Unblocks |
|---|---|---|---|---|
| G1 | 1 | this plan; `docs/rungs/2/acceptance.json` | Approve; write the criteria in 1b's format; decide O1-O13 and A1-A5. `current.txt` stays `1b` (O1) | everything after it |
| G2 | 1 | `architecture.toml`; root `Cargo.toml` `[profile]` | Apply T0's patch: the four crates and their edges; `[profile.release.package.eng-collision-grid] overflow-checks = true`; one commit with the skeletons | T5, T12, T14, T17 |
| G3 | 1 | `deny.toml`; `_typos.toml` if needed | Apply T0b's patch: crate batch 3, the `objc2-game-controller` wrapper ban, a `clarify` entry if needed (a parry3d batch too, if O4 picks it) | T14, T17 (T5 with parry3d) |
| G4 | 1 | ADRs (rule decisions) | Accept, change or refuse ADR-0019 (graph), ADR-0020 (first-person passes, difference 1, and the scene layout), ADR-0021 (`unsafe` in the shim), ADR-0022 (platform contract), ADR-0023 (maps and sim data), ADR-0024 (contact and `CONTACT_GAP`); and 1b's ADR-0017 | T5, T6, T8, T13, T14 |
| G5 | 2 | `xtask/**` | Docs-lint; `bench`; `mutants`; `notices` and `dist`; replay reads `map` (T6); new `test` steps (collision, movement, sim-data, render-graph, sprites, mouse-look) and `native` steps (window-room, look, dist); `gpu-budgets` also renders a room shot; mutants' `in-diff` and `rung-diff`; proof video adds `room.apng`; selftest cases; summary and acceptance for the new jobs; `arch-check` refuses `allow(unsafe_code)` outside the shim's one module, scanning every Rust file in the workspace, as ADR-0021 lists | criteria 1-21 |
| G5 | 2 | `.github/workflows/**` | The `bench` job (valgrind, `gungraun-runner` =0.20.0); the `mutants` job (`cargo-mutants` =27.1.0, four shards, a weekly `schedule` and a manual dispatch for `rung-diff`); `cargo-mutants` in the `selftest` job too, for `violation:mutant-missed`; `cargo-about` =0.9.3 in `lint`; `dist` artifacts on three runners; `room.apng`; actions pinned by SHA, `permissions: {}` | the new jobs exist |
| G5 | 2 | `scenarios.toml`, `**/*.assert.toml` | T7's `room-scenarios.patch`, applied with T6's replay patch: the room's scenarios, assertions and hash streams in one commit (its input scripts are already committed) | 1, 2, 4 |
| G5 | 2 | `docs/rungs/current.txt` (not protected); `docs/rungs/2/acceptance.json` | Set `current.txt` to `2` in G5's commit (O1). Amend `acceptance.json` if a job or step name drifted from this plan, or to add the rows of TA tasks taken | rung 2's acceptance computed by CI |
| G5 | 2 | `budgets.toml` | A new `[gpu.first_person]` table (from T10's counters); `[ci]` minutes for `bench` and `mutants`. `[gpu]` may stay as it is: `shot_video.rs` feeds its own literal, which the parser must keep accepting | 15, 19, 21 |
| G5 | 2 | `protected.toml`, `.github/CODEOWNERS` | Protect: Q1's `crates/eng-gpu/tests/{graph_order,graph_errors,pool,bypass,bypass_render,transients}.rs` and `tests/support/**`; Q2's `crates/kit-fps/tests/{ground,slopes,boxes}.rs` and `tests/support/**`; Q3's `crates/eng-input/tests/{mouse_look,mouse_look_exact,mouse_look_model}.rs` and `tests/look_rig/**`; Q4's `crates/eng-render/tests/{camera,sprite_rotation,palette}.rs` (`palette.rs` holds Ottosson's known answers and the pinned bake hash); `crates/eng-collision-api/src/conformance.rs`, which T5 extends; the grid's oracle test; T6's sim-data tests (criterion 5); T13's new conformance tests; `shots.toml`; `tests/look.rs`; `about.toml` and its template; `**/*.rules.toml` if A2 is taken | criteria cannot be weakened |
| G5 | 2 | `crates/eng-platform-api/src/conformance.rs`, `src/conformance/**` | T13's suite: a new file and its `mod` line; the existing suites unchanged | 10 |
| G5 | 2 | `crates/game-testbed/tests/shots.rs`, `tests/shots/**` | Read `shots.toml` and the per-shot `[flip.<shot>]` limits | 12, 13 |
| G5 | 2 | `.cargo/mutants.toml` | Exclusions, empty to start | 19 |
| G5 | 2 | `.claude/**` | The `test-auditor` subagent and the `perf-investigate` skill (spec 7.5) | T19, T24 |
| G5, if needed | 2 | `**/*.proptest-regressions`; `.config/nextest.toml` | Failing proptests' regression files; a test group for GPU tests | as found |
| G5b, if needed | between 2 and 3 | `xtask/**`, `.github/workflows/**` | T24's first-run fixes as one patch, as 1b's `ci-first-run.patch` (6443ea5): the owner applies it and pushes, a few minutes outside the sittings | a green run, then sitting 3 |
| G6 | 3 | `**/golden/**` | Goldens for every manifest shot from the pinned lavapipe; per-shot `[flip.<shot>]` tables in `thresholds.toml`; 1b's four, if 1b's G6 is still open | 12, 13 |
| G7 | 3 | none | Watch `room.apng`, play, confirm `owner-mac-shots` and `gcmouse-rate`; accept or send back | 14, 23, 25; rung done |

**Protected paths rung 2 leaves alone,** each by a design choice in
"Protected tests that pin today's APIs":
- `known_answers.*`, `eng-core`'s and `eng-schema-core`'s `golden.rs`,
  `exit_codes.rs` and the dot `*.assert.toml`;
- `eng-net-wire`'s `golden.rs` and `never_panic.rs`, `hash_equality.rs`,
  `twin.rs`, `eng-platform-headless/tests/conformance.rs`,
  `eng-platform-winit/tests/conformance.rs` and `version.rs`;
- `play.rs`, `crash_json.rs`, `live_input.rs`, `shot_video.rs`,
  `counters.rs` and `shaders.rs`;
- `clippy.toml`, `.cargo/config.toml`, `oracle.yml`, the root `Cargo.toml`'s
  `[profile]` table beyond G2's one line, and `rust-toolchain.toml` (1.99.1
  is above every new MSRV, the highest being `bincode-next`'s 1.90).

A task that needs one of these after all stops and brings the patch to the
next gate.

## Crate batch 3

*[proposal]* The third-party crates G3 adds to `[bans] allow`: 17 names (T0b found two
more than this table first listed: `rustc_version` 0.4.1 and `semver`
1.0.28, build dependencies of `derive_more-impl`).

- **Source:** a scratch project outside the repo, resolved 2026-11-06, with
  `gungraun =0.20.0` and `objc2-game-controller =0.3.2`
  (`default-features = false`; `std`, `block2`, `GCDevice`, `GCMouse`,
  `GCMouseInput`, `GCPhysicalInputProfile`).
- **Not final:** it ran on this machine's Rust 1.93.1, not the pinned
  1.99.1, and was resolved, not compiled. T0b checks it with cargo-deny, and
  T14 and T17 recompute it against the real lock and report any difference
  as an amendment.
- **Already allowed:** `objc2` 0.6.5, `objc2-foundation` 0.3.2, `block2`
  0.6.2 (batch 2), `serde`, `serde_json`, `syn`, `itoa`, `memchr`, `zmij`,
  and `flate2` (for dist's archive). Every new licence is on the list.
- **Flags:** `bincode-next` writes its licence "MIT or Apache-2.0" with a
  lowercase `or`, so cargo-deny may need a `clarify` entry. It is not the
  banned `bincode` (spec 6.8) but a fork, used inside gungraun only; MSRV
  1.90. gungraun builds only on Linux, but cargo-deny checks every listed
  target, so it needs the entries anyway.

| Crate | Version | Licence | Brought by |
|---|---|---|---|
| `objc2-game-controller` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | the shim (macOS only) |
| `gungraun` | 0.20.0 | Apache-2.0 OR MIT | `eng-bench` (dev) |
| `gungraun-common` | 0.1.0 | Apache-2.0 OR MIT | gungraun |
| `gungraun-macros` | 0.10.0 | Apache-2.0 OR MIT | gungraun |
| `gungraun-runner` | 0.20.0 | Apache-2.0 OR MIT | gungraun (library part) |
| `bincode-next` | 3.1.1 | MIT or Apache-2.0 (see above) | gungraun |
| `unty-next` | 0.1.2 | MIT OR Apache-2.0 | bincode-next |
| `derive_more` | 2.1.1 | MIT | gungraun |
| `derive_more-impl` | 2.1.1 | MIT | derive_more |
| `either-or-both` | 0.3.1 | Apache-2.0 OR MIT | gungraun |
| `pastey` | 0.2.3 | MIT OR Apache-2.0 | gungraun |
| `proc-macro-error3` | 3.1.1 | MIT OR Apache-2.0 | gungraun-macros |
| `proc-macro-error-attr3` | 3.1.1 | MIT OR Apache-2.0 | proc-macro-error3 |
| `rapidhash` | 4.5.1 | MIT OR Apache-2.0 | gungraun |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 | rapidhash, through bincode-next |

**Tools, not dependencies.** They are installed by the workflows (G5), so
they are not in `deny.toml`. Versions were checked with `cargo search` and
`cargo info` on 2026-11-06.

| Tool | Version | Licence | Use | Verified |
|---|---|---|---|---|
| `cargo-about` | 0.9.3 (spec appendix: 0.9.2) | MIT OR Apache-2.0 | notices | yes |
| `cargo-mutants` | 27.1.0 | MIT | mutation testing | yes |
| `gungraun-runner` | 0.20.0 (must equal the library's) | Apache-2.0 OR MIT | benchmarks | yes |
| valgrind | Ubuntu 24.04's package | GPL-2.0-or-later: a CI tool, never linked into a binary | benchmarks | version not checked; gungraun's minimum not checked |
| `cargo-dist` (`dist`) | 0.34.0 | MIT OR Apache-2.0 | only if O5 picks it | version yes; release date not checked |
| `parry3d-f64` (a crate, test-only) | 0.31.1 | its 39-crate Linux tree: every licence on the list, or an OR with one that is (`cargo metadata` in scratch) | only if O4 picks it | version and licences yes; names not compared with the allow-list |

## Risks and unknowns

| Risk | What we do |
|---|---|
| Palette quantization turns tiny adapter differences into whole palette steps, so FLIP may fail first-person shots off lavapipe | per-shot limits; the exact checks of decision 13 hold on every adapter; `room-look-off` is the shot that compares shading |
| GCMouse: `unsafe`, units and signs unverified, and no mouse on macos-26 | the shim's logic is tested with a fake source; real callbacks are checked on the owner's Mac (T14, G7) |
| winit 0.30.13: `Locked` is missing on X11 only (Windows has it, `platform_impl/windows/window.rs:443`), `Confined` on macOS | Locked, then Confined, covers every platform; the report names the mode used |
| Under Xvfb, `xdotool`'s relative motion may not reach XInput2 raw events | the window step checks the grab mode and frames, not mouse motion; mouse look is tested headless |
| AGENTS.md is at 150 of 150 lines | T22 rewrites lines rather than adding them; docs-lint checks what stays |
| A room snapshot over 1,200 bytes | sim data stays out of state (decision 1); criterion 5 checks it |
| T10 and T11 both in `eng-render` | separate modules; T10 lands first; `git apply --3way`; `Cargo.lock` merged as 1b's lesson says |
| A builder's planted bugs are chosen from rules it already has in mind (queue lesson) | core tasks get a reviewer that re-implements the rule; A4's bugs are written by an agent that did not build the code |
| Mutants runs long on core crates with replays | shards and a 20-minute cap per shard; exclusions are the owner's |
| The bench gate is noisy if two runner images differ | the head and the push's base are built and run in the same job |
| The native jobs build `--all-targets` on Windows and macOS, where gungraun is absent | gungraun is a Linux-only dev-dependency and each bench is an empty `main` elsewhere; T17 checks a build on this Mac |
| A protected test pins an API this plan assumed it could extend | "Protected tests that pin today's APIs" lists what stays; a task that hits another one stops and proposes a patch |
| `bincode-next`'s licence string | T0b runs cargo-deny and adds a `clarify` entry if it refuses |
| The owner may dislike the tone curve or the look | every look pass switches off; the curve is one function |
| 1b's G6 and G7 are still open | rung 2 keeps 1b's pictures and streams byte-identical, so either order works |
| Q1 built part of T8 ahead of the plan, and the owner may change ADR-0019 | T8 may change that design; G4 is where the owner first sees it |
| A rung file read through `current.txt` turns `quick` red before its steps exist | `current.txt` moves to `2` at G5, in the commit that adds the steps (O1); T21 checks `quick` with it set in a scratch clone |
