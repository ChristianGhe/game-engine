# Architecture

How the engine fits together, as built so far in rung 2. The plan and its
reasons are in `docs/superpowers/specs/2026-10-20-ai-built-engine-design.md`;
decisions in `docs/adr/`. Keep this file short and true: update it in the
commit that changes what it describes.

## Layers

```
L0  eng-schema-core ▶ eng-derive;  eng-math, eng-core ▶ eng-schema-core   (core set)
L1  eng-ecs  eng-net-transport  eng-net-wire                                (core set)
    eng-input ▶ eng-schema-json, eng-platform-api                           (std)
    eng-platform-headless ▶ eng-platform-api                                (std)
    eng-platform-winit ▶ eng-platform-api, eng-platform-gcmouse             (std)
L2  eng-sim  eng-collision-api;  kit-fps ▶ eng-collision-api                (core set)
    eng-collision-grid ▶ eng-collision-api                                  (core set)
    game-testbed-sim ▶ eng-sim, eng-collision-api, kit-fps,                 (core set)
                       eng-collision-grid
    eng-session ▶ eng-sim                                                   (core set)
L3  eng-render ▶ eng-gpu                                                    (std)
L4  eng-app ▶ eng-cli                                                       (std)
game  game-testbed (binary) ▶ game-testbed-client
tool  xtask  eng-bench
```

A crate depends only on lower layers, or on the same-layer crates its
`same_layer` lists in `architecture.toml`, drawn above as `A ▶ B` (A
depends on B). Rung 1b added seven crates and rung 2 four; their edges to
our others (third-party crates in brackets):

| Crate | Layer | Depends on |
|---|---|---|
| `eng-platform-api` | L1 | none [raw-window-handle, for the window seam: ADR-0016] |
| `eng-platform-winit` | L1 | eng-platform-api, eng-platform-gcmouse [winit, raw-window-handle] |
| `eng-platform-gcmouse` | L1 | none [objc2-game-controller, objc2, block2: macOS only] |
| `eng-net-wire` | L1, core | eng-core |
| `eng-collision-grid` | L2, core | eng-collision-api, eng-math, eng-schema-core |
| `eng-session` | L2, core | eng-sim, eng-ecs, eng-net-wire, eng-net-transport, eng-core |
| `eng-gpu` | L3 | none [wgpu, raw-window-handle] |
| `eng-render` | L3 | eng-gpu, eng-math |
| `eng-app` | L4 | eng-cli, eng-platform-winit, eng-platform-api, eng-input, eng-gpu, eng-render |
| `game-testbed-client` | game | eng-cli, eng-render, game-testbed-sim, eng-collision-grid, kit-fps and the L0-L2 crates they share |
| `eng-bench` | tool | dev-dependencies only: game-testbed-sim, eng-collision-grid, kit-fps, eng-sim [gungraun, Linux only] |

eng-cli, the probe, uses four of them (eng-session, eng-gpu, eng-render
and eng-platform-api) and the headless platform; game-testbed enters
through eng-app, and draws through game-testbed-client.

The four rung 2 crates:
- **`eng-collision-grid`** (core): Level 1's collision, a uniform grid of
  1 m cells over vertical wall segments and solid horizontal slabs, built
  once from a map's sim data. It implements `CollisionWorld`, stops traces
  in contact (ADR-0024), and gives a brute-force oracle's `Trace` bits
  (`tests/oracle.rs`); the crate docs list its sweep's six steps. The
  oracle (`tests/oracle/brute.rs`) repeats every one of those rules (the
  still axis's face, `GRAZE`, the `NOISE` slide, the vertex contact's
  `4 * NOISE`, the 2^-40 sliver, the nudge and the back-off), so its bits
  test the grid's choice of candidates and of the first hit, not the
  rules: a wrong rule is wrong in both. What checks the rules is
  `eng_collision_api::conformance::check` (the contact rule) and
  `tests/contract.rs`'s pass-through check (`tests/oracle/through.rs`:
  the hull shrunk by 1e-6 m free on the whole way to each stop, by its own
  trace, by clipping the path against each primitive in plain arithmetic,
  and at samples).
- **`game-testbed-client`**: the testbed's presentation, read from the
  sim's state and never written back: the top-down view, moved unchanged
  from `game-testbed`, and the room's first-person view ("The first-person
  view"), with textures and sprite frames made by code and the palette
  `palettes/room.gpl` from its script.
- **`eng-platform-gcmouse`**, the GCMouse shim (ADR-0021): on macOS, raw
  mouse deltas from the Game Controller framework, summed for
  eng-platform-winit to drain each frame, and a count of its callbacks;
  empty elsewhere. Its `src/macos.rs` is the engine's only `unsafe` code.
- **`eng-bench`**: gungraun benchmarks of instruction counts (the dot and
  room ticks, the room's build, grid traces, `ground_move`), on Linux
  only; elsewhere each bench is an empty `main`. Its library is empty, and
  nothing at run time depends on it ("Agent tooling", the bench job).

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

   beside the state, sealed once the sim is built:
            constants (sim data: the room's grid, map and tuning)
                 │ visit_constants, read by systems as world.constant::<T>()
                 ▼
     Sim::sim_data_hash ─▶ recording headers, map streams' headers, reports
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
- **Constant resources** (rung 2, ADR-0023): sim data that never changes
  during a run, such as the room's collision `Grid`, its `RoomMap`
  (regions, spawns, things) and its `MoveConfig` and `GroundConfig`
  tuning. They are inserted with their schemas while the sim is built
  (`World::insert_constant`), sealed by `SimBuilder::build` (a later
  insert panics with `[E-SIM-DATA]`), checked finite once, and read by
  systems through `World::constant`. The state hash, snapshots, the
  `[sim]` budget, `restore` and the NaN walk leave them out; the protocol
  version adds their names and schemas, only when a world has any, so the
  dot's protocol keeps its bytes. `Sim::sim_data_hash` is FNV-1a-64 over
  their canonical bytes in name order, 0 with none. `dump-world` shows them
  under `constants`.
- **Tuning in f32 against geometry in f64** (a note for level format v2):
  `GroundConfig`'s `step_height` and `snap_distance` are `Real` (f32),
  widened to f64 where they meet the grid's f64 heights
  (`crates/kit-fps/src/ground.rs`, `step_up` and `step_down`). f32 0.4 is
  about 6e-9 m above 0.4, so the room's 0.4 m rises climb and its 0.4 m
  drop snaps with that to spare; f32 0.7 is about 1.2e-8 m below 0.7, so a
  0.7 m step with `step_height = 0.7` would block, and a 0.7 m drop with
  `snap_distance = 0.7` would fall. No rung 2 map has such a pair. Level
  format v2 should compare tuning and geometry at one precision, or keep
  both on a grid that f32 holds exactly (multiples of 2^-k m).
- **Maps** (eng-cli's `GameMaps`, beside `GameDef`): a map is a name, a
  build function and a view (`MapView`: top-down, first person, or none,
  which `shot`, `video` and `play` refuse). `--map NAME` picks one for
  `run`, `dump-world`, `shot`, `video` and `play`, the first (the testbed's
  `dots`) by default; an unknown name exits 64 with `[E-MAP]`. Only the
  first map may lack sim data, and no two share a hash, so `replay verify`
  picks the map whose hash a recording carries (`[E-RECORDING-BUILD]`
  without one). Entry points that take no `GameMaps` (`PlayRun::start`,
  the twin test) call the first map `default`.
- **Hash streams** start `# eng hash stream v1`, then a line of the run's
  constants: `game`, `seed`, `players`, `tick_rate`, `hash_every` and
  `sim_protocol`, and for a map with sim data `map=<name>
  sim_data=0x<16 hex>`. `compare-hashes` compares whole files, so the sim
  data's hash is held equal across targets too (spec 6.11).

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
  events existed before rung 2; its cursor grab, fullscreen and refresh
  rate were a contract change (ADR-0022).
- The clock is `Control::now()`: an `Instant`, nanoseconds since the run
  started, never going back. Each `frame` answers `Wake::At(t)` or
  `Wake::OnEvent`, so ticks come from the timer, not from presenting
  (spec 6.6); an event's batch wakes the app sooner.
- `Control::exit()` ends the run: only `stop` follows, and the rest of
  the batch is dropped. A close request is an event like any other: the
  platform keeps running until the app exits.
- `Control::grab_cursor(bool)` answers the grab in force (`CursorGrab`)
  and `Control::set_fullscreen(bool)` whether it was taken; they act on
  the window from `App::window` on. `Window::refresh_rate()` is its
  monitor's, in millihertz. Default bodies answer `None` and `false`;
  headless records the requests (ADR-0022 and its proposed Amendment 1).
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
  keys at focus changes are dropped. Without a display `new` is
  `Unavailable`. On macOS, Quit (Cmd+Q, the Dock) ends the process right
  after `stop`, so `run` never returns there and an app saves what it
  must in `stop`.
- **winit's mouse** (rung 2, T13 and T14): buttons, the wheel (where it
  turned, up and right positive: `y` keeps winit's sign and `x` is
  negated, and macOS's natural scrolling is turned back) and raw motion,
  only while the window has focus. On macOS raw
  motion comes from the GCMouse shim instead (winit's own is coalesced,
  one per refresh): `about_to_wait` drains the shim's sum into one
  `MouseMotion` before the frame, and `raw_motion_callbacks()` counts the
  shim's callbacks, focused or not, across runs. A grab tries `Locked`,
  then `Confined`, and hides the cursor; fullscreen is borderless; the
  refresh rate is the window's monitor's. As built, the platform frees the
  cursor before `FocusLost` reaches the app and at a run's end, and a grab
  asked for without focus waits for it (ADR-0022's Amendment 1, proposed:
  winit sends a `FocusLost` before a new window's first `FocusGained`).
  Still open: under X11's `Confined` grab each raw motion arrives twice;
  leaving fullscreen during macOS's enter animation is lost; Xvfb reports
  no refresh rate.
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
  minute. The rules of ADR-0022's requests get a suite of their own, so
  `check` and `check_scripted` keep their expectations: a file under
  `src/conformance/` with its tests for headless and winit, which G5
  applies (`docs/rungs/2/patches/t13-conformance.patch`).

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
  `LiveInput::view_now` reads the angles the motion so far gives without
  taking it, for the camera between ticks; each tick's sample is still
  the only way they reach the simulation. First-person `play` turns it on
  with `--sensitivity` (degrees per count, at most six decimals, parsed
  exactly into millionths), `--pitch-limit` (degrees, rounded down) and
  `--invert-pitch`.
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
  restores a snapshot only from a build with the same protocol and a
  session with the same tick rate, player count and seed (`SimInfo`).
  Restore does not check sim data, which no snapshot carries: the sim
  keeps its own (ADR-0023), and until rung 5's handshake carries the
  sim-data hash nothing tells two maps with one protocol apart.
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
- A per-client relevance hook answers `Everything`, since rungs 1b and 2
  send whole states. Deltas, time sync and prediction come with loss (rung 5).
- A snapshot message must fit one 1,200-byte datagram: 466 bytes for the
  dot, and under 700 for the room with one player, whose sim data stays
  out of it. A larger one fails with `[E-SNAPSHOT-SIZE]`, and `run` and
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
appears in its API. `eng-render` builds rung 1b's top-down passes and
rung 2's first-person ones on it (the next two sections). GPU work never
feeds back into the simulation. The crate docs have the details.

```
Gpu::new(GpuOptions) ──adapter──▶ a Tier 0 device, and its GpuReport
Graph::fixed(Offscreen):
  draw ──▶ scene (Rgba16Float, linear) ──tonemap──▶ output (Rgba8UnormSrgb)
                                    output ──Targets::read_output──▶ Image (RGBA8)
Graph::fixed(Surface) adds present: output ──▶ surface (Bgra8UnormSrgb)
Graph::sorted_without(PassNodes, off) ──order, bypass, cull, pool──▶ Graph
Graph::render ──▶ FrameCounters (draws, switches, uploaded bytes)
Graph::render_frame(FrameInput: uniform, instance records, Scene) ──▶ the same
```

- **Tier 0.** The device asks for WebGPU's default features and limits,
  whatever the adapter supports, plus `TIMESTAMP_QUERY` when the adapter
  has it, only to time passes; the tier stays 0 (rung 2, O11). Times are
  opt-in per `Targets` (`time_passes`, read with `frame_stats`), `None`
  when off or unsupported. `GpuReport` records the adapter, backend,
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
- **The fixed graph** has three passes, not the spec's two (ADR-0017). Passes declare what they read and write (`PassDecl`).
  `Graph::new` checks that each read was written by an earlier pass, that a
  pass writes exactly one resource, that the output is written but never as
  the surface, and that only a surface graph uses the surface, which it
  must write. Its passes run in the order given. `Graph::fixed` is rung
  1b's list, and the one eng-render draws through.
- **The sorted graph** (rung 2, ADR-0019): `Graph::sorted` takes
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
  - A pass may write several resources: `Graph::render` draws up to four
    colour targets and one depth target, of one size (T8, ADR-0020).
    `Format` is `Rgba16Float`, `Rgba8UnormSrgb`, `Rg32Uint` or
    `Depth32Float`; depth is reversed Z, cleared to 0 and compared
    `GreaterEqual`.

  `Graph::render` takes one `Pass` per declaration, in the order declared.
  The passes that run are checked and record in running order; culled and
  bypassed ones are checked and do not record. Each records into a
  `PassRecorder`: a clear (a continuing pass loads its target instead),
  then draws of two kinds from rung 1b, and two from rung 2 (below).
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
- **The scene layout** (rung 2, ADR-0020 and its proposed Amendment 1),
  for `Graph::render_frame`, beside 1b's draws, which stay as the
  protected `counters.rs` uses them:
  - a `Scene`, loaded once from a `SceneDesc`: one vertex and one `u32`
    index megabuffer, a storage-buffer material table (bucket, colour tag,
    layer, flags; bit 0 is the alpha test) and six `texture_2d_array`s of
    square textures, 64, 128 and 256 px, sRGB and linear, sampled nearest
    with no mipmaps;
  - each frame, a `FrameInput`: a uniform of whole 16-byte rows, written
    only in a frame whose passes bind it, and the instance records
    (`InstanceRecord`, 16 bytes: an origin minus the eye, subtracted in
    f64, and a material index), so static meshes stay camera-relative
    without being uploaded again;
  - mesh pipelines (`PassRecorder::mesh`, a `MeshDraw`): indexed triangles
    from the megabuffers, once per instance, into colour and depth
    targets. A pipeline may read several records per instance;
  - screen pipelines (`PassRecorder::screen`): a triangle over the target
    that reads every one of the pass's reads, whatever its format, binds
    the uniform, may bind the scene, and draws into up to four colour
    targets;
  - three bind groups: the frame (uniform, records), the scene (materials,
    arrays, sampler) and the pass's reads. `Gpu::new` refuses an adapter
    without vertex-stage storage buffers (`VERTEX_STORAGE`) with `[E-GPU]`.
- **Low internal resolution** (T10): `Targets::upscaled(gpu, width,
  height, k)` makes targets of the window's size over k, rounded down,
  whose present pass draws each texel as k×k pixels of the window's image,
  centred, the remainder (under k pixels) cleared to opaque black; a window
  smaller than k on a side takes the largest k that fits. Every pass, the
  output included, runs at the targets' size, so a shot of that size shows
  the same pixels. The caller makes them (eng-app's first-person window);
  `render_to` holds the window's image to `Targets::surface_size`, the
  window's size, not the targets' own.
- **Reading a transient back** (T9): `Targets::read_transient` copies what
  the last frame drew into a transient, given the graph that drew it, since
  graphs share the pool; a frame of a graph that pools otherwise is
  refused (`[E-GPU]`).
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
  it after the submit. A surface the system loses is an `[E-GPU]` error
  from `acquire`; `WindowSurface::acquire_remaking` (rung 2, T15) instead
  makes it again from the kept window and skips the frame, and fails with
  `[E-GPU]` only after three remakes in a row with no frame between them.
  Offscreen, `Targets::with_offscreen_surface` (and
  `Targets::upscaled_with_offscreen_surface`) stands a texture in for the
  window's, and `Targets::read_surface` reads it back in RGBA order.
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
  to [0, 1] (plan decision 2) and the output's sRGB format encodes it. The
  top-down view keeps that clamp; the tone curve, light in nits and EV100
  exposure are the first-person view's (next section).
- **Extraction.** A game gives the probe a function from a `TopDownFrame`
  to a `TopDownScene` (`GameDef::top_down`). It reads the previous and
  current states and never writes, and it interpolates: at the frame's
  alpha, exactly the previous state at 0 and the current one at 1. `shot`
  and `video` pass one tick's state as both, at alpha 1, so they are
  exact. The testbed's is `crates/game-testbed-client/src/top_down.rs` (plan
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

## The first-person view

`eng-render`'s `FirstPersonRenderer` (spec 6.7's `render-retro`; ADR-0020
and its proposed Amendment 1) draws a map in first person through
eng-gpu's sorted graph. As with the top-down view, it reads only what a
game's extraction hands it, and nothing flows back.

```
a map's FirstPersonView (eng-cli): world, scene and names, each reading the sim only
  world(sim at tick 0) ─▶ FirstPersonWorld ─GpuWorld::load_with_palette─▶ once per run
  scene(frame) ─▶ FirstPersonScene: camera, sky, regions, EV100, look, sprites
FirstPersonRenderer::render (Graph::sorted_without the look's switched-off passes):
  opaque   ─▶ albedo (Rgba16Float), info (Rg32Uint), depth (Depth32Float)
  sprites    continues opaque: alpha-tested billboards into all three
  light    albedo, info, depth ─▶ lit (Rgba16Float, nits)
  sky        continues light, where depth is 0
  fog      lit, info, depth ─▶ fogged                     (bypassed when off)
  tonemap  fogged ─▶ toned (Rgba8UnormSrgb): EV100, then Khronos PBR Neutral
  palette  toned ─▶ output: the 32³ table, Bayer dither    (bypassed when off)
  present  output ─▶ surface, k×k into upscaled targets      (surface graphs)
```

- **The camera** (`FirstPersonCamera`): an f64 eye, `BAngle` yaw and
  pitch, a vertical field of view and a near plane, `NEAR_PLANE` (0.1 m).
  Positions are subtracted from the eye in f64, then projected with
  reversed Z and an infinite far plane: depth is `near / distance ahead`,
  0 where nothing was drawn. 0.1 m keeps the near rectangle inside the
  hull's 0.25 m half-width while `tan²(fov_y/2)·(1 + aspect²)` is at most
  5.25 (90° vertical up to an aspect of 2.06); a wider window or `--fov`
  brings the near plane in (eng-app's `lens.rs`), and the field of view is
  never clamped.
- **Chunks** (`Chunk::quad`): vertices relative to the chunk's `WorldPos`
  origin, subtracted in f64, each carrying its material, region, class and
  id; corners counter-clockwise seen from the front, back faces culled. A
  drawn chunk is one instance record, its origin minus the eye.
- **`info`** packs a pixel into two `u32`s: word 0 holds the region (bits
  0-15) and the `SurfaceClass` code (bits 16-23), word 1 the object id.
  (0, 0) is nothing drawn, the sky. Walls are classed by facing, so the
  fake contrast needs no normals. Surfaces and sprites share the id space,
  so objects are keyed by (class, id).
- **Sprites** (`Sprite`, plan decision 4): a thing is a billboard standing
  on its feet, turned about Z only, showing the frame of its
  `RotationSheet` that its rotation toward the eye picks
  (`Sprite::frame_toward`; a five-rotation sheet mirrors rotations 5-7).
  It is alpha-tested (material flag bit 0) and depth-tested, so a thing
  behind a pillar is hidden. Each takes two instance records, `2i` (its
  feet minus the eye, its frame's material) and `2i + 1` (signed width,
  height, region, id), and every sprite is one instanced draw.
- **Light, fog, tone map, palette** (plan decisions 7, 8 and 13). A
  region's light (`RegionLook`) is its Doom level, 0-255, in 32 steps of
  lux (`LIGHT_STEP_LUX`), falling off with distance from the eye, plus a
  fake contrast on walls facing the axes; luminance is the albedo times
  that, in nits. Fog moves a pixel toward its region's colour by `1 -
  e^(-density·distance)`. The tone map exposes by the scene's EV100
  (`exposure`; 5 maps 38.4 nits to 1) and maps by Khronos PBR Neutral. The
  palette indexes a 32³ table (`PaletteLut`, baked by OKLab distance from a
  `.gpl` file that `GplPalette` reads strictly) by `code >> 3` of each
  8-bit sRGB code, with a 4x4 Bayer dither; the table is 8 layers of the
  scene's 64 px sRGB array.
- **The look** (`Look`): `falloff`, `banding`, `contrast`, `sky_gradient`,
  `fog`, `palette` and `dither` each switch off alone, fog and palette by
  bypassing their passes; with all off the image is the plain pipeline's
  (`FirstPersonRenderer::render_plain`). `low_resolution` (k, 4 by
  default) is for a window's targets only; a shot ignores it.
- **The uniform** is 39 rows of 16 bytes: the projection, the camera's rays
  and near plane, the sky, the look, the 32 light steps, and the light and
  fog of up to 16 regions (`MAX_REGIONS`). Every first-person shader
  declares the same `Frame` struct, which a layout test pins.
- **Reading back**: `read_depth`, `read_info`, `read_albedo`, `read_lit`
  and `read_fogged` read the targets through the graph that last rendered;
  the first-person graph pools five transient textures.
- **Cost.** A frame uploads 624 bytes of uniform, plus 16 per drawn chunk
  and 32 per sprite. The room's shot is 7 draws, 7 pipeline switches, 15
  bind-group switches and 736 bytes offscreen; a presented frame is 8, 7
  and 16. A budgets file's `[gpu.first_person]` table, when it has one,
  holds first-person frames instead of `[gpu]` (`budgets.toml` gains it at
  G5).
- **The room's extraction** (`game-testbed-client`, its `room` module):
  one chunk of quads from the `Grid` constant; a look per region (region
  2, the alcove, has green fog); the sky through the skylight; one sprite
  per `testbed.Thing`, with the region its feet stand in; and player 0's
  camera, the eye 1.6 m above the feet with the angles of `testbed.View`.
  Textures are tiles made by code, and the sprites' frames are ray-cast on
  the CPU from small 3D primitives, so no image file enters the repo. The
  palette, `palettes/room.gpl`, comes from the script `room_gpl.py` (its
  `--check` mode tells a hand edit), with a provenance sidecar.

## The frame

`play` (spec 6.6) is the game. `eng-app` composes the desktop platform,
the in-process session, live input and the renderer, and the testbed
enters through it (`eng_app::main`, eng-cli's probe plus `play`). eng-cli
parses `play`'s flags and hands eng-app a `PlayRun`: `run`'s session,
tick, hash stream, recording and report. eng-app owns the clock, the
window and drawing. A map whose view is top-down (the dot map) runs rung
1b's loop, below, unchanged; a first-person map runs its own beside it
("The first-person loop"). One top-down frame, from the OS to the screen:

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

### The first-person loop

`play --map room` (rung 2, T15; plan decisions 9 and 10), in
`crates/eng-app/src/first_person.rs`:

```
OS events ─eng-platform-winit─▶ keys, raw mouse (macOS: the GCMouse shim) ─▶ eng-app
  LiveInput: held keys + MouseLook ─sample(tick)─▶ PlayerCommand (forward, side, view angles)
  each tick: PlayRun::step ─▶ Session::tick over Memory ─▶ Sim::step: ground_move on the grid
  a frame when one is due: the scene at the clock's alpha, the live angles (view_now)
    ─FirstPersonRenderer─▶ upscaled targets ─present k×k─▶ WindowSurface::acquire_remaking
```

- **The window** opens borderless fullscreen unless `--windowed`, and the
  cursor is grabbed while it has focus (ADR-0022 and its proposed
  amendments). A scripted run (`--input`) never grabs, so a test never
  takes the desktop's cursor.
- **Mouse look** starts where the map's scene looks at tick 0 (the room's
  spawn, yaw 0), not `GameDef::view_yaw`, which is the top-down view's.
  Raw motion turns the angles; each tick's sample carries them in that
  tick's command, the only way they reach the simulation. Between ticks
  the camera's position is interpolated at the clock's alpha and its
  angles are the live ones (presentation only); a scripted run shows the
  script's, from the state.
- **Frames** come at the window's refresh rate when it reports one, else
  once per tick (`pace.rs`), however often events wake the loop: raw
  motion comes up to a thousand times a second, and Xvfb has no vsync.
  Ticks still come from the timer, at most five a wake.
- **The lens** (`lens.rs`): `--fov`, the near plane brought in for a wide
  window, and `--look` over the scene's look. Targets are
  `Targets::upscaled` at the look's `low_resolution`, made again when the
  window's image (`surface_size()`) or the scale changes.
- **A lost surface** is made again from the kept window and the frame
  skipped; one still lost after three remakes in a row panics (`[E-GPU]`),
  and so does any other failure once the GPU is open, so the hook writes
  `crash.json`. Without an adapter, or with a surface that offers neither
  BGRA format, `play` exits 64 before the first tick, writing nothing.
- **The report** gets a `window` object ("The report").
- **Tests.** `crates/game-testbed/tests/play_room.rs` runs the loop on the
  headless platform with scripted events (raw motion, focus, the flags),
  and the binary's `play --map room --windowed` with the room's walk in a
  real window, whose hash stream must equal `run --headless`'s for the same
  script and seed (criterion 1; the committed stream lands at G5).

## Shots, the proof video and goldens

`shot` and `video` (`crates/eng-cli/src/capture.rs`) run as `run` does,
on the headless platform, and draw the map's view offscreen from the
client's state at alpha 1, so the pictures are exact: the top-down view,
or for a first-person map player 0's camera at the tick.

- **Files.** They open the GPU before the simulation runs; without an
  adapter they exit 64 with `[E-NO-ADAPTER]` and write nothing (plan
  decision 4). `shot` writes one PNG at its tick; `video` an animated PNG
  of tick 0 and every K-th tick after it, each frame shown for K ticks.
  The last frame is the last multiple of K not past N, so `--ticks` a
  multiple of `--every` ends on the run's last state. Frames are kept in
  memory and the file is written once the run reaches its last tick, so
  a run that a budget stops early leaves none. Both files are 8-bit sRGB
  RGBA with an `sRGB` chunk and nothing else, so the same pixels give the
  same bytes. Beside each goes its frame record (below). A frame that
  fails once the GPU is open (`[E-SCENE]`, `[E-GPU]`) is a bug: it panics,
  and the hook writes `crash.json` (exit 12).
- **What to draw** (`crates/eng-cli/src/shot_view.rs`, T16 and plan
  addition A3), all checked before the session or a GPU opens (exit 64):
  - `--view top-down|first-person` names the view the command expects. A
    map has one, so the flag only confirms it, and a view the map lacks is
    refused with `[E-MAP]`;
  - `--pose X,Y,Z,YAW,PITCH`, for a first-person map: the eye in absolute
    meters and the angles in degrees, a camera independent of movement. It
    keeps the scene's field of view and near plane;
  - `--look` with a look file (`crates/game-testbed/looks/off.look.toml`
    is one, version 1): each key given sets its look feature over the
    scene's, for a first-person map. Shots ignore `low_resolution`.

  The report's `view` names the kind drawn, the pose and look asked for,
  and the camera of the last frame.
- **Golden shots** (plan decisions 3 and 12, spec 8.3):
  `crates/game-testbed/tests/shots.rs` (with `shots/compare.rs` and the
  GPU-free tests in `shots/parts.rs`) runs the binary's `shot` for each
  shot that `crates/game-testbed/shots.toml` names (with G5's harness;
  before it, rung 1b's four from a constant), at 320x240 and seed 1: 1b's
  four top down (dot-square at ticks 0, 120 and 240, ten dots at tick 0),
  and seven of the room in first person, from player 0's camera at a tick
  of a room script or from a pose the manifest's `[pose]` table names. It
  compares each with `crates/game-testbed/golden/<name>.png` as decoded
  RGBA8 pixels, never as PNG bytes. Without an adapter it skips, unless
  `ENG_REQUIRE_GPU=1`.
  - On the pinned configuration the pixels must be equal: Linux x64 with
    Mesa 25.2.7's lavapipe (gfx-rs/ci-build build26), LLVM 18.1.3 and
    256-bit vectors, as the report's `gpu` object names them. The harness
    passes `LP_NATIVE_VECTOR_WIDTH=256`, and no other `LP_*` or
    `GALLIVM_*` variable, to every shot. `ENG_SHOT_MODE=exact` fails on
    any other configuration.
  - Elsewhere FLIP's mean and its 99th and 99.9th percentiles must stay
    within `golden/thresholds.toml`: its `[flip]` table, or with G5's
    harness the shot's own `[flip.<shot>]` (a table that names no shot
    fails). The 99.9th sees a dot that is missing, which the other two do
    not. The room's shots differ between Metal and lavapipe by
    palette-neighbour flips (a FLIP mean of 0.012-0.015, over `[flip]`'s
    0.01), so they need tables of their own.
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
  writes there. It holds `thresholds.toml`, the owner's FLIP limits, and
  rung 1b's four goldens (its gate G6). A shot without a golden is saved as
  a candidate in `target/golden-candidates/` and reported as
  `[W-NO-GOLDEN]`. A FLIP comparison with no `thresholds.toml` beside the
  goldens (another machine's candidates through `ENG_GOLDEN_DIR`, say) is
  measured and reported as `[W-NO-THRESHOLDS]`. Neither fails the test,
  and neither is a pass for acceptance (the native job's `shot-exact` and
  `shot-flip` steps are pending): the run's `golden-candidates/shots.json`
  gives the configuration, the machine, each shot's status and the views
  drawn. Goldens are all or nothing: once one exists every shot needs its
  own, and a golden that no shot names fails. A failing shot is saved as a
  candidate, and every FLIP comparison leaves its error map in `flip/`.
  Each native runner uploads these as `golden-candidates-<runner>`, and
  the `golden-update` skill brings them to the owner.
- **The owner's marker** (with G5's harness). A shot `shots.toml` marks
  `golden = "pending"` has no golden yet by the owner's choice, whatever
  the others have: it saves its candidate and fails nothing, with
  `ENG_GOLDEN_DIR` too, where a golden it has is measured but cannot fail
  it. A golden for a marked shot in `golden/` itself fails: the owner
  clears the marker in the commit that adds the golden (rung 2's G6).
  Unmarked shots keep the all-or-nothing rule. Rung 2's seven room shots
  are marked.

## The frame record

`shot` and `video` also write `<out>.frame.json` (plan addition A1;
`crates/eng-cli/src/frame_record.rs`): what the frame drew, as text an
agent can check without looking at the picture. `s.png` gives
`s.png.frame.json`; a video's holds one entry per frame. Version 1, keys
sorted, in one fixed layout, read back strictly by `FrameRecord::parse`.

- **Each entry** has the tick; the camera (eye, yaw, pitch, field of view
  and near plane, exactly the binary angles drawn); every object the
  extraction asked for, in (class, id) order, drawn or not, with the
  game's name, its world box, and from `info` and the output its visible
  pixels, screen box, mean luminance (Rec. 709, linear), the mean of a 4 px
  ring around its box, and the Michelson contrast of the two; `unlisted`,
  any (class, id) that `info` shows but no object asked for (normally
  empty: an extraction or shader bug); `invalid_info`; each class's pixels
  in the image's top, middle and bottom thirds; depth at a fixed 8x6 grid
  of probes, with the class, id, distance, edge and slope at each; and the
  image's mean luminance and a 16-bin histogram on the sRGB scale. A
  top-down view has no `info` or depth, so its record keeps only the image's
  luminance and histogram.
- **Exact where the pixels are.** Everything comes from the frame's own
  targets, summed in a fixed order through eng-math, so the same targets
  give the same bytes. Two adapters' records are compared within
  `Tolerances` (`FrameRecord::compare`), with per-shot overrides in
  `crates/game-testbed/frame-tolerances.toml`, whose `[default]` a test
  holds equal to `Tolerances::DEFAULT`.
- **The harness**, `crates/game-testbed/tests/frame_record.rs`: every
  manifest shot is rendered twice and must give the same bytes; with
  reference records in `golden/frames/` (the owner's, from rung 2's G6) or
  `ENG_FRAME_DIR`, each is compared exactly on the pinned lavapipe and
  within its tolerances elsewhere. As for goldens: all or nothing, a
  reference no shot names fails, and the marker holds. Every record goes
  to `target/frame-candidates/<shot>.frame.json`, with `frames.json`
  saying where they were rendered and how each compared.
- **Planted bugs**, `crates/game-testbed/tests/frame_bugs.rs`, a test
  binary of its own: a thing not drawn, a pillar face not drawn (the
  hidden crate appears) and a thing drawn black each change the record
  and fail the check.
- **Known misses** (plan addition A4 measures the rest): a thing of 2 px
  or less, or 16 px spread over a large box, can vanish within tolerance;
  a drawn-black object under 50 px is not compared; luminance tolerance is
  loose for dark objects; and a top-down record cannot see a missing dot.
- **Mesa's shader cache.** Two lavapipe processes side by side once
  corrupted it, after which every lavapipe process died. The frame-record
  harness sets `MESA_SHADER_CACHE_DISABLE=true` for its children; set it
  too when running GPU tests in parallel on lavapipe.

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
6. The engine's only `unsafe` code is in one file,
   `crates/eng-platform-gcmouse/src/macos.rs` (ADR-0021). No other crate
   or file may allow `unsafe_code`; G5's `arch-check` scans every Rust
   file for it.
7. Sim data is constant: inserted while a sim is built, sealed after, and
   covered by `sim_data_hash` rather than the per-tick state (ADR-0023).

## Oracles

| Oracle | Where | Changes need |
|---|---|---|
| Cross-architecture equality | `cargo xtask compare-hashes` in CI's summary job | never changes |
| `eng-math` known answers | `crates/eng-math/tests/known_answers.txt` | the owner |
| Golden vectors | `crates/*/tests/golden.rs` | the owner |
| Golden images and FLIP thresholds | `crates/game-testbed/golden/` (rung 1b's four; rung 2's at its G6), compared by `crates/game-testbed/tests/shots.rs` and `tests/shots/` | the owner |
| The shot manifest | `crates/game-testbed/shots.toml`: the shots, poses and `golden = "pending"` markers (protected from G5) | the owner |
| Reference frame records | `golden/frames/` beside the goldens (from rung 2's G6) and `crates/game-testbed/frame-tolerances.toml`, compared by `crates/game-testbed/tests/frame_record.rs` | the owner |
| Harnesses acceptance relies on | the test files `protected.toml` lists | the owner |
| Semantic assertions | `crates/game-testbed/scenarios/*.assert.toml` | the owner |
| Budgets | `budgets.toml` | the owner |
| Benchmarks (no stored baseline) | `crates/eng-bench/benches/`, the push's base against its head (G5's bench job) | over 5% needs a performance ADR the owner accepts |
| Mutation exclusions | `.cargo/mutants.toml` (G5) | the owner |
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
- **What gate G5 adds** (rung 2's xtask and workflow patches, under
  `docs/rungs/2/patches/`, which T21 assembles; step names as
  `docs/rungs/2/acceptance.json` binds them):
  - **Docs-lint** in `docs-check` (decision 16): every `cargo xtask`
    command, probe command, repository path, `ENG_*` variable and `[E-*]`
    or `[W-*]` tag that AGENTS.md, this file and the skills name must
    exist, checked against xtask's usage, the probe's (`const USAGE` in
    `crates/eng-cli/src/args.rs`), git's file list and the code. Each stale
    one fails as `[E-DOCS-STALE] <file>:<line>`; write `<name>` for a part
    that varies. The selftest `docs-stale-path` proves it.
  - **The `bench` job** (decision 14, criterion 15), on Linux x64 with
    valgrind and `gungraun-runner` 0.20.0: `cargo xtask bench [--base REV]`
    builds and measures the head and the push's base (on `main` the
    push's `before`, from `ENG_BENCH_BASE`; elsewhere the merge base with
    `origin/main`), each in its own target directory, in steps `head`,
    `base` and `bench`. A count over 5% above the base's, or a benchmark
    gone, fails with `[E-BENCH]` unless the push adds an accepted
    performance ADR naming it in backticks, read only from text an agent
    cannot have written; an excuse prints `[W-BENCH-EXCUSED]`. It is
    pending off Linux or without the tools. The report's `bench` key lists
    every benchmark's counts and verdict (the `perf-investigate` skill).
  - **The `mutants` job** (decision 15, criterion 19), in four shards
    (`ENG_MUTANTS_SHARD`): `cargo xtask mutants --in-diff` mutates the core
    crates' diff from the push's base (pending when no core crate
    changed), and `--rung-diff` the diff from the commit that added the
    current rung's `plan.md`, on the weekly schedule and a manual dispatch
    (pending on pushes). A missed mutant fails (`[E-MUTANT-MISSED]`) unless
    `.cargo/mutants.toml`, the owner's, excludes it; a `mutants::skip` in a
    core crate fails (`[E-MUTANTS-SKIP]`). Each shard has a time box
    (`budgets.toml [ci] mutants_shard_minutes`), proptest's seed is fixed,
    the summary's `mutants-shards` step needs every shard's report, and the
    `test-auditor` subagent reads `target/mutants/<step>/mutants.out/` to
    sort misses into gaps and equivalent mutants.
  - **Notices and dist** (decision 17, criteria 17 and 18; ADR-0012's
    rung 2 note): `cargo xtask notices` runs cargo-about 0.9.3 over the
    testbed's normal dependency graph on the three playable targets,
    configured by `about.toml` and `about.hbs` (protected from G5), and
    fails with `[E-NOTICES]` on a licence off `deny.toml`'s list or a crate
    missing; the lint job's `notices` step runs it, into
    `target/notices/THIRD-PARTY-NOTICES.txt`.
    `cargo xtask dist [--target T]` makes `target/dist/<target>/` and its
    reproducible `.tar.gz`: the stripped release binary, its debug info
    apart, its `version.json` and the notices (`[E-DIST]`). The native
    `dist` step, on `ubuntu-24.04`, `windows-2025` and `macos-26`, unpacks
    it and runs the binary.
  - **New steps:** `test`'s `collision`, `movement`, `sim-data`,
    `render-graph`, `sprites` and `mouse-look` (`xtask/src/ci_harness.rs`
    names each one's test files); `native`'s `poses`, `look`,
    `frame-record`, `scene-rules`, `window-room` and `dist`
    (`ci_native_fps.rs`), with `gpu-budgets` also rendering a room shot
    under `[gpu.first_person]` and checking its `pass_times` and
    `transient_textures`, `proof-video` adding `room.apng`, and
    `clippy-shim` on `macos-26`; `lint`'s `palette`
    (`crates/game-testbed-client/palettes/room.gpl` equals its script's
    output); `summary`'s `bench-duration` and
    `mutants-duration` (`[ci] bench_job_minutes`, `mutants_job_minutes`);
    `replay` reading each scenario's `map` from `scenarios.toml`, with
    the room's three scenarios and their streams. The native job turns
    Mesa's shader cache off job-wide (`MESA_SHADER_CACHE_DISABLE`), as
    the selftest job does, and uploads `frame-candidates-<runner>`,
    `scene-rules-<runner>` and, from `ubuntu-24.04`, `proof-video` (both
    videos, kept out of `report-native-<runner>`); the summary downloads
    only the `hashes-*` and `report-*` artifacts.
  - **New selftest cases:** `unsafe-outside-shim` (`arch-check` scans
    every Rust file of every workspace crate, `[E-ARCH-UNSAFE]`: the name
    `unsafe_code` only inside `deny(…)` or `forbid(…)`, and no `$` a
    macro could fill with a lint (in an attribute naming `allow`, `warn`
    or `expect`, or one that is itself a macro variable), except the
    leading `#![allow(unsafe_code)]` of the one file
    `architecture.toml`'s `unsafe_file` names, which declares no module
    or macro; no `include`, by any name; every `#[path]` naming a
    workspace crate's Rust file without a backslash, never that one, and
    none inside an inline module; thirteen plants, each matched to its
    own finding), `gcmouse-outside-shim` (`[floor]` and
    `deny.toml`'s wrapper ban), `docs-stale-path`, `mutant-missed`,
    `rule-1` to `rule-3` and `render-bug-1` to `render-bug-12`.
  - `docs/rungs/current.txt` becomes `2` in the same commit, so the
    native-steps test reads rung 2's checks only once its steps exist.
- The probe (`eng-cli`, linked by every game) runs `run`, `dump-world`,
  `shot`, `video` and `play` through the in-process session: headless
  with scripted input, offscreen for `shot` and `video`, in a window for
  `play` ("The frame"), on the map `--map` names. It writes reports,
  events, hash streams, recordings and frame records; `replay verify`
  steps the sim directly from a recording, on the map whose sim-data hash
  the recording carries.
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

- Rung 2 adds, within version 2 (ADR-0023):
  - `map`, the map's name (`dots` or `room` for the testbed's binary;
    `default` through entry points without a `GameMaps`), `sim_protocol`
    and `sim_data_hash`, both `0x` and 16 hex digits (the hash 0 for a map
    without sim data);
  - `files_read`: the format version of each file the run read, keyed as
    the stamp's `formats` names them (`input_script`, `assertions`,
    `budgets`), plus `bindings` and `look`, which the stamp cannot list;
  - for `shot` and `video`, `view`: the `kind` drawn (`top-down` or
    `first-person`), `camera_from` (`game` or `pose`), the `pose` and
    `look` asked for, and the last frame's `camera`;
  - for a first-person `play`, `window` (ADR-0022's Amendment 2):
    `fullscreen_asked`, `fullscreen`, `grab_asked`, `grab`, `grab_at_end`
    (`none`, `confined` or `locked`), `focus_lost`, `refresh_rate_mhz`
    (`null` when unknown), `frame_period_ns`, the last frame's lens
    (`fov_y_deg`, `near_m`, `upscale`) and, on macOS, the GCMouse shim's
    `callbacks`, `peak_per_second` and `focused_ms` (criterion 25).
- `budgets` holds each limit from `budgets.toml` (`null` when unset): the
  `[sim]` limits with the peaks seen, the four `[gpu]` limits,
  `first_person`, the `[gpu.first_person]` table's four (`null` without
  the table), and `exceeded`, the first excess (exit 13).
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
  before the first frame (`frames` 0). `shot` and `video` also write
  criterion 21's keys, from their last frame, the only one timed
  (`Targets::time_passes`; a video's last frame is at the last multiple
  of `--every`): `pass_times`, each pass that ran in order with its
  `nanoseconds` (`null` for a pass the GPU left untimed; the list is
  `null` without timestamp queries, and when a budget stopped the run
  before that frame), and `transient_textures`, the targets' pool size;
  `per_frame`, `tier` and the capability names stay as `shot_video.rs`
  pins them.
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
| `formats` | object | the versions, as integers, of the report, crash file, recording and hash stream the probe writes and of the input script, assertions and budgets files it reads: `report`, `crash`, `recording`, `hash_stream`, `input_script`, `assertions`, `budgets`. The bindings file that `play --bindings` reads, the look file that `--look` reads and the frame record that `shot` and `video` write (each version 1) are not listed, since `version.rs` pins the list; a run's report names the files it read (`files_read`) |

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

`sim` comes from a simulation of the game's first map built with 1
player and seed 0; the protocol version depends on neither. Other maps'
protocols and sim-data hashes are in their reports and stream headers
(ADR-0023).

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
