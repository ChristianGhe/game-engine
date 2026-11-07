# ADR-0021: `unsafe` in the GCMouse shim

- **Status:** Accepted
- **Date:** 2026-11-07
- **Decision:** the engine's first `unsafe` code. One module of a new
  crate, `eng-platform-gcmouse`, is excepted from the workspace lint
  `unsafe_code = "deny"`; every other crate stays as it is (spec 6.10;
  rung 2 plan, decision 11 and difference 4)
- **Who:** Owner decided (2026-11-07, at rung 2 gate G4, with O3: its own
  crate), on Claude's proposal (rung 2 plan, decision 11; owner decision
  O3).

## Context

Spec 6.10: on macOS "a small GCMouse shim (rung 2) supplies true raw mouse
deltas. winit's macOS deltas arrive coalesced, one per display refresh,
which would mislead playtests on the owner's Mac."

- **The binding forces `unsafe`.** In `objc2-game-controller` 0.3.2
  (crate batch 3, gate G3), every method the shim needs is an `unsafe fn`:
  `GCMouse::mice`, `GCMouse::current`, `GCMouse::mouseInput` and
  `GCMouseInput::setMouseMovedHandler` (the crate's generated
  `GCMouse.rs` and `GCMouseInput.rs`). The handler is a raw pointer,
  `GCMouseMoved = *mut block2::DynBlock<dyn Fn(NonNull<GCMouseInput>,
  c_float, c_float)>`, which "must be a valid pointer or null".
- **Where handlers run.** A device's `handlerQueue` is "main" by default.
  The plan's feature list leaves out the crate's `dispatch2` feature, so
  the shim cannot move it.
- **The rules at HEAD.**
  - The root `Cargo.toml`'s `[workspace.lints.rust]` (protected) sets
    `unsafe_code = "deny"`, and every crate takes the workspace lints,
    `xtask` included.
  - 21 of the 22 crates under `crates/` also start with
    `#![forbid(unsafe_code)]`; `game-testbed` (`src/main.rs`) and `xtask`
    do not. The core set must (6.1; `[E-ARCH-ROOT-ATTR]`).
  - `arch-check` looks for a silenced `unsafe_code` in core crates only
    (`[E-ARCH-ESCAPE]`, `xtask/src/arch.rs:248-253`). In a crate with
    `deny` but no `forbid`, an `#[allow(unsafe_code)]` compiles today and
    nothing reports it.
  - There is no `unsafe` block, function or impl in the workspace.
- The spec forbids `unsafe` only in the core set (6.1, 6.4). The deny on
  every other crate is the workspace's choice.

## Decision

- **A crate of its own.** `eng-platform-gcmouse`, in L1 (std). Its code is
  macOS-only; elsewhere it is an empty crate.
- **Only `eng-platform-winit` may depend on it.** Layers alone allow any
  L2-L4 crate to. So G2 adds `eng-platform-gcmouse = "eng-platform-winit"`
  under `architecture.toml`'s `[floor]`, beside winit's `same_layer` edge.
  `[floor]` maps a crate to the one crate that may depend on it, whatever
  the crate is (`[E-ARCH-FLOOR]`, `xtask/src/arch.rs:198-204`), so it needs
  no xtask change; its comment, "floor crates", becomes "crates with one
  user". It counts normal dependencies only (`arch.rs:365-372`), so a
  dev-dependency elsewhere would pass; review covers that.
- **One file holds all the `unsafe`.** The crate keeps
  `[lints] workspace = true` and has no `forbid` at its root. One private
  module file starts with `#![allow(unsafe_code)]`. Each `unsafe` block in
  it has a `// SAFETY:` comment that says why the call is sound, and the
  crate root has `#![forbid(clippy::undocumented_unsafe_blocks)]`, so a
  block without one fails clippy. The rest of the crate is safe code.
- **Where clippy sees it.** `quick` runs clippy on its own machine, so on
  this Mac it lints the module. CI runs clippy only in the ubuntu `lint`
  job (`xtask/src/ci.rs:56-61`, `ci.yml`), where the module is compiled
  out. So G5 adds a clippy step for the shim to the macos-26 native job;
  until then the SAFETY rule holds only locally.
- **What the `unsafe` does, and nothing more:**
  - list the mice (`GCMouse::mice`, `GCMouse::current`);
  - on each mouse's `mouseInput`, install a moved handler with
    `setMouseMovedHandler`. The shim keeps the block alive while it is
    installed, and sets the handler to null before dropping it.
- **The handler's body is safe code.** It rounds each delta to the nearest
  1/256 count, ties away from zero, as `MouseLook` does (decision 9), adds
  it into an `i64` sum, and counts the call. A drained sum of `n`
  sub-counts reaches `MouseLook` as `n / 256`, which it keeps exactly, so
  the yaw is the one the separate deltas would give.
- **A safe API.** The winit platform starts the shim, and drains its sums
  and callback count in `about_to_wait`, before the frame. It delivers
  them as one `PlatformEvent::MouseMotion`, and while the shim runs it
  drops winit's own macOS motion, so nothing counts twice. The drain
  follows the frames by design; the callback count is what shows the
  mouse's rate (criterion 25, `gcmouse-rate`).
- **No thread.** The handler runs on the main queue, the thread of winit's
  event loop (6.6's designated thread), so it never runs at the same time
  as a winit callback.
- **No other code may allow `unsafe_code`.** Today `arch-check` scans
  core crates' `src/**` only (`arch.rs:248-253`, `:391`). The G5 xtask
  patch makes it:
  - scan every Rust file in the workspace: each crate's `src`, `tests`
    (20 crates have one), `examples` (`eng-platform-winit`), benches and
    build scripts (`eng-cli/build.rs`), and `xtask`;
  - fail on an `allow`, `warn` or `expect` of `unsafe_code`, `cfg_attr`
    forms included, in any file but one: the path the shim's entry in
    `architecture.toml` (protected) names. That key comes in G5's
    `architecture.toml` change with the patch, since today's parser
    refuses unknown keys (`deny_unknown_fields`);
  - in that file, fail on any `mod` declaration, `#[path]` or `include!`,
    so the exception covers that one file's text.

  The selftest `unsafe-outside-shim` proves it. Crates that `forbid` it
  keep that.
- **One user of the binding.** A wrapper ban in `deny.toml` (G3, T0b)
  lets only the shim depend on `objc2-game-controller`, as ADR-0016 does
  for `raw-window-handle`; the selftest `gcmouse-outside-shim` proves it.

## Consequences

- The engine has `unsafe` code, all of it in one file of one crate that
  only macOS builds. The workspace lint table does not change.
- AGENTS.md's "never silence `unsafe_code` in a core crate" becomes "in
  any crate but the shim's one file" (T22).
- CI's macos-26 runner has no mouse. It builds the shim and runs its safe
  logic against a fake source (criterion 11); real callbacks are checked
  on the owner's Mac (T14, G7, criterion 25). Miri stays deferred.
- GCMouse's sign of `deltaY` and its units are not yet verified (T14's
  carry). The shim maps them to the contract's (`dy` positive towards the
  user) once T14 has checked them on the owner's Mac.
- An update of the binding may mark these methods safe or add others. The
  SAFETY comments are read again with each update, and a method that
  became safe leaves the module.

## Alternatives considered

- **A module inside `eng-platform-winit`** (O3's second answer). winit's
  wrapper, a larger crate with no `unsafe` today, would drop its `forbid`
  for `deny`, and the exception would cover the whole floor wrapper.
- **No shim** (O3's third answer). winit's coalesced deltas stay, which
  6.10 says would mislead playtests.
- **Our own Objective-C calls** through `objc2`'s message macros: more
  `unsafe`, not less, and no generated signatures to check them against.
- **A C or Swift helper.** It needs a build script and a second language,
  and the `unsafe` moves to a foreign-function boundary.
- **Exempting the shim crate from the workspace lints** (`[lints]` of its
  own). Every other workspace lint would then need copying, and the
  exception would cover the whole crate rather than one module.

## Owner decisions, gates and protected files

- **O3.** Recommended: this crate. If the owner picks a module inside
  `eng-platform-winit`, G2 applies T0's patch without the shim crate's
  lines, the wrapper ban names `eng-platform-winit`, and this ADR's
  exception moves to that module, with winit's root going from `forbid` to
  `deny`. ADR-0016's "all four crates forbid it today" then stops being
  true. If the owner picks no shim, this ADR is refused, the ban is not
  added, and criteria 11 and 25 drop.
- **Protected files:** `architecture.toml` (G2: the crate, winit's edge
  and the `[floor]` entry; G5: the exempt file's path); `deny.toml` (G3:
  `objc2-game-controller` in batch 3 and its wrapper ban); `xtask/**` and
  `.github/workflows/**` (G5: the scan, both selftests and the macOS
  clippy step).
- **Gates:** G2, G3 and G4 before T14; G5 for the check.

## Revisit when

winit delivers uncoalesced raw deltas on macOS; the binding marks these
methods safe; another crate needs `unsafe`; or Miri joins CI.
