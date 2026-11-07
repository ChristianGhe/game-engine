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
  guess T13 checks; with none, frames come once per tick.
- Headless tests see requests through the record, not through the screen:
  "focus loss frees the cursor" is a `grab_cursor(false)` after
  `FocusLost` in the record.
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
