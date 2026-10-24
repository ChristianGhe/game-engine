# Rung 1b progress (append-only)

Rung 1b, the windowed dot: see `plan.md`. Each session appends what it
did, costs and timings. Changes to protected paths are written here as
proposals for the owner, never made by agents.

## 2026-10-25, session 1: plan draft (Claude Opus 5.5, open-pulsar task)

**Done**
- Read `AGENTS.md`, `docs/ARCHITECTURE.md`, the 1a plan and progress log,
  `architecture.toml`, `deny.toml`, `budgets.toml`, `protected.toml`,
  ADRs 0002, 0003, 0007 (after the review), 0010-0012, 0014 and 0015, and
  spec sections 6, 8.2, 8.3 and 9.2.
- Wrote `plan.md`: crates, the frame, probe additions, decisions,
  differences from the spec, a draft acceptance list (17 features),
  budgets and time-box, 17 tasks, owner gates G1-G7, risks.
- Worked out allow-list batch 2 from a scratch project outside the repo
  (`wgpu =30.0.1` without `gles`/`webgpu`, `winit =0.30.13` without
  Wayland decorations, `raw-window-handle 0.6.2`, `png 0.18.1`,
  `nv-flip 0.1.2`; toolchain 1.99.1): 132 crates not yet allowed, all
  under licences already on the allow-list (proposal 4).
- No code, no new crates, no protected file touched;
  `docs/rungs/current.txt` stays at 1a; no `acceptance.json`.

**Review**
- One fresh-context `reviewer` pass: no layering blockers; the proposed
  `architecture.toml` entries cover every dependency in the plan. It found
  16 issues, all fixed in the draft:
  - the windowed hash check used frames, not ticks (now 240 ticks,
    seed 1, against `dot-square.seed1.txt`);
  - the golden tasks looped (now T12 harness, T16 candidates after G5,
    with "no golden yet" keeping `main` green until G6);
  - network contracts 3-5 and the protocol version in `Hello` were not
    addressed (now in the crate table and difference 10);
  - G4 missing from three tasks' needs;
  - `golden.rs` for `eng-net-wire` had no owner gate;
  - wasm32 in cargo-deny's targets would need web-only crates;
  - nothing enforced ADR-0016 (now a wrapper ban and a selftest);
  - no mouse or gamepad events in the platform contract;
  - bind-group switches missing from the counters;
  - golden comparison by PNG bytes (now decoded pixels);
  - `crash.json` in `play` had no task;
  - the conditions for unchanged hash streams were unstated (World
    registrations, snapshot within one datagram);
  - the panic hook's crate differed from spec 6.1 without saying so;
  - FLIP's reference was unclear (now the lavapipe golden);
  - patch names clashed with the spec's P1-P11 (now X1-X6);
  - xdotool under xvfb without a window manager, and two unprotected
    harnesses.
- `cargo xtask quick`: all 7 steps green. `typos docs/rungs/1b`: clean.

**Costs and timings**
- One open-pulsar session, about 40 minutes, including the reviewer
  (about 7 minutes and 119k tokens). The main session's token use is not
  visible inside the session (see open-pulsar's log).

**Next**
- The owner's G1-G4 (`plan.md`, "Owner actions"). T1 and T2 need only G1.

## 2026-10-25, session 2: plan revision (Claude Opus 5.5, open-pulsar task)

**Done**
- Rewrote `plan.md` to the queue item's outline. It now has:
  - a "DRAFT FOR OWNER" banner;
  - a tag on every item: *[spec X]*, *[spec X + proposal]* or
    *[proposal]*;
  - new and changed crates in separate tables, each with its proposed
    `architecture.toml` layer, core flag and `same_layer` edges;
  - the acceptance criteria as a numbered list, plus a table mapping
    spec 9.2's done-when items to criteria;
  - a time-box and AI budget labelled as proposals for the owner;
  - "Owner gates": every protected change, by `protected.toml` pattern,
    and the protected paths 1b leaves alone;
  - the empty `## Crate batch 2` placeholder for the next queue item.
- Mapped this window's queue to the rung (`plan.md`, "This window's
  queue"). All three items are 1b, and none needs a gate to start:
  - `crash.json`: T2, criterion 6 (done-when 4);
  - `--version` and `--version --json`: T1, criterion 8;
  - live actions and bindings: T4, new criterion 18 `live-input`, also
    serving 3 and 16 (done-when 2).
- Two plan changes follow from that:
  - T4 no longer waits for `eng-platform-api`. `eng-input` defines the
    plain-data types `Key` and `KeyEvent` now, and T3 moves them into
    the contract.
  - Plain `--version` is added as a proposal (spec 6.13 has only
    `--version --json`).
- Found one protected item the first draft missed. The Intel-Mac
  `cargo check` needs the `x86_64-apple-darwin` target, which
  `rust-toolchain.toml` does not list. The proposal is that X6 installs
  it in the macOS job only, as the RISC-V job does, so the file stays
  unchanged (proposal 11).
- No code, no new crates, no protected file touched; `current.txt` stays
  at 1a; no `acceptance.json`.

**Review**
- One fresh-context `reviewer` pass found 1 blocker, 9 should-fix issues
  and 8 nits. All are fixed in `plan.md`, and proposal 11 records the
  changes to the proposals.
  - **Blocker:** G2 as written turns `main` red, because `arch-check`
    fails on crates listed but absent. G2 now lands in one commit with
    T0's skeleton crates.
  - **Dependencies:** T3 needs G3 (it uses `raw-window-handle`), and T11
    needs T2 and T9.
  - **CI steps:** the `test`-job steps behind criteria 6 and 10 had no
    patch (now X7). Criteria 11 and 18 relied on unnamed, unprotected
    tests (now named steps, and the files join proposal 8).
  - **Completeness:**
    - `.config/nextest.toml` was missing from the protected-paths
      audit, and so were the other names `protected.toml` lists and the
      two existing `golden.rs` files.
    - `exit_codes.rs` constrains more than exit codes: recordings, usage
      errors and the summary length. "This window's queue" now lists it.
  - **Tags:**
    - The shader check is spec 6.7, not a proposal.
    - Exact pixels on Linux with FLIP elsewhere is spec 8.3, and
      physical keys are spec 6.10.
    - `eng-platform-api` follows from spec 6.1's contract pair.
    - Difference 1 now quotes 6.7's own "two-pass" sentence.
  - **Gates:** G6 needs the CI run after G5, so the third sitting is
    G6 with G7, not G5 with G6.
  - **Smaller fixes:** golden candidates go under `target/`, and
    `Snapshot` carries opaque bytes.

**Checks**
- `typos docs/rungs/1b/`: clean.
- `cargo xtask quick`: all 7 steps green, before and after the review
  fixes.

**Costs and timings**
- One open-pulsar session, about 55 minutes, including the reviewer
  (about 7 minutes and 105k tokens). The main session's token use is not
  visible inside the session.

## Proposals for the owner (protected paths)

Nothing below is applied. Each item is what an agent would otherwise have
to change; the owner applies it, or unlocks the path.

1. **`docs/rungs/1b/acceptance.json`** (G1). Make it from the table in
   `plan.md`, "Acceptance", in 1a's format: rung `1b`, spec section 9.2,
   time-box 14 calendar days from approval, AI budget at most 30 agent
   sessions. Feature 16 (`owner-play`) cannot be computed; proposal 7
   (X4) teaches `cargo xtask acceptance` an owner-checked feature.

2. **`architecture.toml`** (G2). Add, and change the two existing entries
   marked below:

   ```toml
   [crates.eng-platform-api]
   layer = "L1"

   [crates.eng-platform-winit]
   layer = "L1"
   same_layer = ["eng-platform-api"]

   # changed: implements the platform contract
   [crates.eng-platform-headless]
   layer = "L1"
   same_layer = ["eng-platform-api"]

   # changed: maps platform key codes to actions
   [crates.eng-input]
   layer = "L1"
   same_layer = ["eng-schema-json", "eng-platform-api"]

   [crates.eng-net-wire]
   layer = "L1"
   core = true

   [crates.eng-session]
   layer = "L2"
   core = true
   same_layer = ["eng-sim"]

   [crates.eng-gpu]
   layer = "L3"

   [crates.eng-render]
   layer = "L3"
   same_layer = ["eng-gpu"]

   [crates.eng-app]
   layer = "L4"
   same_layer = ["eng-cli"]
   ```

   `[floor]` already maps wgpu to `eng-gpu` and winit to
   `eng-platform-winit`; no change there.

3. **Root `Cargo.toml`, `[profile]` table** (G2), for the two new core
   crates:

   ```toml
   [profile.release.package.eng-net-wire]
   overflow-checks = true
   [profile.release.package.eng-session]
   overflow-checks = true
   ```

4. **`deny.toml`, allow-list batch 2** (G3).
   - Limit cargo-deny to the targets whose whole workspace we build, so
     crates for Android, iOS and the web stay out of the list:

     ```toml
     [graph]
     all-features = false
     targets = [
       "x86_64-unknown-linux-gnu",
       "aarch64-unknown-linux-gnu",
       "x86_64-pc-windows-msvc",
       "aarch64-pc-windows-msvc",
       "aarch64-apple-darwin",
       "x86_64-apple-darwin",
       "riscv64gc-unknown-linux-gnu",
     ]
     ```

     `wasm32-unknown-unknown` is left out on purpose: CI builds only the
     core set for it (which reaches nothing but `libm`), while winit would
     add web-only crates to the full graph. RISC-V adds no crates beyond
     the Linux ones (checked). The comment above `allow` ("only these
     crates may appear in Cargo.lock") then becomes "in the dependency
     graph of these targets". Batch-1 entries reached only on other
     targets (`r-efi`) may become unused; keep or drop them.
   - Keep the window-handle exception (proposal 5) inside its seam with a
     wrapper ban next to the floor-crate bans:

     ```toml
     { crate = "raw-window-handle", wrappers = [
       "wgpu", "wgpu-core", "wgpu-hal", "wgpu-types", "winit",
       "eng-platform-api", "eng-platform-winit", "eng-gpu", "eng-app",
     ] },
     ```

     The five third-party names are the crates that use it directly
     today (checked with `cargo tree -i`).
   - Add these 132 crates to `[bans] allow` (computed 2026-10-25; T7 and
     T10 recompute against the real lock and report any difference here):
     - every OS (54): adler2, arrayvec, bit-set, bit-vec, bytemuck,
       bytemuck_derive, cc, cfg_aliases, codespan-reporting, crc32fast,
       cursor-icon, document-features, dpi, fdeflate, find-msvc-tools,
       <https://crates.io/crates/flate2> (see the `_typos.toml` note
       below), foldhash, half, libloading, litrs, lock_api, log,
       miniz_oxide, naga, naga-types, nv-flip, nv-flip-sys, once_cell,
       parking_lot, parking_lot_core, pin-project-lite, png, profiling,
       raw-window-handle, rustc-hash, scopeguard, shlex, simd-adler32,
       smallvec, smol_str, static_assertions, thiserror, thiserror-impl,
       tracing, tracing-core, unicode-width, wgpu, wgpu-core, wgpu-hal,
       wgpu-naga-bridge, wgpu-types, winit, zerocopy, zerocopy-derive
     - Linux and Windows (8, Vulkan): allocator-api2, ash, gpu-allocator,
       ordered-float, presser, renderdoc-sys, spirv,
       wgpu-core-deps-windows-linux-android
     - Linux only (33, X11 and Wayland): ahash, as-raw-xcb-connection,
       calloop, calloop-wayland-source, dlib, downcast-rs, gethostname,
       linux-raw-sys, memmap2, percent-encoding, pkg-config, polling,
       quick-xml, rustix, scoped-tls, slab, smithay-client-toolkit,
       version_check, wayland-backend, wayland-client, wayland-csd-frame,
       wayland-cursor, wayland-protocols, wayland-protocols-plasma,
       wayland-protocols-wlr, wayland-scanner, wayland-sys, x11-dl,
       x11rb, x11rb-protocol, xcursor, xkbcommon-dl, xkeysym
     - Windows only (17): range-alloc, unicode-segmentation, windows,
       windows-collections, windows-core, windows-future,
       windows-implement, windows-interface, windows-link,
       windows-numerics, windows-result, windows-strings, windows-sys,
       windows-targets, windows-threading, windows_aarch64_msvc,
       windows_x86_64_msvc
     - macOS only (20): block2, core-foundation, core-foundation-sys,
       core-graphics, core-graphics-types, dispatch, foreign-types,
       foreign-types-macros, foreign-types-shared, objc-sys, objc2,
       objc2-app-kit, objc2-core-foundation, objc2-core-graphics,
       objc2-encode, objc2-foundation, objc2-metal, objc2-quartz-core,
       raw-window-metal, wgpu-core-deps-apple
   - Licences: every one is MIT, Apache-2.0, Zlib, ISC, BSD-2/3-Clause or
     0BSD as one choice of an OR, or a combination of allowed ones
     (`dpi`: Apache-2.0 AND MIT; `nv-flip-sys`: (MIT OR Apache-2.0 OR
     Zlib) AND BSD-3-Clause). No exceptions needed.
   - Test-only: `nv-flip` and `nv-flip-sys` (FLIP image comparison, as the
     spec chose; builds C++ through `cc`). `png` serves both goldens and
     the proof video.
   - **`_typos.toml`** (protected) needs the deflate crate's name, the one
     given above as a link, under `[default.extend-identifiers]` in the
     same change. typos reads it as a misspelt "flat", so the `lint` job
     fails once that name is in `deny.toml`.

5. **ADR-0016 (G4), an exception to rule 4.** Allow the `raw-window-handle`
   0.6 traits `HasWindowHandle` and `HasDisplayHandle` in public APIs
   between `eng-platform-api`, `eng-platform-winit`, `eng-gpu` and
   `eng-app`, and nowhere else. Why: wgpu needs a window handle from winit;
   this crate is the neutral interop standard both implement (as do SDL3's
   Rust bindings), so the swap path in spec 6.3 still holds. The
   alternative, our own raw-handle enum, needs `unsafe` and copies that
   crate. The ADR is written in T14 if the owner agrees; `AGENTS.md` and
   `docs/ARCHITECTURE.md` then name both exceptions. The wrapper ban in
   proposal 4 and a selftest case (X5) enforce the seam.

6. **`budgets.toml`** (G5):

   ```toml
   # Runner-independent per-frame counters for the testbed's top-down
   # view (spec 6.13), checked by `shot` and `play --report`.
   [gpu]
   max_draws_per_frame = 8
   max_pipeline_switches_per_frame = 4
   max_bind_group_switches_per_frame = 8
   max_upload_bytes_per_frame = 16384

   [ci]
   pr_tier_minutes = 15
   native_job_minutes = 25
   ```

   Starting values, about four times what the dot needs; tightened at the
   first deload.

7. **xtask, workflows and `.claude/`** (G5), prepared as patch files in
   `docs/rungs/1b/patches/` during T1 and T13:
   (Named X1-X6 so they are not confused with the spec's pending
   decisions P1-P11.)
   - X1 version stamps: `cargo xtask` computes `version.json` (6.13) and
     passes its values to builds as `ENG_*` variables.
   - X2 `cargo xtask ci --job native`: build, tests (with
     `ENG_REQUIRE_GPU=1`), replay with hash upload, crash-json, version,
     shot-exact or shot-flip (reporting "no golden yet" without failing
     while a golden is missing), window, gpu-budgets, esc (Linux x64),
     proof-video (Linux x64), check-intel-mac (macOS); every step in
     `target/ci/report.json` with the adapter and driver.
   - X3 summary: compare hash streams across the native jobs too, and
     measure `native-duration`.
   - X4 acceptance: features with `"owner": true` are listed, never
     computed.
   - X5 selftest cases: `wgpu-outside-gpu`, `winit-outside-platform`,
     `rwh-outside-seam`, `core-depends-on-gpu`, `golden-edit`.
   - X6 `.github/workflows/ci.yml`: the five native jobs; Mesa 25.2.7
     lavapipe (gfx-rs/ci-build build26) with a fixed
     `LP_NATIVE_VECTOR_WIDTH` on `ubuntu-24.04`; WARP from its pinned
     NuGet package on Windows; xvfb and xdotool on Linux; artifacts for
     shots, golden candidates, FLIP maps, `crash.json` and `proof.apng`.
   - `.claude/`: the `golden-update` skill (how candidates reach the
     owner) and the advisory `visual-qa` subagent (spec 7.5; 1a deferred
     both to the rung that needs them).

8. **`protected.toml` and `.github/CODEOWNERS`** (G5), once the files
   pass review: add the harnesses that acceptance features rely on (as 1a
   protected `exit_codes.rs`):
   - `crates/game-testbed/tests/shots.rs` (features 4, 5; T12)
   - `crates/game-testbed/tests/crash_json.rs` (feature 6; T2, T11)
   - `eng-gpu`'s shader translation test (feature 10; T7)
   - the headless-twin Esc test and the windowed-run test in `eng-app` or
     `game-testbed` (features 2, 3; T11)

   Exact paths are given when those tasks finish. Goldens and
   `golden/thresholds.toml` are already covered by `**/golden/**`, and
   `eng-net-wire`'s vectors by `crates/*/tests/golden.rs`.

9. **Goldens** (G6): one golden per shot, from the pinned lavapipe
   configuration, committed to `crates/game-testbed/golden/` with
   `thresholds.toml`; every other adapter is compared against it with
   FLIP. Proposed start: mean FLIP at most 0.01 and 99th percentile at
   most 0.2 per image. `eng-net-wire`'s `tests/golden.rs` arrives as a
   patch in T5 and is applied at G5.

10. **`docs/rungs/current.txt`** (G1): set to `1b` when the owner approves.

11. **Amendments from session 2** (to proposals 1, 2, 3, 4, 7 and 8; the
    text above is kept as written):
    - Proposal 1: the acceptance draft is now the numbered list in
      `plan.md`, "Proposed acceptance criteria". Numbers 1-17 are
      unchanged. Criterion 18 `live-input` is new (test x64, arm64:
      live-input; native x5: tests). Criteria 6, 10 and 11 now name the
      `test`-job steps they rely on (`crash-json`, `shaders`,
      `wire-session`).
    - Proposals 2 and 3 (G2) must land in one commit together with
      skeletons of the seven new crates. `arch-check` fails on a listed
      crate that is not in the workspace (`[E-ARCH-UNKNOWN]`) as well as
      on an unlisted one. So entries added before the crates, or crates
      added before the entries, turn `main` red. New task T0 prepares
      the combined patch (skeletons, entries, `[profile]` lines) and
      tests it in a scratch clone.
    - Proposal 4: the crate list moves into `plan.md`, "Crate batch 2",
      in the next queue item. Until then the list above is the working
      copy. T3 also needs this batch, since the window seam uses
      `raw-window-handle`.
    - Proposal 7, X6 (not X2): the macOS native job in `ci.yml` runs
      `rustup target add x86_64-apple-darwin` before `check-intel-mac`,
      as the RISC-V job already does for its target.
      `rust-toolchain.toml` stays unchanged, so other machines do not
      download the Intel-Mac standard library.
    - Proposal 7, new X7: `cargo xtask ci --job test` gains the named
      steps `crash-json` (`run` only; the test job has no display),
      `shaders`, `wire-session` and `live-input`. Today it has only
      tests, known-answers, exit-codes and replay.
    - Proposal 8, more harnesses to protect: `crates/eng-input/tests/live_input.rs`
      (criterion 18), and the `eng-net-wire` never-panic and
      `eng-session` hash-equality tests (criterion 11). Without that,
      deleting them would leave the criteria green. Exact paths are
      given when T4-T6 finish.
    - **`**/proptest-regressions/**`** (as needed, with G5). When a
      proptest in `eng-input`, `eng-net-wire` or `eng-session` fails, it
      writes a regression file that agents may not commit. The file
      reaches the owner as a candidate together with the fix, and the
      owner commits it.
    - **`.config/nextest.toml`** (as needed, with G5): a test group that
      runs the GPU and window tests one at a time, and a longer
      slow-timeout for them, if T7, T11 or T13 shows they need it.
    - Golden candidates are written under `target/golden-candidates/`,
      never into `golden/`, where the guard would restore and quarantine
      them.
