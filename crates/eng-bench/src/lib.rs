//! Benchmarks (spec 6.1, the tools row; 6.13; rung 2 decision 14):
//! instruction counts for the dot and room sim steps, the room's build,
//! grid traces and `ground_move`, through gungraun under valgrind's
//! callgrind. `cargo xtask bench` runs them for the pushed head and the
//! push's base on one runner and compares the counts.
//!
//! **What is where.** The library is empty: the benchmarks measure the
//! testbed's sim, a game crate, which an engine or tool crate may reach only
//! as a dev-dependency (`arch-check`'s `[E-ARCH-GAME-DEP]`). Each bench is a
//! directory, `benches/sim/` and `benches/collision/`: `main.rs` holds the
//! gungraun benchmarks and `cases.rs` each one's input, built the same way
//! on every target, and the one call it measures. `tests/cases.rs` includes
//! both `cases.rs` and checks on every target that each input does what its
//! name says (a walking tick bumps nothing, a trace into a wall stops on
//! it). A benchmark's name in xtask's report is its gungraun id,
//! `<bench>::<group>::<function>` and the `#[bench]` id, as
//! `sim::tick::room::walk`.
//!
//! **Linux only.** gungraun is a dev-dependency only under
//! `cfg(target_os = "linux")` (`Cargo.toml`), and every item of each
//! `main.rs` but an empty `main` sits under the same `cfg`. So
//! `cargo test --all-targets` and `cargo clippy --all-targets` build on
//! every target; elsewhere each bench is an empty `main`. On Linux a run
//! without `--bench` (as `cargo test --all-targets` gives) is gungraun's
//! test mode: each benchmark runs once, natively, through `gungraun-runner`,
//! which must be installed.
//!
//! Nothing at run time depends on this crate.
#![forbid(unsafe_code)]
