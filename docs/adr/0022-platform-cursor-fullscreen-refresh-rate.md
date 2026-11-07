# ADR-0022: Cursor, fullscreen and refresh rate in the platform contract

- **Status:** Accepted
- **Date:** 2026-11-07
- **Decision:** a contract change to `eng-platform-api` (spec 6.2 rule 6;
  spec 6.10): `Control` gains cursor and fullscreen requests and `Window`
  gains `refresh_rate`, all with default bodies. Extends ADR-0016's
  `Window`
- **Who:** Owner decided (2026-11-07, at rung 2 gate G4, with O9: fullscreen
  by default for first-person maps), on Claude's proposal (rung 2 plan,
  decision 10; owner decision O9).

## Context

Spec 6.10 asks for cursor lock ("Locked, with Confined as the fallback")
and borderless fullscreen by default. Rung 2 also paces first-person
frames at the display's refresh rate (6.6, decision 10). The platform
contract has none of these at HEAD:

- `Control` has `now` and `exit` only (`app.rs`); `Window` has `size`
  only (`window.rs:22`).
- `Control` is implemented by headless's `Clock` (`lib.rs:84`), winit's
  `Ctx` (`run.rs:29`), and in tests by `LateClock` (`twin.rs:354`, a test
  platform's own controls), `Lens` and `StopLens`
  (`eng-platform-headless/tests/conformance.rs:227,258`, wrappers that
  inject faults) and `Clock` (`eng-platform-api/tests/window.rs:87`).
  `Window` is implemented by winit's `WinitWindow` (`window.rs:29`) and by
  `FakeWindow` in `tests/window.rs:33`. `twin.rs` and the headless
  conformance test are protected.
- The headless conformance test runs `conformance::check` through `Lens`
  to show that `check` catches a faulty platform.
- `PlatformEvent` is an exhaustive enum (no `#[non_exhaustive]`).
- winit 0.30.13 (`Cargo.lock`) has `set_cursor_grab`, `set_cursor_visible`,
  `set_fullscreen(Some(Fullscreen::Borderless(None)))` and the monitor's
  `refresh_rate_millihertz() -> Option<u32>`. Its docs say `Confined` is
  not implemented on macOS and `Locked` not on X11.
- **The plan and the code differ.** The plan says `Locked` is also missing
  on Windows. winit 0.30.13 implements it there, as a one-pixel clip at
  the cursor (`platform_impl/windows/window.rs:443`, `window_state.rs:489`).
  The fallback below covers both cases, and the report names the mode.

## Decision

- **Three methods with default bodies** (names proposed; T13 may rename):

  ```rust
  pub trait Control {
      fn now(&self) -> Instant;
      fn exit(&mut self);
      /// Grab the cursor for mouse look, or free it. Returns the grab in force.
      fn grab_cursor(&mut self, grab: bool) -> CursorGrab { let _ = grab; CursorGrab::None }
      /// Borderless fullscreen on the window's monitor, or back to the window.
      /// Returns whether the platform took the request; the change may land later.
      fn set_fullscreen(&mut self, on: bool) -> bool { let _ = on; false }
  }
  pub trait Window: HasWindowHandle + HasDisplayHandle + Send + Sync {
      fn size(&self) -> PhysicalSize;
      /// The refresh rate of the window's monitor now, in millihertz.
      fn refresh_rate(&self) -> Option<u32> { None }
  }
  ```

  `CursorGrab` is a new plain enum, `None`, `Confined` or `Locked`, and
  `#[non_exhaustive]`.
- **The rules.**
  - A request acts at once on the window the platform hands to
    `App::window`, from that callback on. Grabbing tries `Locked`, then
    `Confined`, and hides the cursor; freeing shows it. `grab_cursor`
    returns the grab now in force.
  - **During `App::window`**, where eng-app asks, a request takes effect.
    winit's window exists then but is shown only after the callback
    returns (`run.rs:146-148`). T13 checks on each OS that a grab and
    fullscreen asked for there hold once the window shows; where one does
    not, winit applies it again right after showing the window.
  - **Fullscreen may land later.** macOS animates the change, and winit
    queues a request made during a transition until it ends
    (`platform_impl/macos/window_delegate.rs:1308-1312`). So
    `set_fullscreen` returns whether the platform took the request, not
    the window's state.
  - Before `App::window`, and on a platform without a window, a request
    changes nothing and returns `CursorGrab::None` or `false`.
  - The platform never grabs, frees or changes fullscreen unasked. The app
    asks again when focus changes: eng-app frees the cursor on `FocusLost`
    and grabs it on `FocusGained`.
  - `stop` receives `&dyn Control`, so no request can be made there. When
    a run ends, the window goes, and the grab with it.
- **Headless** records every request with the clock's time, in order,
  readable after the run, and returns `None` or `false`, as a platform
  without a window does. **winit** acts on its window and reads the
  refresh rate from `current_monitor()`.
- **A separate conformance suite** checks the new rules: a probe app makes
  requests before and after `App::window` and checks what they return.
  `check` and `check_scripted` keep their expectations.
- **Who asks.** eng-app, for first-person maps only (decision 10,
  difference 9): fullscreen unless `--windowed`, and the grab. The dot map
  asks for nothing. Frames come at `refresh_rate()` when it is known, else
  once per tick; ticks still come from the timer (6.6).

## Migration notes

- **Implementers of `Control` and `Window` compile unchanged.** Every new
  method has a default body: `LateClock`, `Lens`, `StopLens`, the test
  `Clock` and `FakeWindow` need no edit.
- **Only headless and winit change**, overriding them as above.
- **A wrapper that does not forward a request drops it**: the caller gets
  the default answer. A wrapper meant to pass requests through forwards
  `grab_cursor` and `set_fullscreen` to its inner `Control`.
- **Callers** (eng-app's `frame.rs`, eng-cli's `ticker.rs`) need no change
  until they want the requests; eng-cli's headless runs make none.
- **Unchanged:** `PlatformEvent`, `App`, `Platform`, `Wake`,
  `PlatformError`, `check` and `check_scripted`.
- **Protected tests that pin the old API and stay as they are:** `twin.rs`,
  `eng-platform-headless/tests/conformance.rs`,
  `eng-platform-winit/tests/conformance.rs`, `play.rs`, and the existing
  conformance suites (`src/conformance.rs` gains only a `mod` line).
- Rule 6 asks for "an interface-change PR". With direct commits to `main`
  (ADR-0014), T13's commit message repeats these notes.

## Consequences

- A first-person `play` can lock and hide the cursor and go fullscreen;
  its report names the grab in force, fullscreen and the refresh rate
  (criterion 10). Under Xvfb, whether winit reports a refresh rate is a
  guess T13 checks; with none, frames come once per tick. (T13 found
  none: Amendment 1.)
- Headless tests see requests through the record, not through the screen.
  Under Amendment 1 (proposed) the platform frees the cursor on focus loss
  itself, so "focus loss frees the cursor" no longer shows as a
  `grab_cursor(false)` after `FocusLost` in the record: one is there only
  if the app makes it. winit's unit tests (`request.rs`) check the
  freeing instead.
- A later platform (SDL3, spec 6.3) implements the three methods or
  inherits the defaults.

## Alternatives considered

- **Fullscreen as a window attribute at creation.** The app could not
  leave fullscreen or ask later, and headless could not record it.
- **Requests on `Window`.** Headless has no `Window`, so the twin could
  not record them.
- **Requests applied after the callback, as `exit` is.** The app would
  need another query to learn what was granted; acting at once answers in
  the call.
- **New `PlatformEvent` variants or `App` callbacks** to report a grant.
  A grant answers a request, which the call itself can return. And a new
  variant of an exhaustive enum breaks any crate that matches it in full.
- **No default bodies.** Every `impl Control` would need editing, three of
  them in protected tests.
- **The new rules inside `check`.** The headless conformance test runs
  `check` through `Lens`, which drops requests, so it would fail for the
  wrong reason.
- **Exclusive fullscreen.** It changes the display mode; 6.10 asks for
  borderless.

## Owner decisions, gates and protected files

- **O9.** Recommended: fullscreen by default for first-person maps, with
  `--windowed` for tests. If the owner prefers a window by default, the
  contract stays as above and eng-app asks for fullscreen only under a
  flag; nothing here changes.
- **G4** accepts this before T13.
- **G5:** a new file under `crates/eng-platform-api/src/conformance/` and
  its `mod` line in `src/conformance.rs` (both protected), and
  `protected.toml` with `.github/CODEOWNERS` protecting T13's new headless
  and winit tests. No existing protected test changes.

## Revisit when

The platform layer moves to SDL3; winit's next major changes grab modes
or monitors; HDR or variable refresh rate needs more than one number; or
gamepads (rung 6) need requests of their own.

## Amendment 1 (Proposed, for the owner; T13 found)

- **Status:** Proposed (for the owner; T13 found). The decision above
  stays Accepted until the owner takes or refuses this.
- **What:** two of the decision's rules, as T13 built them, and what T13
  found on real windows. Neither changes the contract's methods or types.
- **Who:** Claude's proposal, from rung 2 task T13 and its review
  (2026-11-07). The owner decides.

**Two rules, as built.**

- **The platform frees the cursor on focus loss.** The decision says "The
  platform never grabs, frees or changes fullscreen unasked" and "eng-app
  frees the cursor on `FocusLost`". winit frees it itself, before
  `FocusLost` reaches the app (`eng-platform-winit/src/request.rs`,
  `Requests::focus_lost`), and at the end of a run. It still never grabs
  or changes fullscreen unasked. Why: macOS's lock is the whole system's
  (winit's `Locked` stops the cursor following the mouse everywhere, not
  only over the window), so a grab left in force after another app takes
  focus keeps the user's cursor from that app until ours reacts, and a
  stalled app never reacts. The lock also outlives the window, hence the
  end of a run.
- **A grab asked for without focus waits for it.** The decision says a
  request in `App::window` takes effect and `grab_cursor` returns the grab
  in force. winit's grab asked for while the window lacks focus, as it
  always does in `App::window` (the window is not shown yet), answers
  `CursorGrab::None`, the grab then in force, and is taken when focus
  comes, before `FocusGained` reaches the app; `grab_cursor(false)` drops
  it. winit reports a `FocusLost` before a new window's first
  `FocusGained` (seen on macOS 26 and under Xvfb), so a loss while the
  window already lacks focus leaves the grab waiting. Why: X11 cannot grab
  a window that is not viewable (`XGrabPointer` answers
  `GrabNotViewable`), and on macOS a lock taken without focus would hold
  the cursor while another app has it, which the first rule forbids.
  Fullscreen is unchanged: asked for in `App::window`, it is taken at
  once and asked again once the window shows if it did not land.

**What eng-app does.** It grabs on `FocusGained`, as the decision says,
and reads the grab in force from that answer, not from `App::window`'s,
which on winit is `None`. It may still call `grab_cursor(false)` on
`FocusLost`: the cursor is already free, the call answers `None`, and
nothing changes. On a new window's first `FocusLost` it drops a grab still
waiting from `App::window`, and the grab on `FocusGained` replaces it.

**Found by T13.**

- **Leaving fullscreen during macOS's enter animation is lost.** On
  macOS 26 (an M2 Pro), `set_fullscreen(false)` asked at the first
  full-size `Resized` after `set_fullscreen(true)` answered `true`, but the
  window stayed fullscreen while winit took it as windowed; asked two
  seconds later, it left. winit's queueing during a transition did not
  keep it. An app should not toggle fullscreen during the animation; the
  fullscreen check in T13's G5 patch waits two seconds.
- **Where `App::window`'s requests were checked: macOS only.** The
  by-hand checks `grab_with_focus` and `fullscreen`
  (`eng-platform-winit/tests/requests.rs`, in
  `docs/rungs/2/patches/t13-conformance.patch`) pass on macOS 26. Under
  Xvfb without a window manager no focus comes, so `grab_with_focus`
  cannot run. There `examples/events.rs`, given focus by `xdotool`, got
  `Confined` asking again on `FocusGained`, and its `set_fullscreen(true)`
  in `App::window` answered `true` but no `Resized` came (X11 fullscreen
  needs a window manager). Windows and Wayland are unchecked.
- **Refresh rate.** macOS 26 reports 120000 mHz already in `App::window`,
  before the window shows. Under Xvfb (Debian trixie, arm64, in Docker,
  screen 1280x720x24) winit reports `None` in `App::window` and in the
  first frame, so first-person frames there come once per tick.
- **X11 doubles raw motion under the grab.** With `Confined` (an X
  pointer grab), the X server sends winit each raw motion twice from the
  same device; without the grab, once (Xvfb, XTEST input). SDL drops a raw
  motion with the same X time and deltas as the one before, but winit
  0.30 passes neither on, so both reach the app (`map.rs`). Mouse look on
  X11 would turn twice as far until this is handled.
