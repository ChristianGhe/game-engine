# ADR-0008: Data formats

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D12 (spec section 6.11)
- **Who:** Claude's call

## Context

There is no visual editor. Levels, entities, puzzles, settings and lore are plain-text files that agents read, write, diff and validate. Machine output must be easy for agents to parse. Recordings and cooked data must be compact and exact.

## Decision

Three formats, by purpose:

| Purpose | Format |
|---|---|
| Authored data (levels, puzzles, entity definitions, tuning, scenarios, settings, text tables) | TOML 1.1, with `toml_edit` for edits that keep comments |
| Machine output | JSON and JSON lines, with stable key order |
| Cooked data and recordings | One versioned binary format with a magic number, plus a JSON dump tool |

Rules for authored data:

- Rust types are the source of truth. A JSON Schema is exported from them.
- Parsing is strict. Unknown keys are errors, except in an `x_` extension namespace.
- Every file has a version field, a migration path and a canonical formatter.
- Every player-visible string goes through string IDs and text tables.

Recordings carry a header: format version, engine SHA, game SHA, simulation protocol version, rustc version, sim-data content hash, seeds, players and tick rate.

## Consequences

- Agents get a schema for every authored file.
- Old files migrate instead of breaking.
- In Level 1 a recording is valid only for the exact build in its header.
- Schema code is split: `eng-schema-core` (no_std, binary) and `eng-schema-json` (std, JSON and TOML).

## Revisit when

No trigger planned; change only by a new ADR.
