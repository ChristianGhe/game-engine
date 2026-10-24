# Rung 1a: Headless dot

- **Status:** built locally on 2026-10-24; awaiting the owner's approval of
  `acceptance.json` (criteria, time-box, AI budget), the GitHub setup, and
  the first CI run.
- **Spec:** section 9.1 of `docs/superpowers/specs/2026-10-20-ai-built-engine-design.md`.
- **Goal:** the fixed spine (spec 6.3) exists, is deterministic across CPU
  architectures, and is guarded by checks agents cannot fake. A dot in a 2D
  room moves headless from an input script.

## What was built

| Area | Crates and files |
|---|---|
| Foundation (L0, core) | `eng-schema-core` (Schema trait, Visitor, canonical encoding, registry, NaN finder, ENGF container), `eng-derive`, `eng-math` (scalars over libm, Vec2/Vec3/WorldPos, BAngle, units, PCG32, FNV-1a-64), `eng-core` (Tick, PlayerCommand, PlayerSlot, GenId, GenAllocator) |
| Services (L1, core) | `eng-ecs` (sparse-set world, resources, events, deferred changes, state/dump/decode), `eng-net-transport` (trait, Memory, conformance) |
| Services (L1, std) | `eng-schema-json` (JsonValue, strict authored TOML), `eng-input` (input scripts, recordings), `eng-platform-headless` (virtual clock) |
| Simulation (L2, core) | `eng-sim` (stages, ordering, step, state hash, snapshots, protocol version), `eng-collision-api` (trace contract, BoxRoom, conformance), `kit-fps` (pmove: friction, acceleration, collide-and-slide), `game-testbed-sim` (the dot) |
| Application (L4) | `eng-cli` (`run`, `replay verify`, `replay dump`, `dump-world`; reports, events, hash streams, recordings, assertions, budgets, agent logs, panic hook) |
| Game | `game-testbed` (binary), scenarios, assertions, committed hash streams |
| Tooling | `xtask`: `quick`, `ci --job`, `arch-check`, `size-check`, `docs-check`, `replay`, `compare-hashes`, `selftest`, `acceptance`, `oracle-check`, `hook guard/fmt/stop/session-start`, `install-hooks` |
| Rules | `rust-toolchain.toml` (1.99.1), workspace lints and profiles, `clippy.toml`, `architecture.toml`, `deny.toml`, `protected.toml`, `budgets.toml`, `.github/CODEOWNERS` |
| CI | `.github/workflows/ci.yml` (lint, test x64 and arm64, riscv, wasm, selftest, summary), `oracle.yml` |
| Agent setup | `AGENTS.md`, `docs/ARCHITECTURE.md`, `docs/adr/0001`-`0013`, this folder, `.claude/` (settings with hooks, `reviewer` subagent, `replay-debug` skill) |

## Where this differs from the spec, and why

1. **FNV-1a-64 and PCG32 live in `eng-math`** (as spec 6.1 says; 9.1 lists
   them under `eng-sim`, which uses them). Recorded in ADR-0005.
2. **`eng-input` is added in 1a.** The demo is "moved by an input script"
   and the probe records sessions, so input scripts and recordings need a
   home now. Live input (actions, bindings) joins it in 1b.
3. **`eng-platform-headless` is a virtual clock only.** The platform
   contract (`eng-platform-api`) appears in 1b, when winit is its second
   implementation (rule of two).
4. **`eng-collision-api` carries the trivial implementation** (`BoxRoom`,
   the inside of one box) and a conformance suite, as 9.1 asks;
   `eng-collision-grid` replaces it at rung 2.
5. **`eng-schema-json` has its own `JsonValue`** instead of exposing
   `serde_json` (rule 4: no third-party types in public APIs). `serde`
   derives on authored types are the one accepted exception; recorded in
   `AGENTS.md` and `docs/ARCHITECTURE.md`. JSON Schema export and the
   canonical TOML formatter wait for the level format (rung 3), the first
   authored format big enough to need them.
6. **Deferred to the rung that needs them:** ECS change ticks and snapping
   to wire precision (rung 5, replication); `version.json` and
   `--version --json` (1b); insta snapshot tests (no text output needs one
   yet); the skills `golden-update`, `perf-investigate`, `level-format`,
   `release-build` and the `visual-qa` and `test-auditor` subagents.
7. **Build stamps:** the engine SHA in reports and recording headers comes
   from `ENG_ENGINE_SHA`, set by CI. Local builds say `unknown`.
8. **The `add-crate` skill is not under `.claude/skills/`**: the harness
   refused that write as self-modification. Its steps are in
   `docs/ARCHITECTURE.md`, "Adding a crate"; the owner can turn them into
   the skill.

## Decisions taken while building (Claude's calls)

- `PlayerCommand.tick` is a `Tick` newtype; the encoding is identical to the
  spec's `u64`.
- The world's canonical form encodes components in dense storage order,
  because iteration order is part of the state.
- Semantic assertions are JSON pointers into the world dump with a
  comparison and a timing (`at_tick`, `by_tick`, `always`), so they work for
  any game without game code. A `by_tick` pointer that does not exist yet
  counts as "not yet true".
- Budgets in 1a are runner-independent only: state bytes and entity count.
- Recordings from another build (protocol, engine SHA, game SHA, rustc or
  tick rate differ) exit 64 with `[E-RECORDING-BUILD]`, not 11.
- The guard is defense in depth. It parses shell well enough to follow
  `cd`, subshells, heredocs, substitutions and common writers, and refuses
  write targets it cannot resolve (variables, globs) inside the repo. It
  runs as the same OS user as the agent, so a determined agent can still
  defeat it; the gate is CI's `oracle-check`, which runs only base-branch
  code (`pull_request_target`). A separate macOS user for agents (P7) would
  harden the local side.
- Reviews do not trigger `pull_request_target`, so after approving a PR
  that touches protected paths the owner re-runs the `oracle` check, or
  applies the `oracle-update` label instead.
- `pr-duration` passes when the duration was measured and recorded
  (done-when 5); going over the 15-minute target is reported as "OVER", not
  failed, so a slow runner cannot block merges.
- The determinism lints are `forbid` workspace-wide, so no `#[allow]` or
  lint group can switch them off; `arch-check` also refuses
  `extern crate std`, `#[path]`, `include!`, build scripts, crate-level
  `clippy.toml` files, `[patch]` entries and any change to libm's version,
  source or features in the core set.
- `install-hooks` builds from `main`; before the first commit it builds
  from the working tree, so a fresh repo is not locked out by the
  fail-closed guard.

## Review

Two fresh-context `reviewer` passes. The first found 4 blockers, 6
should-fix issues and 3 nits; the second confirmed those fixed (two
partly) and found 2 more blockers and 4 should-fix issues. All are fixed
except one left to the owner (the hook matcher for MCP tools, owner action
6). Details in `PROGRESS.md`.

## Verified locally (2026-10-24)

| Check | Result |
|---|---|
| Unit, property and integration tests (macOS arm64) | all pass |
| clippy `-D warnings`, rustfmt | clean |
| `arch-check`, `size-check`, `docs-check` | pass |
| Linux arm64 (Docker), `ci --job test` | pass, 48 s wall |
| Linux x86-64 (Docker, emulated), `ci --job test` | pass, 136 s wall |
| RISC-V under QEMU (Docker), `ci --job riscv` | pass, 62 s wall |
| Hash streams, 5 scenario runs | identical on macOS arm64, Linux x64, Linux arm64, RISC-V |
| `eng-math` known answers, 39 functions x 1M inputs | identical on all four |
| `selftest`, 13 deliberate violations | all caught |

Not yet verified: GitHub Actions itself, `cargo deny`, `typos`, the wasm32
check on CI, and the measured PR-tier duration (done-when 5).

## Owner actions

1. Approve or change `acceptance.json`: criteria, the 14-day time-box and
   the 15-session AI budget.
2. Approve the third-party allow-list (batch 1, 37 crates, `deny.toml`).
3. Done 2026-10-24: public repo `ChristianGhe/game-engine`, commits
   straight to `main` without PRs (ADR-0014), commit email the GitHub
   no-reply address, v2 spec kept local. An agent GitHub identity is
   deferred until it is needed.
4. Push `main` (agents are denied `git push origin main` by
   `.claude/settings.json`), then watch the first CI run.
5. Done: `cargo xtask install-hooks`. Still to do: delete
   `.claude/guard-unlock`.
6. Decide on two hardening steps the agent may not take itself, since they
   change its own permissions: add `mcp__.*` to the `PreToolUse` matcher in
   `.claude/settings.json` (MCP write tools bypass the guard today), and run
   agents under a separate macOS user (P7), the only way to stop an agent
   with your OS user from replacing the guard binary.
