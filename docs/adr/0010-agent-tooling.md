# ADR-0010: Agent tooling

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D14 (spec sections 7.1-7.5)
- **Who:** Claude's call

## Context

The engine's main user is an agent. The owner wants it to "work automatically by using llms", and also that "ai should be the one that builds". Agents must be unable to pass checks by editing the checks. The tooling must survive a change of agent vendor.

## Decision

`cargo xtask` plus a probe CLI, a short `AGENTS.md`, protected oracles and a per-rung `acceptance.json`.

- **Probe CLI** (in `eng-cli`, linked by every game binary; `eng-probe` is a thin wrapper): `run --headless`, `replay verify`, `shot`, `dump-world`, `bench`. Exit codes: 0 ok, 10 assertion failed, 11 desync, 12 panic, 13 budget exceeded, 64 usage error. Each has a self-test. Console output is about 20 lines at most; full logs go to `target/agent-logs/`.
- **Twins:** every I/O backend has a null or offline twin.
- **Repo memory:** `AGENTS.md` (at most about 150 lines, no codebase tour, no committed `CLAUDE.md`), `docs/ARCHITECTURE.md`, `docs/adr/`, and `docs/rungs/NN/` with `acceptance.json` and an append-only `PROGRESS.md`.
- **Protected oracles:** semantic assertions, acceptance files, budgets, golden images, known-answer hashes, snapshots, codec vectors, and the check tooling itself. Committed hash streams are not protected.
- **Enforcement:** a fail-closed guard hook built from `main`; a separate agent GitHub identity without admin rights; CI checks who changed an oracle; CI computes every `passes` field.
- **Claude Code setup** under `.claude/`: settings, hooks (rustfmt, guard, `cargo xtask quick`, session start), skills, and the `reviewer`, `visual-qa` and `test-auditor` subagents. All verification logic lives in `cargo xtask`.
- Vision models are advisory only. They never decide pass or fail.

## Consequences

- A better model or agent tool can take over without starting again.
- Changes to protected paths need the owner's approval or the `oracle-update` label (the owner's touchpoint 5).
- Workers do not edit contract crates, `deny.toml` or protected paths; an integrator session does.

## Revisit when

No trigger planned; change only by a new ADR. Live control (a JSON-RPC socket, optional MCP wrapper) is considered after R1.
