---
name: golden-update
description: Bring golden-image candidates from CI to the owner, who alone commits goldens. Use for gate G6 (the first goldens), when shot-exact or shot-flip fails with [E-GOLDEN] after an intended rendering change, or when a shot is added, renamed or dropped.
---

# Golden images: from CI's candidates to the owner

The goldens in `crates/game-testbed/golden/` and `thresholds.toml` are
protected (`**/golden/**`). Agents never write there; the guard puts a
changed file back. The owner copies the chosen candidates in and commits
them. Your job is to hand the owner everything needed to decide.

## Fetch

Agents change nothing on GitHub, and the guard blocks `gh run download`.
Read the artifacts through the API instead, into a scratch directory
outside the repo. The guard refuses to unzip inside the repo.

```sh
gh run list --branch main --limit 3
gh api repos/{owner}/{repo}/actions/runs/<run>/artifacts --jq '.artifacts[] | [.id, .name] | @tsv'
gh api repos/{owner}/{repo}/actions/artifacts/<id>/zip > <scratch>/<name>.zip
unzip -q -d <scratch>/<name> <scratch>/<name>.zip
```

`<runner>` is one of ubuntu-24.04, ubuntu-24.04-arm, windows-2025,
windows-11-arm, macos-26.

- `golden-candidates-<runner>`: `target/golden-candidates/`. It holds the
  candidates (`<shot>.png`, the probe's own files, byte for byte), the
  FLIP error maps (`flip/<shot>.png`) and `shots.json`.
- `report-native-<runner>`: the job's report and step outputs. The
  `shot-exact` or `shot-flip` step says pass, fail or pending ("no golden
  yet").
- `crash-native-<runner>`: the job's `crash-*.json`. A crash inside the
  shots harness is not among them: the harness logs to
  `target/tmp/shots/render/logs/`.

## Compare

1. **The reference is ubuntu-24.04.** Its `shots.json` must say
   `"mode": "exact"`, `configuration.pinned: true`, adapter
   `llvmpipe (LLVM 18.1.3, 256 bits)` and driver info
   `Mesa 25.2.7 (LLVM 18.1.3)`. If any differs, stop: candidates from
   another configuration cannot become goldens.
2. **The other runners, before G6.** With no golden, their shot steps
   compare nothing, so each runner gives only its candidates. `cmp` each
   against the reference. Every runner uses the same PNG encoder, so
   different bytes mean different pixels. A pair that differs has no FLIP
   number yet: look at both images, and ask the `visual-qa` subagent to
   describe them. It is advisory; it never decides.
3. **After G6, or for a later change,** each runner's `shot-flip` step
   compares against the goldens. Its `shots.json` has FLIP `mean`, `p99`
   and `p999` for each shot, and `flip/` has the error maps. A local run,
   `ENG_REQUIRE_GPU=1 ENG_GOLDEN_DIR=<a copy outside target/golden-candidates/> cargo test -p game-testbed --test shots`,
   measures only the local adapter. Without `ENG_REQUIRE_GPU=1`, a
   machine with no adapter skips the shots and passes.

## Hand to the owner

A short table: shot, reference file, and for each runner either "same
bytes" or "differs" (before G6), or its FLIP numbers against
`thresholds.toml` (after G6). Add the candidates and maps themselves as
local files. The owner checks:

- each image shows what its shot names: 320x240, opaque, the dot or dots
  where the scenario puts them at that tick;
- every difference between runners is scattered edge pixels, never a
  shape such as a missing or moved dot; after G6, every runner is within
  the limits;
- the set is complete: one golden per shot in `SHOTS` (`tests/shots.rs`)
  and nothing else. The set is all or nothing, and a golden that no shot
  names fails.

## A shot that changes on purpose

A rendering change can move a shot. `shot-exact` then fails with
`[E-GOLDEN] N of 76800 pixels differ`. Treat it like a regenerated hash
stream: say which change moved which shots and why, in the commit and
`PROGRESS.md`. The failing run already saved each failing shot as a
candidate, so fetch that run's artifacts and hand them over as above.
Never widen `thresholds.toml` and never drop a shot to make a run pass.
Adding, renaming or dropping a shot is the owner's commit, with its
goldens.
