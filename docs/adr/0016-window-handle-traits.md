# ADR-0016: Window-handle traits in public APIs

- **Status:** Accepted
- **Date:** 2026-10-31
- **Decision:** an exception to rule 4 in spec 6.2 (D6, ADR-0002); rung 1b
  gate G4
- **Who:** Owner decided (2026-10-31, at rung 1b gate G4), on Claude's
  proposal (rung 1b plan, difference 4; `docs/rungs/1b/PROGRESS.md`,
  proposal 5)

## Context

Spec 6.2 rule 4 says third-party types "never appear in another crate's
public API". With rule 2, one wrapper per floor crate, it lets a floor
crate be swapped by rewriting one wrapper (ADR-0002). Spec 6.3 gives the
platform layer's path as winit and gilrs to SDL3. Until now the rule had
one exception: `serde` derives on authored-data types (rung 1a plan,
difference 5).

Rung 1b draws the dot in a real window.

- wgpu creates its surface from a window, and the window comes from
  winit. Only `eng-gpu` may import wgpu and only `eng-platform-winit`
  may import winit, so the window has to cross the platform contract
  (`eng-platform-api`) and `eng-app`, which composes the platform and the
  renderer.
- Both speak `raw-window-handle` 0.6 (0.6.2; MIT OR Apache-2.0 OR Zlib).
  winit implements its traits (feature `rwh_06`, plan decision 5), and
  wgpu's safe surface creation takes them. It is a neutral interop crate
  that SDL3's Rust bindings implement too, so the swap path in spec 6.3
  survives.
- Rule 4 as written keeps those traits out of every public API. Without
  an exception the window would have to cross as our own copy of the
  crate's handle types (see "Alternatives considered").

## Decision

- `HasWindowHandle` and `HasDisplayHandle`, the two traits of
  `raw-window-handle` 0.6, are the second exception to rule 4. They may
  appear in public APIs between four crates, and nowhere else:

  | Crate | Layer | Its part in the window seam |
  |---|---|---|
  | `eng-platform-api` | L1 | defines the seam in the platform contract |
  | `eng-platform-winit` | L1 | implements it over winit's window |
  | `eng-gpu` | L3 | creates the wgpu surface from it |
  | `eng-app` | L4 | passes the window from the platform to `eng-gpu` |

- Only these two traits: the rest of the crate, such as its raw handle
  enums, stays under rule 4. None of the four re-exports the traits,
  which would let any crate name them.
- **cargo-deny decides who may depend on the crate.** A wrapper ban
  beside the floor-crate bans in `deny.toml` lets only the four crates,
  and the third-party crates that use it directly (today `wgpu`,
  `wgpu-core`, `wgpu-hal`, `wgpu-types` and `winit`), depend on
  `raw-window-handle`. A crate that cannot depend on it cannot name the
  traits. CI's `lint` job runs `cargo deny check`.
  - `deny.toml` is protected, so the ban reaches the owner as a patch
    with the rest of rung 1b gate G3, which the owner approved the same
    day.
  - That change also puts the crate on the allow-list (crate batch 2),
    so the crate is never allowed without its ban.
- **A selftest proves the ban.** `cargo xtask selftest rwh-outside-seam`
  (patch X5, gate G5) expects a use of the crate outside the seam to
  fail. It is part of acceptance criterion 12, `violations-1b`.
- **`arch-check` needs no change.** The crate is not in
  `[core_third_party]`, so core-set purity already keeps it out of every
  core crate and `game-*-sim`. `[floor]` gives each crate one wrapper, so
  a seam of four crates is cargo-deny's job.
- **Review covers the rest.** The ban sees dependencies, not signatures
  or re-exports; review checks those, as it checks rule 4 everywhere.

## Consequences

- The window reaches wgpu through its safe surface creation, which takes
  both traits. The seam needs no `unsafe` (all four crates forbid it
  today) and no copy of the crate.
- `raw-window-handle`'s major version becomes part of these crates'
  APIs, the platform contract included, so changing it is a contract
  change (spec 6.2 rule 6). wgpu and winit must agree on it (0.6 for
  wgpu 30 and winit 0.30); an upgrade that moves it changes all four.
- An update that gives the crate a new direct third-party user fails
  `cargo deny check` until the owner adds that user to the ban's
  wrappers.
- Every other crate stays outside: `eng-platform-headless`, `eng-input`,
  `eng-render`, `eng-cli`, the core set and the games. None depends on
  the crate, so none can name the traits. The headless twin implements
  the same contract, so rung 1b's T3 has to keep the contract
  implementable without them.
- Rule 4 holds everywhere else; a third exception would again be the
  owner's decision.
- Until the owner applies G3's patch, the crate is not on `deny.toml`'s
  allow-list, so CI's `cargo deny check` fails once rung 1b's T3, T7 or
  T10 adds it. The owner approved G3 with this decision, so those tasks
  go ahead, and the patch lands before their first push.

## Alternatives considered

- **Our own raw-handle enum in the platform contract** (proposal 5).
  Rule 4 would stay unchanged, but the enum copies `raw-window-handle`,
  and it needs `unsafe`: wgpu takes raw handles only through
  `create_surface_unsafe`.

## Revisit when

A crate outside the four needs a window handle; wgpu or winit moves to
another major version of `raw-window-handle` (at the upgrade rung that
starts a level, ADR-0002); or the platform layer moves to SDL3.
