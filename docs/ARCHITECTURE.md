# Architecture

How the engine fits together, as built so far in rung 1b. The plan and its
reasons are in `docs/superpowers/specs/2026-10-20-ai-built-engine-design.md`;
decisions in `docs/adr/`. Keep this file short and true: update it in the
commit that changes what it describes.

## Layers

```
L0  eng-schema-core ▶ eng-derive;  eng-math, eng-core ▶ eng-schema-core   (core set)
L1  eng-ecs  eng-net-transport  eng-net-wire                                (core set)
    eng-input ▶ eng-schema-json, eng-platform-api                           (std)
    eng-platform-headless ▶ eng-platform-api                                (std)
    eng-platform-winit ▶ eng-platform-api, eng-platform-gcmouse*            (std)
L2  eng-sim  eng-collision-api;  kit-fps ▶ eng-collision-api                (core set)
    eng-collision-grid* ▶ eng-collision-api                                 (core set)
    game-testbed-sim ▶ eng-sim, eng-collision-api, kit-fps,                 (core set)
                       eng-collision-grid*
    eng-session ▶ eng-sim                                                   (core set)
L3  eng-render ▶ eng-gpu                                                    (std)
L4  eng-app ▶ eng-cli                                                       (std)
game  game-testbed (binary) ▶ game-testbed-client*
tool  xtask  eng-bench*
```

\* Rung 2 crates, empty skeletons until their tasks in
`docs/rungs/2/plan.md` (T5, T12, T14, T17).

A crate depends only on lower layers, or on the same-layer crates its
`same_layer` lists in `architecture.toml`, drawn above as `A ▶ B` (A
depends on B). Rung 1b added seven crates; their edges to our others
(third-party crates in brackets):

| Crate | Layer | Depends on |
|---|---|---|
| `eng-platform-api` | L1 | none [raw-window-handle, for the window seam: ADR-0016] |
| `eng-platform-winit` | L1 | eng-platform-api [winit, raw-window-handle] |
| `eng-net-wire` | L1, core | eng-core |
| `eng-session` | L2, core | eng-sim, eng-ecs, eng-net-wire, eng-net-transport, eng-core |
| `eng-gpu` | L3 | none [wgpu, raw-window-handle] |
| `eng-render` | L3 | eng-gpu, eng-math |
| `eng-app` | L4 | eng-cli, eng-platform-winit, eng-platform-api, eng-input, eng-gpu, eng-render |

eng-cli, the probe, uses four of them (eng-session, eng-gpu, eng-render
and eng-platform-api) and the headless platform; game-testbed enters
through eng-app.

The **core set** is everything the simulation links, plus the network
codec, transport and session loops: `#![no_std]`,
`#![forbid(unsafe_code)]`, only `libm` as a third-party dependency (plus
`eng-derive`'s build-time `syn`/`quote`), overflow checks in release.
`cargo xtask arch-check` enforces all of it over the transitive
dependency closure. It also enforces the floor rule, which names the one
crate that may depend on each of a few crates: only eng-gpu may depend on
wgpu; only eng-platform-winit on winit and on the GCMouse shim,
eng-platform-gcmouse; and only the shim on objc2-game-controller.
eng-math gives libm `force-soft-floats`, so libm's `arch` code stays off
even in builds where wgpu's shader compiler turns the feature on. The core
set also builds for `wasm32-unknown-unknown` and is tested on RISC-V under
QEMU.

L3 starts in rung 1b with GPU and render; audio and UI come later.

## Data flow of one tick

```
input script (TOML) ──eng-input──▶ PlayerCommand per player, per tick
                                        │
                                        ▼
                     eng-sim::Sim::step(&[PlayerCommand])
   SimTick += 1, TickInputs = commands, events cleared
   for stage in Input, PreUpdate, Update, PostUpdate, Commit:
       for system in the resolved order: system(&mut World)
       (debug: NaN check of every float after each system)
       apply deferred structural changes (sync point)
                                        │
                                        ▼
            World (eng-ecs): entities, components, resources, events
                 │                          │                      │
       visit_state (canonical)        visit_dump (JSON)     visit_events
                 │                          │                      │
     FNV-1a-64 state hash,         dump-world, assertion   events.jsonl
     snapshots, recordings         pointers, NaN paths
```

- **One schema walk, many uses.** Every component, resource and event
  implements `Schema` (derived, with a hand-written stable name and version).
  Encoding, hashing, JSON dumps and the NaN check are all `Visitor`s over the
  same walk, so they cannot disagree.
- **Canonical form.** The world encodes as: entity allocator, then
  resources, components and events, each in stable-name order; components in
  dense storage order (which is part of the state, because iteration order
  affects results). Little-endian, fixed width, `u32` lengths.
- **Protocol version.** A hash of every registered schema, the resolved
  system order and the tick rate. Hash streams and recordings carry it, so
  an unannounced schema or ordering change shows up at once.
- **Randomness.** `GameplayRng` (PCG32, stream "gameplay") is a resource,
  so it is part of the state. Cosmetic randomness never enters the simulation.

## The platform

`eng-platform-api` is the platform contract (spec 6.1, 6.10):
`eng-platform-headless` and `eng-platform-winit` implement it. The
platform owns the event loop and calls the app back, as winit does; the
crate docs list the rules.

```
Platform::run(&mut app):  start ─▶ window (windowed only)
                          ─▶ { events due ─▶ frame, which answers Wake } ... ─▶ stop
```

- Events (`PlatformEvent`) are plain data: physical keys (`Key`, by W3C
  `code`), raw mouse motion, mouse buttons, the wheel, gamepad buttons and
  axes, focus, resize, occlusion and close requests. Mouse and gamepad
  events exist already, so rungs 2 and 6 need no contract change.
- The clock is `Control::now()`: an `Instant`, nanoseconds since the run
  started, never going back. Each `frame` answers `Wake::At(t)` or
  `Wake::OnEvent`, so ticks come from the timer, not from presenting
  (spec 6.6); an event's batch wakes the app sooner.
- `Control::exit()` ends the run: only `stop` follows, and the rest of
  the batch is dropped. A close request is an event like any other: the
  platform keeps running until the app exits.
- The window seam (ADR-0016) is `window::Window`, the one part that names
  `raw-window-handle`'s traits. A windowed platform hands an
  `Arc<dyn Window>` to `App::window` once, after `start`; wgpu's safe
  surface creation takes it as it is. Nothing a platform must implement
  names it, so the headless platform stays outside the seam.
- `HeadlessPlatform` has a virtual clock that jumps to each wake, so a run
  takes no real time and repeats exactly. A script of
  `(Instant, PlatformEvent)` plays the user. A run whose app waits for an
  event when none is left fails with `Idle`, one past its time limit with
  `TimeLimit`, and one whose app keeps asking to be woken at once with
  `Stalled` (after 10,000 frames at one instant), instead of hanging.
  `run`, `dump-world`, `shot` and `video` pace their ticks on it, one per
  frame (`crates/eng-cli/src/ticker.rs`).
- `WinitPlatform` (the only crate that imports winit) opens one window
  per run, hidden until `start` and `window` have run, and hands it over
  as the seam's `Arc<dyn Window>`. winit makes its event loop once per
  process, so the platform owns it and each run is winit's
  `run_app_on_demand`; it must be made and run on the main thread. A
  frame comes from winit's `about_to_wait` only when it is due, so the
  OS events the contract leaves out and early timers never bring one
  early. Keys are physical (W3C codes); key repeat and winit's synthetic
  keys at focus changes are dropped. Rung 1b maps keys and window events
  only; the mouse comes with rung 2's raw-input shim. Without a display
  `new` is `Unavailable`. On macOS, Quit (Cmd+Q, the Dock) ends the
  process right after `stop`, so `run` never returns there and an app
  saves what it must in `stop`.
- The conformance suite is `conformance::check` (any platform: order, the
  clock, timed wakes, exit, running again) and
  `conformance::check_scripted` (event delivery, exact times, batches,
  close requests). Its probes give up after a bounded number of frames,
  so it never hangs. Headless passes both, and deliberately broken
  platforms, at least one per rule, fail on the rule they break. winit
  passes `check` with real windows in
  `crates/eng-platform-winit/tests/conformance.rs` (`harness = false`:
  the main thread), which adds real window handles, a clock that restarts
  each run, and more timed frames than a window's opening brings. Without
  a display (CI's test job) it skips, unless `ENG_REQUIRE_DISPLAY=1`
  makes it fail, as in CI's native jobs; a watchdog fails it after a
  minute.

## Live input

`eng-input` also quantizes the keyboard into one `PlayerCommand` per tick
(spec 6.10). `eng-app`'s frame loop feeds it the window's keys and samples
it once per tick ("The frame", below).

```
PlatformEvent ──key_event()──▶ KeyEvent (Down/Up of a Key, FocusLost)
          ──LiveInput::handle──▶ held keys + taps
          Bindings (strict TOML) ──LiveInput::sample(tick)──▶ TickInput { command, actions }
```

- `Key` names physical keys by their W3C `code` (`KeyW`, `ArrowUp`,
  `Escape`), so bindings do not depend on the layout. `Key` and `KeyEvent`
  are the platform contract's plain data; `eng-input` re-exports them.
- The caller passes the tick to `sample`; `eng-input` reads no clock.
  `quantize` does the same for a whole timeline of `(tick, event)`.
- A key counts for a tick if it is held when the tick is sampled or went
  down since the previous sample, so a tap shorter than a tick counts once.
  A `Down` for a key already held (OS key repeat) changes nothing.
  `FocusLost` releases every key. Opposite actions cancel; movement is
  full speed (127).
- Each command carries the view angles the caller holds with `set_view`.
  Movement goes along the yaw, so `play` holds the game's
  `GameDef::view_yaw`: 0x4000 (+Y, up the top-down view) for the testbed,
  where W then moves the dot up the window and D to its right.
- Mouse look (rung 2; `LiveInput::set_look`, off by default) turns those
  angles by raw motion (`handle_motion`), in `src/look.rs`. Each delta is
  rounded to a 1/256 count and summed exactly in `i64`; sensitivity is an
  integer (millionths of a degree per count, default 0.066°), so the turn
  is one rounded integer division and any split of the motion into ticks
  gives the same yaw. `dx > 0` lowers yaw and `dy > 0` (towards the user)
  lowers pitch unless inverted. Pitch holds at ±80° (14563 units), and
  motion past the limit is dropped at each sample, so only there do tick
  boundaries matter. `FocusLost` drops motion not yet sampled.
- App actions (`quit`) are reported in `TickInput::actions` and never
  change the command.
- Bindings files are `version = 1` plus `[actions] move_forward = ["KeyW",
  ...]`. The defaults (WASD, the arrows, Escape for `quit`) are
  `crates/eng-input/src/default.bindings.toml`. A file replaces them; an
  action it leaves out is unbound. Unknown action or key names, a value
  that is not a list of names, an unknown top-level key and a key bound
  twice fail with `[E-BINDINGS] <file>:<line>: ...`; only a file's first
  error is reported.

## The wire format

`eng-net-wire` turns session messages into bytes and back (spec 6.8).
`eng-session` calls it, in-process over Memory too, so there is one code
path. The crate docs list the five messages and their layouts.

- A message is its kind (`u8`), the low 32 bits of a server tick (`u32`)
  and a body; little-endian, fixed width.
- The receiver extends a 32-bit tick against its baseline, its last
  acknowledged tick, to the one tick within 2^31 of it (`extend_tick`). A
  new client has no baseline, so `Welcome` and `Denied` carry the high 32
  bits too; the `Welcome` tick is the baseline until the first ack.
- `Welcome` also carries the session's player count and seed: a sim
  restores a snapshot only from a session with the same constants.
- Commands use eng-core's canonical `PlayerCommand` layout, the one
  recordings use: up to five per message, oldest first.
- A snapshot's state is opaque to the codec, at most 65,535 bytes.
  `eng-session` sends `Sim::snapshot()`: the protocol version, then the
  canonical state.
- A `Hello` from another wire version is read only up to its version and
  answered with `Denied`. The two become frozen in rung 5, whose handshake
  fixes their final form; until then no two builds exchange messages.
- Decoders never panic, cap every count and length, and accept only
  canonical bytes (`tests/never_panic.rs`).

## The session

`eng-session` runs a server and its clients over a `Transport` (spec 6.6,
6.8), every message encoded with `eng-net-wire`. Single-player is the same
server in-process over Memory: `run`, `dump-world`, `shot`, `video` and
`play` drive a server and player 0's client, all through one per-tick step
(`Session::tick` in eng-cli). The crate docs draw the
exchange: `Hello`, then `Welcome` and the current state (or `Denied`),
then `Commands` up and a `Snapshot` down every tick.

- The server owns the authoritative `Sim`. At each tick a slot with a
  client gets that client's command for the tick, else its last command
  restamped. A slot without a client, never joined or gone, idles, keeping
  its view angles. Commands may arrive up to 64 ticks early (`MAX_LEAD`);
  the first copy of a tick's command is kept.
- `Server::step` returns the commands it applied, one per slot: the session
  recording. `Server::applied` keeps the last tick's, also when its
  snapshot failed. The probe writes them to `--record`'s file, and `replay
  verify` steps the sim directly from them.
- The server keeps each client's newest acknowledged snapshot tick, its
  baseline (contract 5), which also extends the ticks in that client's next
  message. The client extends against its newest snapshot. Both start from
  the `Welcome` tick.
- A per-client relevance hook answers `Everything`, since rung 1b sends
  whole states. Deltas, time sync and prediction come with loss (rung 5).
- A snapshot message must fit one 1,200-byte datagram: 466 bytes for the
  dot. A larger one fails with `[E-SNAPSHOT-SIZE]`, and `run` and
  `dump-world` stop there with exit 13, as for a budget; the testbed
  reaches it at 10 players. Fragmentation arrives in rung 5.
- The probe reads the client's sim, which restores every snapshot, so the
  committed hash streams check the whole round trip. It stops with
  `[E-DESYNC]` (exit 11) if the client lacks the tick just stepped, and
  with `[E-STEP]` (exit 64) if a command is not for that tick.
- The session registers nothing in the sim and reads no clock. It drops
  bytes a peer should not have sent instead of failing; the one error from
  the network is a snapshot the client's sim refuses.

## The GPU layer

`eng-gpu` (spec 6.7) is the only crate that imports wgpu, and no wgpu type
appears in its API. `eng-render` builds rung 1b's passes on it (next
section). GPU work never feeds back into the simulation. The crate docs
have the details.

```
Gpu::new(GpuOptions) ──adapter──▶ a Tier 0 device, and its GpuReport
Graph::fixed(Offscreen):
  draw ──▶ scene (Rgba16Float, linear) ──tonemap──▶ output (Rgba8UnormSrgb)
                                    output ──Targets::read_output──▶ Image (RGBA8)
Graph::fixed(Surface) adds present: output ──▶ surface (Bgra8UnormSrgb)
Graph::sorted_without(PassNodes, off) ──order, bypass, cull, pool──▶ Graph
Graph::render ──▶ FrameCounters (draws, switches, uploaded bytes)
```

- **Tier 0.** The device asks for WebGPU's default features and limits,
  whatever the adapter supports. `GpuReport` records the adapter, backend,
  device type, driver and tier, and what the adapter could do beyond Tier 0
  (timestamp queries, binding arrays, indirect counts, ray queries, mesh
  shaders).
- **Adapters.** `ENG_GPU_ADAPTER` picks the first adapter whose name
  contains it, ignoring case; otherwise wgpu chooses, software last. With
  no adapter `Gpu::new` fails with `[E-NO-ADAPTER]`, and GPU tests skip
  unless `ENG_REQUIRE_GPU=1` makes them fail (CI's native jobs set it).
  Each OS has one backend, so machines agree: Metal on macOS, Vulkan on
  Linux, and DX12 on Windows, which compiles shaders with FXC, part of
  Windows.
- **Flags.** Debug builds use wgpu's release flags, as the game does;
  `WGPU_DEBUG=1` and `WGPU_VALIDATION=1` add debug labels and the
  backend's validation layer when debugging. Every pass, target, pooled
  texture and pipeline has a wgpu label; with `WGPU_DEBUG=1` each pass is
  also wrapped in a debug group of its name.
- **The fixed graph** has three passes, not the spec's two (ADR-0017,
  proposed). Passes declare what they read and write (`PassDecl`).
  `Graph::new` checks that each read was written by an earlier pass, that a
  pass writes exactly one resource, that the output is written but never as
  the surface, and that only a surface graph uses the surface, which it
  must write. Its passes run in the order given. `Graph::fixed` is rung
  1b's list, and the one eng-render draws through.
- **The sorted graph** (rung 2, ADR-0019, proposed): `Graph::sorted` takes
  `PassNode`s, which add `continues`, the earlier pass whose targets a pass
  draws on top of (writing some or all of them), and `bypassable`.
  `Graph::sorted_without` also takes the names of passes switched off.
  - Order: a pass runs after the pass it continues and after the last
    writer of each resource it reads; among passes free to run the
    smallest name goes first, as eng-sim orders systems, so any order of
    declaration gives the same passes, culled and bypassed passes, and
    pool. A cycle, or two passes that write one resource with neither
    continuing the other, fails with `[E-GRAPH]` naming the passes.
  - Bypass: a switched-off bypassable pass is removed and its write takes
    its input's texture (same format and size, or `[E-GRAPH]`), so its
    readers read the input; the rest is sorted as if it were never
    declared, and `Graph::bypassed` names it. Switching off a pass that
    may not be bypassed is `[E-GRAPH]`.
  - Culling: a pass whose writes reach neither the output nor the surface
    is dropped, and `Graph::culled` names it.
  - Pooling: `Resource::Transient` declares a texture that lives within a
    frame (name, `Format`, and a divisor of the targets' size). Transients
    of one format, size and usage share a texture when their lifetimes do
    not overlap (`Graph::pool`): a chain of ping-pong passes needs two.
    `Targets` makes the textures on the first frame that needs them and
    keeps them (`Targets::transient_textures`); a resize makes new targets.
    This GPU side is built ahead of the plan's T8.
  - A pass may write several resources in the plan; `Graph::render` draws
    only passes with one until T8.

  `Graph::render` takes one `Pass` per declaration, in the order declared.
  The passes that run are checked and record in running order; culled and
  bypassed ones are checked and do not record. Each records into a
  `PassRecorder`: a clear (a continuing pass loads its target instead),
  then draws of two kinds.
  - Fullscreen: eng-gpu owns the vertex stage (`src/fullscreen.wgsl`); the
    caller writes the fragment stage, which gets `uv` and may bind the
    pass's one read as `input`, with a sampler that takes the nearest
    texel.
  - Instanced: an `InstancedPipeline` draws a triangle strip per instance
    into any target but the surface, reading each instance's `Attribute`s
    (`f32x2`, `f32x3`, `u32`, packed, little-endian) from bytes the pass
    records.

  Every recording is checked against its declaration before the frame's
  instances are uploaded, in one buffer, and the frame is encoded and
  submitted.
- **Counters.** `Graph::render` returns the frame's `FrameCounters` (spec
  6.13): draws, pipeline switches, bind-group switches and uploaded bytes,
  counted from the commands it encodes, so they are exact and the same on
  every adapter. A pass binds a pipeline only when a draw needs another,
  and its input once; nothing stays bound from one pass to the next.
- **The surface** is `Bgra8UnormSrgb`, not the surface's preferred format
  (spec 6.7, ADR-0017), so the present blit reproduces the output's
  bytes. `Gpu::with_window` takes the window through the seam (ADR-0016),
  opens a device on an adapter that can present to it, and makes a
  `WindowSurface` in that format, or in `Bgra8Unorm` drawn through an sRGB
  view; with neither it fails with `[E-GPU]`. Present mode Fifo (plan
  decision 7). Each frame `acquire` gives a `SurfaceFrame` (or nothing:
  skip the frame), `Graph::render_to` draws into it, and `present` shows
  it after the submit. Offscreen, `Targets::with_offscreen_surface` stands
  a texture in for the window's, and `Targets::read_surface` reads it back
  in RGBA order.
- **Errors.** wgpu's errors, validation included, are caught around each
  call that returns a `Result` and come back as `GpuError` (`[E-GPU]`,
  `[E-SHADER]`, `[E-GRAPH]`). Any other one panics, so validation errors
  are fatal in tests. A readback waits at most 60 s for the GPU
  (`READBACK_LIMIT`), then fails with `[E-GPU]`, so a driver that never
  finishes ends the run instead of hanging it.
- **Shaders.** `tests/shaders.rs` validates every WGSL file in the repo at
  Tier 0 with naga and translates it to SPIR-V 1.0, MSL 2.1 and HLSL
  shader model 5.1, the lowest each backend runs at. Every shader is
  therefore a `.wgsl` file, loaded with `include_str!`; WGSL inline in Rust
  would escape the check.

## The top-down view

`eng-render` (spec 6.7, 6.13) draws rung 1b's top-down view, the one kept
later as the automap and debug view, through eng-gpu's fixed graph. The
probe's `shot` and `video` call it, and so does `play`'s window. It
reads only the scene a game's extraction hands it, never the simulation,
and nothing flows back.

```
GameDef::top_down(TopDownFrame { previous, current, alpha, width, height })
  game extraction ──▶ TopDownScene: camera, background, shapes (WorldPos, f64)
     TopDownScene::quads: camera subtracted in f64, then clip space, then f32
Renderer::render:
  draw     top_down.wgsl  clear to the background, then every quad in one instanced draw ──▶ scene
  tonemap  tonemap.wgsl   clamp to [0, 1], alpha 1 ──▶ output (shot, video, goldens)
  present  present.wgsl   output, texel for texel ──▶ surface (targets with one)
  ──▶ FrameCounters
Renderer::render_to: the same three passes, into a window's SurfaceFrame
```

- **The view** looks straight down: +X to the right of the image, +Y up
  it, heights ignored. `TopDownCamera` gives the world point at the
  image's centre and the zoom in pixels per meter, the same across and
  down. Shapes are rectangles (`min`, `max`) and discs, drawn in order.
- **Precision.** Each position is subtracted from the camera's in f64 and
  scaled to clip space before rounding to f32, so a world 10^9 m from its
  origin draws as well as one at it (AGENTS.md conventions). Rectangles
  are clipped to the image in f64 first, so one far larger than the image
  keeps its edges.
- **Colour.** Shape and background colours are `LinearRgb`, linear and
  scene-referred, and may exceed 1. Rung 1b's tone map clamps each channel
  to [0, 1] (plan decision 2) and the output's sRGB format encodes it; the
  curve, light in nits and EV100 exposure come with lights in rung 2.
- **Extraction.** A game gives the probe a function from a `TopDownFrame`
  to a `TopDownScene` (`GameDef::top_down`). It reads the previous and
  current states and never writes, and it interpolates: at the frame's
  alpha, exactly the previous state at 0 and the current one at 1. `shot`
  and `video` pass one tick's state as both, at alpha 1, so they are
  exact. The testbed's is `crates/game-testbed/src/top_down.rs` (plan
  difference 6): the room centred and fitted to the image (40 px/m at
  320x240), its wall under a floor inset by 0.1 m, and one disc per dot,
  as wide as its hull and coloured by player slot.
- **Culling and checks.** A shape that covers none of the image is left
  out, so it costs nothing to upload. A scene that cannot be drawn (a
  value that is not finite, a rectangle inside out, a zoom that is not
  above 0) fails with `[E-SCENE]` before any GPU work.
- **Cost.** Offscreen a frame is 2 draws, 2 pipeline switches, 1
  bind-group switch and 32 bytes per shape in view; presenting adds 1, 1
  and 1. With nothing in view the top-down draw is skipped. Reports carry
  each frame's peaks ("The report"). The `[gpu]` table in `budgets.toml`
  limits them for each frame of `shot` and `video` (`[E-BUDGET]`, exit
  13): 12 draws, 12 pipeline switches, 8 bind-group switches and 2,048
  bytes, about four times the testbed's view with a window. `play` is not
  limited, but the native job's `gpu-budgets` step holds its window's
  peaks to the same limits.

## The frame

`play` (spec 6.6) is the game. `eng-app` composes the desktop platform,
the in-process session, live input and the renderer, and the testbed
enters through it (`eng_app::main`, eng-cli's probe plus `play`). eng-cli
parses `play`'s flags and hands eng-app a `PlayRun`: `run`'s session,
tick, hash stream, recording and report. eng-app owns the clock, the
window and drawing. One frame, from the OS to the screen:

```
OS events ─eng-platform-winit─▶ PlatformEvent ─▶ eng-app's Frames (an App)
event  keys ─▶ LiveInput (held keys, a latch for taps; focus lost releases all)
       CloseRequested ─▶ exit;  Resized ─▶ surface and targets;  Occluded ─▶ no drawing
frame  TickClock::due(now) ─▶ at most 5 ticks, the rest dropped ([W-TICK-DROP])
       each tick: LiveInput::sample(tick) ─▶ quit? exit : PlayRun::step(command)
         Session::tick: client ─Commands─▶ Memory ─▶ server: Sim::step, record
                        server ─Snapshot─▶ Memory ─▶ client: restore ─▶ hash stream
       draw: extraction(previous, newest, alpha) ─▶ TopDownScene
             ─eng-render─▶ draw ─▶ tonemap ─▶ present ─▶ WindowSurface::present
       Wake::At(the next tick)
stop   PlayRun::finish ─▶ --hashes, --record, --report, the console summary
```

- **Ticks come from the timer** (`TickClock`), a fixed-timestep
  accumulator in whole nanoseconds of the platform's clock (1/60 s
  rounded down, as the headless ticker paces `run`), woken with
  `Wake::At`. A wake runs at most 5 ticks (`MAX_TICKS_PER_WAKE`); the
  clock moves past the rest, which are counted in the report's
  `dropped_ticks` and logged as `[W-TICK-DROP]`. The simulation never
  skips a tick; it falls behind the clock.
- **Input.** Each tick samples `LiveInput` once, before its step, with
  the game's view yaw held ("Live input"). `quit`
  (Esc by default) ends the run before that tick and never reaches the
  simulation; so does a close request. With `--input` the script's
  commands replace the keyboard's and only `quit` is read, so `play
  --input` gives `run`'s hash stream tick for tick (acceptance
  `native-window`).
- **The session.** Each tick is `run`'s `Session::tick` ("The session").
  Before each step `PlayRun` restores the client's state into a second
  sim, so a frame has the previous state too.
- **Drawing.** After the ticks, the game's extraction builds the scene
  from the previous and newest states at the clock's alpha, and
  eng-render draws it through the three passes into the window's image
  (Fifo), the engine-owned targets kept at the window's size. Frames come
  once per tick, when the loop wakes for it, plus one per batch of events
  (keys, a resize), so alpha mostly stays near 0; a display faster than
  the tick rate gets no extra frames yet.
- **Occlusion.** Nothing is acquired while the window is minimised or
  occluded (on macOS acquiring can block for a second); ticks go on. On
  macOS wgpu 30 also refuses to acquire while the window is not visible,
  so a locked or sleeping screen presents nothing. Each frame that is not
  occluded reads the window's size before acquiring (0 x 0 is minimised)
  and compares it with the surface's, so a resize whose event never came
  is caught, and so is a window back from minimised without one.
- **The end.** `--ticks N` ends after tick N; `quit` and a close request
  end it with exit 0. `stop` writes the files, so they are saved even
  where a desktop's quit command ends the process right after it (macOS).
  Without a display `play` exits 64 with `[E-NO-DISPLAY]`, without an
  adapter with `[E-NO-ADAPTER]`, before the first tick and writing
  nothing. A panic, `--panic-at-tick` included, goes to eng-cli's hook
  (`crash.json`, exit 12), and so does a frame that fails once the GPU
  is open, a lost surface included.
- **Tests.** `crates/game-testbed/tests/twin.rs` runs the same loop on the
  headless platform with scripted keys (the headless twin, every target);
  `crates/game-testbed/tests/play.rs` runs the binary in a real window as
  a child process, since winit needs the main thread (plan, risk table).

## Shots, the proof video and goldens

`shot` and `video` (`crates/eng-cli/src/capture.rs`) run as `run` does,
on the headless platform, and draw the top-down view offscreen from the
client's state at alpha 1, so the pictures are exact.

- **Files.** They open the GPU before the simulation runs; without an
  adapter they exit 64 with `[E-NO-ADAPTER]` and write nothing (plan
  decision 4). `shot` writes one PNG at its tick; `video` an animated PNG
  of tick 0 and every K-th tick after it, each frame shown for K ticks.
  The last frame is the last multiple of K not past N, so `--ticks` a
  multiple of `--every` ends on the run's last state. Frames are kept in
  memory and the file is written once the run reaches its last tick, so
  a run that a budget stops early leaves none. Both files are 8-bit sRGB
  RGBA with an `sRGB` chunk and nothing else, so the same pixels give the
  same bytes. A frame that fails once the GPU is open (`[E-SCENE]`,
  `[E-GPU]`) is a bug: it panics, and the hook writes `crash.json` (exit
  12).
- **Golden shots** (plan decision 3, spec 8.3):
  `crates/game-testbed/tests/shots.rs` (with `shots/compare.rs` and the
  GPU-free tests in `shots/parts.rs`) runs the binary's `shot` for four
  320x240 pictures of seed 1 (dot-square at ticks 0, 120 and 240, ten dots
  at tick 0) and compares each with `crates/game-testbed/golden/<name>.png`
  as decoded RGBA8 pixels, never as PNG bytes. Without an adapter it
  skips, unless `ENG_REQUIRE_GPU=1`.
  - On the pinned configuration the pixels must be equal: Linux x64 with
    Mesa 25.2.7's lavapipe (gfx-rs/ci-build build26), LLVM 18.1.3 and
    256-bit vectors, as the report's `gpu` object names them. The harness
    passes `LP_NATIVE_VECTOR_WIDTH=256`, and no other `LP_*` or
    `GALLIVM_*` variable, to every shot. `ENG_SHOT_MODE=exact` fails on
    any other configuration.
  - Elsewhere FLIP's mean and its 99th and 99.9th percentiles must stay
    within `golden/thresholds.toml`; the 99.9th sees a dot that is
    missing, which the other two do not.
  - `ENG_GOLDEN_DIR` compares with another directory, which must exist
    outside `golden-candidates/` and hold every shot's golden: another
    machine's candidates, or an edited copy. An edited copy fails only in
    exact mode, as FLIP passes a single changed pixel by design, so
    `cargo xtask selftest golden-edit` (criterion 12) needs the pinned
    lavapipe. It changes one level of one pixel of a copy and expects
    `[E-GOLDEN]` for that shot alone. CI's `selftest` job and its
    `ubuntu-24.04` native job download the pinned build (Mesa 25.2.7 from
    gfx-rs/ci-build build26, SHA-256 checked) and point `VK_DRIVER_FILES`
    at it; on any other machine the case says it could not run.
- **The golden flow.** `golden/` is the owner's, and the harness never
  writes there. It holds `thresholds.toml`, the owner's FLIP limits; the
  goldens come with gate G6. While it holds no golden, each shot is saved
  as a candidate in `target/golden-candidates/` and reported as
  `[W-NO-GOLDEN]`. A FLIP comparison with no `thresholds.toml` beside the
  goldens (another machine's candidates through `ENG_GOLDEN_DIR`, say) is
  measured and reported as `[W-NO-THRESHOLDS]`. Neither fails the test,
  and neither is a pass for acceptance (the native job's `shot-exact` and
  `shot-flip` steps are pending): the run's `golden-candidates/shots.json`
  gives the configuration, the machine and each shot's status. The owner
  commits candidates from the pinned lavapipe as the goldens, byte for
  byte. From then on every shot needs its own golden, and a golden that no
  shot names fails. A failing shot is saved as a candidate, and every FLIP
  comparison leaves its error map in `flip/`. Each native runner uploads
  these as `golden-candidates-<runner>`, and the `golden-update` skill
  brings them to the owner.

## Invariants

1. The simulation is a pure function of the initial state and the commands.
   Nothing in the core set reads a clock, a file, the OS or unseeded randomness.
2. Results are bit-identical on every target. Floats use only IEEE basic
   operations and `eng-math` (pure-Rust `libm`, pinned, its `arch` code off
   by `force-soft-floats`); no FMA, no std transcendentals, no `HashMap`
   iteration.
3. State is always finite.
4. A system's order depends only on stages, `before`/`after` and names.
5. Third-party types never appear in an engine crate's public API. There
   are two exceptions: serde derives on authored-data types, and
   `raw-window-handle`'s `HasWindowHandle` and `HasDisplayHandle` between
   `eng-platform-api`, `eng-platform-winit`, `eng-gpu` and `eng-app`
   (ADR-0016).

## Oracles

| Oracle | Where | Changes need |
|---|---|---|
| Cross-architecture equality | `cargo xtask compare-hashes` in CI's summary job | never changes |
| `eng-math` known answers | `crates/eng-math/tests/known_answers.txt` | the owner |
| Golden vectors | `crates/*/tests/golden.rs` | the owner |
| Golden images and FLIP thresholds | `crates/game-testbed/golden/` (thresholds only, no images yet), compared by `crates/game-testbed/tests/shots.rs` and `tests/shots/` | the owner |
| Harnesses acceptance relies on | the test files `protected.toml` lists | the owner |
| Semantic assertions | `crates/game-testbed/scenarios/*.assert.toml` | the owner |
| Budgets | `budgets.toml` | the owner |
| Hash streams (change detectors) | `crates/game-testbed/hashes/` | the PR, with a reason; the reviewer approves |

## Agent tooling

- `cargo xtask` is the only entry point; CI only calls it.
- `quick`, `ci --job`, `replay` and `selftest` build into `target/` and
  run what they built, so each holds an OS file lock on
  `target/xtask/run.lock` for its whole run (`xtask/src/run_lock.rs`),
  taken once its arguments are checked. Another session's run prints one
  line naming the file, and the holder's PID except on Windows (which
  will not read a locked file), then waits; with `ENG_XTASK_NO_WAIT` set
  it exits 75 instead. The Stop hook never waits: while the lock is held
  it skips its `quick` and says so in one line. An xtask that a holder
  starts and that would take the same lock fails at once
  (`[E-LOCK-NESTED]`). The lock ends with its process, so it cannot go
  stale. A plain `cargo` command takes no lock.
- CI's `test` job runs the workspace's tests, then, each as a step of its
  own, the harnesses whose results acceptance relies on: `known-answers`,
  `exit-codes`, `crash-json`, `version`, `live-input`, `wire-session` and
  `shaders` (`HARNESSES` in `xtask/src/ci.rs`; `wire-session` runs
  eng-net-wire's `golden` and `never_panic` and eng-session's
  `hash_equality`), then the replays. Each step is an entry in the job's
  report (`target/ci/report.json`, copied to `target/ci/reports/`), which
  `cargo xtask acceptance` reads.
- CI's `native` job runs on each of the five native runners
  (`ubuntu-24.04`, `ubuntu-24.04-arm`, `windows-2025`, `windows-11-arm`
  and `macos-26`; the Linux ones under `xvfb-run`). It sets
  `ENG_REQUIRE_GPU=1` and `ENG_REQUIRE_DISPLAY=1` for the steps that
  render or open a window (the shot steps need only the GPU), so none can
  skip. Its steps are `STEPS` in `xtask/src/ci_native.rs`;
  `ENG_CI_STEPS=<names>` runs only the named ones. Each is an entry in the
  job's report, `target/ci/reports/native-<host>.json`, which also records
  the adapter and driver (`gpu`). The steps:
  - every runner: `build`, `tests`, `replay`, `crash-json`, `version`
    (both forms of the stamp, every build value set), `window` (`play` in
    a real window gives the committed hash stream and presents frames)
    and `gpu-budgets` (a `shot` held to the `[gpu]` limits, and the
    window's peaks too);
  - `shot-exact` on Linux x64 and `shot-flip` on the others: the shot
    harness against the goldens, pending (not a pass) while a golden is
    missing;
  - Linux x64 also runs `esc` (Escape through XTEST quits `play` within
    5 s) and `proof-video` (the scripted session, 241 frames);
  - the macOS runner also runs `check-intel-mac`, a compile check of
    `x86_64-apple-darwin`.

  On Linux x64 the offscreen renders ask for the pinned lavapipe by name,
  and `play` takes Ubuntu's own lavapipe, installed beside it, since the
  pinned build offers no window surface. Each runner uploads
  `report-native-<runner>` (the report and the step outputs),
  `hashes-native-<runner>`, `golden-candidates-<runner>`,
  `crash-native-<runner>` and `logs-native-<runner>`.
- CI's `summary` job waits for all of those. `compare-hashes` needs the
  hash streams of both `test` runners, `riscv` and the five native
  runners, and holds each to the committed streams, so all agree
  (`[E-CROSS-ARCH]`). `pr-duration` times the PR tier, from the run's
  start to the end of its last job. `native-duration` times each native
  job. Going over `budgets.toml`'s `[ci]` limits (`pr_tier_minutes`, and
  `native_job_minutes` for the slowest native job) is reported, never
  failed; a missing or unfinished job fails (`[E-DURATION]`).
- `cargo xtask acceptance DIR` reads every job report under DIR and
  computes each feature's `passes` into `target/ci/acceptance.json`;
  CI's summary job runs it. A feature prints `PASS`, `FAIL` or `PENDING`:
  a pending step, such as a shot without its golden, is not a pass and
  fails no job; its feature gets `passes: false` and `pending: true`. A
  feature with `owner: true` and no checks prints `OWNER`: the owner
  plays the rung and accepts it, and its `passes` is `null`.
- `cargo xtask selftest [CASE]` runs deliberate violations that a check
  must refuse: a change to a copy of the workspace (or of the goldens),
  or a command given to the real guard. CI's `selftest` job runs every
  case as a step. Rung 1b added `wgpu-outside-gpu`,
  `winit-outside-platform` and `core-depends-on-gpu` (arch-check),
  `rwh-outside-seam` (`cargo deny check bans`, ADR-0016) and
  `golden-edit` ("Shots, the proof video and goldens"). A case that needs
  what the machine lacks (cargo-deny, the pinned lavapipe) is never a
  pass: it prints `[W-SELFTEST-NOT-RUN]` and is pending in the job, and
  with `CI` set or `ENG_REQUIRE_GPU=1` it fails as
  `[E-SELFTEST-NOT-RUN]`.
- The probe (`eng-cli`, linked by every game) runs `run`, `dump-world`,
  `shot`, `video` and `play` through the in-process session: headless
  with scripted input, offscreen for `shot` and `video`, in a window for
  `play` ("The frame"). It writes reports, events, hash streams and
  recordings; `replay verify` steps the sim directly from a recording.
  Exit codes are listed in `AGENTS.md`. `<game> --version` and `<game>
  --version --json` print the version stamp (below).
- **`crash.json`.** The probe's panic hook (`crates/eng-cli/src/exit.rs`),
  installed for every command, `play` included, writes
  `crash-<game>-<pid>.json` beside the agent log (`target/agent-logs/`,
  or `$ENG_AGENT_LOG_DIR`) and exits 12. It holds `crash_version` 1, the
  game, the version stamp's `build` object (below) as `version`, the
  arguments, seed and tick, the thread, the panic's message and location,
  the backtrace (cut at 400 lines), and the log file's path and last 50
  records. Only the first panic writes the file. Nothing in the hook may
  panic or wait on a lock for long, because a panic inside the hook
  aborts the process instead. `crates/game-testbed/tests/exit_codes.rs`
  runs the real binary for each exit code, and `crash_json.rs` uses `run
  --panic-at-tick` to check every field of the crash file, and `play
  --panic-at-tick` to check it again from a window. That half skips
  without a display or a GPU, unless `ENG_REQUIRE_DISPLAY=1` or
  `ENG_REQUIRE_GPU=1`, which CI's native jobs set.
- Claude Code hooks (`.claude/settings.json`) run a copy of xtask built
  from `main` into `<git dir>/eng-hooks/`: the guard (protected paths, bless
  commands), rustfmt after edits, `quick` at the end of a turn. Before the
  first commit, `install-hooks` builds from the working tree instead.
- Gates: commits go straight to `main` (ADR-0014), so the local guard is
  what stops an agent changing a protected path; it catches accidents and
  the obvious forms but runs as the agent's own OS user. Agents push with
  `cargo xtask push`: to `agent/*` branches, and only commits whose
  protected files match GitHub's `main`. `settings.json` and the guard
  refuse them `git push`, and `main` stays the owner's (ADR-0018). CI runs
  every check on every push to either. If pull requests return, CI's
  `oracle` workflow (base-branch code only, owner approval or label)
  becomes the gate.

## The report

`--report` writes one JSON object (`crates/eng-cli/src/report.rs`),
`report_version` 2. `run`, `shot`, `video` and `play` share it (`replay
verify` writes its own): `command`, the stamp's `build`, `platform`
(`headless`, or `winit` for `play`), the run (seed, players, ticks,
`dropped_ticks`, hashes, state size, tick times), `assertions`, `budgets`
and `gpu`. `ticks_requested` is `null` for a `play` without `--ticks`.

- `budgets` holds each limit from `budgets.toml` (`null` when unset): the
  `[sim]` limits with the peaks seen, the four `[gpu]` limits, and
  `exceeded`, the first excess (exit 13).
- `gpu` is `null` for `run` and for a `play` without a window. Otherwise
  it holds eng-gpu's capability report (`adapter`, `backend`,
  `device_type`, `vendor`, `device`, `driver`, `driver_info`, `tier` and
  `capabilities`, five booleans), the image's `width` and `height` (for
  `play`, the window's last size), the `frames` rendered or presented,
  and `per_frame`: the largest `draws`, `pipeline_switches`,
  `bind_group_switches` and `upload_bytes` of any frame (`null` before
  the first). `play` adds `surface_format`, the format its surface got
  (`Bgra8UnormSrgb`, or `Bgra8Unorm` drawn through an sRGB view). For
  `shot` and `video` it is an object even when a budget stopped the run
  before the first frame (`frames` 0).
- Stability follows the stamp's rule: keys may be added within a version,
  as `gpu` and the `[gpu]` limits were in rung 1b, and readers look keys up
  by name. Renaming, removing or changing a key bumps `report_version`.

## The version stamp

`<game> --version` prints the stamp on one line, and `<game> --version
--json` prints all of it (spec 6.13). Both exit 0; any other argument
after `--version` exits 64. The code is `crates/eng-cli/src/version.rs`.
`crates/game-testbed/tests/version.rs` pins the format: the keys, their
order, their types and the build values, for both forms.
`version_testbed.rs` pins what changes with the testbed's sim (its
systems and schemas). It also checks that `dump-world`, reports and
recordings agree with the stamp.

```text
game-testbed 0.1.0 (engine 0.1.0 0123456789ab, dirty) aarch64-apple-darwin release, rustc 1.99.1, sim protocol 0x69fd4996880a6508
```

The line holds the game and its version; the engine's version and commit
(12 characters), then `game <commit>` if the game's commit differs, then
`dirty` (`dirty unknown` when not known, nothing when clean); then the
target, profile, compiler version and protocol version.

The JSON object, `stamp_version` 1:

| Key | Type | Value |
|---|---|---|
| `stamp_version` | integer | the format version, 1 |
| `game` | string | the game's name |
| `build` | object | the build (below). Reports carry the same object as `build`, and `crash.json` as `version` |
| `sim` | object | the simulation (below) |
| `formats` | object | the versions, as integers, of the report, crash file, recording and hash stream the probe writes and of the input script, assertions and budgets files it reads: `report`, `crash`, `recording`, `hash_stream`, `input_script`, `assertions`, `budgets`. The bindings file that `play --bindings` reads (version 1) is not listed yet |

`build` has the fields of spec 6.13's `version.json`. Facts about the
compilation come from `crates/eng-cli/build.rs`. Facts about the source
tree come from four environment variables read at build time.
`cargo xtask` sets all four for every cargo command it runs
(`xtask/src/stamp.rs`), from git and GitHub Actions' variables, once per
run. A non-empty value the environment already has wins, such as CI's
`ENG_ENGINE_SHA`. A plain `cargo` build sees only what the environment
sets. Without them a value reads `unknown` (strings) or `null`. Any of
the four set to an empty string counts as unset. A malformed `ENG_DIRTY`
or `SOURCE_DATE_EPOCH` fails the build with `[E-VERSION]`.

| Key | Type | From |
|---|---|---|
| `game_version` | string | the game's crate version (the spec's "semver") |
| `engine_version` | string | the `eng-cli` crate's version |
| `engine_sha` | string | `ENG_ENGINE_SHA` (xtask: `git rev-parse HEAD`), else `unknown` |
| `game_sha` | string | the game's `GameDef::build_sha`, else `unknown`; the testbed passes the engine's |
| `dirty` | bool or null | `ENG_DIRTY`, `0` or `1`: the source tree had uncommitted changes (xtask: `git status --porcelain` lists anything, untracked files included) |
| `source_date_epoch` | integer or null | `SOURCE_DATE_EPOCH`: the commit time in Unix seconds (xtask: `HEAD`'s committer time) |
| `ci_run` | string | `ENG_CI_RUN` (xtask: `github-<run id>/<attempt>` on GitHub Actions, `local` elsewhere), else `unknown` |
| `target` | string | the target triple (Cargo's `TARGET`) |
| `rustc` | string | `rustc -V` of the compiler that built it |
| `profile` | string | `debug` or `release`: the Cargo profile the build's profile inherits from (Cargo's `PROFILE`) |
| `debug_assertions` | bool | whether debug checks such as `[E-NAN]` are compiled in |

`sim` comes from a simulation built with 1 player and seed 0; the
protocol version depends on neither.

| Key | Type | Value |
|---|---|---|
| `tick_rate` | integer | ticks per second |
| `protocol` | string | the simulation protocol version, `0x` and 16 hex digits, as in hash streams and recordings |
| `systems` | array of strings | the resolved system order, each `<Stage> <system>` |
| `schemas` | array of objects | every named type the world encodes, sorted by name, then version: `name`, `version` and `kind` (`component`, `resource`, `event`, or `nested` for a type used only inside others) |

Stability:
- While `stamp_version` stays the same, no key is renamed or removed and
  none changes its type or meaning. Keys may be added; readers look keys
  up by name and ignore ones they do not know. Any other change bumps
  `stamp_version`, and a change to `build` also bumps `report_version`
  and `crash_version`.
- Version 1 reports, from rung 1a, had a smaller `build` object, with
  `target` as `<arch>-<os>` (such as `aarch64-macos`) and `rustc`
  usually `unknown`. Version 2 reports carry the stamp's `build`.
  `crash.json` started at version 1 with the stamp's `build`.
- One binary always prints the same bytes: every value is fixed when it
  is built or comes from the simulation's registrations, never from the
  clock, the environment at run time or a path.

## Adding a crate

1. Pick the layer from the spec's table (6.1) and whether it is in the core
   set (everything the simulation links is). Contract crates (traits, plain
   data, a conformance suite) exist only once there are two implementations,
   counting null and offline twins.
2. Ask the owner to add it to `architecture.toml` (protected): `layer`,
   `core = true` if core, `same_layer` dependencies. A core crate also needs
   `[profile.release.package.<name>] overflow-checks = true` in the root
   `Cargo.toml` (a protected table). Give the exact lines in your summary.
3. Create `crates/<name>/Cargo.toml` with `version.workspace = true` (and
   `edition`, `rust-version`, `license`, `publish`), `[lints] workspace = true`,
   and add `<name> = { path = "crates/<name>" }` to `[workspace.dependencies]`.
4. Start `src/lib.rs` with a doc comment, then for core crates exactly
   `#![no_std]` and `#![forbid(unsafe_code)]` (plus `extern crate alloc;`).
5. New third-party crates need the owner's approval (`deny.toml`).
6. Put tests in `tests/`, then run `cargo xtask quick`.
