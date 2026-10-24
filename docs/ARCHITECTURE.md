# Architecture

How the engine fits together as of rung 1a. The plan and its reasons are in
`docs/superpowers/specs/2026-10-20-ai-built-engine-design.md`; decisions in
`docs/adr/`. Keep this file short and true: update it in the commit that changes
what it describes.

## Layers

```
L0  eng-schema-core  eng-derive  eng-math  eng-core           (core set)
L1  eng-ecs  eng-net-transport                                 (core set)
    eng-schema-json  eng-input  eng-platform-headless          (std)
L2  eng-sim  eng-collision-api  kit-fps  game-testbed-sim      (core set)
L4  eng-cli                                                    (std)
game  game-testbed (binary)        tool  xtask
```

A crate depends only on lower layers, or on same-layer crates declared in
`architecture.toml`. The **core set** is everything the simulation links:
`#![no_std]`, `#![forbid(unsafe_code)]`, only `libm` as a third-party
dependency (plus `eng-derive`'s build-time `syn`/`quote`), overflow checks
in release. `cargo xtask arch-check` enforces all of it over the transitive
dependency closure. The core set also builds for `wasm32-unknown-unknown`
and is tested on RISC-V under QEMU.

L3 (GPU, render, audio, UI) arrives in rung 1b and later.

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

## Invariants

1. The simulation is a pure function of the initial state and the commands.
   Nothing in the core set reads a clock, a file, the OS or unseeded randomness.
2. Results are bit-identical on every target. Floats use only IEEE basic
   operations and `eng-math` (pure-Rust `libm`, pinned, `arch` off); no FMA,
   no std transcendentals, no `HashMap` iteration.
3. State is always finite.
4. A system's order depends only on stages, `before`/`after` and names.
5. Third-party types never appear in an engine crate's public API (serde
   derives on authored-data types are the one exception).

## Oracles

| Oracle | Where | Changes need |
|---|---|---|
| Cross-architecture equality | `cargo xtask compare-hashes` in CI's summary job | never changes |
| `eng-math` known answers | `crates/eng-math/tests/known_answers.txt` | the owner |
| Golden vectors | `crates/*/tests/golden.rs` | the owner |
| Semantic assertions | `crates/game-testbed/scenarios/*.assert.toml` | the owner |
| Budgets | `budgets.toml` | the owner |
| Hash streams (change detectors) | `crates/game-testbed/hashes/` | the PR, with a reason; the reviewer approves |

## Agent tooling

- `cargo xtask` is the only entry point; CI only calls it.
- The probe (`eng-cli`, linked by every game) runs headless with scripted
  input, writes reports, events, hash streams and recordings, and verifies
  replays. Exit codes are listed in `AGENTS.md`.
- Claude Code hooks (`.claude/settings.json`) run a copy of xtask built
  from `main` into `<git dir>/eng-hooks/`: the guard (protected paths, bless
  commands), rustfmt after edits, `quick` at the end of a turn. Before the
  first commit, `install-hooks` builds from the working tree instead.
- Gates: commits go straight to `main` (ADR-0014), so the local guard is
  what stops an agent changing a protected path; it catches accidents and
  the obvious forms but runs as the agent's own OS user. CI runs every
  check on every push. If pull requests return, CI's `oracle` workflow
  (base-branch code only, owner approval or label) becomes the gate.

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
