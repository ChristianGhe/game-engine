# ADR-0009: Levels and puzzles

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D13 (spec section 6.11)
- **Who:** Claude's call

## Context

The owner wants "a small puzzle, like an escape room that can be played in lan". Agents build the levels and puzzles, so the format must be plain text, and a machine must be able to prove each level solvable.

## Decision

Text sector maps plus a declarative puzzle model, compiled into engine-generic data and proven by a solver and bot replays.

- **Level source format v1 (2.5D sectors):** named vertices in integer millimetres; sectors as vertex loops with flat floor and ceiling heights, materials, light and tags; walls inferred from shared edges; things, triggers, doors, lifts and puzzle entities by stable string IDs; large levels split into zone files. Slopes wait for format v2.
- **Puzzle model (`kit-puzzle`):** entities with finite states, triggers and actions, conditions over flags, items, counters and sequences, presence predicates for co-op, and facts revealed in regions. Games add bespoke mechanics only through the kit's transition trait, so the solver still sees them.
- **Level compiler (`eng-levelc`):** exact validation on integer coordinates, T-junction splitting, triangulation, walls with world-anchored UVs. It outputs sim data (target-independent, cooked once) and presentation data (may differ per target).
- **Validation:** schema and references; geometry; a traversal graph; the puzzle solver (breadth-first search; shared knowledge, presence-gated actions; shortest solution and softlocks; a co-op room needs a 2-player plan and no 1-player plan); a headless bot replay re-run on every platform; accessibility rules.
- **Movement:** a pure function in `kit-fps` against the `eng-collision-api` trace interface. `eng-collision-grid` arrives at rung 2.

## Consequences

- CI cooks sim data on x64 and arm64 and compares the bytes. The handshake hash covers sim data only.
- Every puzzle level has a solver result and a bot replay.
- No puzzle may depend on colour alone or sound alone, and flashing stays under 3 per second.
- Slopes, brushes and BVH collision are later work (Level 2 sketch).

## Revisit when

When a game needs slopes (format v2), and at Level 2, which adds a brush source format compiled into the same engine data.
