# Design: an AI-built game engine and the games that pay for it

- **Status:** draft v3 for the owner's review. Nothing has been built yet.
- **Date:** 2026-10-20
- **Written by:** Claude (Opus 5.5), from the brainstorming conversation with the owner and a nine-topic research pass (sources in Appendix B; versions and prices as of 2026-10-19).
- **v2:** revised after four independent reviews: a builder check, an audit against the owner's words, a fact check and a consistency review.
- **v3 (2026-10-24):** the owner decided not to sell anything. The engine is open source and is tested with personal games that are never sold, so stores, signing, revenue and the selling entity are gone (G2, 10.4). v2 is kept locally beside this file as `2026-10-20-ai-built-engine-design-v2.md` and is not published.

How to read the labels:

- **Owner decided:** the owner's own choice, quoted where possible.
- **Claude's call:** a default I chose. The owner can override any of these.
- **Pending:** needs the owner before it can be final. Section 14 collects them, each with the default this document assumes until then.

---

## 1. Summary

We are building a game engine in Rust that AI agents build and AI agents use: "a new engine, built by ai for ai" (owner). The owner sets direction and plays the builds ("I can do the testing"). Agents write the engine, its upgrades, the games and the tools ("ai should be the one that builds").

**Starting point.** The first games look and play like the first Doom (owner decided: "I would start with games like first doom game"). Underneath, the engine starts on a modern GPU pipeline so that it can grow toward the owner's end goal, "highly optimised, ultrarealistic graphics", without being replaced (Claude's call, D7-D8).

**Growth.** The engine grows by "progressive overload" (owner). I read "scaling to infinity" as "no rung rewrites an earlier one"; that reading is Claude's.

**Not for sale.** Nothing is sold (owner decided, 2026-10-24: "we will not sell anything. we will build the open source engine and test it with personal games"). The engine is open source, and the games exist to drive and test it. This replaces the earlier plan to pay for the project by "releasing games at each level".

**First game.** "The first game should be a lan game" (owner). The proposed first game is a small two-player co-op escape-puzzle game with lore that feels "familiar, but not really" (owner's phrase).

**Platforms.** Level 1 targets Windows, macOS and Linux (owner: "at least windows, mac and linux") on x86-64 and ARM64. Whether "the three main architectures" also means keeping RISC-V working is pending (P9).

**"For AI".** The engine's main user is an agent, not a person in a visual editor:
- There is no editor. Everything is code and plain text.
- The engine can run without a window, replay inputs exactly, take screenshots, dump its whole state and report in JSON.
- Its checks are built so an agent can't pass them by editing the tests.
- Within a rung, agents plan, build, check, review and merge on their own; the owner steps in only at the few points listed in 2.2 ("we need a way to make it work automatically by using llms").

---

## 2. Intent

### 2.1 In the owner's words

- "I would like to create a base for generating video games using only llms for testing and running. the end goal is to create games that run on all the main platforms (at least windows, mac and linux). I would like a game engine that is modular - it allows for improving together with hardware, software and ai"
- "I want an engine like subnautica's" (long-term target)
- "but we need a way to make it work automatically by using llms"
- "I can do the testing". This changed the opening message, where LLMs were also to do the testing. The owner now tests for feel and fun; agents still run all automated checks.
- "but ai should be the one that builds"
- "I would like a small puzzle, like an escape room that can be played in lan. first the puzzle, then multiplayer"
- "you decide, keep it simple. I would like to have a sense of deep lore that looks familiar, but not really"
- "I would like something more serious. maybe even building our own engine, in rust, brick by brick". Said right after "I don't like three.js". The owner had agreed to mockups, and also asked for no sandbox and no hosted pages ("don't use artifacts"): show things locally.
- "yes, I like this idea. a new engine, built by ai for ai. this is the goal"
- "we need the architecture to allow the three main architectures"
- "I would start with games like first doom game"
- "I would like to start low and build steadily and carefully to have an engine that can be built by agents"
- "the first game should be a lan game. but progressive overload is the word of the night :). we should build in a way that allows even built. scaling to infinity ! :))"
- "end goal is highly optimised, ultrarealistic graphics ai built engine"
- "another important part is financing. we need to be self-supported by the products we sell. we need to support ourselves through each level. like releasing games at each level"
- "ok, I get it. we will not sell anything. we will build the open source engine and test it with personal games that will not be selled" (2026-10-24, after reading that Anthropic's EU consumer terms don't allow commercial use). This replaces the financing quote above.

The owner never answered one question: should Level 1 have monsters and shooting like Doom, or stay a puzzle built Doom-style? See P6.

### 2.2 Who does what

| Role | Who | Responsibilities |
|---|---|---|
| Builders | AI agents (Claude Code today; any capable agent later), under their own GitHub identity | Write and maintain the engine, tools, tests, levels, puzzles and games. Run every automated check. Plan each rung. Act as integrator for workspace and contract changes. |
| Reviewer | A fresh-context reviewer agent plus CI | Review every PR against the rung's acceptance list. Approve regenerated hash baselines. |
| Owner (director and playtester) | The owner | See the touchpoints below. |

**Owner touchpoints** (Claude's call, kept to a minimum to honour "automatically"):

1. Play each playable rung and accept it, with a short scripted session and a proof video, delivered as local files.
2. Approve each rung's acceptance criteria and budgets once, when the rung starts.
3. Approve money spent: AI plans and hardware.
4. Approve additions to the third-party crate allow-list, in batches.
5. Approve rare changes to protected oracles: semantic assertions, acceptance lists, budgets and check tooling (7.3).
6. Set up the GitHub account or organization and the agents' GitHub identity.

**Not the owner's job by default:** integrating, reviewing code, regenerating hash baselines, posting to upstream projects (none by default; 7.7), and writing lore (pending P4).

### 2.3 What "built by AI, for AI" means in practice (Claude's call)

1. **No visual editor.** Levels, entities, puzzles, settings and lore are plain-text files and Rust code that agents read, write, diff and validate.
2. **The engine checks its own work.** Every program can run headless for N ticks from scripted inputs, and write a JSON report, per-tick state hashes, screenshots and a world dump.
3. **Same inputs, same result.** The simulation is bit-identical across operating systems and CPU architectures, so a session recorded on the owner's Mac can be checked on Linux CI.
4. **Small parts with clear rules.** Small crates in strict layers, with rules checked by machine. Several agents can then work in parallel, and parts can be swapped as hardware, software and AI improve ("modular", owner).
5. **Checks agents cannot fake.** Protected oracles, mutation testing, and a solver plus bot replays for levels. Vision models may help sort failures but never decide pass or fail.
6. **Automatic by default.** Within a rung, agents plan, build, verify, review and merge without the owner. The owner enters only at the touchpoints in 2.2.
7. **Swappable AI.** Instructions and tools are vendor-neutral: AGENTS.md, a `cargo xtask` command line, and JSON reports. A better model or agent tool can take over without starting again.

---

## 3. Goals, non-goals, anti-goals

### Goals

- **G1. Built by agents, used by agents** (owner decided; 2.3).
- **G2. Non-commercial and open source** (owner decided, 2026-10-24). The engine is public; the games are personal test games and are never sold (10.4).
- **G3. Platforms.**
  - Level 1: Windows, macOS and Linux (owner decided), on x86-64 and ARM64 (Claude's call). RISC-V compiles and is tested under emulation, pending P9.
  - The owner's end goal is "all the main platforms": consoles, mobile and web are revisited at each level start (section 12).
- **G4. Grow without rewrites** from Doom-style to ultra-realistic. "Progressive overload" and the end goal are the owner's. "No rewrites" is Claude's reading of "scaling to infinity".
- **G5. Modular** (owner decided): subsystems sit behind contracts, so backends can be swapped.
- **G6. Deterministic simulation,** checked continuously across platforms (Claude's call).
- **G7. Multiplayer that works over LAN** (owner decided: "the first game should be a lan game"). Built after the first puzzle ("first the puzzle, then multiplayer", owner). The client/server split exists from rung 1 so adding LAN is not a rewrite. Level 1 plays over LAN and direct IP only (10.2).

### Non-goals for Level 1 (Claude's call)

- Selling anything: no stores, prices, code signing, company or revenue (G2, 10.4).
- A visual editor, or support for outside users. The engine is public, but built for our own games.
- Building for consoles, mobile, VR or web. A wasm32 compile check of the core crates keeps the web possible at low cost (6.1).
- Playable builds for RISC-V, Windows ARM64, Linux ARM64 or Intel Mac. They keep compiling in CI; we just don't package them.
- Dedicated servers we host, player accounts, or a backend of our own.
- Mods and Workshop (possible later; section 12).
- Procedurally generated campaigns, and AI world models as the core of the engine.

### Anti-goals (failure even if it technically works)

- An engine with no game. The Machinery shut down in 2022 and Ambient is paused; both built engines without games driving them. Here, a personal game drives every rung.
- Tests that pass because an agent changed the test, the reference or the check.
- Game content that reads as "AI slop" instead of "familiar, but not really".
- A codebase agents can no longer navigate: one huge crate, stale instruction files, unchecked layering.
- Desyncs or non-determinism discovered late.
- Costs that outrun the owner's budget (P1), or engineering time that runs with no time-box.
- Drifting into commercial use while agents run on a consumer plan (10.4).

---

## 4. Constraints

- **Language and toolchain:** Rust, stable, edition 2024.
  - Pin the exact toolchain in `rust-toolchain.toml`: current stable is 1.99.1 (2026-10-13). The owner has 1.93.1, and rustup installs the pinned version automatically.
  - Nightly is allowed only for optional developer tools pinned to a dated nightly (Miri, cargo-fuzz, cargo-public-api), never for shipped builds.
- **Floor crates we borrow** (Claude's call). Each is wrapped by exactly one engine crate:
  - winit 0.30.13 for windows and input
  - wgpu 30.0.x for the GPU, through DX12, Metal and Vulkan
  - cpal 0.18.x for audio devices
  - gilrs 0.11.x for gamepads

  Everything above the floor is ours.
- **Development and test machines.** Development happens on the owner's Apple M2 Pro (macOS 26.5.1, Metal 4). CI tests natively on Linux x64 and arm64, Windows x64 and arm64, and macOS arm64. RISC-V gets cross-compile checks and tests under QEMU (P9).
- **Licences in the binaries we build:** an exact SPDX allow-list, enforced by cargo-deny:
  - Allowed: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, CC0-1.0, BSL-1.0 (Boost).
  - MPL-2.0 only for unmodified crates; cargo-about generates the source links.
  - CDLA-Permissive-2.0 only if rustls is used.
  - Never GPL or AGPL.
- **Non-commercial use:** the owner appears to be in the EU (EEA). Anthropic's EU consumer terms (Free, Pro and other individual plans) say "You agree that you will not use our Services for any commercial or business purposes". Nothing is sold (G2), so agents run on the owner's current plan. Before anything is ever sold, agent work moves to commercial terms: Team, Enterprise or API (10.4).
- **Owner time** is the scarcest resource. Automated checks come first, and owner touchpoints are batched (2.2).
- **Local previews** (owner preference): mockups, reports and proof videos are local files or localhost pages, not hosted pages.

---

## 5. Decisions at a glance

| # | Decision | Choice | Who |
|---|---|---|---|
| D1 | Engine | Our own, in Rust, built brick by brick | Owner decided |
| D2 | Roles | AI builds all code, tools, levels and puzzles; owner directs and playtests. Player-facing content per P4 | Owner decided (content part pending) |
| D3a | Starting point and growth | Games like the first Doom first, growing by progressive overload toward an ultra-realistic engine | Owner decided |
| D3b | No rewrites | No rung rewrites an earlier one (reading of "scaling to infinity") | Claude's call |
| D4 | First game | A LAN game; the first idea was an escape room | Owner decided |
| D5 | Not for sale | Non-commercial: an open-source engine, tested with personal games that are never sold | Owner decided (2026-10-24) |
| D6 | Floor crates | winit, wgpu, cpal (+ gilrs), each wrapped once | Claude's call |
| D7 | Graphics | GPU from rung 1 through a render graph; the Doom look is a set of passes on a linear HDR pipeline | Claude's call |
| D8 | Units and coordinates | SI and photometric units; meters; right-handed, Z-up; f64 world positions; camera-relative rendering | Claude's call |
| D9 | Simulation | Fixed 60 Hz tick; bit-identical across platforms; no_std core crates; libm-backed math; integer commands | Claude's call |
| D10 | World model | Our own small plain-data ECS with a schema registry | Claude's call |
| D11 | Netcode | Server-authoritative client/server (Quake 3 lineage) for every mode, including single-player through an in-process server | Claude's call |
| D12 | Data formats | TOML for authored data (with JSON Schema), JSON for machine output, versioned binary for cooked data and recordings | Claude's call |
| D13 | Levels and puzzles | Text sector maps plus a declarative puzzle model, compiled into engine-generic data; proven by a solver and bot replays | Claude's call |
| D14 | Agent tooling | `cargo xtask` plus a probe CLI; short AGENTS.md; protected oracles; acceptance.json per rung | Claude's call |
| D15 | CI | GitHub Actions; logic lives in xtask; five native targets plus RISC-V | Claude's call |
| D16 | Playable builds | macOS arm64, Windows x64 and Linux x64, built locally or by CI; no stores | Claude's call |
| D17 | Level 1 ladder and games | Puzzle first, then LAN. Two milestone games: R1, a 2-player co-op escape game; R2, a Doom-style co-op game | Pending (P6) |
| D18 | Online play | LAN and direct IP only in Level 1. Steam's relay would need a Steam app, so it is dropped | Claude's call |
| D19 | Player-facing content | Recommended: made by agents through our own code plus CC0 libraries; no generative image, voice or music models | Pending (P4) |
| D20 | Repos | Public open-source engine repo (MIT OR Apache-2.0) plus private game repos | Owner decided (P3, 2026-10-24) |
| D21 | R1 player count | Exactly 2 players (co-op only) | Claude's call |

---

## 6. Architecture

### 6.1 Layers and crates

The workspace is one Cargo workspace of small crates, with a glob member list (`crates/*`). Names use a placeholder prefix `eng-`.

**The core set.** These crates are `#![no_std]` (with `alloc`) and `#![forbid(unsafe_code)]`, and follow the determinism rules in 6.4. Their only third-party dependencies are `libm` (pinned exactly) and our own derive-macro crate, whose `syn`/`quote` dependencies are build-time only. `arch-check` verifies this over the transitive dependency closure. Because the core set is no_std, CI can also compile it for RISC-V and for `wasm32-unknown-unknown` at almost no cost.

| Layer | Crates | Notes |
|---|---|---|
| L0 Foundation (core) | `eng-core`: ids, generational handles, `Tick` (u64), errors, `PlayerCommand`. `eng-math`: `Real` (= f32), scalar vectors, `WorldPos` (f64), binary angles, units such as Meters, Lux, Nits and Ev100, PCG32, FNV-1a-64, libm-backed functions. `eng-schema-core`: type registry with stable names and versions, canonical binary serialization, schema walker. `eng-derive`: proc-macro crate | `eng-math` is the only math the simulation may use |
| L1 Services (core) | `eng-ecs`. `eng-net-wire`: codec. `eng-net-proto`: protocol, no I/O. `eng-net-transport`: transport trait plus the Memory and Sim implementations | |
| L2 Simulation (core) | `eng-sim`: tick driver, stages and system ordering, state hash, snapshots. `eng-collision-api`: trace and overlap traits, shapes. `eng-collision-grid`: Level-1 implementation. `eng-replication`: snapshot building, deltas, prediction and interpolation helpers. `eng-session`: server and client loops over the transport, command buffering, time sync. `kit-sectors`: Doom-style movers and sector specials. `kit-fps`: movement as a pure function. `kit-puzzle`: the declarative puzzle model (6.11). `game-<name>-sim` | No wgpu, winit or cpal in the dependency tree |
| L1 Services (std) | `eng-schema-json`: JSON dump, JSON Schema export, migrations, TOML loading. `eng-input`: actions, bindings, quantization into `PlayerCommand`, input scripts and recording I/O. `eng-assets`: ids, async handles, virtual file system, hot-reload events. `eng-net-udp`. `eng-platform-api` with `eng-platform-winit` and `eng-platform-headless`. `eng-profile`. `eng-jobs`, only when first needed | |
| L3 Presentation (std) | `eng-gpu`: the only crate that imports wgpu; owns the render graph. `eng-render`: extraction, camera-relative GPU scene, the `render-retro` pipeline. `eng-audio-api`: the sink contract. `eng-audio`: our mixer and an offline sink. `eng-audio-cpal`. `eng-ui`: text, HUD and menus | glam may be used only inside L3 crates |
| L4 Application (std) | `eng-app`: composes client, listen server and headless modes; settings, saves, crash handler. `eng-cli`: the probe command set (7.1), linked by every game binary | Only L4 and game binaries choose concrete backends |
| Games | `game-<name>-sim` (core set), `game-<name>-client`, `game-<name>` (binary) | Private game repo if the engine is public (P3) |
| Tools | `xtask`, `eng-probe` (thin binary over `eng-cli`), `eng-levelc` (level compiler and validator), `eng-cook` (asset cooker), `eng-testkit` (scenarios, bots), `eng-bench` | Nothing at runtime depends on tools |

Crates are added when a rung first needs them.

**A contract crate** contains only traits, plain data types and a conformance test suite. Contract crates exist only where Level 1 has at least two implementations, counting null and offline twins (the "rule of two"):
- transport: Memory, Sim, UDP
- platform: winit, headless
- audio sink: cpal, offline
- collision: grid, plus a parry3d-based test oracle

Platform services (Steam and the like) have no real implementation while nothing is sold, so Level 1 has no services contract (6.12).

### 6.2 Dependency rules, enforced by machine

1. **Layering:** L0 < L1 < L2 < L3 < L4 < tools. Edges between crates in the same layer are allowed only as declared in `architecture.toml`, as a per-layer graph with no cycles (for example `eng-net-wire < eng-net-proto`, and `eng-gpu < eng-render < eng-ui`). There is no grab-bag `util` crate.
2. **One wrapper per floor crate:**
   - `wgpu` only in `eng-gpu`
   - `winit` and `gilrs` only in `eng-platform-winit`
   - `cpal` only in `eng-audio-cpal`

   Enforced with cargo-deny `[bans]` wrapper lists.
3. **Core set purity:** core-set crates and `game-*-sim` never depend on std crates, presentation, L4, or GPU, window or audio crates. Enforced by `cargo xtask arch-check` over the transitive closure.
4. **No third-party types in public APIs:** they never appear in another crate's public API.
5. **Direction:** engine crates never depend on game crates. Kits never depend on each other unless declared.
6. **Contract changes:** contract crates change only through an interface-change PR with migration notes and an ADR.
7. **Third-party allow-list:** new third-party crates go on an allow-list in `deny.toml` that the owner approves in batches. Changes to `Cargo.lock` for crates already allowed are ordinary.

### 6.3 What is fixed and what can be swapped

**The fixed spine.** Changing any of these is a rewrite, so we get them right in rung 1:
- the world model and schema system
- the tick contract, stages and determinism rules
- the unit, coordinate and color conventions
- the render-graph rule that passes declare their reads and writes (the API itself grows by ADR)
- the asset ID and handle model
- `PlayerCommand`
- the network contracts in 6.8
- the state-hash function (FNV-1a-64) and the PRNG (PCG32), both frozen in rung 1a

**Swappable later:**

| Area | Path over time |
|---|---|
| GPU backend | wgpu → wgpu-hal escape hatches → a native backend for one subsystem, only if needed |
| Render pipelines | retro → clustered → visibility buffer → ray traced, chosen by settings and capability tier |
| Platform layer | winit + gilrs → SDL3 if ever needed |
| Audio | cpal device; panning → HRTF → Steam Audio |
| Transport | Memory / Sim / UDP LAN → Steam relay → others |
| Collision and physics | own 2.5D → own 3D, Rapier (`rapier3d-f64` offers cross-platform determinism) or Jolt |
| Platform services | none in Level 1 → Steam or others, if a game ever needs them |
| Game-module host | native Rust → WASM when mods ship |

### 6.4 The simulation contract

- **Tick:** a fixed 60 Hz tick (a per-game constant), a `u64` tick counter, constant dt.
  - The frame loop is a fixed-timestep accumulator with a catch-up cap, and rendering interpolates between the last two states.
  - When a listen server drops ticks at the catch-up cap, it reports it.
- **Pure step:** `(World, inputs for tick N) -> World + events`.
  - The simulation never reads a clock, the OS, the window, the GPU, audio, files or unseeded randomness.
  - GPU results never feed back into it.
- **Stages and order:** `eng-sim` defines a fixed stage enum, for example Input, PreUpdate, Update, PostUpdate and Commit.
  - Within a stage, systems declare before/after constraints. A deterministic topological sort resolves them, with name tie-breaks.
  - `probe` prints the resolved order, and the order is hashed into the simulation protocol version, so an unannounced reorder fails a test.
  - Each crate's `register()` adds systems to stages. Ordering is never implied by call order.
- **`PlayerCommand`** (in `eng-core`) is the one type that humans, agents, the network and recordings all produce:

  ```
  PlayerCommand {
      tick: u64,
      buttons: u32,          // bitfield
      forward: i8,
      side: i8,
      view_yaw: u16,         // absolute 16-bit binary angle, as in Quake 3's usercmd
      view_pitch: u16,
      select: u8,
  }
  ```

  - The simulation widens view angles to u32 binary angles (`<< 16`) and derives deltas itself.
  - Absolute angles make the missing-command policy safe: repeat the last command.
  - Quantization happens before the simulation, both live and on replay.
- **Numbers:**
  - `Real = f32` for local math.
  - f64 for absolute world positions (`WorldPos`).
  - Integers for ticks, angles (u32 binary angles, wrapping math), health, ammo, timers and IDs.
  - At the end of each tick, authoritative positions and velocities snap to wire precision, so what the network carries equals the server's state and prediction from a received baseline can match exactly.
- **Math:**
  - Use only IEEE basic operations and sqrt. Rust never fuses `a*b+c` into an FMA unless `mul_add` is called.
  - Transcendental functions come from `eng-math` over the pure-Rust `libm` crate, pinned to `=0.2.16`. Never std's `sin`, `cos`, `exp`, `powf`, `to_radians` and similar, which Rust documents as varying by platform.
  - Use our own `min`, `max` and `clamp`: std's versions may return either signed zero.
  - No `mul_add`, no `algebraic_*` methods, no approximate SIMD reciprocals.
- **Containers:**
  - Iterate only `Vec` or arena storage in index order, or `BTreeMap`/`BTreeSet`. `IndexMap` isn't needed.
  - Never iterate a `HashMap`. Its iteration order differs between x86 and ARM even with a fixed hasher (measured on the owner's machine).
  - Every sort uses a total key with a unique tie-break. Floats sort with `total_cmp`.
- **Randomness:** PCG32, with gameplay and cosmetic streams kept separate (Doom's P_Random and M_Random). Seeds come from the session header.
- **Enforcement:**
  - The core set is `no_std` and `forbid(unsafe_code)` (6.1), so std math, HashMap, clocks and threads are compile errors.
  - `overflow-checks = true` in release builds for core crates.
  - A clippy `disallowed-methods`/`disallowed-types` list in deny mode, with replacement hints. It covers what `core` still allows: `min`, `max`, `clamp`, `to_radians`, `algebraic_*`, `mul_add` and approximate intrinsics.
- **NaN policy:** simulation state stays finite. In debug builds, a schema walker checks every float field after each system. The state hasher fails with the entity and field path if it finds a NaN.
- **State hash:** FNV-1a-64 over the canonical little-endian serialization of the whole world, every tick.
- **Three kinds of oracles, kept separate:**
  1. *Cross-architecture equality.* The x64 and arm64 hash streams from the same commit must match. This needs no baseline and is always required.
  2. *Semantic assertions*, such as "the exit opens by tick 900", "puzzle solved" and "no softlock". These are protected, and only the owner approves changes (7.3).
  3. *Committed hash streams*, used as change detectors. A sim-changing PR may regenerate them in the same PR. CI attaches the field-level diff at the first divergent tick, and the reviewer agent approves.
- **Test fixtures and recordings:**
  - CI fixtures are input scripts (TOML), keyed by simulation protocol and schema version, not tied to a single build.
  - Recordings (binary, with a JSON dump) carry a header: format version, engine SHA, game SHA, simulation protocol version, rustc version, sim-data content hash, seeds, players and tick rate.
  - In Level 1 a recording is valid only for the exact build in its header.
- **CI:**
  - Replays run on Linux x64 and arm64 on every PR, and on all platforms nightly (section 8).
  - The `eng-math` known-answer tests run everywhere: about 1M seeded inputs per function, hashed.
- **Why determinism, if the network doesn't need it:** network correctness does not depend on determinism (6.8). Determinism buys exact replays, exact prediction between mixed Mac and PC peers, and reproducible bug reports.

### 6.5 World model

`eng-ecs` is our own small ECS-style world:

- **Entities** are generational IDs (u32 index + u32 generation) from a deterministic free list.
- **Components** are plain-data structs: no references, no `Box<dyn>`, and serializable through `eng-schema-core`. They are stored densely per type (sparse sets first; the layout stays private).
- **Singletons** are resources. Hierarchy and attachments are components, not a scene graph.
- **Systems** are plain functions placed in stages (6.4). There is no automatic parallel scheduling in the simulation.
- **Structural changes** go through command buffers applied at fixed sync points. Change ticks support replication.
- **Schemas:** every component and resource registers a stable name and schema version. The whole world can be snapshotted, hashed, diffed, replicated and saved. `eng-schema-json` dumps it to JSON for agents.

Not bevy_ecs: four breaking releases in 13 months, it needs Rust 1.95+, and its parallel scheduler works against determinism.

### 6.6 The presentation boundary and the frame loop

- After each tick, presentation reads an immutable `FrameSnapshot` (the previous and current state), interpolates, and produces render and audio work. The only way back into the simulation is commands.
- Single-player and listen-server play run `eng-session`'s server in-process. In Level 1 it shares the main thread with rendering, but only across this boundary. Pipelined rendering later is a scheduling change.
- Ticks are driven from the event loop's timer (`ControlFlow::WaitUntil`), not from presenting frames.
  - While the window is minimised or occluded, the engine skips surface acquisition; on macOS that call can block for up to a second.
  - While hosting on macOS, the app holds an `NSProcessInfo` activity assertion so App Nap can't throttle it.
- Dedicated threads exist only in designated crates: the winit event loop (main thread), the cpal audio callback, network I/O and asset I/O. No other crate spawns threads or reads the wall clock.

### 6.7 Rendering

- **wgpu:**
  - `eng-gpu` is the only crate that imports wgpu, pinned at 30.0.x.
  - wgpu major versions change only in the upgrade rung at the start of a level, or mid-level by ADR to fix a shipping blocker. They ship about every three months.
  - Validation errors are fatal in tests.
- **Render graph:**
  - Passes declare the virtual resources they read and write (a fixed rule).
  - Rung 1b uses a fixed two-pass graph: draw, then present.
  - Rung 2 adds ordering, culling of unused passes, transient-texture pooling, labels and offscreen targets.
  - Each render feature is a pass, so agents build passes in parallel, and a backend swap becomes a port rather than a rewrite.
- **Capability tiers:**
  - Level 1 uses only Tier 0: wgpu's default (WebGPU-core) features and limits. This reaches Steam Deck, older DX12 and Vulkan PCs, and every Metal Mac.
  - Higher tiers (binding arrays, multi-draw-indirect, ray query, mesh shaders) are optional paths for later levels.
  - Capability detection exists from rung 1b, so reports can record what the adapter supports.
  - GPU timestamp queries are used when available, with `null` otherwise. Pass timing never assumes them.
- **Color and light from day one:**
  - Shading is scene-referred, linear and HDR, into `Rgba16Float` targets. Textures are tagged sRGB or linear at import.
  - Lights use photometric units: lux, lumens or candela, and nits. Camera exposure is in EV100.
  - Tone mapping is the last graph pass. Its output goes to an engine-owned texture, and screenshots and golden tests read that texture.
  - A final fullscreen blit pass draws it into the surface's preferred format, which is often BGRA. We blit rather than copy, because swapchain textures can't be relied on for copies.
  - HDR display output comes later.
- **Coordinates:**
  - The CPU subtracts the camera position in f64 and uploads f32: camera-relative rendering.
  - Depth is reversed-Z `Depth32Float` with an infinite far plane.
  - Right-handed, Z-up, meters.
- **GPU scene layout** (fixed at rung 2): shared vertex and index megabuffers; per-instance data in storage buffers; materials as indices into tables; textures as indices into texture arrays bucketed by size. This is the GPU-driven layout later levels need.
- **Pipelines as plug-ins** over the same scene data:
  - `render-retro` (Level 1)
  - `render-clustered` (Level 2)
  - `render-visbuffer` (Level 3)
  - `render-rt` (Level 4)

  Older pipelines stay as tested low-end options.
- **The Doom look is a stack of switchable passes:**
  - Doom's light falloff per region, with optional 32-step banding
  - fake contrast by wall direction
  - coloured fog per region
  - a palette post-process: a 3D lookup table baked on the CPU from a plain-text palette using OKLab distance, with optional dither
  - optional low internal resolution with integer upscaling
  - a sky material

  Every pass can be switched off. That keeps it accessible (6.14) and lets later levels drop the look.
- **Sprites:** one instanced billboard primitive that rotates only about Z, is alpha-tested and writes depth. It picks one of 8 rotations on the CPU with Doom's integer rule, and uses mirror flags.
- **Shaders:**
  - WGSL files, hot-reloaded in dev builds.
  - CI validates every shader with naga and translates it to SPIR-V, MSL and HLSL, so a Mac-only shader fails at once.
- **When wgpu lacks a feature,** escalate one pass at a time:
  1. a wgpu feature flag
  2. a wgpu-hal escape hatch inside that pass
  3. an upstream report
  4. a native backend for that one subsystem

  Re-check at Level 3 against the known gaps: no async compute, no multi-draw-indirect-count on Metal, unfinished bindless, ray-tracing pipelines on Vulkan only, and no Metal 4 APIs.

### 6.8 Networking

**Model, for every mode:** server-authoritative client/server, in the Quake 3 / Source tradition.

- The server ticks at 60 Hz. Clients send only commands, unreliably, and each packet repeats the last 3-5 commands.
- The server sends snapshots, delta-compressed against each client's last acknowledged snapshot: 60 Hz on LAN, 20-30 Hz online.
- The client predicts only its own player. It draws everything else interpolated about two snapshot intervals in the past: about 33 ms at 60 Hz on LAN, and about 67-100 ms at 20-30 Hz online.
- Lag compensation arrives with shooting, if Level 1 includes it (P6).

**Listen server.**

- One player's game hosts.
- Single-player is the same server in-process over the Memory transport. Messages are still encoded to bytes, so there is one code path.
- No socket opens until a player picks Host or Join.
- In Level 1, the session ends when the host quits.

**Rejected:**

- Lockstep (Doom): every peer needs bit-exact results, the game runs at the slowest peer's pace, nobody can join late, and a desync is fatal.
- Rollback (GGPO): built for two-player games with small state.

**Contracts fixed in rung 1:**

1. Every message carries a server tick. On the wire, that is the low 32 bits of the u64 tick, extended against the last acknowledged tick.
2. Everything that changes the simulation is a `PlayerCommand`.
3. Generational network entity IDs, separate from ECS IDs.
4. Replicated state is kept apart from presentation-only state.
5. A per-client relevance hook (initially "everything") and per-client acknowledged baselines.
6. The protocol does no I/O: it takes `(now, incoming bytes)` and returns outgoing bytes.
7. An explicit little-endian, fixed-width wire format, and an integer protocol version in the handshake.

**Time sync and command buffering** (`eng-session`):

- The client runs ahead of the server: it targets server tick + RTT/2 + a jitter margin.
- Every snapshot reports that client's buffer slack. The client corrects by time dilation (about ±5% tick rate), or by a hard resync if it drifts too far.
- The server buffers a few ticks of commands per client, and repeats the last command when one is missing.
- The server records the command it actually applied for each player and tick. That record is the session recording, so bug-report and crash replays come from the host's buffer.

**Movers** (doors, lifts) are deterministic functions of the server tick plus replicated trajectory parameters: start tick, endpoints, speed. Clients evaluate them at the predicted tick, so a predicted player riding a lift isn't corrected.

**Transport trait:** `send(peer, bytes)` and `poll_recv() -> (peer, bytes)`, carrying unreliable, unordered datagrams of at most 1,200 bytes. `PeerId` is opaque. Implementations:

- Memory
- Sim: Memory plus seeded latency, jitter, loss, duplication, reordering, bandwidth caps and clock drift
- UDP: `std::net` + socket2 0.6

**We build:**

- the codec, with golden vectors
- packet headers: sequence, ack, ack bitfield
- the connection state machine
- reliable-ordered and unreliable channels
- fragmentation with hard caps
- snapshots, deltas, prediction and interpolation
- LAN discovery
- the simulator
- a `net decode` CLI

**We borrow:** std sockets and socket2. RustCrypto/snow only if direct internet play is ever encrypted.

**We avoid:** laminar and bincode (both unmaintained), and async runtimes.

**LAN discovery.**

- A small UDP broadcast query and reply, sent on every broadcast-capable IPv4 interface plus 255.255.255.255.
- The port is configurable and stays out of 27000-27100.
- Replies are never larger than queries. The client connects to the reply's source address.
- "Connect to IP:port" is always available.

**Security baseline, even on LAN:**

- The handshake is padded and carries magic, protocol version, game version, a hash of the *sim data* (6.11) and a client salt.
- The server answers with a stateless challenge cookie, or with Denied (version mismatch or full).
- Every packet carries a session token and a CRC32.
- Decoders never panic, and every count and length is capped.
- Network floats are checked to be finite.
- Decoders are fuzzed: proptest or bolero in CI, cargo-fuzz nightly.
- No encryption in Level 1.

**OS realities:**

- **macOS:** Local Network privacy prompts both host and joiner.
  - Builds carry `NSLocalNetworkUsageDescription`. Without the Apple Developer Program they are ad-hoc signed, so rung 5 confirms the prompt still appears for an ad-hoc signed app.
  - Explain first, then trigger the prompt on Host or Join.
  - Retry while the prompt is showing, and re-create sockets to work around known 26.x bugs.
  - Show a help screen if access is denied.
  - Dev runs from Terminal are exempt.
- **Windows:**
  - Expect a Firewall prompt on the first host.
  - Ignore `WSAECONNRESET`.
  - Set `IPV6_V6ONLY` explicitly.

**Player count:** R1 is exactly 2 players (D21). The model supports more for later games.

**Agent testing:**

- An in-process harness with a virtual clock and the Sim transport, running one server plus N headless bots, faster than real time.
- Scenario files with budgets produce JSON reports covering RTT, loss, bandwidth, corrections and interpolation underruns.
- Exact session replays.
- Real-socket loopback smoke tests on all OSes.
- A Linux network-namespace job with `tc netem` for broadcast discovery.
- Nightly soak tests of parallel 2-player sessions with 5% loss, for 30 simulated minutes, plus fuzzing.
- A Mac + PC playtest at the end of the LAN rung.

### 6.9 Audio

- **Device:** cpal 0.18.x, pinned; its next release breaks the API.
- **Mixer:** our own, running in the cpal callback. It is fed through a lock-free single-producer, single-consumer queue (rtrb) and keeps time in samples. A test fails if the callback allocates.
- **Offline mode:** renders exactly N samples per simulation tick, so tests can hash or measure the output.
- **Decoding:** start with permissive decoders, hound for WAV and claxon for FLAC. symphonia (MPL-2.0) is allowed unmodified if more formats are needed.
- **Sound events:** gameplay sounds are tick-stamped simulation events. Every gameplay-relevant sound has a caption entry (6.14).
- **Spatial audio path:**
  - Level 1: panning, distance attenuation and low-pass occlusion
  - next: HRTF (sofar)
  - Level 3: Steam Audio, optional

### 6.10 Input

- `eng-platform-winit` turns OS events into engine input events: physical key codes, raw mouse deltas, buttons, wheel, gamepad axes and focus.
- `eng-input` maps actions (with rebinding) and quantizes them into `PlayerCommand`s per tick.
- The headless platform has no window and no winit.
- **Mouse capture:**
  - Cursor lock (Locked, with Confined as the fallback).
  - Borderless fullscreen by default.
  - On macOS, a small GCMouse shim (rung 2) supplies true raw mouse deltas. winit's macOS deltas arrive coalesced, one per display refresh, which would mislead playtests on the owner's Mac.
- **Gamepads:** gilrs from rung 6, with on-screen glyphs that follow the active device.

### 6.11 Data, levels, puzzles and assets

**Authored data** (levels, puzzles, entity definitions, tuning, scenarios, settings, text tables):

- TOML 1.1, with `toml_edit` for edits that keep comments.
- Rust types are the source of truth, and a JSON Schema is exported from them.
- Parsing is strict: unknown keys are errors, except an `x_` extension namespace.
- Every file has a version field and a migration path, and a canonical formatter.

**Machine output:** JSON and JSON lines with stable key order.

**Cooked data and recordings:** one versioned binary format with a magic number, plus a JSON dump tool.

**Level source format v1 (2.5D sectors):**

- named vertices in integer millimetres
- sectors as vertex loops (outer plus holes), with flat floor and ceiling heights, materials, light and tags
- walls inferred from shared edges, with per-edge overrides
- things, triggers, doors, lifts and puzzle entities referencing stable string IDs
- large levels split into zone files

Slopes come in format v2, when a game needs them.

**The puzzle model (`kit-puzzle`)** is declarative and lives in the level files:

- entities with finite states (door: closed, open or locked; switch; keypad; lamp)
- triggers that lead to actions (use, enter a region, pick up an item, a timer in ticks)
- conditions over flags, items, counters and ordered sequences
- presence predicates: "a player is in region R" and "N players are in regions R1..Rn at the same tick"
- facts revealed in regions

The simulation runs it, and the solver searches it. Games add bespoke mechanics only by implementing the kit's transition trait, so the solver still sees them.

**Level compiler (`eng-levelc`):**

- validates polygons exactly on integer coordinates
- splits T-junctions
- triangulates (spade or i_triangle behind one function, with earcut as a fallback)
- generates walls with world-anchored UVs

It outputs engine-generic data:

- *sim data:* collision geometry, region volumes, the region adjacency graph, spawns, puzzle definitions and mover trajectories
- *presentation data:* meshes per material and tagged vertex ranges for moving planes

It also writes a JSON debug dump and an annotated automap PNG.

**Sim data vs presentation data:**

- Sim data is target-independent. It is cooked once, and every platform ships the same bytes.
- The code paths that emit it follow the determinism rules: sorted iteration, libm, no order-dependent parallelism.
- CI cooks sim data on x64 and arm64 and compares the bytes.
- The handshake's content hash covers sim data only.
- Presentation data may differ per target.

**Level validation** (JSON findings plus the automap PNG):

1. Schema and references.
2. Geometry.
3. A traversal graph built from the movement config the simulation uses.
4. The puzzle solver: breadth-first search over abstract states. Knowledge is shared between players (they talk), while actions need presence. It returns a shortest solution and lists softlocks. A co-op room must have a 2-player plan and no 1-player plan.
5. Proof by play: a headless bot follows the plan and records a replay, and CI re-runs it on every platform.
6. Accessibility rules: no puzzle depends on colour alone or sound alone, and flashing stays under 3 flashes per second.

**Collision and movement:**

- Movement is a pure function in `kit-fps`. It runs against the `eng-collision-api` trace interface (Quake 3 `bg_pmove` style): collide-and-slide against up to 5 planes, step-up and step-down, and ground snapping.
- The shape is a vertical box or cylinder.
- The rung-1 dot demo already moves through this interface, with a trivial implementation.
- `eng-collision-grid` (a uniform grid broadphase) arrives at rung 2, and a BVH at Level 2.

**Assets:**

- Sources live in git: glTF 2.0, PNG or EXR, WAV or FLAC.
- `eng-cook` is deterministic. Its cache key is `blake3(source + importer id and version + settings + target)`. Stable asset IDs are kept apart from content hashes.
- Asset handles are asynchronous from day one.
- Release builds read packed cooked data.
- Data, shaders and textures hot-reload in dev builds from rung 3.

**Text:** every player-visible string goes through string IDs and text tables.

**Provenance ledger** (from the first non-code asset). Every asset has a text sidecar recording:

- source type, and who or what made it (including which agent or generator, for levels and lore)
- tool or model and version
- licence or terms, and their date
- human edits and reviewer
- status: placeholder or final

CI blocks milestone-game builds that contain placeholder or unrecorded assets. It generates the credits and the third-party notices from the ledger.

### 6.12 Platform services

Not in Level 1. Identity, invites, lobbies, achievements and cloud saves come from a store's SDK, and nothing is sold (G2). A `PlatformServices` contract appears only when a game needs a real implementation beside the null one.

### 6.13 Performance, profiling, crashes and version stamps

- **Profiling:** `tracing` spans and the `profiling` crate's macros (Tracy, puffin or none). Every headless run writes a performance JSON: tick-time percentiles, frame time, GPU pass times when timestamps exist, and memory.
- **CI gates:**
  - Instruction-count benchmarks (gungraun, Linux) from rung 2, covering the simulation, collision, the level compiler, the cooker and the mixer. A regression over 5% fails without a performance ADR.
  - Runner-independent counters: draws, pipeline and bind-group switches, bytes uploaded.
  - Wall-clock and GPU timing only on real hardware (P7, P8).
  - Budgets per rung in `budgets.toml`.
- **Version stamp:** `cargo xtask` computes `version.json` once per build: semver, engine and game SHAs, dirty flag, commit time as `SOURCE_DATE_EPOCH`, CI run, target, rustc and profile. `--version --json` prints it.
- **Crashes:**
  - From rung 1b, a panic hook writes `crash.json` (version, thread, backtrace, last log lines, seed) and exits with code 12.
  - Native minidumps arrive in rung 7. There is no crash-reporting service: crash files stay local, and the owner hands them to agents.

### 6.14 Accessibility (checklist for each finished game, with validator rules from rung 4)

- Captions for gameplay sound cues.
- No puzzle depends on colour alone or sound alone; the validator enforces this.
- Flashing under 3 per second, plus a photosensitivity warning.
- FOV, mouse sensitivity, invert and text-size settings.
- Every Doom-look pass can be switched off.

---

## 7. Built for agents: verification and workflow

### 7.1 The probe command line

The commands live in `eng-cli`, linked by every game binary. `eng-probe` is a thin wrapper over it.

```
<game> run --headless --ticks N --seed S --input script.toml \
           --report report.json --events events.jsonl --hash-every 1
<game> replay verify <recording>
<game> shot --at-tick N --out shot.png
<game> dump-world --at-tick N --out world.json
<game> bench --scene <name>
```

- **Exit codes:**

  | Code | Meaning |
  |---|---|
  | 0 | ok |
  | 10 | assertion failed |
  | 11 | desync |
  | 12 | panic (the hook exits with it) |
  | 13 | budget exceeded |
  | 64 | usage error |

  Each code has a self-test.
- **Output:** the console prints a summary of at most about 20 lines. Full logs go to `target/agent-logs/` as JSON lines, with greppable tags such as `[E-DESYNC]`.
- **Twins:** every I/O backend has a null or offline twin.
- **Proof video:** each playable rung produces a short fixed-step proof video (frames rendered offscreen and encoded), saved as a local file for the owner.
- **Live control (after R1, unless a rung needs it):** a JSON-RPC control socket on a running game, plus an optional MCP wrapper.

### 7.2 Test layers

| Layer | Tool | From |
|---|---|---|
| Unit tests and doctests | cargo-nextest, plus `cargo test --doc` | 1a |
| Property tests (math, collision, codecs, level round-trips) | proptest, with regression files committed | 1a |
| Snapshot tests of text output | insta | 1a |
| Replays: cross-arch equality, hash change detectors, semantic assertions (6.4) | eng-cli | 1a |
| Golden images: exact on one pinned configuration, FLIP thresholds elsewhere (8.3) | nv-flip (test-only) | 1b (exact), 2 (all) |
| Conformance suites per contract crate | eng-testkit | when the contract appears |
| Fuzzing (level format, network decoders) | proptest or bolero in CI, cargo-fuzz nightly | 3 and 5 |
| Solver and bot tests for every puzzle level | eng-testkit | 4 |
| Mutation testing | cargo-mutants: a separate, sharded, time-boxed check, required only when core-set crates change; full run weekly | 2 |

Vision models are advisory only. A `visual-qa` subagent may sort failures, but never decides pass or fail and never updates a reference.

### 7.3 Protecting the oracles

**Protected:**

- semantic assertions, `docs/rungs/*/acceptance.json` and `budgets.toml`
- exact golden images and known-answer hashes
- insta snapshots, proptest regression files, codec golden vectors and scenario budgets
- the check tooling itself:
  - the `xtask` source
  - `clippy.toml`
  - the workspace `Cargo.toml` `[lints]` and `[profile]` tables
  - `architecture.toml`, `deny.toml` and `rust-toolchain.toml`
  - `CODEOWNERS`, `.github/workflows/` and `.claude/`
- the required crate-root attributes (`no_std`, `forbid(unsafe_code)`). `arch-check` verifies these against a protected manifest, with self-test fixtures.

**Not protected:** committed hash streams. They are change detectors, regenerated in the PR with a diff attached (6.4).

**Enforcement:**

1. **A Claude Code guard hook.** It's a prebuilt binary built from `main`, not compiled from the working tree, and it fails closed: any error exits with 2.
   - It blocks Edit and Write on protected paths.
   - A Bash matcher refuses writes to protected paths and refuses "bless/accept" commands (`cargo insta accept`, `--bless`, `UPDATE_*`).
2. **A separate agent identity.** Agents use their own GitHub identity (a machine user or GitHub App) with write access and no admin rights. The owner's CODEOWNERS approval therefore means something.
3. **CI checks who changed an oracle.** CI accepts a protected change only if the owner approved the PR, or the owner applied the `oracle-update` label (checked through the label event's actor).
4. **CI decides `passes`.** CI computes each `passes` field in `acceptance.json` from the test commands listed there. Agents never flip it by hand.

### 7.4 Repo memory for agents

- **`AGENTS.md`** at the root, at most about 150 lines. It holds:
  - the exact commands
  - the non-standard conventions: meters, Z-up, linear color, determinism rules
  - the protected paths and why
  - pointers to the current rung and the ADR index

  There is no codebase tour.
  - Don't commit a `CLAUDE.md`: Claude Code reads `AGENTS.md` natively when none exists. If that ever fails, fall back to a `CLAUDE.md` that contains only `@AGENTS.md`.
  - Per-crate `AGENTS.md` files only where a crate has unusual rules.
  - A CI docs-lint (from rung 2) checks that every command and path these files mention still exists.
- **`docs/ARCHITECTURE.md`**: layers, data flow and invariants, kept short.
- **`docs/adr/NNNN-*.md`**: one-page decision records. The first ones record D6-D16 and D21.
- **`docs/rungs/NN/`**: the rung plan, `acceptance.json` (feature list, test commands, CI-computed `passes`, time-box and AI-spend budget), and an append-only `PROGRESS.md` with work done, costs and timings.
- **Rustdoc** with doctests and runnable `examples/`. Public-API snapshots of contract crates via cargo-public-api, diffed only when contract crates change.

### 7.5 Claude Code setup (committed under `.claude/`)

- **`settings.json`:**
  - Allows cargo, `git add`, `git commit`, `git branch`, `git worktree`, push to non-default branches, and `gh pr`.
  - Denies push to `main`, force-push, and reading secrets.
- **Hooks** run in exec form through prebuilt xtask binaries, so they work on every OS:
  - `PostToolUse`: rustfmt on edited files
  - `PreToolUse`: the guard
  - `Stop`: `cargo xtask quick`, but only when Rust files changed since the last green run, and honouring `stop_hook_active`
  - `SessionStart`: prints the rung, the acceptance status and the last CI result

  Green is enforced at push and PR, not at every turn end.
- **Skills:** `add-crate`, `replay-debug`, `golden-update`, `perf-investigate`, `level-format`, `release-build`.
- **Subagents:**
  - `reviewer`: read-only, fresh context, checks the diff against the rung's acceptance list, and approves regenerated hash streams
  - `visual-qa`
  - `test-auditor`: runs cargo-mutants on the diff
- **The rust-analyzer LSP plugin.**

All verification logic lives in `cargo xtask`, so the harness survives a change of agent tool.

### 7.6 Parallel agents

- Start modest: one main session plus 1-3 workers in git worktrees, each owning a set of crates listed in the rung plan.
- The interface PR (traits, conformance tests and a null implementation) merges first.
- Workers may change `Cargo.lock` and add crates under `crates/`. They don't edit contract crates, `deny.toml` or protected paths. An integrator agent session makes those changes.
- Merges go through CI one at a time. Private repos require branches to be up to date, because merge queue there needs Enterprise.
- sccache shares dependency builds.

### 7.7 Rhythm, security and upstream policy

- **Deload rungs** (Claude's proposal, extending the owner's "progressive overload" metaphor): after every 3-4 rungs and before each milestone game is finished, a short rung with no new features.
  - It cleans up duplicate code, refreshes docs and ADRs, audits instruction files and pays down performance debt.
  - It bumps only non-floor dependencies and patch versions.
- **Time-boxes:** every rung has a time-box and an AI-spend budget in its `acceptance.json`, reviewed at each deload.
  - **Pivot rule:** if rungs 1a-3 together take more than twice their time-boxes, cut R1's scope: fewer rooms, and no controller glyphs.
- **Security on the owner's Mac** (P7):
  - Agents and any self-hosted runner run under a separate macOS user or a VM, with no access to the owner's keychain or browser sessions.
  - The runner triggers only on pushes to `main` and on schedules.
  - Tokens and other secrets live only in CI secrets or in the agents' macOS user.
- **Upstream:** by default we don't post to upstream projects. wgpu rejects AI-authored commit trailers, and Bevy, Godot and rust-lang restrict AI-written contributions. Fixes we need live as `[patch]` forks with a tracking note and a test. The owner may choose to report issues personally.
- **Cost tracking:** AI token spend and CI minutes are logged per rung in `PROGRESS.md`.

---

## 8. CI and builds

### 8.1 Repos (owner decided, P3)

- The engine and its agent tooling live in a **public** repo under MIT OR Apache-2.0 (owner decided, 2026-10-24). Public repos get free standard runners on every target we need, including macOS and ARM64, and RISE's free native RISC-V runners.
- The engine repo keeps a public **testbed game** with test levels that exercise every feature, so engine CI sees real content.
- Each personal game lives in a **private** repo (owner decided, 2026-10-24). Its CI:
  - Linux x64 and arm64 on every PR
  - macOS on the owner's runner (P7)
  - the full matrix nightly and on tags
  - a nightly canary against engine `main`
  - the engine pinned by commit within a level
- Branch protection and CODEOWNERS enforcement on a private repo need a paid GitHub plan. With nothing to sell, a private game repo is optional: the games could live in the public repo at no cost (P11).

### 8.2 Tiers and matrix

- **The only entry point is `cargo xtask`.**
  - Workflows check out the code, restore the cache and call xtask.
  - Every job writes `target/ci/report.json` (version, target, runner, GPU adapter and driver, OS version, test counts, hashes, counters), JUnit and screenshots.
  - Actions are pinned by SHA; `permissions: {}`; `--locked`.
- **Every PR.** The target is under 15 minutes, measured from rung 1a.
  - fmt and clippy, with `-D warnings` in CI only
  - nextest and doctests
  - cargo-deny
  - arch-check and the file-size check (warn over 500 lines, fail over 1,000)
  - replays with the cross-architecture check on Linux x64 and arm64
  - typos
  - the RISC-V cross-build plus QEMU tests of the core set
  - the `wasm32-unknown-unknown` compile check of the core set
- **Native jobs, from rung 1b.** These run on every PR if the repo is public, otherwise on merge and nightly. Each builds, tests, renders headless to PNG, and opens a window for N frames (under xvfb on Linux), with audio going to a null sink.
  - `ubuntu-24.04`
  - `ubuntu-24.04-arm`
  - `windows-2025`
  - `windows-11-arm`
  - `macos-26`, plus `cargo check --target x86_64-apple-darwin` (Intel Mac is compile-checked only)
- **Nightly:**
  - the full matrix with cross-platform replay equality
  - performance benchmarks
  - the weekly mutation run
  - a coverage floor that cannot drop
  - native RISE RISC-V runs (public repo only)
- **Deferred until after R1, unless a rung's acceptance list needs it:** Miri replays, reproducible-build diffs, GPU capture tooling.

### 8.3 GPU tests without GPUs

| Platform | Adapter | Image comparison |
|---|---|---|
| Linux | Mesa lavapipe/llvmpipe, pinned to the build wgpu's CI used for our wgpu version. For wgpu 30.0.x that is Mesa 25.2.7 (gfx-rs/ci-build build26); wgpu's trunk has moved to Mesa 26.1.3, so switch at the next wgpu upgrade. | Exact golden on `ubuntu-24.04` x64 only, with the llvmpipe environment fixed (for example `LP_NATIVE_VECTOR_WIDTH`). Linux arm64 uses FLIP thresholds. |
| Windows | WARP from the pinned NuGet package | FLIP thresholds |
| macOS | the runner's paravirtual Metal device (verified on macos-14; confirm on macos-26 in rung 1b) | FLIP thresholds |
| The owner's Mac | real M2 Pro GPU | FLIP thresholds, because Metal shader compilers change with macOS updates. OS and driver versions are recorded, so a post-update mismatch is recognised as such. |

All image tests use fixed sizes, a fixed frame index, seeded noise and no temporal jitter. Prefer structured assertions where possible: an object-ID buffer, depth probes, luminance histograms and draw counts.

### 8.4 Builds for playing (rung 7 and later)

Nothing is sold, so there is no store pipeline, no code signing and no notarization (G2). Each milestone game gets builds that the owner and invited playtesters can run for free.

**macOS arm64**
- A `.app` bundle, ad-hoc signed, built on the owner's Mac or by CI.
- macOS quarantines downloaded apps that lack a Developer ID signature, so a CI-built app needs the quarantine flag removed before it opens (`xattr -dr com.apple.quarantine <app>`). Apps built on the Mac itself don't.

**Windows x64**
- Build with `+crt-static`, the Windows subsystem, and icon and version resources.
- Unsigned, so SmartScreen warns on first run, and PCs with Smart App Control turned on block it. Playtesters get written steps.

**Linux x64**
- A tarball built against an old glibc baseline, so it runs on most distributions.

**Delivery and housekeeping**
- Builds come only from `v*` tags in the game repo; only the owner creates tags.
- Ship stripped binaries, and archive symbols and `version.json` per build.
- Generate third-party notices with cargo-about.

---

## 9. The Level 1 ladder (Claude's proposal, pending P6)

This ladder is Claude's proposal. An earlier version was played back to the owner during brainstorming; the owner didn't object, but didn't confirm it either. This version follows the owner's explicit order, "first the puzzle, then multiplayer". The client/server split exists from rung 1b, so LAN is an addition, not a rewrite. Monsters and shooting (rung 8) and the Doom-style R2 answer a question the owner never answered, so they are pending too.

Each rung ends with something checkable, and most with something playable. Each passes CI and adds to the previous rung without rewriting it. Each rung gets its own short plan, with a time-box and an AI budget, when it starts. Rungs 1a and 1b are detailed enough below to plan from.

| Rung | Name | Done when (summary) |
|---|---|---|
| 1a | Headless dot | See 9.1 |
| 1b | Windowed dot | See 9.2 |
| 2 | Inside the room | First-person Doom-style view of a simple room. Movement and collision through `kit-fps` and `eng-collision-grid`. The render graph grows (ordering, culling, pooling). The retro pipeline and look stack. Billboard sprites. The GCMouse shim. Goldens on every adapter. gungraun benchmarks, docs-lint, cargo-about and dist, mutation testing. |
| D1 | Deload | 7.7 |
| 3 | Levels from text files | Level format v1, its schema and canonical formatter. The compiler and validation tiers 1-3 with automap PNGs. Doors and lifts as deterministic movers. Sim data vs presentation data, with the byte-equality check. Hot reload. Fuzz maps. |
| 4 | The puzzle (single-player) | `kit-puzzle` with keys, switches, keypads, sequences and lamps. Solver and bot replays in CI. Validator accessibility rules. A first solo puzzle room playable end to end. |
| 5 | Multiplayer on LAN | UDP transport, discovery, handshake, time sync and command buffering, prediction and interpolation, delta snapshots and quantization. Co-op presence predicates and the solver's 2-player check. Reconnect. Session recording. macOS and Windows permission handling. A host-minimized test (30 s, joiner sees no stall). The Linux netns job. A lift ride at 150 ms RTT with zero corrections. Mac + PC playtest. |
| 6 | Sound and controls | Our mixer on cpal, offline render tests, positional tick-stamped events and captions. gilrs controllers with glyphs. Versioned saves. |
| D2 | Deload | |
| 7 | Finish R1 | Menus, settings and rebinding. Minidumps. The accessibility checklist. The remaining rooms (4-6 in total) and polish. Playable builds (8.4). **R1 finished:** the owner plays it through with a second player. |
| D3 | Deload | |
| 8 | Monsters and shooting (pending P6) | Monster AI, weapons, damage, lag compensation, replication budgets, simple relevance. |
| 9a | R2 content | The Doom-style game's levels and lore. |
| D4 | Deload | |
| 9b | Finish R2 | Polish and playable builds. **R2 finished.** |

### 9.1 Rung 1a: Headless dot

**Owner gates before it starts:** P3 (answered 2026-10-24) and the GitHub accounts (2.2). ADR-0001 records P3 and the non-commercial rule (G2).

**Builds:**
- **Repo setup:** the repo and the agents' GitHub identity; the pinned toolchain; `Cargo.lock`; workspace lints; profiles (opt-level 1 for our crates, 3 for dependencies in dev; overflow checks for the core set).
- **`xtask` subcommands:** `quick`, `ci`, `arch-check` (layers, the core set's transitive closure, crate-root attributes against the protected manifest), `size-check`, and `hook guard`, built as a prebuilt binary.
- **Dependency rules:** `deny.toml` with the licence and crate allow-lists and the floor wrapper bans.
- **Core crates:** `eng-core`, `eng-math` (with known-answer tests), `eng-schema-core`, `eng-derive`, `eng-ecs`, `eng-sim` (stages and ordering, FNV-1a-64 hash, PCG32), `eng-net-transport` (trait plus Memory), `eng-collision-api` with a trivial implementation, and `kit-fps` (pure movement). Also `eng-platform-headless`, `eng-cli` (`run`, `replay verify`, `dump-world`) and `eng-schema-json`.
- **Demo:** a headless dot in a 2D room moved by an input script.
- **CI:** Linux x64 and arm64, RISC-V QEMU, the wasm32 check.
- **Agent setup:** `AGENTS.md`, `docs/ARCHITECTURE.md`, the first ADRs, `docs/rungs/1a/` (plan, acceptance, budget), `PROGRESS.md`, CODEOWNERS and `.claude/`.

**Done when:**
1. The input-script replay produces identical per-tick hashes on Linux x64 and arm64, and under RISC-V QEMU.
2. The `eng-math` known-answer hashes match everywhere.
3. Deliberate violations fail, each with a self-test:
   - `f32::sin` in a core crate
   - a core crate depending on a std crate
   - removing `#![no_std]`
   - editing a protected file without unlocking it
   - a "bless" command
4. Every exit code has a self-test.
5. The PR tier's duration is measured and recorded.
6. `AGENTS.md` is at most 150 lines.

### 9.2 Rung 1b: Windowed dot

**Builds:**
- **Window and GPU:** `eng-platform-winit`. `eng-gpu` with capability detection, the fixed two-pass graph (draw, then a blit into the surface's preferred format) and offscreen readback. `eng-render` with the top-down pass, kept later as the automap and debug view.
- **Session:** `eng-session` running single-player through the in-process server over Memory, so the app loop already has its final shape. `eng-app`.
- **Probe:** `shot` and `--version --json`; the panic hook and `crash.json`; the proof video.
- **CI:** the five native jobs (8.2) and exact goldens on the pinned Linux configuration, plus FLIP elsewhere.

**Done when:**
1. CI is green on all five native targets.
2. On the owner's Mac the demo opens a window and the dot moves; Esc quits.
3. `shot` matches the exact golden on the pinned configuration, and FLIP thresholds elsewhere.
4. A panic writes `crash.json` and exits with 12.
5. The owner plays it and accepts it.

---

## 10. Level 1 games

### 10.1 The milestone games (pending P6)

The games exist to drive and test the engine, and they are never sold (G2). Each level still ends in finished games, so the engine always has real content to serve. The labels R1 and R2 are kept from v2.

- **R1, a small co-op escape-puzzle game**, finished at rung 7. This matches the owner's first idea.
  - Exactly 2 players (D21), with 4-6 handcrafted rooms, full sound and no combat.
  - Played over LAN or direct IP, on macOS arm64, Windows x64 and Linux x64 (8.4).
- **R2, a Doom-style co-op game**, finished at rung 9b, if the owner confirms monsters and shooting for Level 1. It has puzzles, monsters and lore in the same universe.
- Builds may be given to friends for free, for co-op playtests. Taking money for them would make the project commercial (10.4).

### 10.2 Online play

Level 1 plays over LAN and "Connect to IP:port" only. Steam's relay network would need a Steam app, and a Steam app needs store onboarding, so it is dropped (D18). Internet play works through direct IP and a forwarded port. If a game later needs easier online play, the transport is swappable, and a relay is about one rung of work.

### 10.3 Content and AI (pending P4)

**What the owner said:**
- "a base for generating video games using only llms"
- "ai should be the one that builds"
- "you decide" (on the escape room and its lore)

With nothing sold, Steam's AI-disclosure rules and the sales risk of visible AI content no longer apply. What still matters: purely AI-generated material has weak copyright protection in the US and EU, and assets in a repo need a clear licence.

| Option | What it means | Cost to the owner |
|---|---|---|
| **A (recommended)** | Agents make levels, puzzles and lore. Art and sound come from procedural generators that agents write (textures, low-poly models rendered to sprites, the "almost Latin" script font, synthesized sound effects and ambience), plus CC0 libraries (Poly Haven, ambientCG). No generative image, voice or music models. Everything can be rebuilt from the repo. The owner approves the lore direction, not the writing. | None |
| **B** | As A, plus generative image and audio models, recorded in the provenance ledger. Each model's own terms must allow the use. | Model subscriptions |
| **C** | Humans make the art, sound and lore: the owner, friends or free asset packs. | Owner time |

### 10.4 Staying non-commercial

- **Why:** the owner appears to be in the EU. Anthropic's consumer terms for the EU and Switzerland (Free, Pro and other individual plans; effective 2025-10-08) cover someone "acting wholly or mainly outside your trade, business, craft or profession", and say "You agree that you will not use our Services for any commercial or business purposes". Agents run on the owner's current plan, so the project stays non-commercial (owner decided, 2026-10-24).
- **What that rules out:** selling games, paid access or paid builds, and any other income from the project.
- **What it allows:** an open-source engine that anyone may use under its licence, personal games, and free builds for friends.
- **If that ever changes:** move agent work to commercial terms (Team, Enterprise or API) before anything is sold, then bring back the business plan from v2 (stores, signing, selling entity, revenue gates).
- **Costs** are the owner's own: the current Claude plan, free public CI, and optionally a paid GitHub plan (8.1, P11) and test hardware (P8). AI usage is still logged per rung in `PROGRESS.md`, against a cap the owner sets (P1).

---

## 11. Creative direction for Level 1 (Claude's call; the owner said "you decide, keep it simple")

- **One universe for every game**, so the lore carries over.
- **"Familiar, but not really":**
  - Everyday places and objects with one consistent difference: a 10-hour clock with unfamiliar numerals, a 13-month calendar, a script that looks almost Latin, a coastline that is almost a real one.
  - Lore is discovered through objects and puzzles, never explained.
  - Puzzles teach the player something about the world.
  - A hidden lore bible lives in the game repo; each game shows fragments of it.
- **Authorship of lore** follows P4. Under option A, agents write it and the owner approves its direction.
- **R1 seed concept:** the rooms of a lighthouse keeper's station that now stands under the sea.
  - The clock teaches the numerals.
  - A logbook gives a code.
  - A sea chest yields a lamp lens.
  - Lights out in the dark water answer the lamp with a sequence that opens the hatch.
  - Co-op puzzles separate the players physically: one can see the clue, the other can act on it.
- **Look:** the Doom look stack on the modern pipeline, with strong lighting and fog. Assets come per P4. Sprites are rendered from 3D sources for consistent rotations.
- **Legal hygiene:** draw on public tropes, not protected works. SCP texts are share-alike, and specific Backrooms works and the Doom and id trademarks are protected. Never prompt with names of existing franchises or artists.

---

## 12. Later levels (Claude's call: a sketch; nothing here is decided)

- **Level 2, Quake-like full 3D:**
  - A brush source format (Valve 220 `.map`) compiled into the same engine data. A BVH broadphase.
  - An `eng-physics` contract with a chosen implementation.
  - The clustered renderer, glTF models and skeletal animation.
  - Area-based relevance in snapshots.
  - A small game early in the level and a bigger one at its end.
- **Level 3, open worlds (Subnautica-like):**
  - Streaming, plus terrain and water kits.
  - The visibility-buffer renderer.
  - Interest management and bandwidth budgets.
  - Authority handoff for physics objects near players.
  - Possibly WASM mods.
  - Re-check the wgpu ceiling and budget any native backend here.
- **Level 4+, ultra-realistic** (the owner's end goal):
  - Ray-traced global illumination and shadows, with a software fallback.
  - Neural upscaling and denoising, and HDR output.
  - Native GPU backends where wgpu falls short.
- **Every level** starts with an upgrade rung (toolchain and floor majors). It revisits platforms toward the owner's "all the main platforms": consoles, mobile and web. It finishes at least one game.

---

## 13. Risks

| Risk | Mitigation |
|---|---|
| Engineering overruns | Rung time-boxes, the pivot rule, the rule of two for seams |
| Drifting into commercial use while agents run on a consumer plan | G2 and ADR-0001 record the rule; commercial terms before anything is sold (10.4) |
| Agents produce wrong-but-running behaviour or game their checks | Protected oracles and tooling, fail-closed guard, agent identity, CI-computed `passes`, mutation testing, solver and bots |
| Determinism drifts | no_std core set, lint bans, pinned libm, cross-architecture replays, sim-data byte equality |
| wgpu churn and ceiling | Upgrade only at level starts, Tier-0 baseline, escape hatches, render-graph seam, a planned re-check at Level 3 |
| OS permission quirks (macOS Local Network and Gatekeeper, Windows Firewall, SmartScreen and Smart App Control) | Permission UX and help screens, written steps for playtesters (8.4) |
| Weak copyright on AI-made content | P4 options, provenance ledger |
| AI and CI costs | Free public CI, AI usage logged per rung, a cap the owner sets (P1) |
| Supply-chain compromise via build scripts on the owner's Mac | Separate macOS user or VM, crate allow-list, runner only on `main`, secrets only in CI |
| Owner time | Batched touchpoints (2.2), proof videos, reviewer agent for routine approvals |
| No second playtester for co-op | P5; free builds for friends (10.1) |
| Bus factor | Everything lives in the repo: ADRs, rung plans, PROGRESS.md |

---

## 14. Pending owner decisions

| # | Question | Default assumed here |
|---|---|---|
| P1 | How much can you spend on the project per month (AI plan, GitHub, hardware)? | Your current Claude plan and nothing else |
| P2 | Selling entity and country; company or individual; which Claude plan agents run on. **Gate for rung 1a.** | **Resolved 2026-10-24:** nothing is sold, so there is no selling entity. Agents run on the owner's current plan (10.4). |
| P3 | Engine repo public (open source) with private game repos, or everything private? Which licence? **Gate for rung 1a.** | **Owner, 2026-10-24:** public engine under MIT OR Apache-2.0; private game repos |
| P4 | What may AI make that players see, hear and read? (10.3) | Option A |
| P5 | Your time: hours per week. Can you review Rust code, or only play builds and watch videos? Is there a second person for co-op playtests? | A few hours a week; builds and videos only; a friend for co-op playtests from rung 5 |
| P6 | Ladder and games: puzzle first, then LAN; R1 a 2-player co-op escape game over LAN and direct IP; R2 a Doom-style co-op game with monsters and shooting. Keep, change, or drop R2's combat? | As proposed |
| P7 | May agents run unattended (overnight) on your Mac, under a separate macOS user? May your Mac double as a test and benchmark runner? | Yes, separate user; runner only for `main` and nightly |
| P8 | Buy a Windows PC (dual-boot Linux) for the Mac + PC playtest at rung 5? A Steam Deck? | A PC only if you have none; no Deck. CI covers Windows and Linux. |
| P9 | "The three main architectures": the CPU types (Intel/AMD, ARM, RISC-V) or the three operating systems? | CPU types. The other reading drops the RISC-V work. |
| P10 | Is there a date you want R1 finished by? | No fixed date; rung time-boxes |
| P11 | With nothing to sell, keep the games in a private repo (a paid GitHub plan for branch protection), or put them in the public engine repo for free? | Private, per P3 |

---

## 15. Left to the builder

- Final crate and binary names (`eng-` is a placeholder), and the module layout inside crates.
- Exact CI YAML, cache keys and job sharding.
- The triangulation crate (spade or i_triangle).
- The tone-mapping curve (AgX, Tony McMapface or Khronos PBR Neutral).
- FLIP thresholds per test.
- Starting budgets per rung, taken from the research numbers and approved by the owner at rung start.
- How rung plans split into tasks and agents.
- The discovery port number, exact snapshot rates within the ranges above, and redundancy counts.
- The wire quantization steps for positions and velocities.

---

## Appendix A: Versions as of 2026-10-19

| Item | Version | Note |
|---|---|---|
| Rust stable | 1.99.1 (2026-10-13) | owner has 1.93.1 |
| winit | 0.30.13 | 0.31 has been in beta for 11 months; move only when stable |
| wgpu | 30.0.1 | MSRV 1.87; breaking majors about every 3 months |
| Mesa for lavapipe CI | 25.2.7 (gfx-rs/ci-build build26) | matches wgpu v30.0.x's CI |
| cpal | 0.18.2 | next release breaks the API |
| gilrs | 0.11.2 | |
| libm | 0.2.16 | pin exactly |
| socket2 | 0.6.5 | |
| toml / toml_edit / schemars | 1.1.8 / 0.25.x / 1.2.3 | |
| cargo-nextest / cargo-deny / cargo-about / cargo-mutants | 0.9.148 / 0.20.2 / 0.9.2 / 27.1.0 | |
| gungraun | 0.20.0 | successor to iai-callgrind |
| nv-flip | 0.1.2 | crate MIT/Apache-2.0/Zlib, plus BSD-3-Clause through nv-flip-sys; unchanged since July 2023; test-only. Avoid dssim (AGPL-3.0). |

## Appendix B: Key sources

- **Rust float semantics and determinism:** RFC 3514; std f32 docs; the Box2D determinism post (box2d.org, 2024); Rapier's determinism guide.
- **wgpu:** CHANGELOG and CI workflows (github.com/gfx-rs/wgpu, including `.github/actions/install-mesa` at v30.0.1); winit issue #4581 (macOS mouse deltas).
- **Networking:** Gaffer on Games; the Quake 3 network model (fabiensanglard.net); Apple TN3179 (Local Network privacy); Steamworks networking and Steam Datagram Relay docs.
- **GitHub:** Actions billing and runner docs; environments and deployment protection rules (required reviewers in private repos need Enterprise).
- **Agents:**
  - Anthropic, "Building a C compiler with a team of parallel Claudes" (2026-02-05) and "Effective harnesses for long-running agents" (2025-11-26)
  - the ETH AGENTS.md study (arXiv 2602.11988)
  - ImpossibleBench (arXiv 2510.20270)
  - VideoGameQA-Bench (arXiv 2505.15952)
  - GameLogicBench (arXiv 2609.21562)
- **Business:**
  - Steamworks docs: app fee, onboarding, content survey, Deck compatibility, Next Fest
  - howtomarketagame.com 2026 data
  - US Copyright Office Part 2 report (2025-01-29)
  - Anthropic Commercial and Consumer Terms
  - Microsoft's Smart App Control FAQ
- **Doom tech:** id-Software/DOOM linuxdoom-1.10 source; GZDoom shaders; DSDA-Doom build-mode docs.
