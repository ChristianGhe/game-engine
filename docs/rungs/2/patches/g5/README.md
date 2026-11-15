# The G5 package (rung 2, T21)

Gate G5 is sitting 2's one set of protected-path changes: every reviewed
patch in `docs/rungs/2/patches/`, stacked, with small rebases where two of
them meet, T21's own additions and T23's fix, as 25 patches that apply
in order. The owner applies them in their own terminal (the guard refuses an agent's
`git apply` of protected paths), checks, commits once (O1: `current.txt`
becomes `2` in G5's commit), and pushes.

Built and checked in a scratch clone of b433f85 on the M2 Pro, macOS 26,
Metal (2026-11-15). Linux, Windows and the pinned lavapipe were not run:
the first CI run after the push (T24) is their first.

## Before applying

Main must have T21's three ordinary commits, which touch no protected
path (3893051, bac7064 and d992062 on T21's branch, on b433f85, before
the commit that adds this directory):

- 3893051: the shot and video reports' `gpu.pass_times` and
  `gpu.transient_textures` (criterion 21, T8's carry: the last frame
  rendered, a video's at the last multiple of `--every`, is timed with
  `Targets::time_passes(true)`, then `Targets::frame_stats`), with
  `crates/game-testbed/tests/gpu_report.rs`;
- bac7064: ARCHITECTURE.md's "What gate G5 adds", naming what the
  package builds (docs-lint reads it once patch 06 is in);
- d992062: `eng-platform-api`'s `keys!` macro passes doc comments only
  (`#[doc = $doc]`), where it passed `#[$meta]`, an attribute that is
  itself a macro variable, which patch 20 refuses.

The patches also apply on b433f85 itself (they touch none of those
files), but without the first commit `gpu-budgets` fails on every runner
for want of the two keys, and without the third `arch-check` fails on
`key.rs`.

## The patches, in order

"Reviewed" means the file is byte for byte the reviewed patch of the same
name in `docs/rungs/2/patches/` (its SHA-256 is in PROGRESS.md). "Rebased"
means its `+`/`-` lines are the reviewed patch's except where it meets an
earlier one, as listed; nothing else changed.

| # | File | SHA-256 | From | What it changes |
|---|---|---|---|---|
| 01 | `01-ci-xvfb-noreset.patch` | `92106cdea24b1f872c8f271fc1ac9187254f1668eb6d5592568bdc89e0b40c6a` | reviewed | `ci.yml`: Xvfb with `-noreset`, so a window started while it resets can connect (the `esc` flake, T12) |
| 02 | `02-t6-replay-map.patch` | `702972bb0aa3927abaea45623df68cf03504fe767fbef2eb8730ad4a014979ed` | reviewed | `xtask/src/replay.rs`: replay reads each scenario's `map` from `scenarios.toml` (T6) |
| 03 | `03-room-scenarios.patch` | `e68459a6d1f17f7cb2d165657461d903033cdde1a0598d6606514c8f7a0b0672` | reviewed | the room's three scenarios, their assertions and four hash streams, in `scenarios.toml` (T7; needs 02) |
| 04 | `04-t13-conformance.patch` | `797639f9e26dd11d87edb5281fbfdb4d72b663ba0867c869deb8d40b8e3c016e` | reviewed | the platform's cursor and fullscreen requests suite, `conformance/requests.rs`, and three test files (T13) |
| 05 | `05-t16-shots.patch` | `f053ac5a3daed2e40bf271cadfdb91ec7871b7257cf8097e447437e60bd90ec6` | reviewed | the shots harness reads `shots.toml`, the `golden = "pending"` marker, `[flip.<shot>]`, `tests/look.rs` (T16, TA3) |
| 06 | `06-t18-docs-lint.patch` | `7138e5de2811121cb444678fbac3a65c363b6b6ff41e169100d663b12cef5115` | reviewed | docs-lint in `docs-check`, selftest `docs-stale-path` (T18) |
| 07 | `07-t17-xtask-bench.patch` | `9f6bca4b39f6f80b23ea797e6a4cc6ffaa79c0019a082ada65ddcbddeadbe619` | reviewed | `cargo xtask bench`, `ci --job bench` (T17; with 08 in one commit) |
| 08 | `08-t17-workflow-bench.patch` | `bb5201a2283853195b508978b9ef8b2a5d1ac22977b3c79ec5e4fa4af4cb4973` | reviewed | the `bench` job (valgrind, gungraun-runner 0.20.0) (T17) |
| 09 | `09-t19-xtask-mutants.patch` | `1df06ba805ac80c1377c5821a1a4394e44b46fc63fd327cd1b8a3471468f876b` | rebased | `cargo xtask mutants`, `ci --job mutants`, the summary's `mutants-shards`, selftest `mutant-missed` (T19) |
| 10 | `10-t19-workflow-mutants.patch` | `367ddb3aec1e28777b8bcdef6b97d0caba35ba9459e58c1e89fb1a4214e4bb4f` | rebased | the four-shard `mutants` job, the weekly schedule and dispatch, cargo-mutants in `selftest` (T19) |
| 11 | `11-t19-mutants-config.patch` | `66e6d4bc367e2a7814bd679afbf80cc8d990dd7d1099d57a196d42956ddc4b14` | reviewed | `.cargo/mutants.toml`, exclusions empty (T19) |
| 12 | `12-t19-test-auditor.patch` | `4c1873ad29516749130ddd1b8bba045493a69fb01733a8bb66750a52c4303ae3` | reviewed | the `test-auditor` subagent (T19) |
| 13 | `13-t20-xtask-notices-dist.patch` | `26d7ad77edc9e1809374d213673fb393e941622176791e28c863134d282e0341` | rebased | `cargo xtask notices` and `dist`, the lint job's `notices`, the native `dist` (T20; with 14 and 15 in one commit) |
| 14 | `14-t20-about.patch` | `3fb1ccae1646934c44dbda180f177f08737ba1251c5d656c246a28406cdb8ff9` | reviewed | `about.toml` and `about.hbs` (T20) |
| 15 | `15-t20-workflow.patch` | `e1c0be694542a1beb4040096a15b5538d231009244628fae1d3b914f12c86305` | reviewed | cargo-about 0.9.3 in lint and three native runners, the `dist-<runner>` archives (T20) |
| 16 | `16-t22-skills.patch` | `3a4fcf95ff135c5e3b53eda6637987837884cfb174729a33ce9214e34207edee` | reviewed | the `golden-update` skill for `shots.toml` and frame records, the `perf-investigate` skill (T22) |
| 17 | `17-ta2-scene-rules.patch` | `904c2e9716eefca29da38cec71fd360fb0c3f1eaf96ce24c37a289e7b8a97a28` | rebased | `room.rules.toml`, the strict `tests/scene_rules.rs`, selftest `rule-1` to `rule-3` (TA2) |
| 18 | `18-ta4-render-bugs.patch` | `12bcc5bc3bd83f9c3d30fca63355f7646de380c85467380521d85e519d76e5d1` | rebased | 12 planted render bugs, `known-misses.toml` (empty), selftest `render-bug-1` to `-12` (TA4; needs 17) |
| 19 | `19-t21-arch-scan-chars.patch` | `8dcfe7f042bd76eb6fda07a1b02e75aff37a60cc0d403093cf6647d8e520333e` | T21 | the tokenizer's char literals, strings' escaped line ends and raw identifiers (a fix to 1b's `arch_scan.rs`) |
| 20 | `20-t21-unsafe-scan.patch` | `1e9b6439785d23a1b764f85c6c7794bd22d5dd0d66dba8ad6290f11cb54d57c0` | T21 | `arch-check`'s `[E-ARCH-UNSAFE]` scan, `architecture.toml`'s `unsafe_file`, selftest `unsafe-outside-shim` and `gcmouse-outside-shim` |
| 21 | `21-t21-ci-steps.patch` | `98c0442473890da883ccd7eabfb3d6e9cbc1fc04f1f8df510a04064113139f47` | T21 | the test, native, lint and summary steps, their verdicts and tests; two file splits |
| 22 | `22-t21-workflow.patch` | `6bb4e0da82c301dcd77247aad8e4a19f8999f43ba34921e2bb9805f8df1274f6` | T21 | the shader cache off job-wide, three new artifacts, the summary's download patterns; the skill names an artifact |
| 23 | `23-t21-budgets-protection.patch` | `de76320efd0c5dff294323ca5614f5f5b264e4b1f31c645625d61cf32bd3ddf1` | T21 | `[gpu.first_person]`, `[ci]` minutes, `protected.toml` and `CODEOWNERS` |
| 24 | `24-t21-rung-2.patch` | `b38ed92901dbcb28a3caa26497eea049504229f04090372cd02daf2d82335792` | T21 | `current.txt` to `2`; `acceptance.json`: criterion 19's check, `scene-rules`' wording |
| 25 | `25-run-lock-flake.patch` | `d364bbdfc0bed2b255186cae697d334dd2439d444ee8e4b84d203ad0dddfaa77` | T23 | `run_lock_tests.rs`: the killed-holder test keeps the child's stdin open until it has reaped the child (the lock-test flake; below) |

**25, the lock-test flake.** `run_lock::tests::a_killed_holder_leaves_no_stale_lock`
failed now and then under load on this Mac (`run_lock_tests.rs:217`, "its
PID stays behind", left `None`), most recently in the Stop hook's `quick`
on 2350d7b. The cause: `Child::wait` closes the stdin it holds before
waiting, so a child that the kill has not stopped yet reads end of input,
drops its lock and clears its PID. The test now takes the stdin out and
drops it after `wait`; a panic before the kill still drops it, so the
child never outlives the test. Measured with 16 copies at once: the old
test failed 258 of 1,200 runs, all at line 217 with `None`; the new one
failed 0 of 1,200. No other patch touches the file, so 25 applies
anywhere in the series, or on `main` alone.

The rebases, in full:
- **09** (T19 xtask) after T17's and T18's: `ci.rs` `JOBS` holds both new
  jobs (`[&str; 10]`: `… native bench mutants summary all`), so does
  `main.rs`'s usage line; `selftest.rs` `CASES` is `[&str; 24]` with
  `mutant-missed` under T18's comment, and its dispatch arm follows
  `docs-stale-path`'s. Applied alone, the reviewed patch merged cleanly
  but wrongly (`JOBS` and `CASES` one short).
- **10** (T19 workflow) after T17's: `summary` and `ci` need both `bench`
  and `mutants`.
- **13** (T20 xtask): the same lines as reviewed; only the context of
  `main.rs`'s `mod` list and usage (T18's and T19's lines) moved, so the
  reviewed file needs `git apply --3way`.
- **17** (TA2) after T18's and T19's: the module doc keeps both T19's
  "cargo-mutants" and TA2's "any adapter"; `CASES` is `[&str; 27]`
  (22 + 1 + 1 + 3), `mod selftest_rules;` follows `selftest_mutants`.
  TA2's `copy()` stamping is kept.
- **18** (TA4) after TA2's: `CASES` is `[&str; 39]` (+ 12), its dispatch
  arm follows the rules' arm, `mod selftest_render;` sits between
  `selftest_mutants` and `selftest_rules`. TA4's own render target is
  kept.

## What T21 adds (patches 19 to 24)

- **19, a fix to 1b's tokenizer** (`xtask/src/arch_scan.rs`), from a
  security review of T19's skip scan, which uses it: after `'\` the lexer
  skipped one character and then searched for a `'`, so `'\''` ended at
  its escaped quote, and in `['\'','"']` the real closing quote opened a
  literal `','` and the `"` after it a string, which could hide a
  `mutants::skip` or an `allow(unsafe_code)` later on the line. Char
  literals now end where rustc ends them (`\'`, `\\`, `\n`, `\t`, `\r`,
  `\0`, `\"`, `\x` and two hex digits, `\u{…}`); lifetimes, labels and
  `b'…'` byte literals are told apart; anything that does not close is a
  lone `'`. Two new tests fail on the old lexer: the literals, and a
  `mutants::skip` and an `allow(unsafe_code)` after each such literal on
  the same line, found by both scans. From T21's review, also: a
  string's escaped line end (`\` then a newline) is counted, so later
  findings keep their line numbers; and a raw identifier (`r#unsafe_code`)
  is its plain name, not `r` with the `#` dropped.
- **20, ADR-0021's scan**: `arch-check` reads every `.rs` file of every
  workspace crate (`src`, `tests`, `examples`, `benches`, build scripts,
  `xtask`), and fails with `[E-ARCH-UNSAFE]` on:
  - the name `unsafe_code` anywhere but inside `deny(…)` or `forbid(…)`:
    an `allow`, `warn` or `expect` (`cfg_attr` forms too), and also a
    macro's argument, which a macro could put in an allow; and a `$`
    that a macro could fill with any lint: anywhere inside an attribute
    that names `allow`, `warn` or `expect` at any depth (`#[allow $x]`
    and `cfg_attr` forms too), in an attribute that is itself a macro
    variable (`#[$meta]`; the one such today, `keys!` in
    `eng-platform-api`, now passes doc comments only, commit d992062),
    or in such a list outside an attribute. The one exception is the
    leading inner attribute
    `#![allow(unsafe_code)]` of the file `architecture.toml` names for
    the shim (the ADR's new key, `unsafe_file = "src/macos.rs"`, which
    `macos.rs` has today at line 56, after its doc comments);
  - in that file, a module declaration, a macro (`macro_rules!` or
    `macro`, which could hand its allow to code elsewhere), or a
    `#[path]`; or a named file that does not exist, or more than one;
  - `include` in any file, by any path or alias: every `include`
    identifier outside strings and comments, so `use core::include as
    inc;` fails too (none exists today; `include_str!` and
    `include_bytes!` are other names);
  - a `#[path]` in any file that has a backslash (rustc decodes the
    string's escapes before it reads the path, so the scan refuses
    rather than guess), or that does not name, from its file's
    directory, a workspace crate's `.rs` file (which this scan reads
    too), or that names the exempt file, or that sits inside an inline
    module (where rustc looks in a subdirectory; none does today). The
    `#[path]`s there today all pass: xtask's own child modules (28 of
    its files), kit-fps's, eng-collision-grid's and game-testbed's test
    support files, and two that reach another crate's tests,
    `crates/eng-bench/benches/collision/main.rs` and `tests/cases.rs`
    (eng-collision-grid's brute-force oracle) and
    `crates/game-testbed/tests/look.rs` (eng-render's `tests/tone/`).
  - Known limit: a proc-macro crate (`eng-derive`) could build the
    lint's name from a string; review covers that.

  Selftest `unsafe-outside-shim` plants thirteen, in one copy, and fails
  on a finding missing or one it did not plant: an allow in the
  testbed's `main.rs` (which only denies), a `cfg_attr` expect in a test
  file, a warn in the shim's safe `shim.rs`; in `macos.rs` a `mod` and a
  `macro_rules!` handing out an allow (the review's case); a
  `#[path = "hidden.txt"]` with an allow in the file it names; another
  crate mounting `macos.rs` through `#[path]`; an `include!` in
  `eng-cli`'s build script; a lint list taking `$l`;
  `planted!(unsafe_code)`; `#[$m]` in eng-gpu; `use core::include as
  planted_include;` in eng-render; and a `#[path]` with a backslash in
  an eng-input test. `gcmouse-outside-shim` (eng-platform-winit
  takes `objc2-game-controller`) needs `[E-ARCH-FLOOR]` and nothing else
  from arch-check, then cargo-deny's wrapper ban. `selftest.rs` was
  split (`selftest_floor.rs`); `CASES` is `[&str; 41]`.
- **21, the CI steps**:
  - `test`: `collision`, `movement`, `sim-data`, `render-graph`,
    `sprites` and `mouse-look` (`ci_harness.rs`, split out of `ci.rs`;
    each step's test files are listed there). `sprites` is
    `sprite_rotation` alone: the test job has no GPU, so `sprites` and
    `sprites_look` run in the native `look` step, where criterion 8's
    native check is.
  - `native` (`ci_native_fps.rs`, `ci_native_mac.rs`): `poses` (the
    shot step's `shots.json` against `shots.toml`'s `[pose]`), `look`
    (eng-render's `look_checks`, `look_fog_tone`, `sprites`,
    `sprites_look` and the testbed's `look`), `frame-record`
    (`frame_record` and `frame_bugs`; exact on the pinned lavapipe on
    Linux x64; pending until G6's references), `scene-rules` (never
    `ENG_RULES_DIR`; reads `rules.json`), `window-room` (criterion 1's
    command line against `hashes/room-walk.seed1.txt`, and criterion
    10's `window` keys), `clippy-shim` on macOS (`cargo clippy -p
    eng-platform-gcmouse --all-targets -- -D warnings`), `gpu-budgets`
    also rendering `shot --map room --at-tick 60 --budgets budgets.toml`
    and holding it and the room window to `budgets.first_person`, with
    its `pass_times` and `transient_textures` (pending, never a pass,
    without `window-room`'s report or with no frame presented), and
    `proof-video` adding `room.apng` (601 frames). `dist` stays last.
    The GPU steps set `MESA_SHADER_CACHE_DISABLE` too, and the pinned
    adapter on Linux x64 as the shots do. `scene-rules` fails on a
    `rules.json` without its failures list.
  - `summary`: `bench-duration` and `mutants-duration`, against
    `budgets.toml [ci]`; a bench job skipped by its `if` (the weekly
    schedule) is pending.
  - `lint`: `palette` runs `room_gpl.py --check room.gpl` (T11's carry:
    otherwise a hand edit to `room.gpl` passes every test).
  - Tests: `ci_native_tests.rs` holds the new step order per runner,
    rung 2's 102 native checks once `current.txt` is `2`, and the
    workflow's new artifacts, download patterns and shader-cache switch;
    `ci_harness.rs` holds every `test` and `selftest` check of the
    current rung to a step that exists.
  - `dist_tests.rs` was split (`dist_tests_binaries.rs`); with it and
    `ci_harness.rs`, `ci_native_mac.rs` and `selftest_floor.rs`, no
    xtask file is over 500 lines.
- **22, the workflow**: `MESA_SHADER_CACHE_DISABLE: "true"` job-wide in
  `native` and `selftest`; new artifacts `frame-candidates-<runner>`,
  `scene-rules-<runner>` and `proof-video` (ubuntu-24.04: both videos,
  their reports and frame records, about 50 MB, now kept out of
  `report-native-<runner>`); the summary downloads only `hashes-*` and
  `report-*`, not the dist archives, candidates and logs. The
  `golden-update` skill names `frame-candidates-<runner>`.
- **23, budgets and protection**: `[gpu.first_person]` 28/28/60/2,944
  (four times the room shot's 7/7/15/736, as `[gpu]` is for the
  top-down view; play's presented 8/7/16 fits); `[ci]`
  `bench_job_minutes = 15` and `mutants_job_minutes = 35`, placeholders
  that no CI run has measured (T24 sets them; budgets.toml says so), and
  `mutants_shard_minutes = 20`. `protected.toml` gains 85 paths: the 82
  that G5's protection row names (`about.toml` and `about.hbs` among
  them; T12's client tests as `src/**/tests.rs`, which takes
  `src/room/geometry/tests.rs` too), and three beyond it (item 6
  below). `.github/CODEOWNERS` gains the same 85, in the same order.
- **24, rung 2**: `docs/rungs/current.txt` is `2`; `acceptance.json`
  gains criterion 19's `{"job": "summary", "step": "mutants-shards"}`,
  and the `scene-rules` description says what TA2 built (proposed
  wording, below). No other job or step name drifted: every check names
  a step that exists.

## What the owner decides at G5

1. **TA2's rules** (`crates/game-testbed/scenes/room.rules.toml`, patch
   17) and the `scene-rules` wording in patch 24. It changes four things
   in the approved text: "contrast at least 0.1" becomes an OKLab
   distance of at least 0.03 and a luminance at least 0.2 of the
   surfaces'; "no first-person pixel shows the clear colour" becomes no
   void below the horizon and no sky but within a shot's bounded
   exception; "floor shows in the lower third and ceiling or sky in the
   upper third" becomes a share of at least 0.1 of each, with bounded,
   reasoned exceptions; and "Every shot passes" becomes "Every
   first-person shot passes". In full:
   > Agent seeing A2: protected <scene>.rules.toml files (the room's:
   > crates/game-testbed/scenes/room.rules.toml), judged over frame
   > records by eng-cli's scene_rules, with three rules: an object a
   > shot names in must_see has at least 50 visible pixels, an OKLab
   > distance of at least 0.03 between its mean colour and its
   > surroundings', and a mean luminance at least 0.2 of that of the
   > surfaces in view, so one drawn black fails; nothing shows where the
   > world should be: no void below the horizon, and no sky but where a
   > shot's bounded exception gives the skylight's; the floor covers at
   > least 0.1 of the lower third, and ceiling or sky at least 0.1 of
   > the upper third, a shot excused only within a bound and with a
   > reason. Every first-person shot passes, and each rule has a
   > self-test that breaks it.
2. **Render bug 3's known miss** (TA4): add it to
   `xtask/selftest/render-bugs/known-misses.toml` with the reason, the
   date and your name, or reclassify it as caught by TA2's rule 1. Until
   then `selftest render-bug-3` fails, and so would CI's strict selftest
   job: decide before the push.
3. **Mutation exclusions** (T19b, `t19b-mutants-exclusions.md`):
   `.cargo/mutants.toml` is empty, so criterion 19 fails on the weekly
   run until the 54 missed mutants are killed or excluded: (1) a grid
   task now for breaks (A) and (B) (proposed), (2) exclude all 54 and fix
   in rung 3, or (3) keep the criterion failing.
4. **ADR amendments** (T22, dated 2026-11-15, all Proposed): ADR-0020;
   ADR-0022 Amendment 1 (T13: the platform frees the cursor before
   `FocusLost`; a grab asked without focus waits for it) and Amendment 2
   (T15: scripted runs never grab, departing from decision 10's
   wording); ADR-0023; ADR-0024 Amendment 2 (T19b's still-axis rule, and
   the open breaks); ADR-0012's rung 2 note (decision 17's findings).
5. **Notices** (T20): keep or drop the Rust standard library section;
   accept the 24 crates whose MIT text keeps the template's unfilled
   copyright line, or add clarifications later.
6. **T21's own choices**: the `unsafe_file` key in `architecture.toml`
   (ADR-0021 asked for it there); the videos in their own `proof-video`
   artifact; the shader cache off in the selftest job too; and three
   protected paths beyond the plan's row:
   `crates/game-testbed/tests/gpu_report.rs` (criterion 21's keys),
   `crates/eng-input/tests/look_between_ticks.rs` (criterion 9's
   `mouse-look` step runs it), and `crates/eng-gpu/tests/rig/**` (the rig
   of T8's seven protected eng-gpu tests). The sweep: every test file a
   named acceptance step runs (the test job's harness steps, the native
   job's `look`, `frame-record`, `scene-rules`, `crash-json`, `esc` and
   shot steps), and every support module those files declare, is now
   protected; the whole-workspace steps (`tests`, `core-tests`) were not
   swept file by file. `crates/eng-cli/src/view.rs`'s tests are inline
   (not split out), so the row's "if split out" adds nothing.
7. **Not in the package, for later**: the lint job's `palette` step
   checks `room.gpl` against its script, but neither is protected, so an
   edit to both passes; protecting them is the owner's call. On the
   weekly schedule criterion 15 shows "no report from job bench", since
   bench is skipped there (T19's carry). The selftest job's time on CI's
   4-vCPU runner (5 min here) and the mutants shards' 20-minute box are
   T24's to measure.

## Apply and verify

From the repo root, on main with T21's three ordinary commits:

```sh
for p in docs/rungs/2/patches/g5/[0-9][0-9]-*.patch; do
  git apply --index "$p" || { echo "stopped at $p"; break; }
done
cargo xtask quick
CI=1 cargo xtask ci --job lint
cargo xtask ci --job test
ENG_CI_STEPS=shot-flip,poses,look,frame-record,scene-rules,window,window-room,gpu-budgets,check-intel-mac,clippy-shim,dist \
  cargo xtask ci --job native    # the screen unlocked, for the two windows
cargo xtask selftest unsafe-outside-shim
cargo xtask selftest gcmouse-outside-shim
cargo xtask ci --job selftest    # about 5 min here; render-bug-3 fails until item 2
git commit -m "Rung 2: G5, the package (docs/rungs/2/patches/g5/)"
```

Then push `main` (`git push origin main`), and T24 reads the first run.

## What was run, in the scratch clone (Metal)

After the two review rounds (2026-11-15):

- **The series**: all 24 apply in order with plain `git apply --index` on
  a clean checkout of b433f85, and on T21's three ordinary commits; with
  those commits, the tree is byte for byte the scratch clone's that the
  checks below ran on.
- **`cargo xtask quick`**, `current.txt` at `2`: 7 of 7, its tests
  including `ci_native_tests.rs`'s check that each of rung 2's 102 native
  checks names a step the native job runs on its runner, and
  `ci_harness.rs`'s that every `test` and `selftest` check does too.
- **`CI=1 cargo xtask ci --job lint`** (`--locked`, `-D warnings`): 9 of
  9, `notices` and `palette` included; docs-lint finds every name T22
  left as plain words (the test, native and selftest steps, the room shot
  under `[gpu.first_person]`, `room.apng`, the `unsafe_code` scan,
  `frame-candidates-<runner>`, and `shots.toml`, `about.toml` and the
  benches protected).
- **`cargo test -p xtask`**: 227 passed, 1 ignored (the run-lock test did
  not flake this time). `arch-check` on the tree: no violations.
- **`cargo xtask selftest unsafe-outside-shim`** (thirteen plants, each
  matched to its own finding, nothing unplanted) and
  **`gcmouse-outside-shim`**: ok.
- **`cargo xtask ci --job test`** (first round): 15 of 15, the six new
  steps 2.6 to 21.2 s each.
- **`ENG_CI_STEPS=… cargo xtask ci --job native`** on Metal, the screen
  unlocked: `poses`, `look`, `scene-rules` (seven shots), `window`,
  `window-room` (600 ticks, the committed `room-walk.seed1.txt`, 600
  frames of 800x600 presented, refresh rate 60,000 mHz, no grab asked),
  `gpu-budgets` (the room shot 7/7/15/736 and the room window's
  8/8/16/736 within `[gpu.first_person]`, seven passes timed, five pooled
  textures) and `clippy-shim` ok; `shot-flip` and `frame-record` pending
  (no goldens or references yet). Earlier, `check-intel-mac` and `dist`
  ok. `proof-video` runs on Linux x64 only; its room command, run by
  hand, wrote 601 frames, 38 MB, in 9 s.
- **`cargo xtask ci --job selftest`** on Metal, about 5 min: 39 of 41
  ok; `golden-edit` pending (it needs the pinned lavapipe);
  `render-bug-3` fails by design, "awaiting the owner's acceptance of a
  known miss" (decision 2 above).
- **`cargo xtask replay`**: 9 runs, the five dot streams and the room's
  four byte for byte; no stream was regenerated.
- **Not run here**: Linux, Windows, the pinned lavapipe (exact shots and
  records, `esc`, `proof-video`), and the bench and mutants jobs, which
  their own patches' builders ran (T17 in Docker, T19 on this Mac).
