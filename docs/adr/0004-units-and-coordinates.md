# ADR-0004: Units and coordinates

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D8 (spec sections 5, 6.1, 6.4, 6.7)
- **Who:** Claude's call

## Context

Conventions for units, axes and colour touch every crate. Changing them later is a rewrite, so they are part of the fixed spine (6.3). They must hold from the Doom-style start to the ultra-realistic end goal.

## Decision

- **Units:** SI and photometric units. Distances are in meters.
  - `eng-math` has unit types such as Meters, Lux, Nits and Ev100.
  - Lights use lux, lumens or candela, and nits. Camera exposure is in EV100.
- **Axes:** right-handed, Z-up.
- **Numbers.**
  - `Real = f32` for local math.
  - `WorldPos` is f64 for absolute world positions.
  - Integers for ticks, health, ammo, timers and IDs.
  - Angles are binary angles (u32, wrapping math).
- **Camera-relative rendering.** The CPU subtracts the camera position in f64 and uploads f32.
- **Depth:** reversed-Z `Depth32Float` with an infinite far plane.
- **Colour:** linear, scene-referred HDR. Textures are tagged sRGB or linear at import.
- **Level source:** integer millimetres for vertices (format v1).
- `AGENTS.md` states these conventions, since they are non-standard.

## Consequences

- Large worlds keep precision without changes to the simulation.
- No later rung has to convert units or axes.
- Plain `f32` positions in the simulation are not allowed for absolute positions.
- The conventions are listed in `AGENTS.md`.

## Revisit when

No trigger planned; change only by a new ADR.
