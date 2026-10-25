# AGENTS.md

An open-source Rust game engine built by AI agents, for AI agents. The plan
is `docs/superpowers/specs/2026-10-20-ai-built-engine-design.md`; how the
code fits together is `docs/ARCHITECTURE.md`; decisions are `docs/adr/`.

Current rung: see `docs/rungs/current.txt`, then `docs/rungs/<rung>/plan.md`
and its `acceptance.json`. Append what you did to that rung's `PROGRESS.md`.

## Commands

```sh
cargo xtask quick              # before every commit: fmt, clippy, arch, size, docs, tests, replays
cargo xtask ci --job lint      # CI jobs: lint test riscv wasm selftest summary all
cargo xtask arch-check         # layering and core-set rules (architecture.toml)
cargo xtask replay             # testbed scenarios vs committed hash streams
cargo xtask replay --regen     # after an intended simulation change; say why in the commit
cargo xtask selftest           # deliberate violations must fail
cargo test -p <crate>          # one crate
cargo run -p game-testbed -- run --headless --ticks 120 --seed 1 --report r.json
cargo run -p game-testbed -- dump-world --at-tick 60 --out world.json
cargo run -p game-testbed -- replay verify rec.rec
cargo run -p game-testbed -- --version --json   # build stamp, sim protocol, schema versions
```

Probe exit codes: 0 ok, 10 assertion failed, 11 desync, 12 panic,
13 budget exceeded, 64 usage. Full logs: `target/agent-logs/*.jsonl`; grep
for tags such as `[E-DESYNC]`, `[E-ASSERT]`, `[E-NAN]`, `[E-ARCH-*]`. A
panic also writes `target/agent-logs/crash-<game>-<pid>.json` (backtrace,
seed, tick, last log lines).

## Conventions that differ from usual Rust

- Units: SI meters and seconds; photometric light units (lux, nits, EV100).
- Coordinates: right-handed, Z up. Yaw 0 looks along +X; positive yaw turns
  towards +Y. Absolute positions are `WorldPos` (f64); local math is `Real`
  (f32). Renderers subtract the camera in f64 first.
- Angles in the simulation are `BAngle` (u32, a full turn is 2^32, wrapping).
  `PlayerCommand` carries u16 angles; input scripts take degrees.
- Colour is linear and scene-referred; textures are tagged sRGB or linear.
- One tick is 1/60 s. Tick 0 is the initial state; stepping with the
  commands for tick N gives the state at tick N.

## Determinism rules (core set and `game-*-sim`)

The simulation must give bit-identical results on every OS and CPU.

- Core crates are `#![no_std]` and `#![forbid(unsafe_code)]`. Their only
  third-party dependency is `libm` (pinned `=0.2.16`, `arch` off).
- Math comes from `eng-math` only: `eng_math::{sqrt, sin, min, max, clamp, ...}`.
  Never std float functions, `mul_add`, `algebraic_*`, `to_radians`, or
  approximate SIMD reciprocals. Clippy denies them (`clippy.toml`).
- No `HashMap`/`HashSet` (iteration order differs between x86 and ARM). Use
  `Vec` in index order or `BTreeMap`. Sorts need a total key with a unique
  tie-break; floats sort with `total_cmp`.
- No clocks, files, threads, OS calls or unseeded randomness in the
  simulation. Gameplay randomness is `eng_sim::GameplayRng` (PCG32).
- Simulation state stays finite. Debug builds check every float after every
  system and panic with `[E-NAN]` and a JSON pointer to the field.
- Everything that changes the simulation is a `PlayerCommand`.
- Systems are plain `fn(&mut World)` in a fixed `Stage`; order them with
  `.before()`/`.after()`, never by registration order. Changes made through
  `world.defer()` apply at the end of the stage.
- Every component, resource and event derives `Schema` with a hand-written
  stable name: `#[schema(name = "kit_fps.MoveState", version = 1)]`. Bump the
  version when the encoding changes.
- Never silence `clippy::disallowed_*` or `unsafe_code` in a core crate;
  `arch-check` fails on it.

## Crates and layers

L0 `eng-schema-core`, `eng-derive`, `eng-math`, `eng-core` < L1 `eng-ecs`,
`eng-net-transport`, `eng-schema-json`, `eng-input`, `eng-platform-headless`
< L2 `eng-sim`, `eng-collision-api`, `kit-fps`, `game-testbed-sim` < L4
`eng-cli` < `game-testbed` (binary). `xtask` is the tool layer.

- A new crate goes in `crates/<name>/` and must be listed in
  `architecture.toml` (protected: ask the owner). Follow "Adding a crate"
  in `docs/ARCHITECTURE.md`.
- Third-party types never appear in our public APIs. `serde` derives on
  authored-data types are the one exception.
- New third-party crates need the owner's approval (`deny.toml` allow-list).
  Updating the lock for crates already allowed is fine.
- Files over 500 lines warn and over 1,000 fail: split them.

## Tests and oracles

- Unit and property tests (`proptest`) live in each crate's `tests/`.
- Testbed scenarios: `crates/game-testbed/scenarios/` (input scripts,
  assertions, a manifest). Their hash streams in `crates/game-testbed/hashes/`
  are change detectors: regenerate them with `cargo xtask replay --regen`
  when a simulation change is intended, and explain the change in the commit.
- Cross-architecture equality (x86-64, arm64, RISC-V under QEMU) is always
  required. A difference there is a determinism bug, never a baseline to update.
- Debug a divergence with `dump-world --at-tick N` on both sides and diff
  the JSON (the `replay-debug` skill).
- Vision models may sort failures but never decide pass or fail.

## Protected paths

Listed in `protected.toml`: semantic assertions (`*.assert.toml`),
acceptance lists, `budgets.toml`, known-answer hashes, golden vectors
(`crates/*/tests/golden.rs`), snapshots, proptest regressions, and the check
tooling (`xtask/`, `clippy.toml`, `architecture.toml`, `deny.toml`,
`rust-toolchain.toml`, `.cargo/config.toml`, `.github/`, `.claude/`, and the
`[workspace.lints]` and `[profile]` tables of the root `Cargo.toml`).

- Agents never edit them, and never run bless/accept commands
  (`cargo insta accept`, `--bless`, `UPDATE_*=`). The guard hook blocks it.
- Need a change? Stop and propose it in your summary; the owner applies it,
  or unlocks it by writing `.claude/guard-unlock` themselves.
- With direct commits to `main` the guard is the gate (ADR-0014); CI's
  `oracle` check applies only to pull requests. The guard also refuses git
  commands that would bring an old version of a protected file back
  (`revert`, `reset`, `checkout` of a commit, ...), git settings that run
  programs, code piped into a shell, and archive extraction in the repo
  (ADR-0015). Its message says what to do instead. After each tool call
  it puts back any protected file the call changed anyway, keeping what
  was written under `target/guard-quarantine/`.
- `passes` in `acceptance.json` is computed by CI; never set it.

## Workflow

- Commit directly to `main`; there are no PRs (ADR-0014). Before each
  commit run `cargo xtask quick`; for anything non-trivial, have the
  `reviewer` subagent check the diff against the rung's acceptance list.
- Agents never push or change GitHub (`gh` writes); the owner pushes
  (ADR-0015).
- Under the open-pulsar loop (your instructions say the outer loop handles
  version control), commit nothing and only read jj: the loop records each
  task as a jj change. Finish each task with `cargo xtask quick` green.
- Keep commits small. CI runs on every push; fix a red `main` first.
- Write down costs and timings in the rung's `PROGRESS.md`.
- Nothing from this project is sold (ADR-0001): no stores, prices or paid builds.
