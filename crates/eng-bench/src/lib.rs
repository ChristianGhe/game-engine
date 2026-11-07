//! Benchmarks (spec 6.1, the tools row; 6.13): instruction counts for the
//! dot and room sim steps, grid traces and `ground_move`, through gungraun,
//! a dev-dependency on Linux only. On every other target each bench is an
//! empty `main`, so the workspace still builds there. Nothing at run time
//! depends on this crate.
//!
//! An empty skeleton until rung 2's task T17 (`docs/rungs/2/plan.md`).
#![forbid(unsafe_code)]
