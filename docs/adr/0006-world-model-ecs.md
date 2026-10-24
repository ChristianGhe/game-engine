# ADR-0006: World model and ECS

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D10 (spec section 6.5)
- **Who:** Claude's call

## Context

The world model is part of the fixed spine. It must be snapshotted, hashed, diffed, replicated and saved, and it must iterate in a deterministic order (ADR-0005).

## Decision

`eng-ecs` is our own small plain-data ECS with a schema registry.

- **Entities** are generational IDs (u32 index + u32 generation) from a deterministic free list.
- **Components** are plain-data structs: no references, no `Box<dyn>`, serializable through `eng-schema-core`. Stored densely per type (sparse sets first; the layout stays private).
- **Singletons** are resources. Hierarchy and attachments are components, not a scene graph.
- **Systems** are plain functions placed in stages. No automatic parallel scheduling in the simulation.
- **Structural changes** go through command buffers applied at fixed sync points. Change ticks support replication.
- **Schemas:** every component and resource registers a stable name and schema version. `eng-schema-json` dumps the world to JSON for agents.

Not bevy_ecs. The spec gives these reasons:

- four breaking releases in 13 months
- it needs Rust 1.95+
- its parallel scheduler works against determinism

## Consequences

- We own and maintain the ECS.
- Snapshots, hashes, diffs, replication and saves come from the schema registry.
- Network entity IDs are separate from ECS IDs (ADR-0007).
- Replicated state is kept apart from presentation-only state.

## Revisit when

No trigger planned; change only by a new ADR.
