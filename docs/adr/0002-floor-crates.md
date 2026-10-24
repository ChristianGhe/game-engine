# ADR-0002: Floor crates

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D6 (spec sections 4, 6.2, 6.3)
- **Who:** Claude's call

## Context

The owner chose to build the engine in Rust, brick by brick (D1). Windowing, GPU access, audio devices and gamepads are large, platform-specific jobs. Everything above them can be ours.

## Decision

We borrow four floor crates, as of the spec's version table (2026-10-19):

- winit 0.30.13: windows and input
- wgpu 30.0.x: the GPU, through DX12, Metal and Vulkan
- cpal 0.18.x: audio devices (its next release breaks the API)
- gilrs 0.11.x: gamepads

Each is wrapped by exactly one engine crate (rule 2 in 6.2):

| Floor crate | Only importer |
|---|---|
| `wgpu` | `eng-gpu` |
| `winit`, `gilrs` | `eng-platform-winit` |
| `cpal` | `eng-audio-cpal` |

- The rule is enforced with cargo-deny `[bans]` wrapper lists.
- No third-party types appear in another crate's public API (rule 4).
- New third-party crates go on an allow-list in `deny.toml`. The owner approves additions in batches.
- Everything above the floor is ours.

## Consequences

- A floor crate can be swapped by rewriting one wrapper. The spec lists winit and gilrs to SDL3, and wgpu to wgpu-hal and then a native backend for one subsystem, as the paths if ever needed.
- Floor major versions change only in the upgrade rung at the start of a level (wgpu mid-level only by ADR, for a shipping blocker).
- Fixes we need from upstream live as `[patch]` forks with a tracking note and a test. By default we do not post to upstream projects.
- winit 0.31 is not used until it is stable.

## Revisit when

The upgrade rung at the start of each level (floor majors). Re-check the wgpu ceiling at Level 3.
