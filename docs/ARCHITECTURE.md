# Architecture

How the engine fits together as of rung 1a. The plan and its reasons are in
`docs/superpowers/specs/2026-10-20-ai-built-engine-design.md`; decisions in
`docs/adr/`. Keep this file short and true: update it in the commit that changes
what it describes.

## Layers

```
L0  eng-schema-core  eng-derive  eng-math  eng-core           (core set)
L1  eng-ecs  eng-net-transport  eng-net-wire*                  (core set)
    eng-schema-json  eng-input  eng-platform-headless          (std)
    eng-platform-api*  eng-platform-winit*                     (std)
L2  eng-sim  eng-collision-api  kit-fps  game-testbed-sim      (core set)
    eng-session*                                               (core set)
L3  eng-gpu*  eng-render*                                      (std)
L4  eng-cli  eng-app*                                          (std)
game  game-testbed (binary)        tool  xtask
```

\* Rung 1b crates, empty skeletons until their tasks in
`docs/rungs/1b/plan.md`.

A crate depends only on lower layers, or on same-layer crates declared in
`architecture.toml`. The **core set** is everything the simulation links,
plus the network codec, transport and session loops: `#![no_std]`, `#![forbid(unsafe_code)]`, only `libm` as a third-party
dependency (plus `eng-derive`'s build-time `syn`/`quote`), overflow checks
in release. `cargo xtask arch-check` enforces all of it over the transitive
dependency closure. The core set also builds for `wasm32-unknown-unknown`
and is tested on RISC-V under QEMU.

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

## Live input

`eng-input` also quantizes the keyboard into one `PlayerCommand` per tick
(spec 6.10). Nothing calls it yet; `eng-app`'s window will (rung 1b, T11).

```
KeyEvent (Down/Up of a Key, FocusLost) ──LiveInput::handle──▶ held keys + taps
          Bindings (strict TOML) ──LiveInput::sample(tick)──▶ TickInput { command, actions }
```

- `Key` names physical keys by their W3C `code` (`KeyW`, `ArrowUp`,
  `Escape`), so bindings do not depend on the layout. `Key` and `KeyEvent`
  are plain data; they move to `eng-platform-api` in T3.
- The caller passes the tick to `sample`; `eng-input` reads no clock.
  `quantize` does the same for a whole timeline of `(tick, event)`.
- A key counts for a tick if it is held when the tick is sampled or went
  down since the previous sample, so a tap shorter than a tick counts once.
  A `Down` for a key already held (OS key repeat) changes nothing.
  `FocusLost` releases every key. Opposite actions cancel; movement is
  full speed (127).
- App actions (`quit`) are reported in `TickInput::actions` and never
  change the command.
- Bindings files are `version = 1` plus `[actions] move_forward = ["KeyW",
  ...]`. The defaults (WASD, the arrows, Escape for `quit`) are
  `crates/eng-input/src/default.bindings.toml`. A file replaces them; an
  action it leaves out is unbound. Unknown action or key names, a value
  that is not a list of names, an unknown top-level key and a key bound
  twice fail with `[E-BINDINGS] <file>:<line>: ...`, the first in the file
  being reported; `eng_schema_json::TomlSpans` finds the line.

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
- CI's `test` job runs the workspace's tests, then, each as a step of its
  own, the harnesses whose results acceptance relies on: `known-answers`,
  `exit-codes`, `crash-json`, `version` and `live-input` (`HARNESSES` in
  `xtask/src/ci.rs`), then the replays. Each step is an entry in the job's
  report (`target/ci/report.json`, copied to `target/ci/reports/`), which
  `cargo xtask acceptance` reads.
- The probe (`eng-cli`, linked by every game) runs headless with scripted
  input, writes reports, events, hash streams and recordings, and verifies
  replays. Exit codes are listed in `AGENTS.md`. Its panic hook (in
  `exit.rs`) writes `crash-<game>-<pid>.json` beside the agent log and
  exits 12. The file holds the version stamp's `build` object (below) as
  `version`, the command line, seed, tick,
  thread, the panic's message and location, the backtrace, and the log
  file's path and last 50 lines. Only the
  first panic writes the file. Nothing in the hook may panic or wait on a
  lock for long, because a panic inside the hook aborts the process
  instead. `crates/game-testbed/tests/exit_codes.rs` runs the real binary
  for each exit code, and `crash_json.rs` uses `run --panic-at-tick` to
  check every field of the crash file.
- `<game> --version` and `<game> --version --json` print the version
  stamp (below).
- Claude Code hooks (`.claude/settings.json`) run a copy of xtask built
  from `main` into `<git dir>/eng-hooks/`: the guard (protected paths, bless
  commands), rustfmt after edits, `quick` at the end of a turn. Before the
  first commit, `install-hooks` builds from the working tree instead.
- Gates: commits go straight to `main` (ADR-0014), so the local guard is
  what stops an agent changing a protected path; it catches accidents and
  the obvious forms but runs as the agent's own OS user. CI runs every
  check on every push. If pull requests return, CI's `oracle` workflow
  (base-branch code only, owner approval or label) becomes the gate.

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
| `formats` | object | the version of each file the probe reads or writes, as integers: `report`, `crash`, `recording`, `hash_stream`, `input_script`, `assertions`, `budgets` |

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
