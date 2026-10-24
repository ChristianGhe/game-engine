---
name: replay-debug
description: Find the cause of a desync, a cross-architecture hash difference, or a changed hash stream. Use when replay verify exits 11, compare-hashes reports [E-CROSS-ARCH], or replay reports [E-HASH].
---

# Debugging a replay or hash divergence

Know which oracle failed first (spec 6.4):

- **`[E-CROSS-ARCH]`**: two CPU architectures disagree. This is always a
  determinism bug. Never "fix" it by regenerating a baseline.
- **`[E-HASH]`** from `cargo xtask replay`: this build differs from the
  committed hash streams. Expected after an intended simulation change; then
  `cargo xtask replay --regen` and explain the change in the PR.
- **`[E-DESYNC]`** from `replay verify`: a recording no longer reproduces.

## Steps

1. Find the first divergent tick `N`; every message above names it.
2. Dump the world just before and at `N` on each side:
   ```sh
   cargo run -p game-testbed -- dump-world --at-tick N --seed S \
       --input crates/game-testbed/scenarios/<name>.input.toml --out a.json
   ```
   For cross-architecture bugs, run the same command on the other machine
   (CI artifacts, or Docker: `--platform linux/amd64`), then diff the files.
3. The first differing field names the component, and so the system that
   wrote it. Look there for: std float functions, `mul_add`, `HashMap`
   iteration, sorts without a unique tie-break, reading uninitialized or
   platform-sized data (`usize` in state), or order that depends on
   registration rather than `.before()/.after()`.
4. If the field is a float that differs only in the last bits, suspect a
   function outside `eng-math` or a fused multiply-add. `eng-math`'s
   known-answer test (`cargo test -p eng-math --test known_answers`) tells
   you whether the math library itself drifted.
5. Write a regression test that fails before your fix, in the crate that
   owns the system.
