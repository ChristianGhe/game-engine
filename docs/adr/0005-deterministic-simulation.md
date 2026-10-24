# ADR-0005: Deterministic simulation

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D9 (spec section 6.4; goal G6)
- **Who:** Claude's call

## Context

A session recorded on the owner's Mac should check out on Linux CI. Network correctness does not depend on determinism (6.8). Determinism buys exact replays, exact prediction between mixed Mac and PC peers, and reproducible bug reports.

## Decision

The simulation is bit-identical across operating systems and CPU architectures.

- **Tick:** fixed 60 Hz (a per-game constant), `u64` counter, constant dt. The frame loop is a fixed-timestep accumulator with a catch-up cap.
- **Pure step:** `(World, inputs for tick N) -> World + events`. No clocks, OS, window, GPU, audio, files or unseeded randomness. GPU results never feed back.
- **Stages:** a fixed stage enum in `eng-sim`. Systems declare before/after constraints. A deterministic topological sort resolves them, with name tie-breaks. The resolved order is hashed into the simulation protocol version.
- **`PlayerCommand`** (in `eng-core`) is the one input type for humans, agents, the network and recordings. View angles are absolute 16-bit binary angles.
- **Math:** basic IEEE operations and sqrt only. Transcendentals come from `eng-math` over `libm`, pinned `=0.2.16`. No `mul_add`, no `algebraic_*`, our own `min`, `max` and `clamp`.
- **Containers:** `Vec`, arenas, `BTreeMap`, `BTreeSet`. Never iterate a `HashMap`. Sorts use a total key with a unique tie-break.
- **Randomness:** PCG32, separate gameplay and cosmetic streams.
- **State hash:** FNV-1a-64 over the canonical little-endian serialization of the whole world, every tick.
- **Enforcement:** core crates are `#![no_std]` and `#![forbid(unsafe_code)]`; `overflow-checks = true`; clippy `disallowed-methods` and `disallowed-types` in deny mode.
- **Oracles, kept separate:** cross-architecture equality (always required), protected semantic assertions, and committed hash streams as change detectors.

**Implementation note.** Rung 1a puts FNV-1a-64 and PCG32 in `eng-math`, as section 6.1 says. Section 9.1 lists them under `eng-sim`, which uses them for the state hash and the gameplay RNG stream. Both are frozen from rung 1a.

## Consequences

- Core crates must build for RISC-V and `wasm32-unknown-unknown` at almost no cost.
- Recordings are valid only for the exact build in their header in Level 1.
- A sim-changing PR may regenerate hash streams; the reviewer agent approves the diff.
- No automatic parallel scheduling in the simulation.

## Revisit when

No trigger planned; change only by a new ADR. The hash function and PRNG are frozen from rung 1a.
