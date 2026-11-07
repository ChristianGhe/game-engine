//! Instruction counts of the simulation (rung 2, criterion 15): one walking
//! tick of each map and the room's build. Each input is built outside the
//! measurement (`cases.rs`), and each benchmark returns what it made, so its
//! drop is not counted either.
//!
//! - `sim::tick::dots::walk`: one tick of the dot map, one player walking.
//! - `sim::tick::room::walk`: one tick of the room, one player walking.
//! - `sim::build::room_build`: `game_testbed_sim::room::build`, one player:
//!   the grid, the sim data's hash and the protocol.
//!
//! gungraun runs only on Linux; elsewhere this is an empty `main`
//! (`eng_bench`'s crate docs).
#![forbid(unsafe_code)]

#[cfg(target_os = "linux")]
mod cases;

#[cfg(target_os = "linux")]
use std::hint::black_box;

#[cfg(target_os = "linux")]
use cases::Walking;
#[cfg(target_os = "linux")]
use eng_sim::Sim;
#[cfg(target_os = "linux")]
use gungraun::{library_benchmark, library_benchmark_group, main};

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::walk(cases::dots_walking())]
fn dots(walking: Walking) -> Walking {
    black_box(cases::tick(black_box(walking)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::walk(cases::room_walking())]
fn room(walking: Walking) -> Walking {
    black_box(cases::tick(black_box(walking)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
fn room_build() -> Sim {
    black_box(cases::room_build(black_box(cases::config())))
}

#[cfg(target_os = "linux")]
library_benchmark_group!(name = tick, benchmarks = [dots, room]);

#[cfg(target_os = "linux")]
library_benchmark_group!(name = build, benchmarks = [room_build]);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = tick, build);

#[cfg(not(target_os = "linux"))]
fn main() {}
