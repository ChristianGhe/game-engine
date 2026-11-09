# T19 follow-up: the rung's mutation gaps, and the exclusions proposed

For the owner at G5, with `t19-mutants-config.patch`, which adds
`.cargo/mutants.toml` with both lists empty. This is not a patch: that file
exists only after the patch is applied. Nothing below is excluded until the
owner adds it.

## The runs

cargo-mutants 27.1.0 over `git diff 314383d` of the core crates' `*.rs`, as
`t19-xtask-mutants.patch` runs it (`--in-diff`, `PROPTEST_RNG_SEED=1`,
`--cap-lints true`, the core crates as `--package`, each mutant tested
against its own package's tests), on this Mac (macOS, arm64), `-j 3`:

- Before (8797105): 776 mutants in 18 min: 588 caught, 148 missed, 7 timed
  out (counted as caught), 33 unviable. Missed: 117 in eng-collision-grid
  (`sweep.rs` 78, `grid.rs` 22, `shape.rs` 11, `lib.rs` 3, `GridError`'s
  `Display` among them), 15 in eng-collision-api (`conformance.rs`), 11 in
  kit-fps (`ground.rs`), 3 in eng-ecs, 1 in eng-sim, 1 in game-testbed-sim.
- After (this commit): 776 mutants in 17 min: 681 caught, 55 missed, 7
  timed out (the same 7), 33 unviable. The 55 are the 25 equivalent and the
  30 open ones below; no other mutant is missed. (kit-fps alone, after its
  new tests moved to `tests/steps.rs`: 68 mutants, 2 missed, the two
  equivalent.)

93 of the 148 now have a test that catches them. 25 are equivalent (below,
with the exclusion proposed). 30, all in the grid's thin-solid rule (step 4
of the crate docs), are neither caught nor shown equivalent (the last
section); criterion 19 passes only if the owner excludes those too.

## Equivalent: the exclusions proposed (`exclude_re`)

Each pattern leaves the line out, as the config's comments ask. Where the
same operator in the same function has another mutant that a test catches,
the pattern keeps the column (`:[0-9]+:COL:`), which moves only when that
line itself is edited. Two patterns keep the line as well, because the same
operator sits at the same column in a caught mutant of the same function;
they must follow edits above them. Each pattern was checked against
`cargo mutants --list` of the rung's diff: it matches only its mutant(s).

```toml
exclude_re = [
  # kit-fps, ground.rs
  "ground\\.rs:[0-9]+:23: replace \\|\\| with && in step_up$",
  "ground\\.rs:.*: replace > with >= in step_down$",
  # eng-collision-grid, grid.rs
  "grid\\.rs:[0-9]+:40: replace - with / in Grid::swept$",
  "grid\\.rs:[0-9]+:62: replace && with \\|\\| in Grid::each$",
  "grid\\.rs:.*: replace < with <= in Grid::onto_faces$",
  "grid\\.rs:[0-9]+:32: replace - with / in axis_range$",
  "grid\\.rs:.*: replace && with \\|\\| in axis_range$",
  "grid\\.rs:.*: replace > with >= in axis_range$",
  "grid\\.rs:.*: replace < with <= in axis_range$",
  "grid\\.rs:[0-9]+:87: replace < with <= in <impl CollisionWorld for Grid>::trace$",
  "grid\\.rs:.*: replace > with >= in settle$",
  # eng-collision-grid, lib.rs (MARGIN) and shape.rs
  "collision-grid/src/lib\\.rs:[0-9]+:25: replace / with [%*]$",
  "shape\\.rs:[0-9]+:22: replace - with / in add_axes$",
  "shape\\.rs:[0-9]+:21: replace > with >= in strictly_convex$",
  # eng-collision-grid, sweep.rs
  "sweep\\.rs:[0-9]+:10: replace > with >= in crossing$",
  "sweep\\.rs:123:27: replace < with <= in crossing$",
  "sweep\\.rs:138:27: replace > with >= in crossing$",
  "sweep\\.rs:[0-9]+:18: replace < with <= in cross$",
  "sweep\\.rs:[0-9]+:22: replace > with >= in sweep$",
  "sweep\\.rs:[0-9]+:27: replace < with <= in sweep$",
  "sweep\\.rs:[0-9]+:35: replace > with >= in sweep$",
  "sweep\\.rs:[0-9]+:25: replace > with >= in sweep$",
  "sweep\\.rs:[0-9]+:31: replace && with \\|\\| in sweep$",
]
```

Why each is equivalent (mutants named as at 8797105):

- `ground.rs:193:23` `||` to `&&` in `step_up`: a grounded hull's start is
  free (`on_floor` traced from it), so `up.start_solid` is false and the
  mutant only drops the `rise <= 0` return; with a rise of 0 the up trace
  ends at the start, the second slide repeats the flat one, and the tie
  keeps the flat one. (The same text at column 25 is a real gap, now
  caught.)
- `ground.rs:221:24` `>` to `>=` in `step_down`: a hit at fraction 0 ends
  where the trace started (in the grid, the box and slope test worlds and
  `BoxRoom`), so landing there leaves the hull where it is.
- `grid.rs:177:40`, `179:40` `-` to `/` in `Grid::swept`: `h / MARGIN`
  widens the candidate cells by about 256 h; more candidates change nothing,
  since the hit is the least (fraction, index) of all of them.
- `grid.rs:195:62` `&&` to `||` in `Grid::each`: a primitive is visited once
  per cell instead of once, and every caller's visit is idempotent (an equal
  hit does not replace the best; `inside` and `onto_faces` answer the same).
- `grid.rs:215:55` `<` to `<=` in `onto_faces` and `grid.rs:276:87` `<` to
  `<=` in `trace`: the indices are equal only for the same primitive, which
  `each` visits once.
- `grid.rs:247:32` `-` to `/` in `axis_range`: `last = size` lets a range
  start at cell `size`, which clamps to an empty range: no candidates either
  way.
- `grid.rs:248:19` `&&` to `||` in `axis_range`: a range off the grid becomes
  its edge cell or an empty range instead of none; the extra candidates lie
  beyond the swept bounds, so they are never hit or entered.
- `grid.rs:251:18` `>` to `>=`, `252:18` `<` to `<=` in `axis_range`: at
  equality both branches give the same cell (0, `size - 1`).
- `grid.rs:358:22` `>` to `>=` in `settle`: `hit.t - step` is +0.0 when they
  are equal, so the clamp gives 0.0 either way.
- `lib.rs:126:25` `/` to `%` or `*` (`MARGIN` of 1 m or 1024 m): wider
  candidate ranges only, as for `swept`.
- `shape.rs:120:22` `-` to `/` in `add_axes`: a wall gets its closing edge
  too, b to a, whose normal is the exact negation of a to b's, and an exact
  opposite is skipped.
- `shape.rs:218:21` `>` to `>=` in `strictly_convex`: `side` is never 0 there
  (it returned false above).
- `sweep.rs:114:10` `>` to `>=` in `crossing`: with `v == 0` the first branch
  returns what the still branch does (none outside, `(-inf, inf, lo)`
  inside).
- `sweep.rs:123:27` `<` to `<=` and `138:27` `>` to `>=` in `crossing`: an
  exit exactly at the end becomes infinite instead of 1. Exits count only as
  `min(exit, 1)`, except in `cross`'s `t_in >= t_out` miss; there the
  infinite exit passes the entry on to `sweep`, whose graze test then
  misses it (the end lies on that axis's face, so it is not inside, and
  under an ulp of travel is under `GRAZE` for any motion within `MAX_COORD`).
- `sweep.rs:175:18` `<` to `<=` in `cross`: an equal exit is assigned again.
- `sweep.rs:232:22` `>` to `>=` in `sweep`: the corner rule then also looks at
  axes the hull was inside from the start, and the stop never leaves those
  on the side it tests (it moves monotonically from inside).
- `sweep.rs:234:27` `<` to `<=` and `236:35` `>` to `>=`: at equality the stop
  is set to the face it is already on.
- `sweep.rs:252:25` `>` to `>=` and `252:31` `&&` to `||`: after a hit on X,
  Y or Z the stop is on that face, so the rest of the motion enters that axis
  at +0 or -0; an edge off the axes is the rest's entry only if it enters
  strictly later (rest > 0), and no X, Y or Z axis enters later (the corner
  rule put the stop on every face entered). So `rest > 0 && j >= 3` equals
  both mutants.

## Not caught, and not shown equivalent (the owner's call)

All in step 4 of the crate docs, the hit on X, Y or Z with less than an ulp
of solid ahead (both of the check's ulp probes free):

- `edge_at_entry`'s `near` and `touching` (4): `298:44` `-` to `+` and to
  `/`, `299:57` `+` to `*`, `299:70` `+` to `*`.
- `sliver` (26): `398:5` returning `true`; `398:31` `+` to `*` and `-`;
  `398:51` `*` to `+` and `/`; all six of `400:14`, `400:22`, `400:30`, and of
  `401:14`, `401:22`, `401:30`; `402:14` `+` to `-`; `402:30` `*` to `/`;
  `404:28`, `404:41`, `404:54` `+` to `*`; `404:61` `/` to `%` and `*`;
  `408:37` `<` to `<=` and `>`.

The search for them (all on this Mac, none of it committed): cargo-mutants
with the grid's proptests at 128 times their cases (`PROPTEST_CASES=131072`,
seed 1; stopped, for its pace, after 34 of its 80 mutants), and with an
aimed proptest of hull corners placed at slab and wall corners, nudged by
ulps, moving along edges or at random (1 million cases at seed 1, then 4
million at seed 2, over every mutant left). These found the cases in
`tests/found.rs`, which catch the other 8 `sliver` mutants, 12 of
`edge_at_entry`'s 16 and more of `sweep.rs`, but none of these 30. Then a
copy of the crate instrumented to evaluate every one of the 30 beside the
real code at each call, over the crate's tests, the proptests at 128 times
(seed 3) and 8 million aimed cases (seed 5): `edge_at_entry` ran 64,000
times and none of its 4 mutants ever changed its answer; `sliver` ran about
1,000 times with the end outside the solid and returned true every time
(never false, so `sliver -> true` changed nothing), and 24 of its 26 mutants
never changed its answer; 2 (`401:22` `-` to `/`, `401:30` `*` to `+`) did,
8 times in the contract proptest, but changed no trace (cargo-mutants missed
them on those cases).

So, by every search, `sliver` only ever answers true, which makes the depth
test's arithmetic dead in practice; and no case has yet been seen where
`near` or `touching` are at the margin the mutants move. Two choices:

- exclude them as never observed, with these patterns (each checked to
  match only these 30; four keep the line, since lines 400 to 402 share
  their columns with mutants that are caught):

  ```toml
    "sweep\\.rs:.*: replace sliver -> bool with true$",
    "sweep\\.rs:[0-9]+:31: replace \\+ with [*-] in sliver$",
    "sweep\\.rs:[0-9]+:51: replace \\* with [+/] in sliver$",
    "sweep\\.rs:40[01]:(14|22|30): replace [-+*] with [-+*/] in sliver$",
    "sweep\\.rs:402:14: replace \\+ with - in sliver$",
    "sweep\\.rs:402:30: replace \\* with / in sliver$",
    "sweep\\.rs:[0-9]+:(28|41|54): replace \\+ with \\* in sliver$",
    "sweep\\.rs:[0-9]+:61: replace / with [%*] in sliver$",
    "sweep\\.rs:[0-9]+:37: replace < with (<=|>) in sliver$",
    "sweep\\.rs:[0-9]+:44: replace - with [+/] in edge_at_entry$",
    "sweep\\.rs:[0-9]+:57: replace \\+ with \\* in edge_at_entry$",
    "sweep\\.rs:[0-9]+:70: replace \\+ with \\* in edge_at_entry$",
  ```

- or have step 4's rule revisited (T5's), together with the contract break
  below, which is in the same rule.

## A contract break found on the way (not fixed)

`tests/oracle.rs`'s `the_oracle_keeps_the_contract` fails with
`PROPTEST_CASES=16384` (seed 1) on the unmutated tree, and the grid gives the
oracle's bits for the same trace, so `Grid` breaks ADR-0024's contact rule
there too:

- A slab, the diamond with corners (-0.9795673034842626,
  -0.2785587047970818), (-0.7184421868659554, -0.01743358817877455),
  (-0.9795673034842626, 0.24369152843953268), (-1.2406924201025697,
  -0.01743358817877455), z 1.0 to 1.4; a hull of half extents (0.62900436,
  0.8207159, 0.62513196); the trace from (-0.08943782762065511,
  0.8032823160570652, 2.125) to (-0.3394378276206551, 1.0532823160570652,
  1.875).
- It stops on the slab's top (normal +Z) at (-0.18930586293712248,
  0.9031503513735326, 2.0251319646835326), and `conformance::check` fails
  with "one ulp further on is still free".
- The motion runs exactly along the diamond's north-east edge: its
  projection on that edge's axis does not change (`v == 0`), an ulp inside
  the grown face at the start, and exactly on the face (`hi`) at the stop.
  So the hull's corner only touches the slab along a line, and one ulp on,
  down or down and north-west, is free. `crossing` reports an axis the
  motion does not move along as entered through `lo`, so `edge_at_entry`
  takes `lo`, the far face, as this edge's entry face, and the edge is
  neither near nor touching; the hit on the top is kept, and the hull is
  held in mid-air.
- A fix would take the face nearer the stop as the entry face of such an
  axis, in `src/sweep.rs` and `tests/oracle/brute.rs` alike, with step 4 of
  the crate docs saying so, and the oracle and contract proptests run at
  many times their cases. It is T5's rule and a change to production code,
  so it is left for its own task; `cargo xtask replay` would have to show
  the hash streams unchanged.
