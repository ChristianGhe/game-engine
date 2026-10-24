# ADR-0003: Graphics and the render graph

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D7 (spec section 6.7)
- **Who:** Claude's call

## Context

The first games look like the first Doom. The owner's end goal is "highly optimised, ultrarealistic graphics". The engine must grow from one to the other without a rewrite (G4).

## Decision

The GPU is used from rung 1. The Doom look is a set of passes on a modern linear HDR pipeline.

- **Render graph.** Passes declare the virtual resources they read and write. This rule is fixed.
  - Rung 1b: a fixed two-pass graph, draw then present.
  - Rung 2: ordering, culling of unused passes, transient-texture pooling, labels and offscreen targets.
  - The API grows by ADR.
- **Colour and light.**
  - Shading is scene-referred, linear and HDR, into `Rgba16Float` targets.
  - Lights use photometric units. Camera exposure is in EV100.
  - Tone mapping is the last graph pass, into an engine-owned texture. Screenshots and golden tests read that texture.
  - A final blit pass draws into the surface's preferred format.
- **Tier 0 only in Level 1:** wgpu's default (WebGPU-core) features and limits. Capability detection exists from rung 1b.
- **GPU scene layout** (fixed at rung 2): shared vertex and index megabuffers, per-instance storage buffers, material tables, texture arrays bucketed by size.
- **Pipelines are plug-ins** over the same scene data: `render-retro` (Level 1), `render-clustered` (Level 2), `render-visbuffer` (Level 3), `render-rt` (Level 4). Older ones stay as low-end options.
- **The Doom look is switchable passes:** light falloff, wall-direction contrast, coloured fog, a palette post-process, optional low resolution, a sky material. Every pass can be turned off.
- **Shaders** are WGSL, validated by naga in CI and translated to SPIR-V, MSL and HLSL.
- **When wgpu lacks a feature:** a feature flag, then a wgpu-hal escape hatch in that pass, then an upstream report, then a native backend for that subsystem.

## Consequences

- Agents can build render passes in parallel.
- A backend swap is a port, not a rewrite.
- Reaches Steam Deck, older DX12 and Vulkan PCs, and every Metal Mac.
- Ray query, mesh shaders and similar are optional later paths.

## Revisit when

The upgrade rung at the start of Level 2, and the wgpu re-check at Level 3. wgpu majors change only at level starts, or mid-level by ADR for a shipping blocker.
