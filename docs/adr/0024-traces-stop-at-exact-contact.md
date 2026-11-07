# ADR-0024: Traces stop at exact contact

- **Status:** Accepted
- **Date:** 2026-11-07
- **Decision:** a contract change to `eng-collision-api` (spec 6.2 rule 6;
  spec 6.11): `CollisionWorld::trace` promises that a blocked hull stops in
  contact with what it hit, and `conformance::check` checks it
- **Who:** Owner decided (2026-11-07, at rung 2 gate G4, with O4: the
  brute-force oracle), on Claude's proposal (rung 2 plan, decision 2 and
  T5's carry from queue item Q2; owner decision O4).

## Context

- **What needs it.** `kit-fps`'s `ground_move` (Q2) decides "on the
  ground" with a trace `GROUND_PROBE`, 1/1024 m, down (`ground.rs:56`,
  `:157`). The step-up's push-down and the step-down look one probe
  further (`:193`, `:213`). Its doc says this "assumes every
  `CollisionWorld` stops a trace exactly at contact", that "neither the
  collision contract nor `conformance::check` promises that", and that a
  world stopping short by more would leave a resting hull "to fall a little
  every tick".
- **What the contract says** (`lib.rs`). `Trace::end` is "where the hull
  stopped. Never inside solid"; `hit()` is `fraction < 1`.
  `conformance::check` asks for a free start not reported as solid, a
  fraction in [0, 1], an end outside solid, a unit normal not along the
  motion, an unblocked trace reaching its end, and the same trace twice.
  Nothing bounds how far short a blocked trace may stop.
- **What the implementations do.**
  - `BoxRoom` counts its boundary as free (`lo..=hi`, `room.rs:95`) and
    puts a hit's axis exactly on it (`p[axis] = limit`, `:75`).
    At the east wall `dot-corner.assert.toml:10` asserts `x == 7.75`, and
    `dot-square.assert.toml:17` asserts `x <= 7.75` on every tick.
  - kit-fps's test world `Boxes` puts the hit's axis on the face
    (`tests/support/boxes.rs:200`) and counts only interiors as solid
    (`:89`); `tests/boxes.rs` runs `check` on it. `Slope` and `Both`
    (`tests/slopes.rs`) stop where the hull is free by a 1 µm skin (`:58`)
    and do not run `check`.
  - Rung 2's grid (T5) sweeps a box in f64 against vertical wall segments
    at any angle and against horizontal slabs.
- **Where "exact" ends.** f64 holds a contact exactly only against a face
  square to an axis: there the boundary is one number, the face's
  coordinate plus or minus the hull's half extent. Against a wall off the
  axes the contact point is in general not representable, and the nearest
  free point is a rounding error away. The plan says "exact contact"
  without that limit; this ADR gives it one.
- **The Q2 carry in T5's row.** T5 also runs
  `flat_ground_gives_pmoves_result_bit_for_bit` (through `same_as_pmove`)
  over the grid. That test holds the hull's height bit for bit
  (`tests/ground.rs:120`), so the push-down must land exactly where the
  hull stood. The plan adds that with box worlds a tie keeping the stepped
  move cannot be told apart from one keeping the flat move, so the grid run
  is what guards decision 3's tie rule.

## Decision

- **Touching is free.** A hull whose boundary meets solid, with none of
  its inside in it, does not overlap, and a trace from there is not
  `start_solid`.
- **A blocked trace ends in contact.** `end` is free, and the rest of the
  motion, a trace from `end` to the same end point, still hits and is not
  `start_solid`. Then:
  - **on an axis** (the hit's normal has one component ±1 and the others
    0): `end` lies on the boundary the world's own `overlaps` uses. The
    rest of the motion hits with `fraction == 0` and leaves `end`
    unchanged;
  - **off the axes**: the hull is within `CONTACT_GAP` of the face,
    **measured along the hit's normal**: the rest of the motion moves the
    hull by a vector whose component along the normal has magnitude at most
    `CONTACT_GAP`. Measured along the motion, a grazing trace's gap would
    be the normal gap divided by sin θ, and `slide`'s near-parallel
    velocities make exactly those traces.
- **`CONTACT_GAP`**, a new public constant of the contract: 2^-20 m, about
  1 µm (proposed). It bounds the normal gap, which is what rounding
  bounds: a sweep's few f64 operations at room scale err by a few times
  f64's spacing, 2^-43 m at 1 km. T5 may lower it; raising it is an
  amendment.
- **Floors off the axes, later.** A sloped floor (format v2) with normal
  z component `n_z` leaves a vertical gap of at most `CONTACT_GAP / n_z`.
  For the steepest walkable floor (`n_z` = 0.7, `GroundConfig::DEFAULT`)
  that is under 1.5 µm, against `GROUND_PROBE`'s 977 µm, so the ground
  probe still finds it. kit-fps checks at compile time that
  `CONTACT_GAP / 0.7` (from `DEFAULT`) is below `GROUND_PROBE`.
- **How `conformance::check` verifies it.** After a hit it traces the
  rest of the motion and checks the rules above.
  - On an axis it also moves `end` one ulp towards the end point on every
    axis where they differ, and requires that position to overlap. Free
    at `end` and solid one ulp on is what "the boundary `overlaps` uses"
    means: a world that stops a skin short, and blocks within the skin,
    fails it. Stepping every moving axis, not only the hit one, keeps an
    exact corner hit, which touches along an edge only, overlapping.
  - Off the axes it compares the normal component, squared, with eng-math
    only.
  - Its signature and its other rules stay, so every caller of `check`
    checks contact without an edit.
- **Left open:** whether a trace that ends exactly touching reports a hit.
  `BoxRoom`'s does not, and kit-fps looks one probe further either way.

## Migration notes

- **No shape changes.** `CollisionWorld`, `Trace`, `BoxHull`, `BoxRoom` and
  `check`'s signature stay. New: `CONTACT_GAP`. The docs of `Trace::end`,
  `CollisionWorld::trace` and the crate root state the promise and cite
  this ADR.
- **Implementers.**
  - `BoxRoom`: no change. It meets the rule by construction, and
    `tests/room.rs` checks it once `check` has the rule.
  - `eng-collision-grid` (T5): must meet it, axis walls and slab faces on
    the axis branch (criterion 3). Its stop and its `overlaps` compute the
    boundary with the same expression, or the check fails.
  - `Boxes`: puts each swept hit on the face. Its fallback does not: it
    returns `end = start` with an axis normal
    (`tests/support/boxes.rs:138-155`), which is off the face, so it must
    change when `check` gains the rule, for example into a test failure,
    since its tests already assert it never fires. T5 makes the change,
    before G5 protects it.
  - `Slope` and `Both`: unchanged. They do not run `check`; their skin
    makes them worlds built to isolate the floor rule, not implementations
    of the contract.
- **Callers.** `ground_move` needs no code change; `GROUND_PROBE`'s doc
  cites this ADR instead of saying nothing promises it. `pmove`'s `slide`
  is unaffected. `game-testbed-sim` keeps `BoxRoom`.
- **Protected files that pin today's behaviour and stay unchanged:** the
  dot `*.assert.toml` (`x == 7.75` and `x <= 7.75`) and `twin.rs`. The dot
  hash streams keep their bytes, since `BoxRoom` does not change. No
  protected path covers eng-collision-api or kit-fps's tests yet.
- Rule 6 asks for "an interface-change PR". With direct commits to `main`
  (ADR-0014), T5's commit message repeats these notes.

## Consequences

- `ground_move`'s assumption becomes a checked rule: a hull that lands
  stands, and its next probe hits. Every floor in rung 2 is a horizontal
  slab, so it hits at fraction 0; slopes come with format v2 (6.11).
- The room's assertions can be exact at walls on the axes, as
  `dot-corner`'s is.
- Exact contact does not fix O12: `slide`'s f32 velocity against a plane
  off the axes can still stop a move.
- The rule binds the next implementations too: the BVH at Level 2 (6.11)
  and any physics crate behind the contract (6.3).
- `crates/eng-collision-api/src/conformance.rs` becomes protected at G5,
  once T5 has extended it, so the rule cannot be weakened later.

## Alternatives considered

- **A skin**: stop a fixed distance short. Hulls would float above floors,
  the probe would have to exceed the skin, and exact assertions such as
  `x == 7.75` would fail.
- **One tolerance everywhere**, on the axes too. Simpler to state, but it
  gives up exactness where f64 has it, and `same_as_pmove` needs the height
  bit for bit.
- **Test only the grid.** The promise would hold for today's two
  implementations, and not bind the next.
- **A separate check function.** Callers would have to opt in; extending
  `check` reaches them all.
- **A `ground_move` that tolerates gaps**, probing further and snapping
  every tick. It would hide a world that stops short, and change Q2's
  results.

## Owner decisions, gates and protected files

- **O4.** Recommended: a brute-force oracle, which must give the grid's
  `Trace` bits and so meets the rule too. With `parry3d-f64` instead, its
  casts work to their own tolerance, so it could not be held to the same
  bits; it would check hits and normals within a tolerance, and contact
  would rest on `check` alone.
- **G4** accepts this before T5.
- **G5:** `protected.toml` with `.github/CODEOWNERS` protecting
  `crates/eng-collision-api/src/conformance.rs`, kit-fps's tests and
  `tests/support/**`, and the grid's oracle test. No protected file is
  needed to make the change itself.

## Revisit when

Level 2's BVH or a third-party physics crate implements the contract;
slopes (format v2) put floors off the axes; or `CONTACT_GAP` proves too
loose or too tight.

## Amendment 1

- **Status:** Accepted (the owner, 2026-11-07, after T5's independent review)
- **What:** changes how `conformance::check` verifies the decision above,
  and records how the grid meets it. The decision itself is unchanged.
- **Who:** Owner decided, on Claude's proposal from rung 2 task T5 and its
  follow-up, from cases that `eng-collision-grid`'s proptests, its review
  and an independent review (an exact rational oracle written from
  decision 2 and this ADR) found.

**Two rules of the check, loosened.**

- **One ulp on, the hit axis alone.** On an axis, the check moves `end`
  one ulp towards the end point on every axis where they differ, as above,
  and now also accepts moving it on the hit axis alone. A hull's corner on
  the end of a wall off the axes, moving down and a little west, is hit on
  Y exactly on the face: one ulp down is inside the wall, but one ulp down
  and west is not, because the wedge of wall ahead is narrower than an ulp
  (`crates/eng-collision-grid/tests/corners.rs`,
  `a_hull_corner_on_a_walls_end_is_solid_one_ulp_on_along_the_hit_axis`).
  The rule as written rejected that correct trace. A world that stops a
  skin short still fails both versions
  (`crates/eng-collision-api/tests/room.rs`).
- **"Faces along the motion", off the axes.** The check required
  `normal · travel <= 0` for every hit. Off the axes it now allows up to
  `CONTACT_GAP` (m); on an axis it is still 0. A hull stopped against a
  diamond's 45-degree edge, then moved a quarter meter along it, starts
  within rounding of the face; the motion seems to leave the face by
  8e-17 m while its end is inside by rounding, so the hit's f32 normal
  faces along the motion by that much (`corners.rs`,
  `sliding_along_a_face_off_the_axes_from_contact_keeps_the_contract`). A
  hull within `CONTACT_GAP` of a face is in contact with it, so the
  contract's own tolerance covers it. Tests pin both sides: a normal off
  the axes facing along the motion by 2^-19 m fails, by 2^-21 m passes,
  and one on an axis facing along it fails (`room.rs`).

**Vertex contacts.** Where a hull's corner meets a primitive's corner, or
the end of an edge off the axes, the solid just ahead is a wedge and one
ulp on can still be free. The grid then gives the hit to an edge off the
axes that the stop touches at its entry face within `4 * NOISE` per meter
of coordinate (`NOISE` = 2^-48), with that edge's normal, so the gap rule
applies. Only if there is none, and the solid ahead is a sliver by depth
(midway through the rest of the motion's overlap the hull is inside by
less than 2^-40 m per meter of coordinate on some axis), is the hit a
graze. Before this rule, the grid let 5 of 5,368 traces starting at vertex
contacts on maps off the axes pass through 1.3 cm to 43 cm of solid
(`crates/eng-collision-grid/tests/vertices.rs`). The grid's proptests now
also check that the hull, shrunk by 1e-6 m, stays free on the traced
segment before the hit.

**f32 normals.** The grid uses each edge's unit normal rounded to f32 as
its separating axis, so each solid is a superset of the authored one by
about 4e-8 times the edge's length. The contact promise holds exactly
against the grid's own geometry; against the authored face it holds
within `CONTACT_GAP` for edges up to about 20 m.
