# Rung 2 progress (append-only)

Rung 2, inside the room: see `plan.md`, a draft the owner has not
approved yet (`docs/rungs/current.txt` stays `1b`). Each session appends
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
- **Amendment 98's first half, the lock in `quick`,** was not started.

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
