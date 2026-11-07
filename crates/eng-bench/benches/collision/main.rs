//! Instruction counts of collision and movement (rung 2, criterion 15) in
//! the room's grid: three traces of the player's hull, the same three
//! against the grid tests' brute-force world for comparison, and one tick
//! of `kit-fps`'s `ground_move` on the flat and up the step. The grid (or
//! the brute-force world) is built outside the measurement and returned
//! with the result, so neither its build nor its drop is counted.
//!
//! - `collision::trace::grid::<case>` and `collision::trace::brute::<case>`,
//!   for `into_wall`, `across_hall` and `ground_probe`
//!   (`cases::{INTO_WALL, ACROSS_HALL, GROUND_PROBE_CASE}`). The brute-force
//!   world tries every primitive, in index order, with no grid: what the
//!   grid's cells save.
//! - `collision::movement::ground_move::walk` and `::step_up`
//!   (`cases::{WALK, STEP_UP}`).
//!
//! gungraun runs only on Linux; elsewhere this is an empty `main`
//! (`eng_bench`'s crate docs).
#![forbid(unsafe_code)]

#[cfg(target_os = "linux")]
mod cases;

// The grid tests' brute-force oracle (T5): every primitive, no grid.
#[cfg(target_os = "linux")]
#[path = "../../../eng-collision-grid/tests/oracle/brute.rs"]
mod oracle;

#[cfg(target_os = "linux")]
use std::hint::black_box;

#[cfg(target_os = "linux")]
use cases::{GroundCase, TraceCase};
#[cfg(target_os = "linux")]
use eng_collision_api::Trace;
#[cfg(target_os = "linux")]
use eng_collision_grid::Grid;
#[cfg(target_os = "linux")]
use game_testbed_sim::room::layout;
#[cfg(target_os = "linux")]
use gungraun::{library_benchmark, library_benchmark_group, main};
#[cfg(target_os = "linux")]
use kit_fps::GroundOutcome;
#[cfg(target_os = "linux")]
use oracle::Brute;

/// The room's grid and one case.
#[cfg(target_os = "linux")]
fn on_grid<T>(case: T) -> (Grid, T) {
    (cases::room_grid(), case)
}

/// The room's brute-force world and one trace.
#[cfg(target_os = "linux")]
fn on_brute(case: TraceCase) -> (Brute, TraceCase) {
    (Brute::new(&layout::walls(), &layout::slabs()), case)
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::into_wall(on_grid(cases::INTO_WALL))]
#[bench::across_hall(on_grid(cases::ACROSS_HALL))]
#[bench::ground_probe(on_grid(cases::GROUND_PROBE_CASE))]
fn grid(input: (Grid, TraceCase)) -> (Grid, Trace) {
    let (world, case) = input;
    let trace = black_box(cases::trace(black_box(&world), black_box(&case)));
    (world, trace)
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::into_wall(on_brute(cases::INTO_WALL))]
#[bench::across_hall(on_brute(cases::ACROSS_HALL))]
#[bench::ground_probe(on_brute(cases::GROUND_PROBE_CASE))]
fn brute(input: (Brute, TraceCase)) -> (Brute, Trace) {
    let (world, case) = input;
    let trace = black_box(cases::trace(black_box(&world), black_box(&case)));
    (world, trace)
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::walk(on_grid(cases::WALK))]
#[bench::step_up(on_grid(cases::STEP_UP))]
fn ground_move(input: (Grid, GroundCase)) -> (Grid, GroundOutcome) {
    let (world, case) = input;
    let outcome = black_box(cases::ground(black_box(&world), black_box(&case)));
    (world, outcome)
}

#[cfg(target_os = "linux")]
library_benchmark_group!(name = trace, benchmarks = [grid, brute]);

#[cfg(target_os = "linux")]
library_benchmark_group!(name = movement, benchmarks = [ground_move]);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = trace, movement);

#[cfg(not(target_os = "linux"))]
fn main() {}
