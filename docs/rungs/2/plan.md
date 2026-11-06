# Rung 2: Inside the room

> **DRAFT, NOT APPROVED.** Drafted 2026-11-06 by Claude (Opus 5.5) while
> the owner was away; nothing here binds until the owner approves it at
> gate G1. `docs/rungs/current.txt` stays `1b`, and
> `docs/rungs/2/acceptance.json` does not exist: the owner writes it from
> "Proposed acceptance criteria". Every proposal below stands until the
> owner changes it.

- **Status:** draft for the owner's first sitting. The four items in
  "This window's queue" may be built before approval; they touch no
  protected path and change no existing output.
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

## Where rung 1b stands

Approved 2026-10-31; CI green on all five native jobs on 2026-11-01 (run
41302087332), 16 of 19 features passing; 31 sessions against a budget of 30.
Still open: G6 (the owner commits the four golden candidates), G7 (the owner
plays), and ADR-0017 (three passes), still "Proposed". Rung 2 keeps 1b's
top-down pictures, five hash streams and assertions byte-for-byte as they are
(decision 1), so G6 and G7 can come before rung 2's first sitting or in it.

**Protected tests that pin today's APIs.** Several protected tests build
1b's types by struct literal or implement 1b's traits. Rung 2 keeps these
as they are, so those tests compile and pass unchanged. New behaviour comes
through new types and functions beside them.

| Kept as in 1b | Pinned by |
|---|---|
| `SimConfig`'s fields, `SimBuilder::new` | `eng-session/tests/hash_equality.rs:48`, `game-testbed/tests/twin.rs:213` |
| `GameDef`'s fields, `PlayRun::start`, `eng_app::play_on` | `twin.rs:33` (a `GameDef` literal), `twin.rs:76,84` |
| `Welcome`'s fields (no wire change in rung 2) | `eng-net-wire/tests/never_panic.rs:44`, `tests/golden.rs` |
| `Control`'s methods; new ones have default bodies | `impl Control` in `twin.rs:354` and `eng-platform-headless/tests/conformance.rs:227,258` |
| The recording format (v1) and `formats` | `game-testbed/tests/version.rs:373` (`recording: 1`), `:379` |
| `Graph::new`, `Graph::fixed`, `PassDecl { name, reads, writes }` | `eng-gpu/tests/counters.rs:181-187` (`Graph::new(&[DRAW, TONEMAP, AGAIN])`: two writers of `Output`) |
| `budgets.toml`'s `[gpu]` keys and values | `game-testbed/tests/shot_video.rs:231-232` (12 and 2,048) |

## What already exists (do not rebuild)

| Crate | What rung 2 starts from |
|---|---|
| `eng-gpu` | `Graph` over a closed `Resource` enum (`Scene` `Rgba16Float`, `Output` `Rgba8UnormSrgb`, `Surface` `Bgra8UnormSrgb`); `PassDecl` checks (each read written earlier, one write per pass); `Graph::fixed` for offscreen and surface. Instanced pipelines (a triangle strip per instance, `f32x2`/`f32x3`/`u32` attributes, one instance buffer per frame); fullscreen pipelines with one input and a nearest sampler. `Targets` with readback, `FrameCounters`, `GpuReport` (records whether timestamp queries exist), `WindowSurface` (Fifo). `tests/shaders.rs` (naga to SPIR-V, MSL, HLSL). No depth target, no textures beyond a pass's input, no uniform, storage or index buffers. |
| `eng-render` | The top-down view (rectangles, discs; camera subtracted in f64), 1b's clamp tone map, the present blit, `[E-SCENE]`. |
| `kit-fps` | `pmove`: friction, acceleration, collide-and-slide against up to 5 planes, creases. Flat floor only: no gravity, step-up, step-down or ground snapping (its docs say "rung 2"). `MoveState` v1, `MoveConfig` v1. |
| `eng-collision-api` | `CollisionWorld` (`trace`, `overlaps`), `BoxHull`, `Trace`, `BoxRoom`, and `conformance::check` (not protected). |
| `eng-input` | Keyboard `LiveInput` with five actions, strict bindings TOML, `set_view` holding fixed angles; input scripts already carry `side`, `yaw_deg` and `pitch_deg`. |
| `eng-platform-api` | Events for raw `MouseMotion`, mouse buttons, wheel and gamepads already exist; `Control` has `now` and `exit` only (no cursor or fullscreen); the window seam (ADR-0016); the protected conformance suite. |
| `eng-platform-winit` | Keys and window events only; no mouse yet. 1b's T10 left notes for rung 2 (task T13 carries them). |
| `eng-app` | `TickClock` (at most 5 ticks per wake), one frame per tick, occlusion, `View`; a lost surface panics. |
| `eng-cli` | `run`, `replay`, `dump-world`, `shot`, `video`, `play`; `GameDef { top_down, view_yaw, .. }`; report v2 with `gpu`; budgets `[sim]` and `[gpu]`. |
| `game-testbed(-sim)` | The 8 m x 6 m x 2 m dot room (`BoxRoom`), `WallBump`, `top_down.rs`, scenarios `dot-square` and `dot-corner`, five hash streams, the shot harness with four shots, `golden/thresholds.toml` (mean 0.01, p99 0.2, p999 0.5), no goldens yet. |
| `xtask` and CI | Jobs lint, test, riscv, wasm, selftest, native (12 steps), summary. `docs-check` checks AGENTS.md's length, CLAUDE.md and CODEOWNERS only: it is not the spec's docs-lint. No schedule (no nightly), no benchmarks, no mutation testing. AGENTS.md is at 150 of 150 lines. |

## What gets built

### New crates (4)

Layers are `architecture.toml`'s. The exact entries are gate G2.

| Crate | Proposed layer and edges | What it does in rung 2 | From |
|---|---|---|---|
| `eng-collision-grid` | `L2`, core, `same_layer = ["eng-collision-api"]`; `game-testbed-sim` gains it in its `same_layer` | Level 1's collision: a uniform grid over static primitives (vertical wall segments with a z range, horizontal convex slabs for floors, ceilings and steps), exact f64 sweeps of a `BoxHull`. It implements `CollisionWorld` and passes `eng-collision-api`'s conformance suite. Built once from sim data (decision 1). | spec 6.1, 6.11, 9 |
| `game-testbed-client` | `game`; `game-testbed` gains `same_layer = ["game-testbed-client"]` | The testbed's presentation: the top-down extraction (moved from the binary, unchanged), the first-person extraction, region looks (light, fog), textures and sprites made by code, and the plain-text palette. | spec 6.1 (games row); rung 1b difference 6 |
| `eng-platform-gcmouse` | `L1`, std; `eng-platform-winit` gains it in its `same_layer` | macOS only: raw mouse deltas from GCMouse, as plain `(dx, dy)` sums the winit platform drains each frame. Empty on other systems. The engine's only `unsafe` code (decision 11, owner decision O3). | spec 6.10 (the shim); its own crate: proposal |
| `eng-bench` | `tool` | gungraun benchmarks: the dot and room sim steps, grid traces, `ground_move`. gungraun is a Linux-only dev-dependency (`[target.'cfg(target_os = "linux")'.dev-dependencies]`); on other systems each bench target is an empty `main`. | spec 6.1 (tools row), 6.13 |

### Changed crates

| Crate | Change in rung 2 | From |
|---|---|---|
| `eng-gpu` | A planned graph beside the fixed one, which orders, culls and pools (decision 5); depth, several colour targets, textures, per-frame uniforms and the scene layout (decision 6); labels; per-pass GPU times (T8, O11). `Graph::new`, `Graph::fixed` and `PassDecl` stay. | spec 6.7 + proposal |
| `eng-render` | First-person camera, opaque, sprite, light, sky, fog, tone-map and palette passes (the `render-retro` pipeline, as modules of this crate); the top-down view unchanged. | spec 6.1, 6.7 |
| `kit-fps` | `ground_move`: gravity, step-up, step-down, ground snapping, beside an unchanged `pmove`. | spec 6.11 |
| `eng-input` | Mouse look into `view_yaw` and `view_pitch`. | spec 6.10, 6.14 |
| `eng-platform-api`, `-headless`, `-winit` | Cursor lock and borderless fullscreen as new `Control` methods with default bodies, and a separate conformance suite for them; winit maps mouse motion, buttons and wheel; macOS motion from the shim. | spec 6.10; rule 6 (ADR-0022, with migration notes) |
| `eng-ecs`, `eng-sim` | Constant resources outside the per-tick state and snapshots, and `Sim::sim_data_hash` (decision 1). | spec 6.11 + proposal |
| `eng-cli`, `eng-app` | Maps beside `GameDef` (a new `GameMaps` and entry points), `--map`, `--view`, first-person `shot`, `video` and `play`; `sim_data_hash` in recordings and hash-stream headers; display-paced frames; cursor and fullscreen. | spec 6.6, 6.10, 6.11, 7.1 |
| `game-testbed-sim` | A second map, `room`; the dot map unchanged. | proposal |
| `xtask` (patches only) | docs-lint, bench, mutants, notices, dist, new CI steps and selftests. | spec 7.4, 6.13, 7.2, 8.4 |

### One frame, first person

```
OS events ─eng-platform-winit─▶ keys, raw mouse (macOS: eng-platform-gcmouse) ─▶ eng-app
  LiveInput: held keys + MouseLook ─sample(tick)─▶ PlayerCommand (forward, side, view_yaw, view_pitch)
  eng-session over Memory ─▶ Sim::step: ground_move against the grid, things turn
  previous, current, alpha; plus the live view angles (presentation only)
  game-testbed-client ─▶ FirstPersonScene (camera WorldPos f64, things, region looks)
  eng-render, graph ordered, culled, pooled:
    opaque ▶ sprites ▶ light ▶ sky ▶ fog ▶ tonemap ▶ palette ▶ output ─present (k×k)▶ surface
                                                           │
                                                 shot, video, goldens
```

### The first-person graph *[spec 6.7 + proposal]*

| Pass | Reads | Writes | Switched off |
|---|---|---|---|
| `opaque` | none | `albedo` (`Rgba16Float`), `info` (`Rg32Uint`: region, surface class, object id), `depth` (`Depth32Float`, reversed-Z, infinite far) | never |
| `sprites` | none | continues `albedo`, `info`, `depth` | never |
| `light` | `albedo`, `info`, `depth` | `lit` (`Rgba16Float`) | falloff and 32-step banding off: the region's light without distance falloff; fake contrast off: no wall-direction term |
| `sky` | `depth` | continues `lit` where depth is the far plane | a flat sky colour |
| `fog` | `lit`, `info`, `depth` | `fogged` | bypassed |
| `tonemap` | `fogged` | `toned` (`Rgba8UnormSrgb`) | never: the curve and EV100 exposure (decision 7) |
| `palette` | `toned` | `output` | bypassed: `tonemap` writes `output` |
| `present` | `output` | `surface` | low resolution off: `output` at the window's size; on: at 1/k, drawn k×k |

- **Light** *[spec 6.7 + proposal]*: region illuminance in lux, Lambertian
  albedo, luminance in nits, a fixed EV100 per camera. Doom's 0-255 light
  levels map to lux by a table, so falloff and banding can use its 32 steps.
- **`info`** carries each pixel's region for light and fog, and an object id
  that addition A1 can read almost for free.
- **The top-down graph** (draw, tonemap, present, with 1b's clamp) stays on
  `Graph::fixed`, so its pictures, 1b's goldens and `counters.rs` are
  untouched. The first-person graph is a planned one (decision 5).

### The room *[proposal]*

A 12 m x 9 m hall with a 4 m ceiling; a 0.4 m platform reached by one step,
and a 0.6 m ledge that must block; a 1 m pillar; an alcove with a 2.5 m
ceiling and green fog; a skylight showing the sky. Four regions, fixed spawn
points and three things: a "marker" that turns once every 8 s, so each of its
8 rotations shows from a fixed view, and one behind the pillar as seen from
the spawn. Exact sizes are T7's call.

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
   - **The wire.** `Welcome` does not change. Over Memory the probe builds
     server and client from one map; another map's snapshot fails on its
     schemas. Sim data between machines is rung 5's handshake (6.8). The
     alternative, replacing the dot room, is O2.
2. **Collision** *[spec 6.11 + proposal]*. 1 m grid cells, visited in cell
   order, then primitive index; ties between hits go to the lower index.
   Sweeps are deterministic (f64, IEEE basic operations only): a box against a
   wall segment is a 2D separating-axis sweep plus the segment's z range,
   against a slab a z sweep inside its polygon. `BoxRoom` and the grid are the
   contract's two implementations; a brute-force oracle in the tests (every
   primitive, no grid) must give the same `Trace` bits (O4: parry3d instead).
3. **Ground movement** *[spec 6.11 + proposal]*: `GroundConfig` v1 (gravity
   9.81 m/s², step 0.4 m, snap 0.4 m, steepest walkable normal) and
   `ground_move(state, cmd, &MoveConfig, &GroundConfig, dt, world)`. On the
   ground: `pmove`'s friction and acceleration, `vel.z = 0`, a flat slide,
   then a stepped try (up, slide, down) that wins only if its horizontal
   travel is strictly greater (squared distances, compared exactly), then a
   snap down. In the air: `vel.z ← vel.z − g·dt`, then the sweep by `vel·dt`.
   Vertical speed is `MoveState.vel.z`, which v1 has, and "on the ground" is
   recomputed each tick, so `MoveState`'s encoding does not change. No
   jumping (Doom has none). Q2 lists the tests. The room's hull is
   0.5 x 0.5 x 1.75 m, the eye 1.6 m above the feet.
4. **Things and sprites** *[spec 6.7, 11 + proposal]*. A thing is an entity
   with a position and a `BAngle` facing.
   - **The rotation, from the geometry:** the one whose 45° sector, centred on
     the facing, holds the direction from the thing to the viewer. A thing
     facing the viewer shows its front, rotation 0; rotation k means the
     viewer is k × 45° counter-clockwise of the facing, seen from above. In
     binary angles, `((dir_to_viewer − facing) + 2^28) >> 29`, wrapping; a
     direction on a boundary goes to the counter-clockwise sector.
   - A five-rotation sheet shows rotation 8 − k as k mirrored left to right.
     Billboards turn only about Z, are alpha-tested and write depth.
   - The client renders each sprite's 8 rotations on the CPU at start-up from
     a small 3D primitive with a marked front, so sprites "are rendered from
     3D sources" (spec 11) and no image file enters the repo.
5. **The graph grows** *[spec 6.7, ADR-0003 + proposal; ADR-0019]*.
   - **Kept as 1b:** `Graph::new`, `Graph::fixed` and `PassDecl`, which the
     protected `counters.rs` uses for `Graph::new(&[DRAW, TONEMAP, AGAIN])`,
     two writers of `Output`.
   - **The planned graph**, a new constructor over a new spec type that can
     also declare continues (T8 extends it), does the rest. **Ordering:** a
     topological sort over reads, writes and continues, ties by pass name;
     a cycle, or two writers where neither continues the other, is
     `[E-GRAPH]`. **Culling:** a pass whose writes reach neither `output`
     nor `surface` is dropped and reported; a switched-off post pass is
     bypassed, its readers reading its input, so `toned` and `output` share a
     format. **Pooling:** transients planned by (format, size, usage) share
     a texture when lifetimes do not overlap, within and across frames of one
     size. **Labels** name every pass, target and pipeline, with debug groups
     under `WGPU_DEBUG=1`.
   - **T8 adds** more than one write (up to four colour targets and one depth
     target per pass, and continues), and GPU time per pass. 6.7 wants
     timestamp queries "when available, with null otherwise". 1b's device
     asks for WebGPU's defaults only (ARCHITECTURE.md, "Tier 0"), and
     `TIMESTAMP_QUERY` is beyond them, so that is O11.
6. **The GPU scene layout** *[spec 6.7 "fixed at rung 2" + proposal;
   ADR-0020]*.
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
   builder]*: Khronos PBR Neutral. It is analytic (no LUT file) and keeps
   base colours, which suits the palette after it; EV100 exposure. The
   top-down graph keeps 1b's clamp.
8. **Palette** *[spec 6.7 + proposal]*.
   - A 32³ LUT baked on the CPU by OKLab distance, ties to the lowest index;
     optional 4x4 Bayer dither.
   - The palette is a plain-text file, as 6.7 says: GIMP's open `.gpl`
     format, game data at `game-testbed-client/palettes/room.gpl`.
   - A script in the repo generates it. It is not Doom's palette, which is id
     Software's work; spec 11 asks for public tropes, not protected works.
   - As the first non-code asset, it gets 6.11's provenance sidecar (source,
     maker, tool, licence, status).
9. **Mouse look** *[spec 6.10, 6.14 + proposal]*. Sensitivity in degrees per
   count (default 0.066, Quake's 0.022 × 3), invert pitch, pitch within ±80°.
   - **Signs:** positive yaw turns toward +Y, so `dx > 0` (right) lowers yaw.
     `dy > 0` is toward the user (`event.rs:19`) and pitch is positive up
     (`script.rs:35`), so `dy > 0` lowers pitch unless inverted.
   - **Rounding:** each delta becomes a count in 1/256ths, summed exactly in
     `i64`. A tick's angle is `round(sum × s × 65536 / (360 × 256))` over the
     sum since the last re-base, ties half away from zero, so any split of the
     same motion into ticks gives the same yaw.
   - **The pitch limit:** at the limit the sum re-bases, dropping motion past
     it (as Quake does). Only that case depends on tick boundaries.
   - Losing focus drops unsampled motion. Buttons stay 0 until doors (rung 3).
10. **Window and frames** *[spec 6.6, 6.10 + proposal]*.
    - First-person maps open borderless fullscreen, with `--windowed` for
      tests. `dots` keeps 1b's window, which the protected `play.rs` runs.
      The cursor is locked (Confined where Locked is missing: Windows, X11)
      and hidden while focused, and released on focus loss.
    - **The contract change** (ADR-0022, rule 6) adds cursor and fullscreen
      requests to `Control`. Its **migration notes**:
      - each new method has a default body that does nothing, so every
        `impl Control` compiles unchanged, including the wrappers in
        `twin.rs` and the headless conformance test;
      - winit and headless override them, and headless records the requests;
      - a wrapper that does not forward a request drops it, so the new rules
        are a separate suite, and `check` and `check_scripted` keep their
        expectations;
      - `PlatformEvent` and `App` do not change.
    - Frames come at the display's refresh rate when winit reports one, else
      once per tick (Xvfb has no vsync); ticks still come from the timer. The
      camera's position is interpolated at alpha, and its angles come from
      live input between ticks (presentation only, contract 4), so the view
      turns at the display rate.
11. **The GCMouse shim** *[spec 6.10 + proposal; ADR-0021]*.
    - The methods it needs in `objc2-game-controller` 0.3.2 (`mice`,
      `current`, `mouseInput`, `setMouseMovedHandler`) are all `unsafe fn`.
    - The workspace lint `unsafe_code = "deny"` refuses `unsafe` wherever the
      workspace lints apply. The shim's crate allows it in one module, with a
      `SAFETY` comment on each block.
    - Its handlers run on the main queue and add into a sum the winit
      platform drains in `about_to_wait`.
    - **The rule:** no other crate may `allow(unsafe_code)`. `arch-check` (G5
      patch) fails on one, and the selftest `unsafe-outside-shim` proves it.
      Crates with `#![forbid(unsafe_code)]` keep it; the core set must have
      it, and `game-testbed` and `xtask` never did.
12. **Shots** *[spec 8.3 + proposal]*. First-person shots take player 0's
    camera at the tick, at alpha 1. The list moves from `shots.rs`'s
    constant into a protected `crates/game-testbed/shots.toml`, with FLIP
    limits per shot (spec 15: "per test"). Proposed shots: `room-spawn`,
    `-step`, `-alcove`, `-sky`, `-sprites`, `-ledge` and `-look-off` (every
    switch off).
13. **Structured look checks** *[spec 8.3 + proposal]*, exact on every
    adapter because they compare a machine with itself or test a property:
    every switch off gives the plain pipeline's image; with the palette on,
    every pixel is a palette colour; on a flat material, banding gives at
    most 32 levels per region; fog moves monotonically toward its colour with
    depth along a probe row; the tone curve is within one 8-bit level of a
    CPU reference on a ramp.
14. **Benchmarks** *[spec 6.13 + proposal]*: instruction counts on
    `ubuntu-24.04` x64.
    - `cargo xtask bench` builds the pushed head and the push's base on one
      runner. The base is the push event's `before`, or a new agent branch's
      merge base with `main`; the parent would see only the last commit.
    - Over 5% fails unless the push adds a performance ADR naming the
      benchmark. There is no stored baseline to go stale.
15. **Mutation testing** *[spec 7.2 + proposal]*.
    - On a push that changes a core crate, `cargo mutants --in-diff` runs on
      the diff from the push's base, in 4 shards of at most 20 minutes. Not
      `origin/main`: an owner push of commits an agent branch already
      carried (ADR-0018) would give an empty diff.
    - Other pushes write an ok `in-diff` step that says it skipped, because a
      job with no report fails (`acceptance.rs:48-52`).
    - A missed mutant fails unless `.cargo/mutants.toml` (protected) excludes
      it. A weekly full run is reported, not failed.
16. **docs-lint** *[spec 7.4 + proposal]*: `docs-check` also checks that
    every `cargo xtask` command, probe command, path, `ENG_*` variable and
    `[E-*]` tag named in `AGENTS.md`, `docs/ARCHITECTURE.md` and
    `.claude/skills/*/SKILL.md` exists.
17. **Notices and dist** *[spec 8.4 + proposal]*. `cargo xtask notices` runs
    cargo-about over the testbed binary's graph, with deny.toml's licence
    list. `cargo xtask dist` makes `target/dist/<target>/`: the release
    binary, stripped, with split debug info, `version.json` and the notices,
    archived. CI makes one on `ubuntu-24.04`, `windows-2025` and `macos-26`
    as an artifact. No tags, release or signing (8.4 stays rung 7's).

## Where this differs from the spec, and why

1. **The graph's shape** (ADR-0019, amending ADR-0003 and ADR-0017).
   - Passes write several targets and continue earlier ones.
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
   xtask as the only entry point, pinned actions and `permissions: {}`. The
   tool stays an option at rung 7 (O5).
6. **Benchmarks run on every push**, not only nightly (8.2), against the
   push's base rather than a stored baseline: with direct commits to `main`
   (ADR-0014) no PR holds one. T17 measures the job's time. Likewise
   "required only when core-set crates change" (7.2) means on a push, not a
   PR.
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
      pinned nightly (spec 4) and protected snapshots. Rung 2's one contract
      change is covered by ADR-0022's migration notes. Proposed for D1.

## Agent seeing: five additions for the owner to take or drop

None of these is in the rung unless the owner takes it at G1.

**Why they are here.**
- The owner's aim (2026-11-01): "changing the industry would mean ai agents
  can develop games without human feedback". A review that day (commit
  353872d) asked whether agents can see enough to check a generated scene.
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

| | Addition | What it adds | Acceptance it adds | Task units | Owner time |
|---|---|---|---|---|---|
| A1 | **Frame record** | `shot` and `video` write `<out>.frame.json`: the camera; every object the extraction asked for (id, kind, world box); then, from `info` and `output`, each object's visible pixels, screen box, mean luminance and contrast against its surroundings; depth at a fixed 8x6 grid of probes; a luminance histogram | `frame-record`: a record for every shot; exact on the pinned lavapipe, within per-shot tolerances elsewhere; a planted "not drawn" bug and a "drawn black" bug each change it | 2 | none |
| A2 | **Rules for "proper"** | Protected `<scene>.rules.toml`, like assertions, checked by a script over frame records. Three starter rules: an object the manifest marks `must_see` has at least 50 visible pixels and contrast at least 0.1; no first-person pixel shows the clear colour; floor shows in the lower third and ceiling or sky in the upper third | `scene-rules`: every shot passes; each rule has a selftest that breaks it | 1 (needs A1) | about 20 min to approve the rules |
| A3 | **First-person poses** | `shot --pose x,y,z,yaw,pitch`, a camera independent of movement; the manifest names poses; the report lists the views checked | `fps-poses`: every named pose renders on every runner, and the report's `views` equals the manifest | 1 | none |
| A4 | **What each check misses** | 10-12 planted render bugs as patch files that `cargo xtask selftest render-bug-<n>` applies to a workspace copy, as selftests do now; one deletion run that switches each check off in turn; a bug-by-check table in `PROGRESS.md` | `check-misses`: every planted bug fails a check that is not hard-coded to one scene, or is a known miss the owner accepted | 2 | about 10 min |
| A5 | **Two texts per frame** (the owner's idea) | One text from game state (what should be seen, by a CPU ray cast of the map from the camera, independent of the GPU), and one from the pixels (A1's record). A script compares them line by line and fails on a difference; a model may explain a difference, never decide (7.2) | `two-texts`: no difference on every pose and over a 600-frame walk; each A4 bug that changes what is seen shows one | 2 (needs A1) | none |

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
  set is 6 units, about 9 sessions (48 in all); all five are 8 units, about
  11 (50). The time-box is unchanged.

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
| 1 | `room-window` | `play --map room --windowed --ticks 600 --input room-walk.input.toml --hashes h.txt` presents first-person frames in a real window, and `h.txt` equals `hashes/room-walk.seed1.txt` | native x5: window-room | 1 |
| 2 | `replay-cross-platform` | As 1b's criterion 7, now with the room scenarios. The five dot streams are unchanged; the room streams are new and identical on every target | test x2, riscv, native x5: replay; summary: compare-hashes | 1, 2 |
| 3 | `collision-grid` | The grid passes the collision conformance suite and gives the brute-force oracle's `Trace` bits on proptest maps and sweeps | test x64, arm64: collision; riscv: core-tests | 2 |
| 4 | `ground-movement` | Steps up to 0.4 m climb and 0.6 m blocks; snapping holds on the way down; gravity applies in the air; a hull never ends inside solid (proptest) | test x64, arm64: movement; riscv: core-tests | 2 |
| 5 | `sim-data` | The room's snapshot fits 1,200 bytes; sim data is outside the state, its hash and snapshots; `sim_data_hash` is in recordings, reports and the room's hash-stream header (so `compare-hashes` covers it) and changes with the room's data only; the dot map's is 0 | test x64, arm64: sim-data | 1, 2 |
| 6 | `render-graph` | In the planned graph: any declaration order gives one order; cycles and double writers fail with `[E-GRAPH]`; unused passes are culled and reported; non-overlapping transients share textures; labels set. `Graph::new`, `Graph::fixed` and 1b's counters unchanged | test x64: render-graph; native x5: tests | 3 |
| 7 | `look-stack` | Decision 13's exact checks, and each look pass switches off alone | native x5: look | 4 |
| 8 | `sprites` | The rotation rule (decision 4) matches an independent float oracle from the geometry and the hand-checked cases; mirror flags; Z-only billboards; alpha test with depth (the thing behind the pillar is hidden) | test x64, arm64: sprites; native x5: look | 5 |
| 9 | `mouse-look` | Decision 9's rules; the same motion timeline gives the same commands on every target (proptest) | test x64, arm64: mouse-look | 6 |
| 10 | `cursor-fullscreen` | The headless platform records cursor and fullscreen requests (conformance); `play --report` names the grab mode and fullscreen; focus loss releases the cursor | native x5: window-room, tests | 1, 6 |
| 11 | `gcmouse` | On macOS the shim builds and its tests pass, and `play`'s report names `gcmouse` as the motion source | native macos-26: tests | 6 |
| 12 | `shots-exact` | Every manifest shot, 1b's four and rung 2's first-person ones, equals its golden exactly on the pinned lavapipe | native ubuntu-24.04: shot-exact | 7 |
| 13 | `shots-flip` | Within each shot's FLIP limits on the other four runners | native (other 4): shot-flip | 7 |
| 14 | `owner-mac-shots` | The shot harness on the owner's M2 Pro is within the limits; `shots.json` records the macOS and Metal driver versions. An agent runs it on the Mac and the owner confirms | the owner | 7 |
| 15 | `benchmarks` | Instruction counts for both sim steps, grid traces and `ground_move`; over 5% against the push's base fails without a performance ADR | bench: bench | 8 |
| 16 | `docs-lint` | Decision 16; a stale path fails | lint: docs-check; selftest: violation:docs-stale-path | 9 |
| 17 | `notices` | Notices list every third-party crate in the testbed binary for the three playable targets, with licence texts; a licence off the list fails | lint: notices | 10 |
| 18 | `dist` | The archive holds the stripped binary, debug info, `version.json` and notices; the unpacked binary runs `--version --json` and `run --headless --ticks 60` | native ubuntu-24.04, windows-2025, macos-26: dist | 10 |
| 19 | `mutants` | On pushes that change core crates, no missed mutant outside the exclusions; other pushes write an ok step that says skipped | mutants: in-diff | 11 |
| 20 | `violations-2` | Each fails, with a selftest: `unsafe` outside the shim; `objc2-game-controller` outside the shim; a stale doc path; a missed mutant | selftest: one step each | 6, 9, 11 |
| 21 | `gpu-report-budgets` | As 1b's criterion 9, plus first-person frames within new keys (`[gpu.first_person]`; `[gpu]`'s keys and values stay), per-pass GPU times or `null`, and the pool's texture count | native x5: gpu-budgets | 3 |
| 22 | `proof-video` | As 1b's, plus `room.apng`, the 600-tick room walk | native ubuntu-24.04: proof-video | 12 |
| 23 | `owner-play` | On the owner's Mac: fullscreen, mouse look, WASD, the step, turning sprites, `--look` switches, Esc quits; the owner plays and accepts | the owner | 12 |
| 24 | carried from 1b | `native-build-test`, `native-window` (the dot window), `esc-quits`, `violations-1b`, `crash-json`, `version-json`, `shaders`, `wire-session` (unchanged: no wire change), `live-input`, `lint-tier`, `tests-and-targets` (the core set adds `eng-collision-grid`), `agents-md-length`, `native-duration` | as in 1b's `acceptance.json` | all |
| 25 | `gcmouse-rate` | On the owner's Mac, while the mouse moves fast, motion events arrive at the mouse's rate, not the display's (`play --report`). An agent runs it there and the owner confirms; `owner: true` with no checks, as `acceptance.rs` requires | the owner | 6 |

A taken addition adds its row from the table above (`frame-record`,
`scene-rules`, `fps-poses`, `check-misses`, `two-texts`).

## This window's queue

These four can be built tonight, before G1, by at most two builders at once:
Q1 and Q2 side by side, then Q3 and Q4.

- **They leave alone:** protected paths, new crates, new third-party crates,
  `Cargo.lock`'s package set, and every existing output. Each is a spec item
  that stands if the owner changes this plan.
- **Where:** a worktree pinned with `git worktree add --detach <scratch>
  <sha>`, its own `CARGO_TARGET_DIR`, and `target/debug` linked for `xtask
  replay`; never the main checkout.
- **Commits and logs:** each builder makes one commit in its worktree, which
  the orchestrator reviews and cherry-picks onto `main`. Builders do not edit
  `PROGRESS.md`; the orchestrator writes `docs/rungs/2/PROGRESS.md` from
  their reports (`current.txt` stays `1b`).
- **Check:** `cargo xtask quick` green in the worktree, and a `reviewer` pass
  against the done-when below (there is no rung 2 `acceptance.json` yet).
  The report names the machine its results come from.

**Q1 (T1), `eng-gpu`: the planned graph orders, culls, pools and labels**
*[spec 6.7]*.
- **Files.** A new module, such as `src/plan.rs` with `plan/order.rs`,
  `plan/cull.rs` and `plan/pool.rs`, holds a new declaration type (a "pass
  spec": name, reads, writes, continues, and whether it may be bypassed),
  its resource descriptors, a new constructor for a planned graph, and an
  error type of its own. Names are the builder's. Labels go in `src/pass.rs`.
  New CPU-only tests: `tests/plan_order.rs` and `tests/plan_pool.rs`. Draft
  ADR-0019 as "Proposed".
- **Not in Q1:** running a planned graph on the GPU (T8), per-pass GPU times
  (T8, O11) and depth or several writes (T8).
- **Done when:**
  1. `Graph::new`, `Graph::fixed`, `PassDecl` (fields `name`, `reads`,
     `writes`), `Resource` and `GraphError` keep their signatures and
     behaviour. The protected `counters.rs`, whose
     `Graph::new(&[DRAW, TONEMAP, AGAIN])` has two writers of `Output`, and
     `shaders.rs` pass unchanged on Metal with `ENG_REQUIRE_GPU=1`.
  2. **Ordering:** every permutation of a spec list gives the same order
     (proptest), with ties broken by pass name. A pass that continues a
     resource comes after its writer and before its readers.
  3. **Errors**, as `[E-GRAPH]` naming the passes: a cycle; two writers of
     one resource where neither continues the other; a read that nothing
     writes.
  4. **Culling:** a pass whose writes reach neither the output nor the
     surface is dropped, and the plan lists it. A disabled pass that may be
     bypassed is removed and its readers read its input; if the formats
     differ, that is `[E-GRAPH]`.
  5. **Pooling** (the plan, on the CPU): transients with the same
     descriptor and lifetimes that do not overlap share a slot. A chain of
     four passes over ping-pong transients needs two slots. The plan
     reports its slot count, and the same specs at the same size give the
     same plan.
  6. **Labels:** each existing pass's render pass and encoder is labelled
     with its declaration's name; with `WGPU_DEBUG=1`, each pass sits in a
     debug group of that name. The counters do not change.
  7. **Pixels:** the four testbed shots rendered on this Mac after the
     change are byte-identical to the same shots rendered from f3d8513 on
     the same machine. The reference, for comparison: 1b's T16 candidates
     have SHA-256 prefixes `c6cb0547eb48a67e` (`dot-square-0`),
     `bd264452cd2f5ef4` (`-120`), `d1726b1ecac42ebc` (`-240`) and
     `8bb9845b5e6aa2c2` (`ten-dots-0`), the same bytes on all five CI
     runners. The files are run 41302087332's `golden-candidates-*`
     artifacts; the scratch copies are gone.

**Q2 (T2), `kit-fps`: ground movement** *[spec 6.11]*.
- **Files.** A new `src/ground.rs` holds `GroundConfig` v1 (gravity, step
  height, snap distance, steepest walkable normal) and `ground_move`, whose
  outcome says whether the hull ends on the ground. It is re-exported from
  `lib.rs`; `pmove`, `MoveState` and `MoveConfig` are unchanged. New
  `tests/ground.rs`, with a test-only world of solid boxes in
  `tests/support/boxes.rs`.
- **The rules** are decision 3's: the integration order, the tie rule,
  `vel.z` for vertical speed, and "on the ground" recomputed each tick.
- **Done when:**
  1. On flat ground with no step in reach, `ground_move` gives `pmove`'s
     `MoveState` bit for bit, with `vel.z` 0 and z unchanged.
  2. A 0.4 m step is climbed within one tick and a 0.41 m one blocks. The
     stepped result is kept only when its horizontal travel is strictly
     greater, comparing squared distances exactly.
  3. Walking off a ledge no deeper than the snap distance keeps the hull on
     the ground.
  4. Off a deeper one, the hull is in the air. Each airborne tick,
     `vel.z ← vel.z − g·dt`, then the sweep by `vel·dt`, so `vel.z` drops by
     exactly g·dt (as f32 computes it) until landing sets it to 0.
  5. Over proptest moves the hull never overlaps solid.
  6. `cargo xtask replay` passes without `--regen` (the testbed does not use
     the new code). `arch-check` passes, and the crate builds for riscv64
     and wasm32.

**Q3 (T3), `eng-input`: mouse look** *[spec 6.10, 6.14]*.
- **Files.** A new `src/look.rs` holds `LookSettings` and `MouseLook`.
  `LiveInput` takes mouse motion when look is on and keeps `set_view`
  otherwise. New `tests/mouse_look.rs`, to be protected at G5.
- **Done when** decision 9's rules hold in tests:
  1. the signs (`dx > 0` lowers yaw; `dy > 0` lowers pitch, raises it when
     inverted);
  2. any split of the same motion into ticks gives the same yaw, from the
     exact sum, ties half away from zero;
  3. the pitch limit, yaw wrapping, and focus loss dropping unsampled motion;
  4. the same timeline gives the same commands (proptest);
  5. the protected `live_input.rs` and `twin.rs` pass unchanged.

**Q4 (T4), `eng-render`: CPU parts of the retro pipeline** *[spec 6.7]*.
- **Files.** New `src/camera.rs` (first-person camera: f64 position,
  `BAngle` yaw and pitch, vertical FOV; view relative to the camera;
  reversed-Z infinite projection); `src/sprite_rot.rs` (decision 4's rule
  and mirror flags); `src/palette.rs` (a LUT from a list of sRGB8 colours by
  OKLab distance, and a Bayer matrix; the `.gpl` reader is T10's). Tests
  `tests/camera.rs`, `tests/sprite_rotation.rs` and `tests/palette.rs`. No
  shader and no GPU code; floats from `eng-math` only.
- **Done when:**
  1. **Camera:** the near plane maps to depth 1 and 10^6 m to under 1e-6,
     and points behind have w ≤ 0. A camera 10^9 m from the origin projects
     like one at it. Yaw 0 looks along +X, positive yaw toward +Y, Z up.
  2. **Rotations** (decision 4) match an independent oracle (f64 `atan2`
     from the two positions, then the 45° sector) away from the
     boundaries. Hand-checked cases: facing the viewer gives 0 (the front);
     a viewer 90° counter-clockwise gives 2 (side-on); behind gives 4; a
     direction exactly `2^28` past the facing gives 1 (the tie rule).
     Mirror flags as decision 4 says.
  3. **Palette:** every cell holds the colour nearest its centre, ties to
     the lowest index, checked against hand-computed cells for a two- and a
     three-colour palette. OKLab conversions match the known-answer values
     Björn Ottosson publishes, within 1e-3. Two bakes give the same bytes.

## Proposed time-box and AI budget (for the owner to set)

**How the numbers were reached, from 1b's actuals.** 1b planned 17 tasks and
two reviews (19 units) and used 31 sessions, five of them planning. So each
planned unit cost 26 / 19 = 1.37 sessions, counting follow-ups, review fixes,
patches and the first-run fix. Rung 2's units are counted the same way, so
nothing counts twice:
- T0-T22, one each: 23;
- T23, the two reviews, as 1b planned them: 2 (their fixes are in the 1.37);
- T24, collecting candidates, as 1b's T16: 1 (first-run fixes are in the
  1.37);
- T25, as 1b's T17: 1.

27 units × 1.37 is about 37, plus two planning sessions (this draft and its
revision): **39**.

**The proposal.**
- **AI budget:** the owner's current Claude plan and nothing else (P1
  default). At most **39 agent sessions** for the base rung, **48** with the
  recommended additions, or **50** with all five, logged in
  `docs/rungs/2/PROGRESS.md`. As in 1b, a task with its reviewer is one
  session, and the queue's sessions count.
- **Time-box:** **14 calendar days from G1**, as in 1a and 1b.
  - Agent time is about 9-10 five-hour windows at two builders each, 3-5 days
    at two or three windows a day. In 1b, four builders and their reviewers
    used up a window in under three hours.
  - The rest is the owner's three sittings and the new jobs' first runs. In
    1b the owner's gates were the long pole: G6 and G7 are still open five
    days after green CI.
  - The pivot rule (7.7) compares rungs 1a-3 with twice their boxes; 1a took
    one day of 14.
- **Usage limit:** at most two builders at once, a large task paired with a
  small one. A builder stopped by the limit keeps its context and resumes
  after the reset (1b lesson).
- **Owner time** (estimate): three sittings of about an hour each. G1-G4
  (decide O1-O11), G5 (apply patches, push), G6-G7 (goldens, play).
- **CI:** free public runners. `bench` and `mutants` run beside the PR tier,
  timed by `budgets.toml [ci]` (G5).

**After rung 2: deload D1** *[spec 7.7]*. D1 follows at once and adds no
features, so rung 2 must not push its work into it. It takes: duplicate code
between the top-down and first-person paths; an audit of AGENTS.md, which is
full; refreshing ARCHITECTURE.md and the ADR index; the performance debt the
first benchmarks show; non-floor patch bumps; reviewing the time-boxes of 1a,
1b and 2 against the pivot rule; and the owner's call on a nightly tier and
the coverage floor (8.2).

## Tasks

All *[proposal]*. Each task ends with `cargo xtask quick` green and one
commit, which the orchestrator reviews and cherry-picks onto `main`; crate
tasks get a `reviewer` pass. The orchestrator writes the `PROGRESS.md` entry
from the builder's report. Protected changes
are patch files in `docs/rungs/2/patches/`, made with `--full-index`, tested
in a scratch clone, and applied by the owner. **Carry:** notes are hand-offs
from 1b or from an earlier task. They live here, because a note left only in
an entry gets lost (1b lesson).

| Task | Needs | Builds | Done when |
|---|---|---|---|
| T0 | G1 | The G2 patch: skeletons of the four crates, their `architecture.toml` entries, the edges added to existing entries, and `overflow-checks` for `eng-collision-grid` | `quick` green in a scratch clone with the patch |
| T1-T4 | none | This window's queue | as in the queue |
| T5 | T0 | `eng-collision-grid` (decision 2), its brute-force oracle test (to be protected), RISC-V. **Carry:** `BoxRoom`'s clamp lands a hull exactly on a wall (`dot-corner` asserts `x == 7.75`); the grid should do the same in f64, so the room's corner assertion can be exact too | criterion 3's tests pass; `arch-check` passes |
| T6 | G1, G4 | Maps and sim data (decision 1, ADR-0023): `eng-ecs`/`eng-sim` constant resources and `Sim::sim_data_hash`; `GameMaps` and the new entry points beside `GameDef`; `--map`; `sim_data_hash` in recordings and in hash-stream headers for maps with sim data; `replay verify` choosing the map by it. Patch: xtask replay reads `map` from `scenarios.toml`. **Carry:** the protected tests in "Protected tests that pin today's APIs" build `SimConfig`, `GameDef` and `Welcome` by literal | the dot streams are byte-identical; a sim with a constant resource snapshots and hashes its state without it; `hash_equality.rs`, `never_panic.rs`, `twin.rs`, `version.rs` and `exit_codes.rs` pass unchanged |
| T7 | T2, T5, T6 | The `room` map in `game-testbed-sim` (the room, spawns, things, `ground_move`); scenarios `room-walk`, `room-ledge`, `room-corner`; their assertions as a patch; streams committed. **Carry:** `[sim]` budgets are 4,096 state bytes and 16 entities | criterion 5; the room streams match across the targets this Mac can run (arm64, Docker x64, QEMU riscv) |
| T8 | T1, O11 | `eng-gpu`: `Graph::render` runs a planned graph (transients from Q1's pool, kept across frames of one size), depth, several writes and continues, textures, the per-frame uniform, the scene layout (decision 6, ADR-0020), and, if O11 says so, timestamps per pass, asking for `TIMESTAMP_QUERY` only when the adapter has it. **Carry:** 1b's one instance buffer per frame and one input bind group per pass are what megabuffers and pooling replace (T8 of 1b, decision 10); exposure needs the uniform (ADR-0017); the surface format stays `Bgra8UnormSrgb` | offscreen depth and texture-array tests on Metal and Docker lavapipe; `counters.rs` unchanged |
| T9 | T4, T8 | `eng-render`: the `opaque` and `sky` passes, camera-relative chunk draws, `info` | a known room renders with depth probes as computed on the CPU |
| T10 | T9 | `eng-render`: the `light`, `fog`, `tonemap` (decision 7) and `palette` passes, the `.gpl` reader, the k×k present, the switches | decision 13's checks pass on Metal and lavapipe |
| T11 | T9 | `eng-render`: the `sprites` pass (decision 4). Built beside T10 in separate modules; T10 lands first if both touch `renderer.rs` | criterion 8's tests |
| T12 | T0, T7, T9, T11 | `game-testbed-client`: the top-down extraction moved unchanged; the first-person extraction; region looks; textures and sprites made by code; `palettes/room.gpl`, its generator script and its provenance sidecar. **Carry:** top-down colours are sRGB-level centres at 40 px/m; `view_yaw` 0x4000 is for the dot map only; `GameDef` keeps its fields (`twin.rs:33`) | the four 1b shots are pixel-identical; a first-person shot renders |
| T13 | G1, G4 | The platform contract change (ADR-0022, with decision 10's migration notes): new `Control` methods with default bodies for cursor lock, the Confined fallback, hiding and borderless fullscreen; headless records requests; winit maps mouse motion, buttons and wheel. Patch: a separate suite, a new file under `src/conformance/` plus its `mod` line in `src/conformance.rs`, and new headless and winit tests for it (to be protected). **Carry (1b T10):** the contract's wheel `y` keeps winit's sign and its `x` is winit's negated; under Xvfb a notch came twice; winit hides macOS's natural scrolling and acceleration; raw motion comes only while focused | the new suite passes headless and on winit where a display exists; the existing suite, `eng-platform-headless/tests/conformance.rs` and `twin.rs` pass unchanged |
| T14 | T0, G3, G4, T13 | `eng-platform-gcmouse` (decision 11, ADR-0021); `examples/events.rs` prints its deltas. **Carry:** GCMouse's sign of `deltaY` and its units are unverified; check them on the owner's Mac with the screen unlocked | builds on macos-26; unit tests with a fake source; on this Mac, deltas arrive per event |
| T15 | T3, T12, T13 | `eng-app`: first-person `play` (decisions 9, 10), the flags, the live view angles, through new functions; `play_on` and `PlayRun::start` keep their signatures (`twin.rs:76,84`). **Carry (1b T11):** waking at once after each present spins under Xvfb (no vsync); a lost surface is a panic and should now be recreated from the kept window; a locked Mac screen counts as occluded, so no frames | `play.rs` and `twin.rs` pass unchanged; a windowed room run gives its committed stream |
| T16 | T10, T12 | Shots: `shots.toml`, the first-person shots, `--view`, per-shot thresholds, and the look harness `tests/look.rs` (protected), all as patches. **Carry:** pass `LP_NATIVE_VECTOR_WIDTH=256` only; candidates go to `target/golden-candidates/`; p999 cannot see under about 77 px | candidates from Metal and lavapipe |
| T17 | T0, G3, T5, T7 | `eng-bench`; `cargo xtask bench` and `ci --job bench` (patch); the workflow job with valgrind and `gungraun-runner` =0.20.0; comparison against the push's base (decision 14). **Carry:** xtask's replay runs `<repo>/target/debug/game-testbed`, so a separate target directory needs that link; gungraun only on Linux, so `cargo test --all-targets` still builds on the other four native runners | a local Docker run on Linux x64 reports counts for a head and its base; `eng-bench` builds on this Mac |
| T18 | G1 | Docs-lint in `docs-check` (patch) and its selftest | a stale path in a scratch copy of AGENTS.md fails |
| T19 | G1 | `cargo xtask mutants` and `ci --job mutants` (shards, time-box, the diff from the push's base, an ok step when skipped), the weekly schedule, `.cargo/mutants.toml`, the `test-auditor` subagent (spec 7.5), all patches | an in-diff run on T2's change reports its mutants; a push with no core change writes an ok step |
| T20 | G1, O5 | `cargo xtask notices` and `dist` (decision 17), `about.toml` and its template (to be protected), patches | notices and an archive on this Mac; Linux in Docker |
| T21 | T15-T20 | The G5 package: new `test` and `native` steps, the `selftest` cases, the summary and acceptance for the new jobs, workflows, `budgets.toml` (a new `[gpu.first_person]` table; `[gpu]` stays), protection. **Carry (1b):** build26's Mesa has no window surface, so Ubuntu's lavapipe goes first in `VK_DRIVER_FILES` for windows; find windows by `--pid`; on Windows a running `xtask.exe` cannot be replaced and `canonicalize` gives `\\?\`; Xcode's git defaults to `main` | each job's steps pass where they can run locally |
| T22 | T15-T20 | Docs, beside T21 (as 1b's T14 ran beside T13): `AGENTS.md` (150 lines already: every new line replaces one), `docs/ARCHITECTURE.md`, ADR-0019 to ADR-0023, the `perf-investigate` skill (patch) | `docs-check` passes, docs-lint included |
| T23 | T21, T22 | Two fresh-context `reviewer` passes over the rung, then fixes | no open blockers |
| T24 | G5 | The first CI runs: fixes, golden candidates and FLIP maps (`golden-update` skill), `owner-mac-shots` on this Mac | the owner has every candidate for G6 |
| T25 | T23, T24 | `room.apng` and a short scripted session for the owner | the owner accepts (G7) |

**Order, with at most two builders at a time:**
1. tonight: T1 with T2, then T3 with T4;
2. after G1-G4: T0 alone, then T5 with T6, T7 with T8, T9 with T13, T10
   with T11, T12 with T14, T15 with T16, T17 with T18, and T19 with T20;
3. T21 with T22;
4. T23;
5. after G5: T24, then T25.

## Owner decisions this rung needs

- **O1. Approve** the plan, acceptance, time-box and budget (G1), and set
  `current.txt` to `2`. Close 1b first or in the same sitting: 1b's G6 and
  G7, and ADR-0017.
- **O2. Maps.** A second map with sim data (recommended: 1b's oracles stay
  byte-identical, no protected test changes, and rung 3 needs both), or
  replace the dot room. Replacing it means regenerated dot streams, changed
  top-down goldens and changed dot assertions.
- **O3. The GCMouse shim.** Recommended: a separate crate with the engine's
  first `unsafe` (ADR-0021). The alternatives are a module inside
  `eng-platform-winit` (`forbid` relaxed there), or no shim, keeping winit's
  coalesced macOS deltas, which spec 6.10 warns mislead playtests.
- **O4. Collision oracle.** Brute force (recommended), or `parry3d-f64`
  0.31.1 as a test-only dependency (a crate batch of its own).
- **O5. dist.** `cargo xtask dist` (recommended), or the `dist` tool
  (cargo-dist 0.34.0) with its generated workflow and `[profile.dist]`.
- **O6. Agent seeing.** Take or drop A1-A5 (recommended: A1, A3, A4 small,
  A2's three rules).
- **O7. Benchmark gate.** Comparison with the push's base plus a performance
  ADR (recommended), or committed baselines (protected).
- **O8. Mutants.** Fail on a missed mutant in the diff (recommended), or
  report only until D1.
- **O9. Fullscreen by default** for first-person maps (spec 6.10), with
  `--windowed` for tests. Confirm.
- **O10. Tone curve** (spec 15 leaves it to the builder): Khronos PBR
  Neutral. Only to inform.
- **O11. GPU timestamps.** Ask for `TIMESTAMP_QUERY` when the adapter has it,
  for per-pass times only (recommended: 6.7 asks for them "when available"),
  or keep 1b's default-features-only device and report `null`. Either way
  ARCHITECTURE.md's "Tier 0" text is updated; T8 waits for the answer.
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
`current.txt`, and G4's ADRs (rule decisions). Agents prepare each change
and the owner applies it. The sittings are batched as in 1b:
1. G1-G4 at the start;
2. G5 when the patches are ready, ending with a push;
3. G6-G7 after that run's candidates.

| Gate | Path | Change | Unblocks |
|---|---|---|---|
| G1 | this plan; `docs/rungs/2/acceptance.json`; `current.txt` | Approve; write the criteria in 1b's format; set `2`; decide O1-O11 | everything after the queue |
| G2 | `architecture.toml`; root `Cargo.toml` `[profile]` | The four crates and the edges in "New crates"; `[profile.release.package.eng-collision-grid] overflow-checks = true`. One commit with T0's skeletons | T5, T12, T14, T17 |
| G3 | `deny.toml`; `_typos.toml` if needed | Crate batch 3 below; a wrapper ban that lets only `eng-platform-gcmouse` depend on `objc2-game-controller`; a `clarify` entry for `bincode-next` if cargo-deny cannot read its licence string | T14, T17 |
| G4 | ADRs (rule decisions) | ADR-0021 (`unsafe` in the shim), ADR-0022 (the platform contract: cursor and fullscreen, with migration notes), ADR-0023 (maps and sim data; no wire change). ADR-0019 and ADR-0020 record Claude's calls, for reading | T6, T13, T14 |
| G5 | `xtask/**` | Docs-lint; `bench`; `mutants`; `notices` and `dist`; replay reads `map`; new `test` steps (collision, movement, sim-data, render-graph, sprites, mouse-look) and `native` steps (window-room, look, dist); proof video adds `room.apng`; selftest cases; summary and acceptance for the new jobs; `arch-check` refuses `allow(unsafe_code)` outside the shim crate | criteria 1-21 |
| G5 | `.github/workflows/**` | The `bench` job (valgrind, `gungraun-runner` =0.20.0), the `mutants` job (`cargo-mutants` =27.1.0, four shards, a weekly `schedule`), `cargo-about` =0.9.3 in `lint`, `dist` artifacts on three runners, `room.apng`; actions pinned by SHA, `permissions: {}` | the new jobs exist |
| G5 | `budgets.toml` | A new `[gpu.first_person]` table for first-person frames (from T10's counters); `[gpu]`'s keys and values stay, as `shot_video.rs:231-232` asserts them; `[ci]` minutes for `bench` and `mutants` | 15, 19, 21 |
| G5 | `protected.toml`, `.github/CODEOWNERS` | Protect `shots.toml`, `tests/look.rs`, `crates/eng-input/tests/mouse_look.rs`, the grid's oracle test, `kit-fps/tests/ground.rs`, `about.toml` and its template, and, if A2 is taken, `**/*.rules.toml` | criteria cannot be weakened |
| G5 | `crates/eng-platform-api/src/conformance.rs`, `src/conformance/**` | The cursor and fullscreen suite: a new file and its `mod` line; the existing suites unchanged | 10 |
| G5 | `scenarios.toml`, `**/*.assert.toml` | The three room scenarios with `map = "room"`, and their assertions | 1, 2, 4 |
| G5 | `crates/game-testbed/tests/shots.rs`, `tests/shots/**` | Read the manifest instead of `SHOTS`; per-shot thresholds | 12, 13 |
| G5 | `.cargo/mutants.toml` | Exclusions, empty to start | 19 |
| G5 | `.claude/**` | The `test-auditor` subagent and the `perf-investigate` skill (spec 7.5) | T19, T24 |
| G5, if needed | `**/*.proptest-regressions`; `.config/nextest.toml` | Failing proptests' regression files; a test group for GPU tests | as found |
| G6 | `**/golden/**` | Goldens for every manifest shot from the pinned lavapipe, and per-shot limits in `thresholds.toml`; 1b's four, if G6 of 1b is still open | 12, 13 |
| G7 | none | Watch `room.apng`, play, confirm `owner-mac-shots` and `gcmouse-rate`; accept or send back | 14, 23, 25; rung done |

**Protected paths rung 2 leaves alone,** each by a design choice in
"Protected tests that pin today's APIs":
- `known_answers.*`, `eng-core`'s and `eng-schema-core`'s `golden.rs`,
  `exit_codes.rs` and the dot `*.assert.toml`;
- `eng-net-wire`'s `golden.rs` and `never_panic.rs`, `hash_equality.rs`,
  `twin.rs`, `eng-platform-headless/tests/conformance.rs` and `version.rs`;
- `play.rs`, `crash_json.rs`, `live_input.rs`, `shot_video.rs`,
  `counters.rs` and `shaders.rs`;
- `clippy.toml`, `.cargo/config.toml`, `oracle.yml` and
  `rust-toolchain.toml` (1.99.1 is above every new MSRV, the highest being
  `bincode-next`'s 1.90).

A task that needs one of these after all stops and brings the patch to the
next gate.

## Crate batch 3

*[proposal]* The third-party crates G3 adds to `[bans] allow`: 15 names.

- **Source:** a scratch project outside the repo, resolved 2026-11-06, with
  `gungraun =0.20.0` and `objc2-game-controller =0.3.2`
  (`default-features = false`; `std`, `block2`, `GCDevice`, `GCMouse`,
  `GCMouseInput`, `GCPhysicalInputProfile`).
- **Not final:** it ran on this machine's Rust 1.93.1, not the pinned
  1.99.1, and was resolved, not compiled. T14 and T17 recompute it against
  the real lock and report any difference as an amendment.
- **Already allowed:** `objc2` 0.6.5, `objc2-foundation` 0.3.2, `block2`
  0.6.2 (batch 2), `serde`, `serde_json`, `syn`, `itoa`, `memchr`, `zmij`.
  Every new licence is on the list.
- **Flags:** `bincode-next` writes its licence "MIT or Apache-2.0" with a
  lowercase `or`, so cargo-deny may need a `clarify` entry (not run). It is
  not the banned `bincode` (spec 6.8) but a fork, used inside gungraun only;
  MSRV 1.90. gungraun builds only on Linux, but cargo-deny checks every
  listed target, so it needs the entries anyway.

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
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 | derive_more |

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
| GCMouse: `unsafe`, units and signs unverified, and no mouse on macos-26 | the shim's logic is tested with a fake source; real deltas are checked on the owner's Mac (T14, G7) |
| winit 0.30: `Locked` is missing on Windows and X11, `Confined` on macOS | the fallback order covers all four; the report names the mode used |
| Under Xvfb, `xdotool`'s relative motion may not reach XInput2 raw events | the window step checks the grab mode and frames, not mouse motion; mouse look is tested headless |
| AGENTS.md is at 150 of 150 lines | T22 rewrites lines rather than adding them; docs-lint checks what stays |
| A room snapshot over 1,200 bytes | sim data stays out of state (decision 1); criterion 5 checks it |
| T10 and T11 both in `eng-render` | separate modules; T10 lands first; `git apply --3way`; `Cargo.lock` merged as 1b's lesson says |
| Shared `target/` between sessions turns `quick` red | own `CARGO_TARGET_DIR` per builder, `target/debug` linked for replay |
| Mutants runs long on core crates with replays | shards and a 20-minute cap per shard; exclusions are the owner's |
| The bench gate is noisy if two runner images differ | the head and the push's base are built and run in the same job |
| The native jobs build `--all-targets` on Windows and macOS, where gungraun is absent | gungraun is a Linux-only dev-dependency and each bench is an empty `main` elsewhere; T17 checks a build on this Mac |
| A protected test pins an API this plan assumed it could extend | "Protected tests that pin today's APIs" lists what stays; a task that hits another one stops and proposes a patch |
| `bincode-next`'s licence string | a `clarify` entry at G3 if cargo-deny refuses it |
| The owner may dislike the tone curve or the look | every look pass switches off; the curve is one function |
| 1b's G6 and G7 are still open | rung 2 keeps 1b's pictures and streams byte-identical, so either order works |
| Queue work lands before G1 and the owner changes the plan | each item is a spec item in an existing crate; a change costs rework, not a revert |
