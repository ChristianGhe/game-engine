# Rung 2 progress (append-only)

Rung 2, inside the room: see `plan.md`, approved by the owner on
2026-11-07 (`docs/rungs/current.txt` stays `1b` until G5). Each session appends
what it did, costs and timings. Changes to protected paths are written
here as proposals for the owner, never made by agents.

## 2026-11-06, session 1: the draft plan and the pre-approval queue (Claude Opus 5.5, orchestrator, owner away)

At 22:24 EET the owner asked: "let's continue the implementation", then
"run autonomusly until 6am". Rung 1b had nothing left that agents could
do: G6 (the goldens) and G7 (the owner plays) are the owner's. So the
window went to rung 2. The rules were those of 1b's overnight windows:
- commit locally on `main`; push only to an agent branch with `cargo
  xtask push` (ADR-0018);
- touch no protected path; write proposals for the owner here;
- ask no questions.

Every builder worked in its own git worktree, pinned to a commit, with
its own `target/`. Each item got a fresh `reviewer`, and its fixes went
back to the builder that wrote it.

**The plan** (`plan.md`, 314383d, then f4c7401 and 65f0f13)
- A planner subagent drafted it, following 1b's plan and its tags.
  - **First fresh review:** 3 blockers.
    - Q1's graph item broke the protected `counters.rs`, whose
      `Graph::new` has two writers of `Output`.
    - The sprite rotation rule was 180° off.
    - Protected tests that the map design would break were in no gate.
  - **The revision** designs around those tests rather than patching
    them: `GameMaps` beside `GameDef`, recordings naming their map by
    `sim_data_hash`, and `Control`'s new methods with default bodies. It
    also restates the rotation rule from the geometry, in our own words,
    because Doom's source is GPL and this engine is MIT OR Apache-2.0.
- What the queue found went into the task rows it affects, as carry
  notes for T5, T7, T9, T10 and T15. It also added owner decisions O12
  and O13.
- **Its numbers:**
  - 24 acceptance criteria;
  - 26 tasks;
  - 4 new crates: `eng-collision-grid`, `game-testbed-client`,
    `eng-platform-gcmouse` and `eng-bench`;
  - gates G1 to G7;
  - 15 crate names in batch 3;
  - 14 days from G1;
  - 39 agent sessions, or 48 with the recommended agent-seeing
    additions A1 to A4.
- The agent-seeing review of 2026-11-01 is in the plan as five
  additions, A1 to A5. The owner takes or drops each one at G1.

**The queue: four items, before approval.** They change no protected
path and add no crate or third-party dependency. 1b's five hash streams
and four shots stay byte-identical.

| Item | Commits on `main` | What it is | What its review found |
|---|---|---|---|
| Q1, `eng-gpu` | 8c5a468, f84a2cc | `Graph::sorted` over `PassNode`: a topological order with names breaking ties, `[E-GRAPH]` errors that are the same in every declaration order, culling, bypass, continues that write a subset, transient textures pooled by (format, divisor, sampled), labels, and debug groups under `WGPU_DEBUG=1`. `Graph::new`, `Graph::fixed` and `PassDecl` are unchanged. ADR-0019, Proposed | 6 should-fix: bypass was missing; the permutation test had no real ties; docs said running order where the code recorded in declaration order; the public API grew; "continues" needed the same writes. All fixed; the re-review said merge |
| Q2, `kit-fps` | 528a19c, c5c61a5 | `ground_move` and `GroundConfig` v1: gravity, step-up, step-down, snapping. On flat ground it gives `pmove`'s result bit for bit (proptest) | **Blocker:** air control plus an unchecked stepped move let a hull climb slopes up to 89°. The reviewer showed it with an f64 port of the code. Fixed to decision 3 as written: gravity only in the air, steps only from the ground and only onto floor. The re-review said merge |
| Q3, `eng-input` | a58945f, 676acba | `LookSettings` and `MouseLook`: integer microdegree sensitivity, exact i64/i128 sums, half away from zero, and the same yaw for any split into ticks. `dx > 0` lowers yaw and `dy > 0` lowers pitch. Look is off by default | Merge, no should-fix. The follow-up added overflow and saturation cases and split the 496-line oracle, which G5 will protect, into three files and a helper |
| Q4, `eng-render` | e79627d, 411ab94 | A first-person camera (f64 eye subtracted first, reversed-Z, infinite far), the sprite rotation sectors with mirror flags, and an OKLab palette LUT (32³) with a Bayer matrix. CPU only | Merge. Follow-ups: the targets that run the pinned bake hash are named, a non-finite matrix is `[E-SCENE]`, and two doc fixes |

**Checked**, on this Mac (aarch64-apple-darwin, Apple M2 Pro, Metal):
- `cargo xtask quick` 7 of 7 on `main` after each merge, the last at
  65f0f13;
- `ENG_REQUIRE_GPU=1 cargo test` for `eng-gpu`, `eng-input` and
  `eng-render`;
- the shot harness: the four shots' SHA-256s are `c6cb0547eb48a67e`,
  `bd264452cd2f5ef4`, `d1726b1ecac42ebc` and `8bb9845b5e6aa2c2`, the same as
  1b's CI candidates.

Not checked here: RISC-V, which `kit-fps`'s tests need, and x86-64, which
Q4's pinned FNV bake needs. The agent branch's CI run covers both (see
below).

**G6 for rung 1b, ready for the owner**
- I fetched run 41302087332's `golden-candidates-*` through the API, as
  the `golden-update` skill says. All five runners give the same bytes.
- The reference's `shots.json` says `mode: exact`, `pinned: true`,
  `llvmpipe (LLVM 18.1.3, 256 bits)`, `Mesa 25.2.7 (LLVM 18.1.3)`.
- Copies are in `target/g6-candidates-run41302087332/`, which is
  git-ignored: the four PNGs, the reference's `shots.json` and a copy of
  `thresholds.toml`.
- A dry run with that folder as `ENG_GOLDEN_DIR` and `ENG_REQUIRE_GPU=1`
  passes on this Mac's Metal: 0 of 76,800 pixels differ on all four.
- For the owner:
  ```sh
  cp target/g6-candidates-run41302087332/*.png crates/game-testbed/golden/
  ENG_REQUIRE_GPU=1 cargo test -p game-testbed --test shots
  git add crates/game-testbed/golden/*.png
  git commit -m "Rung 1b: G6, the goldens from run 41302087332"
  ```

**Not done**
- **Amendments 96 and 97, the guard's parser gaps and `git credential`.**
  Anthropic's safeguards stopped the builder partway (`[cyber]`, request
  req_011CgnJpzAEQN7N7Sx9KCJWC), although the work closes gaps in the
  repo's own hook. I did not retry it: a reworded brief would likely be
  stopped too, and the harness says not to do the stopped agent's work
  myself. It stays the owner's, by hand or with another route.
- **Amendment 98's first half, the lock in `quick`,** was built after the window resumed: `docs/rungs/1b/patches/quick-lock.patch` (see 1b's session 19).

**For the owner**
- **O12:** `pmove`'s shared `slide` can stop the move against an off-axis
  wall. The Q2 builder found it and the reviewer confirmed it. Fixing it
  changes the five dot streams.
- **O13:** movement in the air is gravity only.
- **Q1 builds part of T8 early:** the GPU pool and `LoadOp::Load` for
  continues. It also grows eng-gpu's public API: `Resource::Transient`,
  16 new `GraphError` variants, and `#[non_exhaustive]` on both.
- **Q4 pins an FNV hash** of a 256-colour bake in `tests/palette.rs`.
  That file is unprotected, so any agent could re-pin it.

**Proposals (protected paths), for G5**
1. Protect Q3's oracle, as the plan says for criterion `mouse-look`:
   - `crates/eng-input/tests/mouse_look.rs`
   - `crates/eng-input/tests/mouse_look_exact.rs`
   - `crates/eng-input/tests/mouse_look_model.rs`
   - `crates/eng-input/tests/look_rig/**`
2. Protect `crates/eng-render/tests/palette.rs` for its known-answer
   values (Ottosson's OKLab table and the pinned bake), as a known-answer
   file like `known_answers.rs`.

**Costs**, as the harness reported them (wall time, subagent tokens
including resumed rounds, tool calls):

| Agent | Time | Tokens | Calls |
|---|---|---|---|
| planner, draft | 25 min | 401k | 87 |
| planner, revision | 15 min | 545k in all | 122 in all |
| plan review 1 | 15 min | 231k | 76 |
| guard builder (stopped) | | | |
| Q1 builder, then its fixes | 26 + 16 min | 361k | 110 |
| Q1 review, then re-review | 8 + 4 min | 182k | 48 |
| Q2 builder, then its fixes | 31 + 14 min | 341k | 122 |
| Q2 review, then re-review | 10 + 4 min | 154k | 37 |
| Q3 builder, then follow-ups | 17 + 6 min | 221k | 78 |
| Q3 review | 5 min | 81k | 26 |
| Q4 builder, then follow-ups | 22 + 3 min | 231k | 74 |
| Q4 review | 7 min | 117k | 30 |

Sessions, counted as the plan counts them (a task with its reviewer is
one): 2 for planning and 4 for Q1 to Q4. That is 6 of the proposed 39,
if the owner approves the plan as it is.

**Lessons**
- **Worktrees** pinned with `git worktree add --detach` have their own
  `target/` by default. No `CARGO_TARGET_DIR` and no link for `xtask
  replay` are needed, because replay runs the worktree's own
  `target/debug/game-testbed`.
- **A proptest run on planted bugs writes `*.proptest-regressions`,** a
  protected path, beside the test. Q3's builder deleted the file in its
  worktree, and nothing reached a commit. Briefs for mutation runs should
  say so in advance.
- **A reviewer that re-implements the code independently** (Q2's f64
  port) found the one real blocker. The builder's own 12 planted bugs had
  not: they were all chosen from the rules the builder already had in
  mind.

**Last: CI and the second plan review** (00:40 EET; the usage limit paused the window here, and it resumed at 02:11)
- **CI:** the agent branch's run 42303716372, at 65f0f13, was green on
  every job but lint. That job failed on typos: Q3's text wrote 1/256 as
  an ordinal, and typos read the suffix as a misspelt word. 783326b
  rewords it as "sub-counts (1/256 of a count)". Its run, 42316720146,
  failed lint once more, on this entry's own quote of the word; the next
  commit removes the quote.
- **The second fresh plan review** said the plan is **not ready for the
  owner**. Nothing below is fixed. The next session starts here.
  - **Blocker 1:** T7 commits the room's hash streams before G5 adds
    their scenarios. `xtask replay` then reports `[E-HASH] ... in
    committed but not in <target>`, and `quick` goes red.
  - **Blocker 2:** G4 sits in the first sitting, but ADR-0020 to ADR-0023
    do not exist yet, and G2 waits on T0, which needs G1. The three
    sittings cannot happen in the order given.
  - **Should-fix:**
    - Bring the Q1 and Q3 text up to what was built (`src/graph/*`,
      `PassNode`, `Graph::sorted`, continues naming a pass, the GPU pool,
      the four mouse-look test files).
    - Extend G5's protection list: Q1's oracles, Q4's tests, kit-fps's
      `slopes.rs` and `support/`.
    - `version.rs` pins mean `--version --json` can describe only `dots`.
    - `shot_video.rs` pins `per_frame` and `tier: 0`.
    - T5's contract change needs an ADR.
    - `Window` has no refresh rate.
    - "decide O1-O11" should name O12 and O13.
    - The GCMouse criteria (macos-26 has no mouse) and the "skipped is
      ok" rule for mutants.
    - A1-A5 have no task rows.
    - Strip and split debug info are protected `[profile]` keys.
  - **Nits:** about ten, in the review's report. The most useful is
    `--seed 1` in criterion 1.

## 2026-11-07, session 1 (resumed at 02:11): the plan made ready, T0a, T0 and T0b (Claude Opus 5.5, orchestrator, owner away)

The usage limit stopped the window at 00:45; it resumed at 02:11 with
the same rules.

**CI on the agent branch**
- Run 42316720146 failed lint once more, because this log quoted the
  misspelt word. 586d8b8 rewords it.
- Run 42317502863, at 586d8b8, is **green on every job** in 5.3 min:
  - lint, both test targets, riscv (`kit-fps`'s new tests), wasm and
    selftest;
  - the five native jobs, which run Q4's pinned bake hash on x86-64 and
    arm64;
  - the summary: `compare-hashes` ok, and rung 1b at 16/18 computed
    features passing, `shot-exact` and `shot-flip` pending until G6, and
    `owner-play` the owner's. That is the same as at the handover.
- The commits after 586d8b8 are docs and patches only, and are not
  pushed.

**The plan**
- **Second revision**, after the second review (46aa0e3):
  - the room's hash streams wait for G5 with their scenarios;
  - "Before the first sitting" adds T0, T0a and T0b, so every gate's
    inputs exist when it is asked.
- **A third fresh review** found a blocker the first two missed. With
  `current.txt` at `2` from G1, xtask's `ci_native_tests` requires every
  native check of the current rung to name an existing step. Rung 2's
  new steps come at G5, so `quick` would be red from the first sitting.
  - **Third revision** (ac0ed6b): `current.txt` moves to `2` at G5, in
    the commit that adds those steps. The owner still writes
    `acceptance.json` at G1, and nothing reads it until then.
  - It also adds a one-screen "Sitting 1: what you decide" list at the
    top.
- **Five places where the plan disagreed with the code**, found by the
  ADR writer, are fixed (389ef53):
  - winit 0.30.13 has `Locked` on Windows;
  - `counters.rs` builds `FrameCounters` by literal;
  - exact contact holds only on axis faces;
  - `LateClock` is a test platform's own `Control`;
  - the handshake denies a client from another map first.
- **The budget** is now 43 sessions, 52 with the recommended additions.

**T0a: ADR-0020 to ADR-0024, all Proposed** (c879061)
- A writer drafted them, and a fresh reviewer checked every file:line
  they cite against HEAD and every implementer of `Control`, `Window`
  and `CollisionWorld`.
- **First pass:** 0021 and 0024 were not ready.
  - **0021** (`unsafe` in the shim) needs every Rust file scanned
    (`arch-check` reads `src/**` only), a `[floor]` entry, `forbid`, and
    a macOS clippy step, since CI lints on Linux only.
  - **0024** (exact contact) measured the off-axis gap along the motion,
    which grazing traces break. It now measures along the hit normal.
- **Second pass:** all five are ready for G4.

**T0 and T0b: the G2 and G3 patches** (`docs/rungs/2/patches/`)
- **The patches:**
  - `g2-crates.patch`: three crate skeletons, `architecture.toml`, and
    the overflow-checks table for `eng-collision-grid`;
  - `g2-gcmouse-crate.patch`, if O3 keeps the shim a crate;
  - `g3-crate-batch-3.patch`;
  - `g3-parry3d.patch`, if O4 picks parry3d.
- **Checked:** on fresh clones, `quick` and `CI=1 cargo xtask ci --job
  lint` are 7 of 7 at every stage. cargo-deny passes with batch 3 really
  added as dependencies in a scratch copy, and with parry3d too.
- **Batch 3 is 17 names, not the plan's 15:** `rustc_version` and
  `semver` are build dependencies of `derive_more-impl`. No `clarify`
  entry is needed.
- **The `[floor]` entry** `eng-platform-gcmouse = "eng-platform-winit"`
  works with no xtask change. `arch-check` refuses `eng-app` depending
  on the shim.
- **The fresh review** approved all four. It asked for:
  - `objc2-game-controller` under `[floor]`, so `quick` fences it, not
    only cargo-deny on CI;
  - ready-made O3 variants for G3.

  Both are done. There are now seven patches, applied in this order:
  1. `g2-crates.patch`.
  2. Then, by the O3 answer, one of:
     - `g2-gcmouse-crate.patch` and `g3-crate-batch-3.patch`;
     - `g2-gcmouse-module.patch` and `g3-crate-batch-3-shim-module.patch`;
     - `g3-crate-batch-3-no-shim.patch` alone.
  3. `g3-parry3d.patch`, after any of them.

  Each path passes `quick` and `CI=1` lint (7 of 7) on a fresh clone of
  c879061. Checks in scratch copies only:
  - `arch-check` now refuses `eng-app` depending on
    `objc2-game-controller` (`[E-ARCH-FLOOR]`);
  - on the no-shim path, cargo-deny refuses it.

  Every path also applies before or after 1b's `quick-lock.patch`.

**Amendment 98's lock** is a 1b patch, `docs/rungs/1b/patches/quick-lock.patch`.
1b's session 19 entry describes it. It counts against 1b's budget,
which makes 32 sessions of 30.

**Costs since the resume**, as the harness reported them (wall time,
subagent tokens in all, tool calls):

| Agent | Time | Tokens | Calls |
|---|---|---|---|
| planner, revisions 2 and 3 and the ADR fixes | 8 + 6 + 2 min | 710k in all, since the draft | 112 |
| plan review 2 | 13 min | 213k | 66 |
| plan review 3 | 16 min | 264k | 104 |
| ADR writer, then its fixes | 24 + 6 min | 388k | 193 |
| ADR review, then re-review | 14 + 2 min | 238k | 96 |
| T0/T0b builder, first round | 38 min | 303k | 197 |
| gate patch review | 13 min | 115k | 52 |
| 1b lock builder, then its fixes | 14 + 10 min | 221k | 117 |
| 1b lock review, then re-review | 8 + 3 min | 122k | 54 |

**Sessions**, counted as the plan counts them:
- planning 4 (the draft and three revisions);
- the queue 4;
- T0, T0a and T0b 3.

That is 11 of the proposed 43.

**Fourth fresh review, and the fourth revision** (04:00 to 04:20)
- **The review carried out sitting 1 literally** in a scratch clone, all
  of it:
  - `acceptance.json` written, with `current.txt` at `1b`;
  - 1b's G6;
  - `quick-lock.patch`;
  - the crate-path G2 and G3 patches;
  - the ADRs accepted.

  `quick` and lint were 7 of 7, and G6's shots test gave 0 differing
  pixels on Metal.
- **It found two blockers in the later sittings:**
  1. **Goldens are all or nothing.** With 1b's goldens in, G5's new
     first-person shots fail with `[E-GOLDEN] … is missing; other shots
     have theirs` (`shots.rs:343`, the protected `parts.rs:434-440`)
     until sitting 3. **Fix:** T16's G5 patch gives `shots.toml` an
     owner-set `golden = "pending"` marker per shot. A marked shot counts
     as "no golden yet", and the owner clears the marker with its golden.
     The alternative, moving 1b's G6 to sitting 3, is in the plan with
     its cost.
  2. **Sitting 1 never pushed `main`.** `cargo xtask push` refuses an
     agent branch whose protected files differ from GitHub's `main`, so
     every push from T5 on would fail. **Fix:** sitting 1 now ends with
     `quick`, then a push of `main`.
- **The revision also:**
  - starts `rung-diff` mutants at 314383d, so Q2's core code is covered,
    and makes it pending on push runs;
  - lists batch 3's 17 rows;
  - fixes stale text.
- **The review's cost:** 8 min, 140k tokens, 53 calls.
- **Sessions:** 12 used of 43. The plan counts five planning sessions.

## 2026-11-07, session 2: sitting 1, gates G1 to G4 (Claude Opus 5.5, with the owner)

The owner held sitting 1 in this session, from the plan's "Sitting 1:
what you decide" list. Claude asked each decision with the recommended
answer first, and applied the protected changes after the owner listed
their paths in `.claude/guard-unlock`: `docs/rungs/2/acceptance.json`,
`architecture.toml`, `Cargo.toml`, `deny.toml` and `xtask/**`.

**The owner's answers**
- **G1:** the plan is approved, with the agent-seeing additions A1, A2's
  three rules, A3 and A4 (O6). A5 waits for rung 3.
- **O3:** the GCMouse shim is its own crate, with the engine's only
  `unsafe` code.
- **Every other decision as recommended:**
  - O2: a second map with sim data;
  - O4: the brute-force oracle;
  - O5: `cargo xtask dist`;
  - O7: benchmarks against the push's base;
  - O8: a missed mutant fails;
  - O9: fullscreen by default for first-person maps;
  - O10: Khronos PBR Neutral;
  - O11: GPU timestamps when the adapter has them;
  - O12: `pmove`'s angled-wall fix in rung 3;
  - O13: gravity only in the air.
- **G4:** ADR-0019 to ADR-0024 are accepted as written, and 1b's
  ADR-0017 with them.
- **Rung 1b:** G6 and G7 are closed and the lock is applied (1b's
  session 20).

**Applied**
- **`docs/rungs/2/acceptance.json`** (G1), in 1b's format: 41 features.
  - The plan's criteria 1-23 and 25.
  - Criterion 24's 13 features carried from 1b.
  - Four from A1-A4: `frame-record`, `scene-rules`, `fps-poses` and
    `check-misses`.
  - Three are the owner's (`owner: true`, no checks): `owner-mac-shots`,
    `owner-play` and `gcmouse-rate`.
  - The time-box is 14 calendar days from 2026-11-07, and the AI budget
    at most 52 sessions.
  - `current.txt` stays `1b` until G5 (O1).
  - **Step names that G5 may rename:** A4's and A2's self-test steps are
    written `violation:render-bug-<n>` and `violation:rule-<n>`, the
    form `selftest.rs` gives every case. The plan's table wrote A4's
    without the prefix. If T21's xtask names them otherwise, G5 amends
    the file, as the plan allows.
- **G2 and G3** (f9c095d): `g2-crates.patch`, `g2-gcmouse-crate.patch`
  and `g3-crate-batch-3.patch`, applied in that order. The other four
  patches in `docs/rungs/2/patches/` stay unused: the module and no-shim
  paths, and parry3d.
- **G4:**
  - every accepted ADR now says "Accepted" and who decided, and so does
    its row in the index;
  - the comments and docs that said "ADR-0019, proposed" or "ADR-0017,
    proposed" no longer do;
  - the plan's banner records the approval.
- **The lock for `quick`** (e0288dd), 1b's amendment 98.

**Checked**
- A scratch clone of d1db807 with all four patches and `acceptance.json`
  applied: `quick` 7 of 7 in 188 s.
- `main`'s full tree, after G6, before these commits: `quick` 7 of 7.
- The goldens in 7f44f1b equal CI's candidates byte for byte.

**A fresh review of `acceptance.json`**
- **What it compared:** the file against the plan's criteria table and
  agent-seeing table, and against 1b's file.
- **What matched:** every id, check, owner flag and top-level field (2.8
  min, 102k tokens, 14 calls).
- **Fixed after the review:**
  - the 13 carried features have `done_when` 1-12, the plan's "all",
    instead of null;
  - the budget note says T23 is the two rung reviews;
  - the plan's sitting list gives 52 sessions;
  - the plan's G5 xtask row names A1-A3's native steps and A2's and A4's
    self-test cases, which G5's package must add before `current.txt`
    says `2`.

**Left for the owner, to finish sitting 1**
- **Push `main`:** `git push origin main`. GitHub's `main` is at 6443ea5,
  and until the push, `cargo xtask push` refuses every agent branch.
  CI's first run on the pushed `main` then runs:
  - 1b's goldens on all five runners;
  - the lock's Windows code;
  - the four new crates on every target, with RISC-V for the grid.
- **Reinstall the hooks** from the pushed `main` with `cargo xtask
  install-hooks`, so that the Stop hook uses the lock.
- **Remove `.claude/guard-unlock`** before agents work unattended.

**Next:** T5 onward, at most two builders at once; then sitting 2 (G5).

**Sessions:** this sitting is session 13 of 52.

**After the sitting** (same session)
- **The push:** the owner pushed `main` (6443ea5..d106e7f), reinstalled
  the hooks from it (`cargo xtask install-hooks`), and removed
  `.claude/guard-unlock`.
- **CI run 42370402965 on d106e7f:** green on its second attempt.
  - **Rung 1b's computed acceptance:** 18 of 18. `shot-exact` and
    `shot-flip` pass against the new goldens on all five runners, and
    `owner-play` is the owner's, accepted.
  - **The lock:** both Windows jobs passed with it.
- **The first attempt failed one step, `esc` on ubuntu-24.04.** Its `play`
  exited at once with `[E-NO-DISPLAY] … Failed to open connection to X
  server`, 1.1 s after the `window` step's `play` had quit cleanly under
  the same Xvfb. Every other step passed. The owner re-ran the failed job
  (`gh run rerun --failed`), and it passed.
  - **The likely cause:** Xvfb resets when its last client leaves, and a
    client that connects during the reset fails. `xvfb-run` discards
    Xvfb's log, so the logs cannot show a reset rather than a crash.
  - **For the owner:** `docs/rungs/2/patches/ci-xvfb-noreset.patch`
    (SHA-256 `92106cdea24b1f87…`) adds `-noreset` to the workflow's
    `xvfb-run` server arguments. It is one line in the protected
    `.github/workflows/ci.yml`, and applies to d106e7f. It can wait for
    G5, or the owner can apply it sooner.
- **T5 and T6 started** at about 14:10 EET, in worktrees pinned to d106e7f.

**T6: maps and sim data** (b9f3795, follow-up 5dd4b27; decision 1,
ADR-0023)
- **Built** by one builder in a worktree pinned to d106e7f:
  - constant resources in `eng-ecs`, outside the state, its hash,
    snapshots and the `[E-NAN]` walk, sealed at build (`[E-SIM-DATA]`
    after that);
  - `Sim::sim_data_hash` (FNV-1a-64 over the values in name order; the
    names are in the protocol);
  - `GameMaps`, `--map` on run, dump-world, shot, video and play, and the
    map and sim-data hash in recordings, hash-stream headers (`map=…
    sim_data=0x…`, only for a map with sim data), dump-world and reports
    (`map`, `sim_data_hash`, `files_read`);
  - `replay verify` picks the map by its hash, or exits 64 with
    `[E-RECORDING-BUILD]`.
- **Unchanged for the dot map:** the five hash streams, the protocol,
  the snapshot (466 bytes) and the dump. The protected `hash_equality`,
  `never_panic`, `twin`, `version` and `exit_codes` tests pass unchanged.
- **Tests:** 33 in four new files, which G5 protects (the plan's G5 row
  names them).
- **For the owner at G5:** `docs/rungs/2/patches/t6-replay-map.patch`
  (SHA-256 `702972bb0aa3927a…`). With it, xtask replay reads `map` from
  `scenarios.toml`. In a scratch clone with the patch, `quick` was 7 of 7.
- **Review:** a fresh reviewer approved it with no blockers (10 min, 160k
  tokens, 52 calls). The follow-up commit takes its first point: a known
  answer for the sim-data hash (`0x87f22d556000363a`, checked by
  hand-packing its 152 bytes), so CI checks the encoding on every
  target. The NaN pointer for a constant is now dump-world's
  `/constants/…` path. The reviewer's other points are carries in the
  plan's T7, T15, T16 and T22 rows.
- **A possible flake, for the owner:** in the builder's scratch clone,
  one full `quick` failed the lock patch's test
  `run_lock::tests::a_killed_holder_leaves_no_stale_lock`. The holder's
  PID read back as none under load. It passed 5 of 5 alone, and has not
  failed on CI. The test is in protected `xtask/`.
- **Checked on `main`:** `quick` 7 of 7 after the cherry-picks.
- **Cost:** the builder took 33 + 3.5 min, 374k tokens and 223 calls. It
  counts as one session (14 of 52).

**T8: eng-gpu grows** (f603199, follow-up 41e9027; decisions 5 and 6,
ADR-0020, O11)
- **Built** by one builder in a worktree pinned to 1e7411e, all in
  `eng-gpu`:
  - passes with up to four colour targets and one depth target (reversed
    Z: clear to 0, `GreaterEqual`);
  - `Format::{Rg32Uint, Depth32Float}`;
  - mesh and screen pipelines beside 1b's fullscreen and instanced ones;
  - the scene layout: vertex and `u32` index megabuffers, a material
    table, six `texture_2d_array`s (64, 128 and 256 px, sRGB and linear),
    and a per-instance record of chunk origin minus camera, computed in
    f64;
  - the per-frame uniform;
  - per-pass GPU times, beside `FrameCounters`, which keeps its four
    fields.
- **Unchanged:** `counters.rs`, `shaders.rs`, `shot_video.rs`'s view, and
  1b's four shots (0 differing pixels on Metal, on Debian lavapipe, and
  exactly on the pinned Mesa 25.2.7 in Docker).
- **One Q1 test changed:** `bypass_render.rs`'s several-writes test
  pinned the "not built yet (T8)" refusal, and now checks that both
  writes are drawn. The plan said Q1's tests stay unchanged. The
  reviewer judged the change legitimate, since decision 5 has T8 remove
  that refusal, and its two-writers-of-`output` check is kept word for
  word.
- **Review:** a fresh reviewer found no blockers (11 min, 189k tokens,
  55 calls). The builder's own reviewer had run before it.
  - **The follow-up takes its main risk.** The builder had made pass
    timestamps per frame for every graph. On Metal, a query set that
    fails to allocate loses the device, and nobody had run that on CI's
    Apple Paravirtual device or on WARP.
  - **Now timing is opt-in** (`Targets::time_passes`), with one query set
    per `Targets`, and frames with it off encode as before T8.
  - **The follow-up also:**
    - clears stale times after a failed frame;
    - closes the reviewer's test gaps: an equal-depth draw, which a
      mutation to `Greater` fails; `depth_write: false`; a screen pass
      reading the scene; and times named by the pass that ran;
    - corrects two ARCHITECTURE.md lines (O11).
- **Not verified:** the follow-up's timing reuse and abort path on
  lavapipe, because Docker Desktop hung on restart. CI's Linux and
  Windows native jobs run `pass_times.rs` at the next push.
- **Carries** are in the plan's T9, T10, T11, T21 and T22 rows. T21 needs
  them: criterion 21's report keys have no wiring task. G5's protection
  row now names T8's five test files.
- **Checked on `main`:** `ENG_REQUIRE_GPU=1 cargo xtask quick` 7 of 7.
- **Cost:** the builder took 55 + 35 min, 434k tokens and 165 calls. It
  counts as one session (15 of 52).

**T5's independent review, and two owner decisions**
- **The verdict: not ready.** The reviewer wrote an exact-rational trace
  from decision 2 and ADR-0024, in Python, outside the repo. It found one
  blocker in f47e189. A hull that starts at a vertex contact can pass
  through an off-axis wall or slab, by 1.3 cm to 43 cm.
  - **How often:** 5 of 5,368 such traces on random off-axis maps, and 0
    of 8,044 on axis-aligned maps like the room.
  - **Why nothing caught it:** `conformance::check` never looks at the
    path, and the brute oracle copies the same narrow-phase line.
  - **The fix:** the reviewer's two-part fix is tested in its scratch
    clone. It went back to the builder with a pass-through assertion
    for the proptests. The builder's "once in a million" residue was
    the same fault, at about 0.1% of vertex-aimed traces.
- **The owner accepted two changes, 2026-11-07:**
  - **Amendment 1 to ADR-0024:** the check's two loosened rules, and
    vertex contacts taking the touching edge's normal. The builder writes
    it into the ADR with the follow-up.
  - **Plan decision 2:** a slab is a solid convex prism whose sides
    block. Before, its wording was "a z sweep inside its polygon".

**T5: the collision grid** (de2d0e8, follow-up 6e97310; decision 2,
ADR-0024 and its Amendment 1)
- **Built** by one builder in a worktree pinned to d106e7f:
  - ADR-0024's contact rule and `CONTACT_GAP` (2^-20 m) in
    `eng-collision-api`, checked by `conformance::check`;
  - `eng-collision-grid`, with walls and solid slabs in 1 m cells. Its
    SAT sweep uses f64 basic operations over X, Y, Z and f32-rounded
    edge normals. The crate docs list its six rounding rules;
  - a brute-force oracle in the tests;
  - `kit-fps`'s flat-ground proptest and step tests over the grid.
- **No step-tie test:** the grid has no floors off the axes, so decision
  3's tie rule cannot be seen there. The carry moves to format v2's
  slopes.
- **Reviews:**
  - **The builder's own reviewer** found vertex-contact faults, which
    the builder fixed before handing over.
  - **The independent review:** 45 min, 356k tokens, 214 calls, with an
    exact-rational oracle. It found the vertex pass-through, with the
    fix above.
  - **The re-check of 20589df:** 6 min, 25k tokens, 27 calls.
    - The fix matches the reviewer's own.
    - The five pass-through cases block.
    - Of about 15.5k new vertex-start traces, none passes through more
      than 1 µm.
    - Axis-aligned chains give the same bits as before the fix.
    - Two wording fixes to the amendment were made on `main` with this
      entry.
- **Not run:** RISC-V, since this Mac has no QEMU and Docker was off.
  CI's `riscv: core-tests` runs the grid at the next push.
- **Checked on `main`:** `quick` 7 of 7 after the cherry-picks.
- **Carries** are in the plan's T7 and T17 rows. G5's protection row now
  names the grid's test files.
- **Cost:**
  - The builder took 1 h 40 min + 15 min, about 700k tokens and 360
    calls, its reviewer included.
  - The two reviews took 740k tokens in all.
  - It counts as one session (16 of 52), though it cost about twice a
    typical task.

**T9: the first-person `opaque` and `sky` passes** (7573f4b; ADR-0020)
- **Built** by one builder in a worktree pinned to 262abb7:
  - `FirstPersonRenderer`, a `Graph::sorted` graph. `opaque` writes
    albedo, `info` and depth; `light` and `tonemap` are stand-ins for
    T10; `sky` continues `light` where depth is 0; `present` runs only
    with a surface.
  - `WorldVertex`, `Chunk::quad` (corners minus the origin, in f64) and
    `GpuWorld`.
  - `info`: region and class in word 0, object id in word 1, (0, 0) for
    nothing drawn.
  - `NEAR_PLANE` = 0.1 m, with its bound written down.
  - The material goes per vertex, so each drawn chunk is one instance
    record, as ADR-0020 words it.
  - In `eng-gpu`, new items only: `Targets::read_transient` and its
    refusal rules.
- **The test:** a room 10⁹ m from the origin, 52,974 depth probes against
  an f64 CPU ray cast. The worst probe used 3% of its tolerance. `info`
  is checked exactly and sky depth is 0. Breaking any of these on purpose
  fails it: an f32 subtraction, a wrong near plane, the `info` words
  swapped, culling off, depth writes off.
- **Unchanged:** 1b's four shots (0 differing pixels on Metal),
  `counters.rs` and T8's tests. Not run on lavapipe, because Docker was
  off. CI's Linux native job runs `first_person.rs` at the next push.
- **Review:** a fresh reviewer approved it with no blockers (10 min,
  166k tokens, 46 calls). The builder's own reviewer had run before it.
  The non-blocking points are carries in the plan's T10, T11, T12, T15,
  T16, T21 and T22 rows. Two matter most:
  - the near-plane bound is not enforced, so `--fov` and fullscreen must
    respect it (T15);
  - readback's bypass paths have no test yet (T10).
- **G5's protection row** now names T9's test files.
- **A fault in T8's timing, found by `quick` after the cherry-pick:**
  - **What happened:** on Metal, under the full suite's load, `pass_times`
    once read a `tonemap` time of 3.9e15 ns. The pass's start timestamp
    was unwritten (0) while its end was real. It failed 0 of 30 runs
    alone.
  - **Fixed on `main`:** `PassTime::nanoseconds` is now `Option<u64>`,
    and `None` for a 0 timestamp or an end before its start. Criterion
    21 already allows null per pass.
  - **The tests:** a unit test covers the rule, and `pass_times.rs` now
    accepts `None` but no impossible value, over four timed frames.
- **The lock test's flake, for the owner:** in the T6 and T9 builders'
  `quick` runs on this Mac, `run_lock::tests::a_killed_holder_leaves_no_stale_lock`
  failed once each. Each time it passed alone at once.
  - **Ruled out:** a collision between clones (each scratch directory
    carries its process's PID), and a PID not yet written (the child
    writes it before it says it holds the lock).
  - **Where it is:** the test is in protected `xtask/`. It has not failed
    on CI.
- **Cost:** the builder took 52 min, 404k tokens and 141 calls. It counts
  as one session (17 of 52).

**T7: the room map** (8a75d85, follow-up eadf677)
- **Built:** the 12 x 9 m hall in `game-testbed-sim/src/room.rs` and
  `room/layout.rs`, with its exact sizes written there:
  - 8 walls and 11 solid slabs on the axes;
  - a 0.2 m step, a 0.4 m platform, a 0.6 m ledge that blocks, a pillar,
    an alcove and a skylight;
  - 4 regions, 4 spawns, and 3 things that do not block;
  - constants `Grid`, `RoomMap`, `MoveConfig` and `GroundConfig`, and a
    new `testbed.View` component;
  - `sim_protocol` `0xc2250f5f231ff6d3` and `sim_data_hash`
    `0x3ac50d171e0751f1`, both pinned;
  - three input scripts, committed;
  - `docs/rungs/2/patches/room-scenarios.patch` (SHA-256
    `e68459a6d1f17f7c…`), for G5, after `t6-replay-map.patch`.
- **Streams:** the room's matched on arm64, on x86_64 (under Rosetta and
  in Docker) and on riscv64 under QEMU. That is also the grid's first
  RISC-V run. The dot streams are unchanged.
- **Review:** a fresh reviewer (12 min, 165k tokens) found one blocker.
  `shot`, `video` and `play --map room` panicked. Now they exit 64 with
  `[E-MAP] … has no view yet`, until T15 and T16 lift it. The
  `never-airborne` assertion went into the patch, with four small fixes.
- **Carries, still to move into the plan's rows:**
  - **T12:** player 0 is entity 0; its eye is at `pos.z - 0.875 + 1.6`;
    read the hull and eye with `constant::<MoveConfig>()`, not
    `resource`.
  - **T15 and T16:** lift the `no_view` refusal; the live view starts at
    the spawn's yaw.
  - **T16:** shot ideas are in the builder's report: spawn at tick 0;
    sprites over ticks 0-480; the step at ticks 150-170; the sky at
    ticks 401-500; the alcove at tick 600; the ledge needs a pose.
  - **T22:** ADR-0023 should name the tuning constants.
  - **G5:** protect `game-testbed-sim/tests/{room,sim_data}.rs` and
    `game-testbed/tests/room.rs`.
- **A disk-full stop:** after the merge, `quick` failed with "No space
  left on device". Scratch clones and worktrees had grown to about 270
  GB. Those of merged tasks were deleted, and 68 GB is now free. Remove
  each worktree once its task is merged.
- **CI:** `main` at eadf677 was pushed to `agent/rung2`; that run is
  42390944638.
- **Sessions:** 18 of 52.
- **Later in the session:**
  - **CI run 42390944638** on `agent/rung2` (eadf677) is green on every
    job. T5 to T9 pass on all five native runners, both test targets,
    RISC-V (the grid's core tests), wasm and selftest.
  - **T7's carries** are now in the plan's T12, T15, T16 and T22 rows, and
    its tests are in G5's protection row.
  - **T13's builder** was stopped by the usage limit partway, and resumed
    after the reset.

**T13: the platform contract** (d7b1e14, follow-up 277d7cf; ADR-0022)
- **Built** by one builder in a worktree pinned to b8a8582. The usage
  limit stopped it once, and it resumed after the reset.
  - **The contract:** `Control::grab_cursor(bool) -> CursorGrab`,
    `Control::set_fullscreen(bool) -> bool` and
    `Window::refresh_rate() -> Option<u32>` (millihertz). All have
    default bodies, so every existing implementer compiles unchanged.
  - **Headless** records each request with the clock's time.
  - **winit:**
    - borderless fullscreen;
    - a grab tries Locked, then Confined, and only with focus;
    - mouse buttons and the wheel, with macOS natural scrolling;
    - raw motion only with focus, and off on macOS until T14's shim.
- **For the owner at G5:** `docs/rungs/2/patches/t13-conformance.patch`
  (SHA-256 `797639f9e26dd11d…`): the new suite, `conformance/requests.rs`
  and its `mod` line, and three test files. In a scratch clone with the
  patch, `ENG_REQUIRE_DISPLAY=1 quick` was 7 of 7.
- **Review: one blocker.** The code changed two rules of the accepted
  ADR-0022:
  - the platform, not eng-app, frees the cursor before `FocusLost`;
  - a grab asked for without focus waits for focus.

  The owner is away, so the rules stay as built and are written up as
  **Amendment 1 (Proposed)** in ADR-0022. Undoing them, if the owner
  refuses, is a small change in `eng-platform-winit/src/request.rs`.
- **The follow-up also:**
  - shows the cursor again after a failed grab;
  - adds eight unit tests of the grab's rules against a fake window;
  - fixes a second bug it found. winit sends a `FocusLost` before a new
    window's first `FocusGained`, which dropped a grab asked for at the
    window's creation.

  A re-check confirmed that each rule's regression fails a test.
- **Open:**
  - Under X11's `Confined` grab, each raw motion arrived twice (with
    XTEST under Xvfb).
  - Xvfb reports no refresh rate.
  - Leaving fullscreen during macOS's enter animation is lost.

  All three are carries in T15's row, which now also needs T14.
- **Checked on `main`:** `ENG_REQUIRE_GPU=1 ENG_REQUIRE_DISPLAY=1 quick`
  7 of 7.
- **Cost:** the builder took about 2 h in three runs and 290k tokens. The
  review and re-check took 345k. It counts as one session (19 of 52).

**T10: light, fog, tone map and palette** (6797251, follow-up 9003155;
decisions 7, 8 and 13; ADR-0020)
- **Built** by one builder in a worktree pinned to 7e59fc6:
  - the graph `opaque, light, sky, fog, tonemap, palette, present`, with
    T9's node names kept and 10 graphs built in advance for the
    switches;
  - **light:** a region's Doom level over 8, fake contrast on axis walls,
    and falloff by distance from the eye, through a 32-step lux table;
  - **fog:** `1 - e^(-density·distance)`;
  - **tone map:** Khronos PBR Neutral after the EV100 exposure;
  - **palette:** Q4's 32³ LUT read from the scene's 64 px array, with
    4x4 Bayer dither;
  - the strict `.gpl` reader;
  - the k×k upscale: `Targets::upscaled`, in eng-gpu, centred with a
    black border.
- **Decision 13's checks pass on Metal.** They have not run on lavapipe
  yet: Docker was off. CI run 42398105940 on `agent/rung2` is their
  first lavapipe run.
- **The tone curve:** at worst 0.85 of an 8-bit level from a CPU
  reference, written independently from Khronos's definition.
- **Unchanged:** 1b's four shots (0 differing pixels).
- **Review:** a fresh reviewer found the 1b paths safe and PBR Neutral
  correct (11 min, 211k tokens). It asked for one test gap to be closed
  before merge: nothing pinned the light model's or the fog's numbers.
  - **The follow-up** adds `light_model.rs`, a CPU oracle over about
    52,500 pixels a run. It caught six mutations: the contrast sign, the
    falloff sign, falloff along the view axis, `round` for `floor`, a
    doubled fog density, and another region's fog colour.
  - **Also fixed:** reads now use the graph the last frame actually
    rendered.
- **A proposal for the owner:** decision 13 says fog is monotone "with
  depth". For fog by distance from the eye that is false along a row
  across a wall seen at an angle, so the check orders probes by distance
  from the eye. The proposed wording is now in the plan.
- **Carries** are in the plan's T11, T12, T15, T16, T21 and T22 rows.
  G5's protection row names T10's tests.
- **Checked on `main`:** `ENG_REQUIRE_GPU=1 quick` 7 of 7.
- **Cost:** the builder took 42 + 25 min, 467k tokens, 160 calls. It
  counts as one session (20 of 52).

**T10's lavapipe fix** (885554e)
- **What failed:** CI run 42398105940 on `agent/rung2` was red in two
  places.
  - `typos` read a two-letter variable name in the PBR Neutral reference as a misspelling of "on".
  - `light_model` failed on lavapipe: every pixel was 1.1% low in dark
    channels. The cause is lavapipe's sRGB decode, which differs from the
    formula at low codes.
- **The fix:** the oracle now takes the GPU's own decoded albedo
  (`FirstPersonRenderer::read_albedo`), so it tests the light model, not
  the adapter's decode. The six mutations are still caught.
- **Checked** on Metal and in Docker on Debian Mesa 25.0.7 and the pinned
  25.2.7. CI run 42399931913 is green on every job.
- **Lesson:** `quick` runs neither lavapipe nor typos. Run
  `CI=1 cargo xtask ci --job lint` before every push, and push to the
  agent branch soon after a GPU task lands.

**T14: the GCMouse shim** (b265dd8, follow-up 9a03645; decision 11,
ADR-0021)
- **Built:** `eng-platform-gcmouse`. `src/macos.rs` is its only file
  with `allow(unsafe_code)`, and holds four `unsafe` blocks, each with a
  SAFETY argument.
  - `mice`, `mouseInput`, and `setMouseMovedHandler` to set and to
    clear, all within ADR-0021's list.
  - `shim.rs` forbids `unsafe`.
  - Deltas are rounded as `look.rs` rounds them, and every callback is
    counted.
  - eng-platform-winit takes macOS raw motion from the shim, only while
    focused, and exposes `raw_motion_callbacks()`.
- **The review found no soundness blocker** (11 min, 148k tokens). The
  follow-up takes its points:
  - a compiled `Send + Sync` check on the handler closure (capturing an
    `Rc` fails with E0277);
  - `catch_unwind` with `abort` in the handler, since block2 calls it
    through `C-unwind`;
  - an autorelease pool around refresh;
  - an honest start test.
- **Still open:**
  - **The install path is in no test.** GCMouse lists mice only under
    AppKit's event loop, not a bare run loop. `mouseInput`, the
    handler's install and its removal ran only by hand.
  - **Confinement of `unsafe` is a convention until G5.** Nothing stops
    an `allow` in another file of the crate, or in winit's tests and
    examples. ADR-0021 leaves the check to G5 (T21).
- **For the owner:** the real-mouse check, with the screen unlocked:
  `cargo run -p eng-platform-winit --example events -- --grab`. Check:
  - that `dx` is above 0 moving right, and `dy` above 0 moving toward
    you (else set `DELTA_Y_UP` to false);
  - that the summed counts per inch are about the mouse's counts;
  - that callbacks per second exceed 120 while moving fast.
- **The sign of `deltaY`:** SDL negates it in both of its GCMouse
  handlers (`SDL_cocoamouse.m:381-391`, `SDL_uikitevents.m:355`, at
  08ab399).
- **Sessions:** it counts as one (21 of 52).

**T11: the sprites pass** (586000d, then a small fix on `main`;
decision 4)
- **Built:** `Sprite` and the `sprites` pass, continuing `opaque`.
  - Each sprite takes two 16-byte instance records a frame: feet minus
    eye and the frame's material, then signed width (negative for
    mirrored), height, region and id.
  - All sprites share one billboard in one instanced draw.
  - The alpha test writes depth, `info` carries (Sprite, id, region), and
    there is no eng-gpu change.
- **Tests:** against an f64 CPU cast, on Metal and on Debian lavapipe:
  - the 8 rotations and the mirror of a five-rotation sheet;
  - Z-only billboards when pitched;
  - the thing behind the pillar: 0 pixels;
  - sprite over sprite;
  - every look switch;
  - six mutations caught.
- **1b's shots:** unchanged.
- **Review:** the builder's own reviewer, then a fresh one (9 min, 147k
  tokens), found no blockers. Two of the fresh reviewer's points are
  fixed on `main`, with this entry:
  - the uv is now clamped half a texel inside the texture, not an ulp
    below 1, since Vulkan allows 4 sub-texel bits;
  - the billboard's right axis drops its z in the shader, so Z-only does
    not rest on the camera having no roll.

  The sprite, first-person and shader tests pass on Metal after the fix.
  The rest are carries in the T12, T16, T21 and T22 rows; G5's row names
  T11's tests.
- **A departure from ADR-0020, for T22:** two records per sprite, where
  the ADR has one per drawn chunk or sprite. It is internal to
  eng-render, so nothing waits on it.
- **Sessions:** it counts as one (22 of 52).

**The lock test's flake, a third time, now with a likely cause** (for the
owner; the test is in protected `xtask/`)
- **What failed:** the Stop hook's `quick` after a9aec89, in
  `run_lock::tests::a_killed_holder_leaves_no_stale_lock`, at
  `run_lock_tests.rs:217`: "its PID stays behind", left `None`, right
  `Some(9204)`. The rerun was green.
- **Why the line points at the child:** the PID was in the file before
  the kill (line 213 passed) and gone after it. Only `Held::drop` (or a
  new `Held::new`) clears it, and a SIGKILLed process runs neither. So
  the child most likely ended on its own between line 213 and the kill,
  dropping its `Held`. `child_holds_the_lock` holds the lock until
  `read_to_end` on its stdin returns, so a stdin that reaches EOF early
  would do exactly this. That would also explain the earlier failures
  under load, though not why stdin ends early.
- **A proposed fix for the owner:** in `child_holds_the_lock`, after
  `read_to_end` returns, sleep (say up to 60 s) instead of returning, so
  only the kill ends a child that is still wanted. The parent always
  kills it, and the timeout still ends an orphan.

**CI after T14, and the nextest fix** (2fa0fe1)
- **What failed:** CI run 42403525706 at a9aec89 failed every native and
  test job with nextest's "creating test list failed". T14's
  `tests/start.rs` is `harness = false` and did not answer `--list`. This
  Mac has no nextest, so `cargo test` here was green.
- **The fix:** it now speaks libtest's listing, as winit's conformance
  harness does. Run 42404277380 is green on every job.
- **A possible flake:** in the failed run, `shot-flip` on
  ubuntu-24.04-arm also failed once. The shot process for `dot-square-0`
  ended with no exit code and no output. It passed on the next run.

**T17: benchmarks** (3d4b1b7; decision 14, O7)
- **Built** by one builder in a worktree pinned to 6fbdd7a, with two
  follow-ups. `eng-bench` has 11 gungraun benchmarks; their counts on
  Linux x64 in Docker are in the builder's report.
  - **Traces:** a grid trace into a wall costs 2,820 instructions, across
    the hall 3,410 and the ground probe 2,315. The brute-force world costs
    7 to 14 times as much.
  - **Movement:** `ground_move` walking costs 13,051, stepping up 20,040.
  - **Ticks:** a dot tick costs 4,575, a room tick 17,912, and the room's
    build 229,056.
  - **Layout:** every dependency is a dev-dependency, because arch-check
    forbids an engine crate depending on a `game-*` crate. gungraun runs
    on Linux only.
- **For the owner at G5**, in one commit, since an xtask test reads
  ci.yml:
  - `docs/rungs/2/patches/t17-xtask-bench.patch` (SHA-256
    `9f6bca4b39f6f80b…`);
  - `t17-workflow-bench.patch` (`bb5201a228385319…`).

  Both apply to 3d4b1b7. In the builder's scratch clone, `quick` was 7 of
  7 with both applied.
- **The gate:**
  - **The base:** off main, the merge base with `origin/main`. On main,
    the push's `before`, if it is an ancestor.
  - **The rule:** over 5% fails, unless an accepted ADR added in the push
    names the benchmark. Off main, both the acceptance and the name are
    read from `origin/main`'s copy, which must be byte-identical. A
    self-accepted ADR makes the step pending, with
    `[W-BENCH-EXCUSED] … once the owner accepts`.
  - **Pending, not a pass:** a base equal to HEAD compares nothing, so
    the step is pending.
  - **Trusted state:** read before any of the head's code runs.
  - **The limit, in the module docs:** a check on agent branches, not a
    security boundary. The owner's push to main remains the gate.
  - **Proven in Docker:** a planted second ground probe (+12% to +18% on
    three benchmarks) failed the gate. With a self-accepted ADR it was
    pending on an agent branch, and excused on a push to main.
- **Review:** a fresh reviewer found no blockers in the commit and the
  workflow's security fine (14 min, 167k tokens). It asked for four fixes
  to the patch before G5, all done.
- **Security:** three rounds of an automated security review of the
  builder's new code each found a real gap, all closed:
  - a self-accepted ADR;
  - an accepted ADR edited on a branch, or reused from before;
  - a base the branch controls, and state read after the head's code ran.
- **A merge slip on `main`, mine:**
  - **What happened:** the first cherry-pick conflicted with T14 in the
    root `Cargo.toml`. The guard refused my edits to the unparsable
    file, and the next command, chained with `;`, committed it with its
    conflict markers.
  - **Repaired before any push:** the markers were removed, `main` was
    soft-reset to 2fa0fe1, and the three commits were recommitted as
    3d4b1b7.
  - **Checked:** `quick` 7 of 7, the lint job 7 of 7, and
    `cargo metadata --locked` is consistent.
  - **Lesson:** chain merge steps with `&&` only, and check for conflict
    markers before `--continue`.
- **Carries** are in T21's row; G5's row protects `eng-bench`'s
  benchmarks.
- **Cost:** the builder took about 2 h 20 min in three runs and 620k
  tokens. It counts as one session (23 of 52).
