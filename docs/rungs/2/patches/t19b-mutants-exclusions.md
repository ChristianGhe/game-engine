# T19 follow-up: the rung's mutation gaps, and the exclusions proposed

For the owner at G5, with `t19-mutants-config.patch`, which adds
`.cargo/mutants.toml` with both lists empty. This is not a patch: that file
exists only after the patch is applied. Nothing below is excluded until the
owner adds it. Mutants are named as at the commit "Rung 2: T19b follow-up, a
still axis enters through its near face, and two mutants caught", whose fix
to the grid moved `sweep.rs`'s lines from 111 on down by 2, and `MARGIN`'s
line in `lib.rs` by 5.

## The runs

cargo-mutants 27.1.0 over `git diff 314383d` of the core crates' `*.rs`, as
`t19-xtask-mutants.patch` runs it (`--in-diff`, `PROPTEST_RNG_SEED=1`,
`--cap-lints true`, the core crates as `--package`, each mutant tested
against its own package's tests), on this Mac (macOS, arm64), `-j 3`:

- Before (8797105): 776 mutants in 18 min: 588 caught, 148 missed, 7 timed
  out (counted as caught), 33 unviable. Missed: 117 in eng-collision-grid
  (`sweep.rs` 78, `grid.rs` 22, `shape.rs` 11, `lib.rs` 3), 15 in
  eng-collision-api (`conformance.rs`), 11 in kit-fps (`ground.rs`), 3 in
  eng-ecs, 1 in eng-sim, 1 in game-testbed-sim.
- After the first follow-up (ccc06fb, tests only): 776 mutants, 681 caught,
  55 missed.
- After the second (the grid's fix, which adds 5 mutants in its own
  expression, all caught): 781 mutants in 23 min: 687 caught, 54 missed, 7
  timed out (the same 7), 33 unviable. The 54 are the 22 equivalent and the
  32 open ones below; no other mutant is missed.

So 94 of the 148 first missed now have a test that catches them (and the
fix's own 5 do). Criterion 19 passes if the owner adds the 20 patterns of the
next section and the 14 of the one after; with the first 20 alone, 32
mutants still fail it.

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
  "sweep\\.rs:125:27: replace < with <= in crossing$",
  "sweep\\.rs:140:27: replace > with >= in crossing$",
  "sweep\\.rs:[0-9]+:18: replace < with <= in cross$",
  "sweep\\.rs:[0-9]+:22: replace > with >= in sweep$",
  "sweep\\.rs:[0-9]+:27: replace < with <= in sweep$",
  "sweep\\.rs:[0-9]+:25: replace > with >= in sweep$",
  "sweep\\.rs:[0-9]+:31: replace && with \\|\\| in sweep$",
]
```

Why each is equivalent:

- `ground.rs:193:23` `||` to `&&` in `step_up`: a grounded hull's start is
  free (`on_floor` traced from it), so `up.start_solid` is false and the
  mutant only drops the `rise <= 0` return; with a rise of 0 the second
  slide repeats the flat one from the up trace's end, and only a strictly
  longer travel is kept, so the flat one stays, even when that end has the
  other sign of a zero. (The same text at column 25 is a real gap, caught.)
- `grid.rs:177:40`, `179:40` `-` to `/` in `Grid::swept`: `h / MARGIN` is
  1024 h (256 m for a 0.25 m hull), so the candidate cells only widen; more
  candidates change nothing, since the hit is the least (fraction, index) of
  all of them, and `inside` and `onto_faces` find the same primitives.
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
  equality both branches give the same cell (0, `size - 1`), whatever the
  sign of a zero.
- `grid.rs:358:22` `>` to `>=` in `settle`: `hit.t - step` is +0.0 when they
  are equal (`hit.t` is never -0.0), so the clamp gives 0.0 either way.
- `lib.rs:131:25` `/` to `%` or `*` (`MARGIN` of 1 m or 1024 m): wider
  candidate ranges only, as for `swept`.
- `shape.rs:120:22` `-` to `/` in `add_axes`: a wall gets its closing edge
  too, b to a, whose normal is the exact negation of a to b's, and an exact
  opposite is skipped.
- `shape.rs:218:21` `>` to `>=` in `strictly_convex`: `side` is never 0 there
  (it returned false above).
- `sweep.rs:125:27` `<` to `<=` and `140:27` `>` to `>=` in `crossing`: an
  exit exactly at the end (`pe` on the face) becomes infinite instead of
  exactly 1. Exits count only as `min(exit, 1)`, except in `cross`'s
  `t_in >= t_out` miss, which an exit at 1 decides only for an entry at or
  after the end. In the trace's own `cross`, the mutant then hands that entry
  to `sweep`, whose graze test misses it: the end lies on the exit's face, so
  it is not inside, and under an ulp of travel is under `GRAZE` for any
  motion within `MAX_COORD`. In the rest of the motion's `cross(q, e)` it
  cannot arise: an entry at or after the end from the stop means the trace
  itself entered that axis within rounding of the end, so its hit is within
  an ulp of the end, and the same graze test, with the end on the exit's
  face, has already missed it before the rest is swept.
- `sweep.rs:177:18` `<` to `<=` in `cross`: an equal exit is assigned again;
  a zero exit of the other sign reaches only `min(exit, 1) - t` at `t = 0`
  and `sliver`'s midpoint, neither of which it changes.
- `sweep.rs:234:22` `>` to `>=` in `sweep`: the corner rule then also looks at
  axes the hull was inside from the start, and the stop never leaves those
  on the side it tests (it moves monotonically from inside).
- `sweep.rs:236:27` `<` to `<=`: at equality the stop is set to the face it is
  on. Only the sign of a zero could differ, and on this branch (entering
  through `lo`, moving up) the stop `s + d t` is -0.0 only if `d t` is, which
  needs `t = -0.0` (it is clamped to +0.0), and a grown `lo` is -0.0 only for
  a hull of no size. (Its mirror at column 35, entering through `hi`, can
  keep a -0.0 that the face's +0.0 would not, and is now caught.)
- `sweep.rs:254:25` `>` to `>=` and `254:31` `&&` to `||`: after a hit on X,
  Y or Z the stop is on that face, so the rest of the motion enters that axis
  at +0 or -0; an edge off the axes is the rest's entry only if it enters
  strictly later (rest > 0), and no X, Y or Z axis enters later (the corner
  rule put the stop on every face entered). So `rest > 0 && j >= 3` equals
  both mutants.

Three mutants that the first follow-up called equivalent are not, and now
have tests: `sweep.rs:238:35` (`>` to `>=` in the corner rule) and
`ground.rs:221:24` (`fraction > 0.0` to `>=` in `step_down`) turn a -0.0 into
+0.0, which the oracle's bits and the state hash see (`corners.rs`
`a_corner_stop_keeps_the_sign_of_a_zero_on_a_face_it_already_touches`,
`steps.rs` `a_hull_resting_on_the_floor_keeps_the_sign_of_a_zero_coordinate`);
and `sweep.rs:116:10` (`v > 0.0` to `>=` in `crossing`) now sends a still
axis down the moving branch, which reports entry through `lo` and undoes the
fix below, which the fix's tests catch.

## Not caught, and not shown equivalent (the owner's call)

All in step 4 of the crate docs, the hit on X, Y or Z with less than an ulp
of solid ahead (both of the check's ulp probes free):

- `edge_at_entry`'s `near` and `touching` (4): `300:44` `-` to `+` and to
  `/`, `301:57` `+` to `*`, `301:70` `+` to `*`.
- `sliver` (26): `400:5` returning `true`; `400:31` `+` to `*` and `-`;
  `400:51` `*` to `+` and `/`; all six of `402:14`, `402:22`, `402:30`, and of
  `403:14`, `403:22`, `403:30`; `404:14` `+` to `-`; `404:30` `*` to `/`;
  `406:28`, `406:41`, `406:54` `+` to `*`; `406:61` `/` to `%` and `*`;
  `410:37` `<` to `<=` and `>`.
- the guard that calls it (2): `267:33` the match guard `grazed() &&
  sliver(..)` replaced with `true`, and `267:42` its `&&` to `||`. Before the
  grid's fix these were caught, but only by cases that broke the contact
  rule (the guard was false on the way to a kept hit short of contact); with
  the fix those cases are held by the edge instead. **Correction (the
  second follow-up's review):** other cases do reach the guard false. Every
  open break (A) below does, on its way to a kept hit short of contact
  (snap on Z, both probes free, `edge_at_entry` finds no edge), so these two
  are not "never observed": the searches here missed (A).

The search for them (all on this Mac, none of it committed): cargo-mutants
with the grid's proptests at 128 times their cases (`PROPTEST_CASES=131072`,
seed 1), and with an aimed proptest of hull corners placed at slab and wall
corners, nudged by ulps, moving along edges or at random (1 million cases at
seed 1, then 4 million at seeds 2 and 3, over every mutant left), before and
after the fix. These found the cases in `tests/found.rs`, which catch the
other 8 `sliver` mutants, 12 of `edge_at_entry`'s 16 and more of `sweep.rs`,
but none of these 32. Then, before the fix, a copy of the crate instrumented
to evaluate the 30 of `sliver` and `edge_at_entry` beside the real code at
each call, over the crate's tests, the proptests at 128 times (seed 3) and 8
million aimed cases (seed 5): `edge_at_entry` ran 64,000 times and none of
its 4 mutants ever changed its answer; `sliver` ran about 1,000 times with
the end outside the solid and returned true every time (never false, so
`sliver -> true` changed nothing), and 24 of its 26 mutants never changed its
answer; 2 (now `403:22` `-` to `/`, `403:30` `*` to `+`) did, 8 times in the
contract proptest, but changed no trace.

So, by these searches, `sliver` only ever answers true; but the guard is
reached false by the open breaks (A) below, so step 4's rule is not dead,
only untested at the margins these mutants move. Don't exclude these 32
until (A) and (B) are fixed: their tests are likely to catch some of them.
Two choices, after that fix:

- exclude them as never observed, with these patterns (each checked to
  match only these 32; three keep the line, since lines 402 to 404 share
  their columns with mutants that are caught):

  ```toml
    "sweep\\.rs:.*: replace sliver -> bool with true$",
    "sweep\\.rs:[0-9]+:31: replace \\+ with [*-] in sliver$",
    "sweep\\.rs:[0-9]+:51: replace \\* with [+/] in sliver$",
    "sweep\\.rs:40[23]:(14|22|30): replace [-+*] with [-+*/] in sliver$",
    "sweep\\.rs:404:14: replace \\+ with - in sliver$",
    "sweep\\.rs:404:30: replace \\* with / in sliver$",
    "sweep\\.rs:[0-9]+:(28|41|54): replace \\+ with \\* in sliver$",
    "sweep\\.rs:[0-9]+:61: replace / with [%*] in sliver$",
    "sweep\\.rs:[0-9]+:37: replace < with (<=|>) in sliver$",
    "sweep\\.rs:.*: replace match guard grazed\\(\\) && sliver\\(.*\\) with true in sweep$",
    "sweep\\.rs:[0-9]+:42: replace && with \\|\\| in sweep$",
    "sweep\\.rs:[0-9]+:44: replace - with [+/] in edge_at_entry$",
    "sweep\\.rs:[0-9]+:57: replace \\+ with \\* in edge_at_entry$",
    "sweep\\.rs:[0-9]+:70: replace \\+ with \\* in edge_at_entry$",
  ```

- or have step 4's thin-solid rule reworked (T5's), with the fix of (A) and
  (B) below.

## Open breaks of the contact rule (ADR-0024), still in step 4

Found by the second follow-up's review with an aimed search (a hull beside a
random convex slab's off-axis edge, 1-4 ulps inside its grown face, sliding
along it and dropping past the top; about 345,000 cases over 4 seeds, at
coordinates up to ±1000). About 0.14% of these cases still break the
contact rule. Both kinds predate the still-axis fix below; the fix added
none (every remaining failure has the same bits before and after it). They
need off-axis geometry, so rung 2's axis-aligned room cannot reach them;
the proposal is to fix them with rung 3's angled walls, beside O12.

- **(A) "one ulp further on is still free"**, the hull held on the top
  (normal +Z): the edge's projection moves by rounding (v of 1e-17 to
  1e-16) towards the near face, so the moving branch's direction-based flag
  names the far face, the still-axis bug's mechanism with v ≠ 0. Reproducer:
  slab [(0.8342246031076089, 0.22523066479828902), (-0.5587184072985766,
  0.5130715888515152), (-1.1209500218753434, -1.661382543707628),
  (0.2649472301686788, -0.6111264719388632)], z 0 to 0.7545323315703691;
  hull (0.26983458, 0.14706548, 0.72871894); trace (-1.1284023191669874,
  -0.49954267624878423, 1.684434015645078) to (-1.2618060870558636,
  -1.0154872840786453, 1.007474930349261). A generalisation, tested on a
  scratch copy of `brute.rs` only, removes about 90% of (A) with no new
  failures: an axis inside throughout (enter −∞, leave +∞) enters through
  its nearer face whatever v is.
- **(B) "the rest of the motion does not hit"**, at any v, plus a few "more
  than CONTACT_GAP": a different flaw. Reproducer: slab
  [(1.8083400316838976, 2.7251160814529642), (-0.35575512839208945,
  2.2516321346073775), (-0.11336571303549503, 1.0940975268012743),
  (1.710195340442799, 1.3599845674512399)], z 0 to 1.4747776309330871; hull
  (0.9827255, 0.48495227, 0.99043757); trace (-1.2732884835113105,
  1.4553536667774594, 2.750962695091877) to (-1.0899426147761797,
  0.5797825208421452, 2.4121203546379055).
- The "cannot arise" argument for the rest of the motion's `cross` (under
  `125:27`/`140:27` above) assumes projection rounding is small next to the
  rest's motion along the axis; that fails for (A)'s near-still axes. It is
  unlikely to matter, but re-check it with (A)'s fix.

## The contact-rule break found on the way, and its fix (T5's rule)

Fixed in the second follow-up, with the owner away; recorded here for the
owner, who may revert it.

- Found: `tests/oracle.rs`'s `the_oracle_keeps_the_contract` failed with
  `PROPTEST_CASES=16384` (seed 1), and the grid gave the oracle's bits, so
  `Grid` broke ADR-0024's contact rule too. The review of the first
  follow-up found another: `found.rs`'s case 3 mirrored. Both are the same
  rule.
- The case: a slab, the diamond with corners (-0.9795673034842626,
  -0.2785587047970818), (-0.7184421868659554, -0.01743358817877455),
  (-0.9795673034842626, 0.24369152843953268), (-1.2406924201025697,
  -0.01743358817877455), z 1.0 to 1.4; a hull of half extents (0.62900436,
  0.8207159, 0.62513196); the trace from (-0.08943782762065511,
  0.8032823160570652, 2.125) to (-0.3394378276206551, 1.0532823160570652,
  1.875). The motion runs exactly along the diamond's north-east edge (its
  projection on that edge's axis is still, `v == 0`), an ulp inside the
  grown face at the start and exactly on it at the stop, where the hull
  lands on the top. Both of the check's ulp probes are free, so step 4 looks
  for an edge at its entry; `crossing` reported a still axis as entered
  through `lo`, the far face here, so the edge was neither near nor
  touching, and the hit on the top was kept, short of contact ("one ulp
  further on is still free").
- The fix: a still axis, inside throughout, counts as entered through the
  face nearer its projection (`lo` on a tie), in `src/sweep.rs`'s
  `crossing` and in the oracle's (`tests/oracle/brute.rs`), and in the crate
  docs' steps 1 and 4. This trace is now held by the edge, in contact (its
  normal the edge's, off the axes; the rest of the motion hits at once).
  That does not make step 4 meet the contact rule in general: see the open
  breaks (A) and (B) above.
  The flag is read only by `edge_at_entry`, so nothing else changes.
- Tests: `corners.rs`
  `a_hull_sliding_along_an_edge_onto_a_slabs_top_is_held_by_the_edge` (the
  case above); `found.rs` now checks every case, and its mirror image, with
  the conformance suite and for pass-through (case 3 mirrored among them),
  and has the case above twice: as found, and moved so that the edge's
  interval lies mostly above 0, which tells the nearer face apart by
  distance, as the fix must (the mutants of its own expression are caught).
  Without the fix both of these tests fail.
- Checked: the contract, oracle and vertex tests with
  `PROPTEST_CASES=16384` at seed 1 and three random seeds (1609876689,
  3980505213, 3762344702), and the contract and oracle tests with 131072 at
  seeds 1 and 1609876689: all pass (they failed at 16384, seed 1, before).
  `cargo xtask replay`: the five dot streams match; and in a scratch clone
  of `main` (d56368d) with both follow-ups, `t6-replay-map.patch` and
  `room-scenarios.patch` applied, all nine streams match the patches'
  files byte for byte (the room is square to the axes).
