# Rung 1b: Windowed dot

> **DRAFT FOR OWNER.** Nothing in this plan is approved. The acceptance
> criteria, the time-box, the AI budget and every protected change below
> are proposals for the owner to accept, change or refuse (touchpoint 2,
> spec 2.2). `docs/rungs/current.txt` stays at 1a, and there is no
> `docs/rungs/1b/acceptance.json`, until the owner acts (gate G1).

- **Status:** draft, 2026-10-25 (second revision). Only the three items
  in "This window's queue" are being built before approval; they need no
  owner gate.
- **Spec:** section 9.2 of `docs/superpowers/specs/2026-10-20-ai-built-engine-design.md`,
  with 6.1-6.2 (crates and rules), 6.6 (frame loop), 6.7 (rendering), 6.8
  (network contracts), 6.10 (input), 6.13 (version stamps, crashes), 7.1
  (probe), 8.2 (native jobs) and 8.3 (GPU tests without GPUs).
- **Goal** *[spec 9.2]*: the 1a dot in a real window, drawn by the GPU
  and moved by the keyboard through the in-process server, so the app
  loop already has its final shape. Screenshots are checked against
  golden images on every CI adapter, and a panic leaves `crash.json`.
- **Where each item comes from.** Every item carries one of three tags:
  - *[spec X]*: the spec asks for it, in section X;
  - *[spec X + proposal]*: the spec asks for it and this plan fills in
    the details, which are proposals;
  - *[proposal]*: Claude's own call, not in the spec.

  The owner can overrule any proposal. Departures from the spec are
  listed in "Where this differs from the spec".

## What gets built

### New crates (7)

Layers are the names in `architecture.toml` (`L0` to `L4`, `game`,
`tool`). Every layer comes from the table in spec 6.1. The `same_layer`
edges are proposals, except `eng-gpu < eng-render`, which spec 6.2 gives
as an example. The exact entries are gate G2.

| Crate | Proposed layer | What it does in 1b | From |
|---|---|---|---|
| `eng-platform-api` | `L1` | The platform contract: plain-data events for everything 6.10 lists (physical keys, raw mouse deltas, mouse buttons, wheel, gamepad axes and buttons, focus) plus resize, occluded and close request; a monotonic clock; the callback trait the app implements; the window seam (ADR-0016); a conformance suite. Mouse and gamepad events are defined now, so rungs 2 and 6 need no contract change. 1b maps only keys, and winit sends no gamepad events until gilrs arrives (rung 6). It takes over `Key` and `KeyEvent` from `eng-input` (see "This window's queue"). | spec 6.1 (the crate, its layer and the winit-headless contract pair), 6.10; the event list beyond keys: proposal |
| `eng-platform-winit` | `L1`, `same_layer = ["eng-platform-api"]` | winit 0.30.13 behind the contract. It is the only crate that imports winit (already the `[floor]` rule). | spec 9.2, 6.2 rule 2; version: proposal |
| `eng-net-wire` | `L1`, core | The codec (6.8 contract 7): little-endian, fixed width, capped counts, and decoders that never panic. It also carries the 32-bit tick and its extension (contract 1) on every message. 1b's messages are `Hello` (wire and sim protocol versions), `Welcome` or `Denied` (version mismatch), `Commands` (the last 3 commands and the last acknowledged snapshot tick) and `Snapshot` (the full state). `Snapshot` carries the sim's canonical state as opaque bytes: `eng-net-wire` (L1) cannot reach `eng-sim`, so `eng-session` encodes and decodes the state. `PlayerCommand` comes from `eng-core` (L0). | spec 6.1, 6.8; building it in 1b: proposal (difference 2) |
| `eng-session` | `L2`, core, `same_layer = ["eng-sim"]` | Server and client loops over `Transport`. It buffers commands and repeats the last one when one is missing. It keeps each client's last acknowledged tick (its baseline) and has a relevance hook that answers "everything" (contract 5). It records the commands actually applied, as plain data that `eng-cli` writes in `eng-input`'s recording format. It has no clock (`now` is passed in) and registers nothing in the sim's World. | spec 9.2, 6.1, 6.6, 6.8 + proposal |
| `eng-gpu` | `L3` | The only crate that imports wgpu (30.0.1, Tier 0). It covers adapter choice, the capability report, surface and offscreen targets, the fixed render graph and readback. Validation errors are fatal in tests. | spec 9.2, 6.7; three passes: proposal (difference 1) |
| `eng-render` | `L3`, `same_layer = ["eng-gpu"]` | The top-down pass (kept later as the automap and debug view), the tone-map pass, the present blit and per-frame counters. | spec 9.2, 6.2, 6.13 |
| `eng-app` | `L4`, `same_layer = ["eng-cli"]` | The frame loop, and the composition of platform, in-process session over Memory, live input and renderer. It runs `play`, quits on the `quit` action (Esc by default) and installs `eng-cli`'s panic hook. | spec 9.2, 6.1, 6.6; the `eng-cli` edge: proposal (difference 9) |

### Changed crates (4)

| Crate | Layer | Change in 1b | From |
|---|---|---|---|
| `eng-platform-headless` | `L1`, unchanged; gains `same_layer = ["eng-platform-api"]` | Implements the contract over its virtual clock; scripted events for tests. | spec 6.1, 6.10 |
| `eng-input` | `L1`, unchanged; gains `eng-platform-api` in `same_layer` | Live input joins scripts and recordings: actions, bindings (strict TOML; WASD, the arrows and Esc by default) and quantization into one `PlayerCommand` per tick. | spec 6.1, 6.10 + proposal |
| `eng-cli` | `L4`, unchanged | `--version` and `--version --json`, `crash.json` from the panic hook, `shot` and `video`; `run` and `dump-world` go through `eng-session`. | spec 9.2, 6.13, 7.1 + proposal |
| `game-testbed` | `game`, unchanged | Enters through `eng-app`; maps the testbed world to top-down shapes. | proposal (difference 6) |

`game-testbed-sim` does not change, and no other crate changes layer.

### One frame

```
OS events ─eng-platform-winit─▶ PlatformEvent ─eng-input─▶ PlayerCommand for tick N
                                                                 │ eng-net-wire bytes
                                                                 ▼
     Memory transport ─▶ eng-session server: buffer, Sim::step, record applied commands
                                                                 │ Snapshot bytes
                                                                 ▼
     eng-session client: FrameSnapshot { previous, current }, alpha = accumulator / dt
                                                                 │
                                                                 ▼
     game extraction ─▶ TopDownScene (WorldPos, f64) ─eng-render─▶
       draw ─▶ scene (Rgba16Float, linear) ─▶ tonemap ─▶ output (Rgba8UnormSrgb, engine-owned)
                                                          │            │
                                         present (blit to surface)    shot, video, goldens
```

- **Ticks come from the timer**, not from presenting *[spec 6.6]*: a
  fixed-timestep accumulator at 60 Hz, woken by `ControlFlow::WaitUntil`.
  At most 5 ticks run per wake; the rest are dropped and reported (a
  count in the report, `[W-TICK-DROP]` in the log) *[proposal]*.
- **Minimized or occluded:** no surface acquisition, because on macOS it
  can block for a second; ticks keep running *[spec 6.6]*.
- **Interpolation:** presentation draws from `previous` to `current` at
  alpha *[spec 6.6]*. `shot` and `video` use alpha 1 at a tick, so they
  are exact *[proposal]*.
- **Camera-relative:** the CPU subtracts the camera in f64 and uploads
  f32 *[AGENTS.md conventions]*.
- **One code path** *[spec 6.8]*: `run --headless` goes through the same
  session over Memory. *[proposal]* The committed hash streams must stay
  byte-identical through the whole rung, with no `--regen` in 1b. That
  holds while three things stay true:
  - `game-testbed-sim` does not change;
  - nothing new is registered in the sim's World, because the protocol
    version hashes every schema and the system order;
  - a full snapshot fits in one 1,200-byte datagram (the dot's state is
    451 bytes).

  A larger snapshot fails with `[E-SNAPSHOT-SIZE]`; fragmentation comes
  in rung 5.

### Probe additions

```
<game> play [--ticks N] [--input script.toml] [--bindings b.toml] [--seed S]
            [--hashes h.txt] [--record out.rec] [--report r.json]
<game> shot --at-tick N --out shot.png [--size 320x240] [--input script.toml] [--seed S] [--report r.json]
<game> video --ticks N --out proof.apng [--every K] [--input script.toml] [--seed S]
<game> --version
<game> --version --json
```

- **`play`** *[spec 9.2 done-when 2 + proposal]*: with no flags it is
  the game: a window, the keyboard, and Esc to quit. `--ticks N` closes
  the window after tick N. With `--input`, commands come from the script
  and only `quit` is read from the keyboard, so its hash stream must
  equal the headless one tick for tick. `--bindings` replaces the default
  bindings.
- **`shot`** *[spec 7.1, 9.2 + proposal]*: renders offscreen at a tick
  and writes a PNG. The size, input and report flags are proposals.
- **`video`** *[spec 7.1 + proposal]*: the proof video, saved as a local
  file. It is an animated PNG, so no video encoder or ffmpeg is needed
  (difference 5).
- **`--version --json`** *[spec 6.13, 9.2 + proposal]*: prints
  `version.json`. Its fields are semver, engine and game SHAs, the dirty
  flag, the commit time (`SOURCE_DATE_EPOCH`), the CI run, target, rustc
  and profile (the spec's list). `cargo xtask` computes the file (spec).
  The proposals are how the values get there: X1 passes them to the
  build as `ENG_*` variables, and a plain `cargo build` says `unknown`
  for each value only xtask knows.
- **`--version`** *[proposal]*: the same stamp on one line for people,
  for example `game-testbed 0.1.0 (engine 1a2b3c4, dirty) aarch64-apple-darwin release`.
- **`crash.json`** *[spec 6.13 + proposal]*: in every mode the panic hook
  writes the version stamp, thread, backtrace, last log lines and seed
  (the spec's fields), plus the tick and the command line (proposed). It
  keeps the last 50 log lines and writes to
  `target/agent-logs/crash-<game>-<pid>.json` (or `$ENG_AGENT_LOG_DIR`),
  then exits 12 as today. Crash files stay local.
- **Reports gain a `gpu` object** *[spec 6.13, 8.2 + proposal]*: adapter
  name, backend, device type, driver and driver info, tier (0), whether
  timestamp queries exist, and the per-frame counters of 6.13 (draws,
  pipeline switches, bind-group switches, bytes uploaded).

## Decisions (*[proposal]* unless tagged: Claude's calls; the owner can overrule any)

1. **The render graph is a fixed list of three passes** that declare what
   they read and write *[spec 6.3, 6.7: declared reads and writes]*:
   `draw` writes `scene`; `tonemap` reads `scene` and writes `output`;
   `present` reads `output` and writes `surface` (absent offscreen).
   Building the graph checks that each read was written earlier and that
   `output` is never the surface. Ordering, culling and pooling are rung 2
   *[spec 6.7]*.
2. **1b's tone map is a clamp to [0, 1]**; the `Rgba8UnormSrgb` target does
   the sRGB encoding. The curve (AgX, Tony McMapface or Khronos PBR Neutral,
   spec 15) is chosen at rung 2 with lights and EV100 exposure.
3. **Shots are 320x240, no MSAA, a fixed clear colour, alpha 1** (fixed
   sizes and frame index: *[spec 8.3]*).
   - There is one golden per shot, rendered on the pinned lavapipe
     configuration and kept in `crates/game-testbed/golden/` (protected).
     The FLIP thresholds are in `golden/thresholds.toml`.
   - Exact on Linux x64 and FLIP on every other adapter is *[spec 8.3]*.
     The proposals are what is compared: decoded RGBA8 pixels, not PNG
     bytes, which change with the encoder, and one lavapipe golden as
     the reference for every adapter.
   - The harness is `crates/game-testbed/tests/shots.rs`, because
     `golden.rs` is a protected name an agent cannot create.
   - Candidates are written under `target/golden-candidates/`, never
     into `golden/`: the guard would put a file there back and
     quarantine it.
   - While a golden is missing, the step saves the candidate, reports "no
     golden yet" and does not fail the job. Acceptance stays false until
     the owner commits the golden (G6).
4. **Adapter choice:** `ENG_GPU_ADAPTER` picks by name. With no adapter a
   GPU test skips locally, but fails in CI (`ENG_REQUIRE_GPU=1`), and the
   probe exits 64 with `[E-NO-ADAPTER]`. No new exit code.
5. **wgpu features:** `std`, `dx12`, `metal`, `vulkan`, `wgsl`; no `gles`
   or `webgpu`. **winit features:** `rwh_06`, `x11`, `wayland`,
   `wayland-dlopen`; no client-side decorations on Wayland. With these,
   both should build without system development packages, loading X11,
   Wayland and Vulkan at run time; T7 and T10 confirm it.
6. **No async runtime or `pollster`:** wgpu's futures are driven by a small
   `block_on` over `Waker::noop()` and `device.poll`.
7. **Present mode Fifo** (the one every backend has). A resizable window,
   not fullscreen; borderless fullscreen and cursor lock arrive with the
   first-person view (rung 2).
8. **Input:**
   - Actions are sampled once per tick from held keys, with a latch so a
     tap shorter than a tick still counts for one tick.
   - Losing focus releases every key.
   - Key codes are physical, so they do not depend on the keyboard layout
     *[spec 6.10]*.
   - `quit` is an app action handled by `eng-app` and never becomes a
     `PlayerCommand`.
   - `eng-input` reads no clock; the caller says which tick it samples.
9. **Session in 1b:** full-state snapshots every tick, no prediction. Over
   Memory in one thread the client is never behind, so neither is needed
   yet; deltas and prediction come with loss (rung 5).
10. **The DX12 shader compiler** is chosen in T7, preferring one that needs
    no extra DLL next to the binary.
11. **Shader checks** *[spec 6.7]*: an `eng-gpu` test validates every
    WGSL file with naga and translates it to SPIR-V, MSL and HLSL, so a
    Mac-only shader fails on every runner. Running it as a named step in
    the `test` job is a proposal (X7).

## Where this differs from the spec, and why

1. **Three passes, not two.** The spec contradicts itself here. 9.2 says
   "draw, then a blit", and 6.7 says "Rung 1b uses a fixed two-pass
   graph: draw, then present". But 6.7 also makes tone mapping the last
   pass, writing into an engine-owned texture that screenshots read, with
   a separate blit into the surface. All of that can hold only with three
   passes. To be recorded as ADR-0017, amending ADR-0003.
2. **`eng-net-wire` arrives in 1b.** 6.8 encodes messages to bytes even
   in-process ("one code path"), and 6.1 puts the codec in `eng-net-wire`.
   Only 1b's four messages are built. `eng-net-proto` (packet headers,
   acks, channels, fragmentation, the challenge cookie) waits for rung 5,
   when there is loss to handle.
3. **`eng-platform-api` is added.** 9.2 does not list it. But 6.1 does,
   and it names "platform: winit, headless" as a contract pair, which 1b
   completes. So this fills a gap in 9.2's list rather than changing the
   design; the 1a plan promised it too.
4. **A window-handle exception to rule 4.**
   - wgpu needs a window handle from winit. Both use `raw-window-handle`
     0.6, a neutral interop crate that SDL3's Rust bindings also
     implement, so the swap path in 6.3 survives.
   - ADR-0016 proposes its two traits (`HasWindowHandle`,
     `HasDisplayHandle`) as the one exception besides serde. They would
     be allowed only between `eng-platform-api`, `eng-platform-winit`,
     `eng-gpu` and `eng-app`.
   - The owner decides (G4).
5. **The proof video is an animated PNG**, made with the `png` crate that
   the goldens need anyway, not a video file.
6. **No `game-testbed-client` crate yet.** The testbed's extraction is one
   small file in the binary crate; it splits out when the first-person
   view needs more (rung 2).
7. **Added checks:**
   - replay equality on all five native targets (8.2 runs it nightly, and
     the jobs exist anyway);
   - the windowed run's hash stream must equal the headless one;
   - Esc is sent through the real OS path with `xdotool` under xvfb on
     Linux x64.
8. **Deferred:**
   - cursor lock, borderless fullscreen and the GCMouse shim (rung 2);
   - the App Nap assertion, which is for hosting (rung 5);
   - shader hot reload (rung 3, with data);
   - gilrs (rung 6).

   The native jobs' null audio sink has nothing to route until rung 6.
9. **The panic hook stays in `eng-cli`.** 6.1 gives the crash handler to
   `eng-app`. But the probe already owns the hook and its exit code, and
   `eng-app` depends on `eng-cli`, so both modes share one hook.
10. **Network contracts 3-5 (6.8) are only partly built.** Contracts 1, 2,
    6 and 7 are built in full. For the other three:
    - In 1b the snapshot is the whole sim world, and presentation state
      lives outside it by construction (contract 4).
    - The relevance hook answers "everything", and the server keeps each
      client's acknowledged tick (contract 5).
    - Generational network entity IDs (contract 3) mean nothing until
      snapshots list entities one by one. That comes with deltas and
      `eng-replication` in rung 5, and their type is fixed then, before
      any game depends on it.
11. **Done-when 2 and 5 are the owner's checks.** CI cannot see the
    owner's Mac. Criteria 2, 3, 15 and 18 below are the machine-checked
    part.

## Proposed acceptance criteria

These are the draft for `docs/rungs/1b/acceptance.json`, which the owner
writes (G1). The job and step names are proposals; the CI jobs come from
patches X2, X3, X6 and X7. "native x5" means the five runners in spec 8.2:
`ubuntu-24.04`, `ubuntu-24.04-arm`, `windows-2025`, `windows-11-arm` and
`macos-26`. The numbers follow the first draft; 18 was added in this
revision.

1. **`native-build-test`** *[spec 9.2 done-when 1, 8.2]*. The build and
   every test pass on the five native runners, and
   `cargo check --target x86_64-apple-darwin` passes on `macos-26`.
   Checks: native x5: build, tests; native macos-26: check-intel-mac.
2. **`native-window`** *[spec 9.2 done-when 1 and 2, 8.2 + proposal]*.
   `play --ticks 240 --seed 1 --input dot-square.input.toml --hashes h.txt`
   opens a real window (xvfb on Linux), and `h.txt` equals the committed
   `hashes/dot-square.seed1.txt`. Checks: native x5: window.
3. **`esc-quits`** *[spec 9.2 done-when 2 + proposal]*. A running `play`
   gets Escape from `xdotool` and exits 0 within 5 s; a headless-twin
   test covers every target. Checks: native ubuntu-24.04: esc; native x5:
   tests.
4. **`shot-exact`** *[spec 9.2 done-when 3, 8.3]*. On `ubuntu-24.04` x64,
   `shot`'s decoded RGBA8 pixels equal the committed goldens exactly. The
   adapter is Mesa 25.2.7 lavapipe (gfx-rs/ci-build build26) with a fixed
   llvmpipe environment. Checks: native ubuntu-24.04: shot-exact.
5. **`shot-flip`** *[spec 9.2 done-when 3, 8.3]*. On the other four
   runners, the FLIP error against the same goldens is within
   `golden/thresholds.toml`. Checks: native (other 4): shot-flip.
6. **`crash-json`** *[spec 9.2 done-when 4, 6.13 + proposal]*. A panic in
   `run` and in `play` writes `crash.json` with every field and exits 12.
   Checks: test x64, arm64: crash-json (`run` only, since the test job
   has no display; step added by X7); native x5: crash-json (`run` and
   `play`).
7. **`replay-cross-platform`** *[spec 8.2 + proposal]*. Hash streams are
   identical on the five native targets, the PR-tier Linux targets and
   RISC-V, and they equal the committed streams, which are not
   regenerated in 1b. Checks: test, riscv, native x5: replay; summary:
   compare-hashes.
8. **`version-json`** *[spec 9.2, 6.13 + proposal]*. `--version --json`
   prints every field of `version.json`, and `--version` prints the same
   stamp on one line. A CI build has no `unknown`. Checks: native x5:
   version.
9. **`gpu-report-budgets`** *[spec 6.13, 8.2 + proposal]*. Reports record
   the adapter and capabilities. Draws, pipeline switches, bind-group
   switches and uploaded bytes per frame are within `budgets.toml`
   `[gpu]`. Checks: native x5: gpu-budgets.
10. **`shaders`** *[spec 6.7]*. Every WGSL file validates with naga and
    translates to SPIR-V, MSL and HLSL. Checks: test x64: shaders (step
    added by X7).
11. **`wire-session`** *[spec 6.8 + proposal]*. The `eng-net-wire` golden
    vectors hold, and its decoders never panic (proptest). The session
    gives the same hashes as stepping the sim directly. Checks: test x64,
    arm64: wire-session (a named step running those three test files,
    added by X7); riscv: core-tests.
12. **`violations-1b`** *[spec 6.2, 7.3 + proposal]*. Each of these fails,
    with a self-test:
    - wgpu imported outside `eng-gpu`;
    - winit imported outside `eng-platform-winit`;
    - `raw-window-handle` used outside its seam (ADR-0016);
    - a core crate depending on `eng-gpu`;
    - an edited golden image.

    Checks: selftest: one step each.
13. **`native-duration`** *[proposal, like 1a's done-when 5]*. The native
    tier's duration is measured and recorded. The proposed budget is 25
    minutes; going over is reported, not failed, as in 1a. Checks:
    summary: native-duration.
14. **`lint-tier` and `tests-and-targets`** *[as in 1a]*. These are two
    features, unchanged except that the core set now includes
    `eng-net-wire` and `eng-session` (RISC-V tests, wasm32 check). Checks:
    as in 1a.
15. **`proof-video`** *[spec 7.1, 9.2 + proposal]*. CI writes
    `proof.apng`, the scripted dot-square session, as an artifact. The
    spec asks for a proof video saved as a local file; the APNG format
    and the CI artifact are proposals, and the owner still gets it as a
    local file. Checks: native
    ubuntu-24.04: proof-video.
16. **`owner-play`** *[spec 9.2 done-when 2 and 5]*. On the owner's Mac a
    window opens, the dot moves with WASD or the arrows, and Esc quits.
    The owner plays and accepts. Checks: the owner. CI does not compute
    it; patch X4 lists it as owner-checked.
17. **`agents-md-length`** *[as in 1a, its done-when 6]*. Checks: lint:
    docs-check.
18. **`live-input`** *[spec 6.10 + proposal; serves done-when 2]*.
    - The default bindings (WASD, the arrows, Esc for `quit`) load from
      strict TOML.
    - An unknown action or key name fails with `[E-BINDINGS]` and the
      line.
    - The same key timeline gives the same `PlayerCommand`s on every
      target (proptest).
    - A tap shorter than a tick counts for one tick.
    - Losing focus releases every key.
    - `quit` never becomes a `PlayerCommand`.

    Checks: test x64, arm64: live-input (a named step running
    `crates/eng-input/tests/live_input.rs`, added by X7); native x5:
    tests.

Spec 9.2's five done-when items map as follows:

| Done-when | Criteria |
|---|---|
| 1 | 1, 2 |
| 2 | 2, 3, 16 and 18 |
| 3 | 4 and 5 |
| 4 | 6 |
| 5 | 15 and 16 |

Criteria 7-14 and 17 are supporting checks, not done-when items.

## This window's queue

The three queue items all belong to rung 1b. None needs a new crate, a
new third-party crate or a protected path. None touches `game-testbed-sim`
either, so the hash streams stay unchanged. They are being built now,
before G1. If the owner changes this plan, they still stand: the spec
asks for each of them (6.13, 6.10).

| Queue item | Why it is 1b | Task | Criteria | What still waits for the owner |
|---|---|---|---|---|
| `crash.json` from the panic hook in `eng-cli` (`install_panic_hook`, `crates/eng-cli/src/exit.rs`) | spec 9.2 "Probe", done-when 4; fields from 6.13 | T2 | 6 `crash-json` (done-when 4) | The `crash-json` step in the `test` job (X7) and protecting the harness `crates/game-testbed/tests/crash_json.rs` (both G5). The `play` half of criterion 6 waits for `eng-app` (T11, after G2). |
| `--version` and `--version --json` on the probe | spec 9.2 "Probe" and 6.13 ask for `--version --json`; plain `--version` is a proposal | T1 | 8 `version-json`. The stamp is also part of 6 `crash-json` and of the `gpu` report (9). | Patch X1, which makes xtask compute `version.json` (G5). Until then every value only xtask knows prints `unknown`, so 8's "no `unknown` in CI" can pass only after X1 and X6. |
| Live input actions and bindings in `eng-input` | spec 6.10; 1a plan difference 2 ("live input joins it in 1b"); needed for done-when 2, "the dot moves" | T4 | 18 `live-input`; part of 3 `esc-quits` (Esc is the default `quit` binding) and of 16 `owner-play` | The `live-input` step (X7) and protecting `crates/eng-input/tests/live_input.rs` (both G5). Real key events need `eng-platform-api`, a new crate (G2), and a window needs winit (G2-G4). |

**Key codes before the platform contract exists.** Live input needs
physical key codes, and their home is `eng-platform-api`, which waits for
G2. So in this window `eng-input` defines two plain-data types:
- `Key`: physical keys by their W3C `code` names, a closed enum;
- `KeyEvent`: a key going down or up, plus focus lost.

Tests drive these with scripted timelines. In T3 both types move into
`eng-platform-api` and `eng-input` depends on it (the `same_layer` edge
in G2). The move is mechanical: behaviour and bindings files stay the
same. Putting the types in `eng-input` for good was rejected: the
platform contract would then depend on a std crate that parses TOML.

**The protected test these items must keep green.**
`crates/game-testbed/tests/exit_codes.rs` is protected and checks more
than exit codes, so T1, T2 and T4 (and later T6) keep all of this:
- a panic still prints `[E-PANIC]` and exits 12 (T2 adds `crash.json`
  beside it);
- no arguments, and an unknown command such as `fly`, still exit 64, so
  a bare `game-testbed` does not start `play`;
- `eng_input::{read_recording, write_recording}` keep their signatures,
  and recordings keep `header.sim_protocol` and `hashes` (T4, T6);
- the `run` console summary stays at 20 lines or fewer (T6).

## Proposed time-box and AI budget (for the owner to set)

These are proposals; the owner sets the real values in `acceptance.json`
(G1).

- **Time-box:** 14 calendar days from G1, the same as 1a. 1a finished
  within its first day, so the risk for 1b is waiting on the owner gates
  and on GitHub's GPU runners (T13), not agent time. The pivot rule (spec
  7.7) compares against twice this.
- **AI budget:** the owner's current Claude plan and nothing else (spec
  14, P1 default). At most 30 agent sessions, logged in `PROGRESS.md`.
  Each open-pulsar task counts as one session, including its reviewer
  subagent. That covers 17 tasks, two rung reviews with their fixes, and
  a margin for the GPU-runner unknowns. 1a's budget was 15.
- **Owner time** (an estimate, not a budget): three sittings, as in
  "Owner gates". Roughly an hour to read and decide G1-G4, an hour to
  apply G5 and push, and an hour to judge the G6 candidates and play
  (G7).
- **CI:** the native jobs run on every push (public repo, free runners).
  The PR tier stays under 15 minutes; building wgpu in the Linux test
  jobs adds a few minutes, which `pr-duration` will show. The native
  tier's own target is in criterion 13.
- **`budgets.toml`:** `[gpu]` per-frame limits for the testbed's top-down
  view, and `[ci] native_job_minutes` (G5).

## Tasks

All *[proposal]*. Each task ends with `cargo xtask quick` green and a
`PROGRESS.md` entry; crate tasks also get a `reviewer` pass. Changes to
protected paths are prepared as patch files in `docs/rungs/1b/patches/`,
tested in a scratch clone (as 1a's guard hardening was), and applied by
the owner.

| Task | Needs | Builds | Done when |
|---|---|---|---|
| T0 | G1 | The G2 change as one tested patch: skeletons of the seven crates (crate root with the required attributes, `Cargo.toml` with `[lints] workspace = true`), their `architecture.toml` entries and the two `[profile]` lines. `arch-check` fails both on a listed crate that does not exist and on a crate that is not listed, so all of it lands in one commit | `cargo xtask quick` green in a scratch clone with the patch applied |
| T1 | none (this window) | `eng-cli`: `--version` and `--version --json` from build-time values; patch X1 (xtask computes `version.json`) | prints every field; `unknown` outside xtask builds |
| T2 | T1 | `eng-cli`: `crash.json` from the panic hook, with T1's stamp | `crates/game-testbed/tests/crash_json.rs` (it runs the binary) passes for `run`, with every field |
| T3 | G2, G3, G4, T4 | `eng-platform-api` and its conformance suite; move `Key` and `KeyEvent` in from `eng-input`; port `eng-platform-headless` to it | suite passes for headless, including a scripted close request reaching the app |
| T4 | none (this window) | `eng-input`: actions, bindings, quantization over its own `Key` and `KeyEvent` | criterion 18's tests pass |
| T5 | G2 | `eng-net-wire`: codec, the four messages, tick extension | round trips and never-panic proptests; golden vectors prepared as a patch (`tests/golden.rs` is protected); builds for RISC-V and wasm32 |
| T6 | T5 | `eng-session`; `run` and `dump-world` go through it. The recording writer (`eng-input`, std) stays in `eng-cli`; core `eng-session` hands it plain data | hash streams unchanged; missing commands repeat; the applied-command record makes a recording that `replay verify` accepts |
| T7 | G2, G3, G4 | `eng-gpu`: adapter, capability report, fixed graph, offscreen targets, readback, shader test | offscreen clear plus readback works on Metal locally and lavapipe in Docker; naga translation test passes |
| T8 | T7 | `eng-render`: top-down, tonemap and present passes; counters | a known scene reads back as expected; counters exact |
| T9 | T6, T8 | `eng-cli`: `shot` and `video`; testbed extraction | `shot` writes the PNG and the `gpu` report; `video` writes the APNG |
| T10 | G2, G3, G4, T3 | `eng-platform-winit` | conformance suite passes where a display exists |
| T11 | T2, T3, T6, T8, T9, T10 | `eng-app`: frame loop, composition, `play`; testbed enters through it | windowed hash stream equals headless; dropped ticks reported; Esc quits; `crash_json.rs` also covers `play` |
| T12 | T9 | `crates/game-testbed/tests/shots.rs`: exact pixels on the pinned configuration, FLIP elsewhere, candidates under `target/golden-candidates/` when a golden is missing | candidates from the Mac (Metal) and Docker (lavapipe) produced and compared |
| T13 | T11, T12 | Patches X2-X7: native job, summary, acceptance, selftests, workflow, test-job steps. Under xvfb there is no window manager, so the Esc step targets the window directly: `xdotool search --sync --name <title>`, then `key --window <id> Escape` | each job's steps pass locally where they can (macOS natively, Linux in Docker); the owner's first push after G5 runs them on GitHub |
| T14 | T13 | docs: `AGENTS.md` commands, `docs/ARCHITECTURE.md` (layers, the frame), ADR-0016 and ADR-0017 | `docs-check` passes; AGENTS.md at most 150 lines |
| T15 | T14 | two fresh-context `reviewer` passes over the rung, then fixes | no open blockers |
| T16 | G5 | collect golden candidates and FLIP maps from the first CI run after G5 | the owner has them for G6; shot steps report "no golden yet" until then, so `main` stays green |
| T17 | T15, T16 | the proof video (from T16's CI run) and a short scripted session for the owner | the owner accepts (G7), in the same sitting as G6 |

T1, T2 and T4 are this window's queue. After G2-G4, T3, T5 and T7 can
run in parallel, then T6 alongside T8 and T10.

## Owner gates

Every change here is to a path that `protected.toml` lists, apart from
`current.txt` (held at 1a by this run's rules) and ADR-0016 (a rule
decision). Agents prepare each change and the owner applies it, or
unlocks the path. The full text is in `PROGRESS.md`, "Proposals for the
owner"; "Prop." below is the proposal number there. The gates are
batched so that the owner sits down three times:
1. at the start (G1-G4);
2. when the patches are ready (G5), ending with a push;
3. after the first CI run on GitHub, to judge the golden candidates and
   play (G6 and G7). G6 needs that run's candidates, so it cannot share
   a sitting with G5.

| Gate | Path (`protected.toml` pattern) | Change | Prop. | Unblocks |
|---|---|---|---|---|
| G1 | this plan (not protected) | Approve it, with the acceptance criteria, the time-box and the AI budget | 1 | everything after this window |
| G1 | `docs/rungs/1b/acceptance.json` (`docs/rungs/*/acceptance.json`) | Write it from "Proposed acceptance criteria", in 1a's format | 1 | CI's acceptance summary |
| G1 | `docs/rungs/current.txt` (not protected; this run keeps it at 1a) | Set it to `1b` | 10 | hooks and agents see 1b |
| G2 | `architecture.toml` | Add the 7 new crates, and the `same_layer` edges for `eng-platform-headless` and `eng-input`. This lands in one commit with T0's skeleton crates and the `[profile]` lines below, because `arch-check` fails on a listed crate that does not exist as much as on an unlisted one | 2 | T3, T5-T11 |
| G2 | root `Cargo.toml` (`profile` table) | `overflow-checks = true` in release for `eng-net-wire` and `eng-session` (same commit) | 3 | T5, T6 (`arch-check` fails without it) |
| G3 | `deny.toml` | Crate batch 2 (the section below; session 1's working list is `PROGRESS.md` proposal 4); `[graph] targets` without wasm32; the `raw-window-handle` wrapper ban | 4 | T3, T7, T9, T10, T12 |
| G3 | `_typos.toml` | The deflate crate's name under `[default.extend-identifiers]`, in the same change as `deny.toml` | 4 | keeps `lint` green |
| G4 | ADR-0016 (a rule decision, not a protected path) | Allow the two `raw-window-handle` traits in public APIs inside the seam | 5 | T3, T7, T10 |
| G5 | `xtask/**` | X1 version stamps; X2 `ci --job native`; X3 summary across native jobs and `native-duration`; X4 owner-checked features; X5 the five selftest cases; X7 new named steps in `ci --job test` (`crash-json`, `shaders`, `wire-session`, `live-input`) | 7, 11 | criteria 1-13, 15-18 |
| G5 | `.github/workflows/**` | X6: the five native jobs in `ci.yml`; Mesa 25.2.7 lavapipe and WARP pinned; xvfb and xdotool on Linux; `rustup target add x86_64-apple-darwin` in the macOS job only, as the RISC-V job already does for its target; artifacts (shots, golden candidates, FLIP maps, `crash.json`, `proof.apng`). Actions pinned by SHA, `permissions: {}` | 7, 11 | the native jobs exist |
| G5, as needed | `.config/**` | `.config/nextest.toml`: a test group that runs the GPU and window tests one at a time, and a longer slow-timeout for them, if T7, T11 or T13 shows they need it | 11 | stable native jobs |
| G5 | `budgets.toml` | `[gpu]` per-frame limits; `[ci] native_job_minutes = 25` | 6 | criteria 9, 13 |
| G5 | `protected.toml` and `.github/CODEOWNERS` | Protect the new harnesses: `shots.rs`, `crash_json.rs`, the shader test, the windowed-run and Esc tests, `crates/eng-input/tests/live_input.rs`, and the `eng-net-wire` never-panic and `eng-session` hash-equality tests | 8, 11 | criteria 2-6, 10, 11 and 18 cannot be weakened |
| G5 | `crates/*/tests/golden.rs` | `eng-net-wire`'s golden vectors (patch from T5) | 9 | criterion 11 |
| G5 | `.claude/**` | The `golden-update` skill and the advisory `visual-qa` subagent (spec 7.5) | 7 | T16 |
| G5, as needed | `**/proptest-regressions/**` | Any regression file that a failing proptest writes in T4-T6 reaches the owner as a candidate, with the fix | 11 | keeps the failing case |
| G6 | `**/golden/**` | The lavapipe golden images in `crates/game-testbed/golden/` and `golden/thresholds.toml` (proposed: mean FLIP at most 0.01, 99th percentile at most 0.2) | 9 | criteria 4, 5 |
| G7 | none | Play from a short scripted session and watch the proof video; accept or send back | none | criterion 16, rung done |

**Protected paths 1b leaves alone**, so the list above is complete:
- `crates/eng-math/tests/known_answers.*` (no math change).
- The existing `crates/eng-core/tests/golden.rs` and
  `crates/eng-schema-core/tests/golden.rs`. `PlayerCommand` and the
  canonical encoding do not change.
- `crates/game-testbed/tests/exit_codes.rs`. There is no new exit code
  (decision 4), and exit 12 is already covered; `crash.json` gets its own
  harness. The test checks more than exit codes, and "This window's
  queue" lists what 1b must keep.
- `crates/game-testbed/scenarios/scenarios.toml` and `**/*.assert.toml`.
  The windowed run reuses `dot-square`.
- `**/snapshots/**` (no insta tests planned).
- `clippy.toml` and `.clippy.toml`; `.deny.toml` (the rules stay in
  `deny.toml`); `typos.toml` and `.typos.toml` (the change goes in
  `_typos.toml`); `.cargo/**`.
- `rust-toolchain.toml` and `rust-toolchain`. 1.99.1 builds wgpu 30.0.1
  and winit 0.30.13, checked in a scratch project. The Intel-Mac target
  is installed by X6 in the macOS job only, so other machines do not
  download it.
- The root `Cargo.toml` tables `workspace.lints`, `patch` and `replace`
  (no patched crates).
- `.github/workflows/oracle.yml`.

If a task finds that it needs one of these after all, it stops and adds a
proposal to `PROGRESS.md`.

## Crate batch 2

*[proposal]* The third-party crates that G3 adds to `[bans] allow` in
`deny.toml`: 136 names and 146 versions. They are the names of proposal 4
with amendment 12 in `PROGRESS.md`, no more and no fewer. The owner
applies the list from this table.

- **Source.** A scratch project outside the repo, resolved on 2026-10-25
  with toolchain 1.99.1. It holds `wgpu =30.0.1` and `winit =0.30.13`
  with the features of decision 5, `png =0.18.1` and `nv-flip =0.1.2`.
  The repo's `Cargo.toml` and `Cargo.lock` are unchanged. T7, T9, T10
  and T12 add these crates to the repo; each recomputes the list against
  the real lock and reports any difference to the owner as an amendment.
- **Method.**
  - `cargo tree --target <t> -e normal,build` for each of proposal 4's
    seven targets.
  - `cargo metadata` for the licences.
  - `cargo deny check bans licenses` (0.20.2) on a copy of the policy
    with proposal 4's `[graph] targets`. Run on one target at a time,
    cargo-deny lists exactly the crates that `cargo tree` builds there.
- **One change.** The list is complete only together with the
  `[graph] targets` line and the `_typos.toml` entry (proposal 4).
  Without `targets`, as `deny.toml` is today, cargo-deny sees 193 names,
  among them Android, web, Redox and `tokio` crates.
- **Columns.**
  - **Version:** as resolved today. `[bans] allow` takes names, as batch
    1 does, so a crate with two versions needs one entry.
  - **Licence:** the crate's `license` field as published. `/` is the old
    way of writing OR.
  - **Targets:** where cargo builds the crate.
    - "all seven" means macOS (arm64, x64), Linux (x64, arm64, RISC-V)
      and Windows (x64, arm64). A family named without an architecture
      means all of its architectures.
    - "cargo-deny graph only" means no target builds the crate, but
      `cargo deny check` still fails unless it is allowed. cargo-deny
      tests each platform-specific dependency against every listed
      target, whichever target builds the parent. For example, `polling`
      and `rustix` are built only on Linux, but their Windows-only
      dependencies count. `windows-targets` reaches `windows_x86_64_gnu`
      because the `cfg` on that edge (x86-64 and `target_env = "gnu"`)
      also matches x86-64 Linux.
  - **Licence not on the list:** any SPDX ID in the licence that is not
    on `[licenses] allow`.
- **Licence flags: one, and it needs no change.**
  - `adler2` offers 0BSD, MIT or Apache-2.0. 0BSD is not on the list,
    but MIT is, so the crate passes.
  - Two crates need both parts of an AND, and every part is on the list:
    `dpi` (Apache-2.0 AND MIT) and `nv-flip-sys` (BSD-3-Clause AND one of
    MIT, Apache-2.0 or Zlib).
  - Every crate has an SPDX `license` field. cargo-deny reports no
    licence error, so no `exceptions` or `clarify` entry is needed.
- **Counts by target** (versions):
  - all seven: 55;
  - Linux: 37;
  - macOS: 23;
  - Windows: 17 (one of them x64 only and one arm64 only);
  - Linux and Windows (Vulkan): 8;
  - cargo-deny graph only: 6.
- **Where the crates come from.**
  - `wgpu` and `winit` bring 127 of the names.
  - `png` (`eng-cli`'s `shot` and `video`, and the golden tests) brings
    `adler2`, `crc32fast`, `fdeflate`, the deflate crate (shown in the
    table as a link, for typos), `miniz_oxide` and `simd-adler32`.
  - `nv-flip` (tests only) brings `nv-flip-sys`, which compiles C++
    through `cc`. `cc`, `find-msvc-tools` and `shlex` are on Linux anyway
    for `wayland-backend`; `nv-flip-sys` adds them to every target.
- **Duplicate versions.** `multiple-versions = "warn"`, so these are
  warnings, not errors.
  - macOS: `block2`, `objc2` and `objc2-foundation` (winit 0.30 uses
    objc2 0.5, `wgpu-hal` uses 0.6).
  - Linux: `linux-raw-sys`, `rustix`, `thiserror` and `thiserror-impl`.
  - Every target: `miniz_oxide` (`png` 0.18.1 uses 0.8, the deflate crate
    0.9).
  - cargo-deny's graph: three versions of `windows-sys`.
  - Batch-1 crates are duplicated too: `bitflags`, `hashbrown` and `syn`.
- **Locked but never checked.** The deflate crate's default
  `runtime_detection` feature names `zlib-rs?/std`, so cargo writes
  `zlib-rs 0.6.8` (Zlib) into `Cargo.lock` without building it.
  cargo-deny leaves it out of the graph, so it needs no entry. That makes
  the old comment above `allow` ("only these crates may appear in
  Cargo.lock") untrue. Proposal 4's new wording, "in the dependency graph
  of these targets", is the accurate one.

| Crate | Version | Licence | Targets | Licence not on the list |
|---|---|---|---|---|
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | all seven | **0BSD**: not listed, but an OR choice; passes on MIT or Apache-2.0 |
| `ahash` | 0.8.12 | MIT OR Apache-2.0 | Linux | no |
| `allocator-api2` | 0.2.21 | MIT OR Apache-2.0 | Linux, Windows | no |
| `arrayvec` | 0.7.8 | MIT OR Apache-2.0 | all seven | no |
| `as-raw-xcb-connection` | 1.0.1 | MIT OR Apache-2.0 | Linux | no |
| `ash` | 0.38.0+1.3.281 | MIT OR Apache-2.0 | Linux, Windows | no |
| `bit-set` | 0.10.0 | Apache-2.0 OR MIT | all seven | no |
| `bit-vec` | 0.9.1 | Apache-2.0 OR MIT | all seven | no |
| `block2` | 0.5.1 | MIT | macOS | no |
| `block2` | 0.6.2 | MIT | macOS | no |
| `bytemuck` | 1.25.2 | Zlib OR Apache-2.0 OR MIT | all seven | no |
| `bytemuck_derive` | 1.12.1 | Zlib OR Apache-2.0 OR MIT | all seven | no |
| `calloop` | 0.13.0 | MIT | Linux | no |
| `calloop-wayland-source` | 0.3.0 | MIT | Linux | no |
| `cc` | 1.10.0 | MIT OR Apache-2.0 | all seven | no |
| `cfg_aliases` | 0.2.2 | MIT | all seven | no |
| `codespan-reporting` | 0.13.1 | Apache-2.0 | all seven | no |
| `concurrent-queue` | 2.5.0 | Apache-2.0 OR MIT | cargo-deny graph only | no |
| `core-foundation` | 0.9.4 | MIT OR Apache-2.0 | macOS | no |
| `core-foundation-sys` | 0.8.7 | MIT OR Apache-2.0 | macOS | no |
| `core-graphics` | 0.23.2 | MIT OR Apache-2.0 | macOS | no |
| `core-graphics-types` | 0.1.3 | MIT OR Apache-2.0 | macOS | no |
| `crc32fast` | 1.5.3 | MIT OR Apache-2.0 | all seven | no |
| `crossbeam-utils` | 0.8.23 | MIT OR Apache-2.0 | cargo-deny graph only | no |
| `cursor-icon` | 1.2.0 | MIT OR Apache-2.0 OR Zlib | all seven | no |
| `dispatch` | 0.2.0 | MIT | macOS | no |
| `dlib` | 0.5.3 | MIT | Linux | no |
| `document-features` | 0.2.12 | MIT OR Apache-2.0 | all seven | no |
| `downcast-rs` | 1.2.1 | MIT/Apache-2.0 | Linux | no |
| `dpi` | 0.1.2 | Apache-2.0 AND MIT | all seven | no |
| `errno` | 0.3.14 | MIT OR Apache-2.0 | cargo-deny graph only | no |
| `fdeflate` | 0.3.7 | MIT OR Apache-2.0 | all seven | no |
| `find-msvc-tools` | 0.1.15 | MIT OR Apache-2.0 | all seven | no |
| <https://crates.io/crates/flate2> | 1.1.10 | MIT OR Apache-2.0 | all seven | no |
| `foldhash` | 0.2.0 | Zlib | all seven | no |
| `foreign-types` | 0.5.0 | MIT/Apache-2.0 | macOS | no |
| `foreign-types-macros` | 0.2.4 | MIT/Apache-2.0 | macOS | no |
| `foreign-types-shared` | 0.3.1 | MIT/Apache-2.0 | macOS | no |
| `gethostname` | 1.1.0 | Apache-2.0 | Linux | no |
| `gpu-allocator` | 0.28.0 | MIT OR Apache-2.0 | Linux, Windows | no |
| `half` | 2.7.1 | MIT OR Apache-2.0 | all seven | no |
| `libloading` | 0.8.9 | ISC | all seven | no |
| `linux-raw-sys` | 0.4.15 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Linux | no |
| `linux-raw-sys` | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Linux | no |
| `litrs` | 1.0.1 | MIT OR Apache-2.0 | all seven | no |
| `lock_api` | 0.4.14 | MIT OR Apache-2.0 | all seven | no |
| `log` | 0.4.34 | MIT OR Apache-2.0 | all seven | no |
| `memmap2` | 0.9.12 | MIT OR Apache-2.0 | Linux | no |
| `miniz_oxide` | 0.8.9 | MIT OR Zlib OR Apache-2.0 | all seven | no |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 | all seven | no |
| `naga` | 30.0.1 | MIT OR Apache-2.0 | all seven | no |
| `naga-types` | 30.0.1 | MIT OR Apache-2.0 | all seven | no |
| `nv-flip` | 0.1.2 | MIT OR Apache-2.0 OR Zlib | all seven | no |
| `nv-flip-sys` | 0.1.1 | (MIT OR Apache-2.0 OR Zlib) AND BSD-3-Clause | all seven | no |
| `objc-sys` | 0.3.5 | MIT | macOS | no |
| `objc2` | 0.5.3 | MIT | macOS | no |
| `objc2` | 0.6.5 | MIT | macOS | no |
| `objc2-app-kit` | 0.2.2 | MIT | macOS | no |
| `objc2-core-foundation` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | macOS | no |
| `objc2-core-graphics` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | macOS | no |
| `objc2-encode` | 4.1.0 | MIT | macOS | no |
| `objc2-foundation` | 0.2.2 | MIT | macOS | no |
| `objc2-foundation` | 0.3.2 | MIT | macOS | no |
| `objc2-metal` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | macOS | no |
| `objc2-quartz-core` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | macOS | no |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 | all seven | no |
| `ordered-float` | 5.5.0 | MIT | Linux, Windows | no |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 | all seven | no |
| `parking_lot_core` | 0.9.12 | MIT OR Apache-2.0 | all seven | no |
| `percent-encoding` | 2.3.2 | MIT OR Apache-2.0 | Linux | no |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT | all seven | no |
| `pkg-config` | 0.3.34 | MIT OR Apache-2.0 | Linux | no |
| `png` | 0.18.1 | MIT OR Apache-2.0 | all seven | no |
| `polling` | 3.11.0 | Apache-2.0 OR MIT | Linux | no |
| `presser` | 0.3.1 | MIT OR Apache-2.0 | Linux, Windows | no |
| `profiling` | 1.0.18 | MIT OR Apache-2.0 | all seven | no |
| `quick-xml` | 0.41.0 | MIT | Linux | no |
| `range-alloc` | 0.1.5 | MIT OR Apache-2.0 | Windows | no |
| `raw-window-handle` | 0.6.2 | MIT OR Apache-2.0 OR Zlib | all seven | no |
| `raw-window-metal` | 1.1.0 | MIT OR Apache-2.0 | macOS | no |
| `renderdoc-sys` | 1.1.0 | MIT OR Apache-2.0 | Linux, Windows | no |
| `rustc-hash` | 1.1.0 | Apache-2.0/MIT | all seven | no |
| `rustix` | 0.38.44 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Linux | no |
| `rustix` | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Linux | no |
| `scoped-tls` | 1.0.1 | MIT/Apache-2.0 | Linux | no |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 | all seven | no |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 | all seven | no |
| `simd-adler32` | 0.3.10 | MIT | all seven | no |
| `slab` | 0.4.12 | MIT | Linux | no |
| `smallvec` | 1.16.4 | MIT OR Apache-2.0 | all seven | no |
| `smithay-client-toolkit` | 0.19.2 | MIT | Linux | no |
| `smol_str` | 0.2.2 | MIT OR Apache-2.0 | all seven | no |
| `spirv` | 0.4.0+sdk-1.4.341.0 | Apache-2.0 | Linux, Windows | no |
| `static_assertions` | 1.1.0 | MIT OR Apache-2.0 | all seven | no |
| `thiserror` | 1.0.69 | MIT OR Apache-2.0 | Linux | no |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | all seven | no |
| `thiserror-impl` | 1.0.69 | MIT OR Apache-2.0 | Linux | no |
| `thiserror-impl` | 2.0.21 | MIT OR Apache-2.0 | all seven | no |
| `tracing` | 0.1.44 | MIT | all seven | no |
| `tracing-core` | 0.1.36 | MIT | all seven | no |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 | Windows | no |
| `unicode-width` | 0.2.2 | MIT OR Apache-2.0 | all seven | no |
| `version_check` | 0.9.5 | MIT/Apache-2.0 | Linux | no |
| `wayland-backend` | 0.3.17 | MIT | Linux | no |
| `wayland-client` | 0.31.15 | MIT | Linux | no |
| `wayland-csd-frame` | 0.3.0 | MIT | Linux | no |
| `wayland-cursor` | 0.31.14 | MIT | Linux | no |
| `wayland-protocols` | 0.32.13 | MIT | Linux | no |
| `wayland-protocols-plasma` | 0.3.12 | MIT | Linux | no |
| `wayland-protocols-wlr` | 0.3.12 | MIT | Linux | no |
| `wayland-scanner` | 0.31.11 | MIT | Linux | no |
| `wayland-sys` | 0.31.11 | MIT | Linux | no |
| `wgpu` | 30.0.1 | MIT OR Apache-2.0 | all seven | no |
| `wgpu-core` | 30.0.1 | MIT OR Apache-2.0 | all seven | no |
| `wgpu-core-deps-apple` | 30.0.1 | MIT OR Apache-2.0 | macOS | no |
| `wgpu-core-deps-windows-linux-android` | 30.0.1 | MIT OR Apache-2.0 | Linux, Windows | no |
| `wgpu-hal` | 30.0.1 | MIT OR Apache-2.0 | all seven | no |
| `wgpu-naga-bridge` | 30.0.1 | MIT OR Apache-2.0 | all seven | no |
| `wgpu-types` | 30.0.1 | MIT OR Apache-2.0 | all seven | no |
| `windows` | 0.62.2 | MIT OR Apache-2.0 | Windows | no |
| `windows-collections` | 0.3.2 | MIT OR Apache-2.0 | Windows | no |
| `windows-core` | 0.62.2 | MIT OR Apache-2.0 | Windows | no |
| `windows-future` | 0.3.2 | MIT OR Apache-2.0 | Windows | no |
| `windows-implement` | 0.60.2 | MIT OR Apache-2.0 | Windows | no |
| `windows-interface` | 0.59.3 | MIT OR Apache-2.0 | Windows | no |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 | Windows | no |
| `windows-numerics` | 0.3.1 | MIT OR Apache-2.0 | Windows | no |
| `windows-result` | 0.4.1 | MIT OR Apache-2.0 | Windows | no |
| `windows-strings` | 0.5.1 | MIT OR Apache-2.0 | Windows | no |
| `windows-sys` | 0.52.0 | MIT OR Apache-2.0 | Windows | no |
| `windows-sys` | 0.59.0 | MIT OR Apache-2.0 | cargo-deny graph only | no |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 | cargo-deny graph only | no |
| `windows-targets` | 0.52.6 | MIT OR Apache-2.0 | Windows | no |
| `windows-threading` | 0.2.1 | MIT OR Apache-2.0 | Windows | no |
| `windows_aarch64_msvc` | 0.52.6 | MIT OR Apache-2.0 | Windows arm64 | no |
| `windows_x86_64_gnu` | 0.52.6 | MIT OR Apache-2.0 | cargo-deny graph only | no |
| `windows_x86_64_msvc` | 0.52.6 | MIT OR Apache-2.0 | Windows x64 | no |
| `winit` | 0.30.13 | Apache-2.0 | all seven | no |
| `x11-dl` | 2.21.0 | MIT | Linux | no |
| `x11rb` | 0.13.2 | MIT OR Apache-2.0 | Linux | no |
| `x11rb-protocol` | 0.13.2 | MIT OR Apache-2.0 | Linux | no |
| `xcursor` | 0.3.11 | MIT | Linux | no |
| `xkbcommon-dl` | 0.4.2 | MIT | Linux | no |
| `xkeysym` | 0.2.1 | MIT OR Apache-2.0 OR Zlib | Linux | no |
| `zerocopy` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | all seven | no |
| `zerocopy-derive` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | all seven | no |

## Risks and unknowns

| Risk | What we do |
|---|---|
| winit needs the main thread on macOS, and libtest runs tests on other threads | windowed tests run the binary as a child process; unit tests use the headless twin |
| Mesa 25.2.7 from gfx-rs/ci-build may have no arm64 build | Linux arm64 uses FLIP anyway; it then takes the distro's lavapipe, recorded in the report |
| WARP on `windows-11-arm`, and the paravirtual Metal device on `macos-26`, are unconfirmed (8.3) | T13 confirms both first; if one is missing, that runner renders nothing and the owner decides (skip with a report, or another adapter) |
| Building wgpu and winit on five runners slows CI | rust-cache per runner; measured by `native-duration`; sccache if it is over |
| The guard may refuse the download of the pinned Mesa for local Docker runs | the exact golden is authoritative only in CI; locally, FLIP on Metal and Docker with the distro's lavapipe |
| `nv-flip-sys` compiles C++ at build time (test-only) | GitHub's images have compilers; if it is a problem, FLIP is ported to Rust in `eng-gpu`'s tests |
| Agents cannot write goldens, `golden.rs` or the acceptance file | candidates and patches go to the owner in batches (G5, G6) |
| Between G5 and G6 the shot steps have no golden | they report "no golden yet" and upload the candidate without failing; only acceptance waits |
| The queue items land before G1, and the owner may change the plan | they are spec items (6.13, 6.10) in existing crates; a change costs a small rework, not a revert |
| `Key` and `KeyEvent` move from `eng-input` to `eng-platform-api` in T3 | a mechanical move in one task; bindings files and tests do not change |
