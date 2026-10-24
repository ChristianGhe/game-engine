//! The simulation contract (spec 6.4): a pure, fixed-rate step
//! `(World, commands for tick N) -> World + events`.
//!
//! - Systems run in fixed [`Stage`]s. Within a stage, `before`/`after`
//!   constraints are resolved by a topological sort with name tie-breaks;
//!   registration order never matters.
//! - Deferred structural changes apply at the end of each stage.
//! - The state hash is FNV-1a-64 over the world's canonical encoding.
//! - The resolved system order and every registered schema are hashed into
//!   the [simulation protocol version](Sim::protocol_version), so an
//!   unannounced change shows up in every hash stream header.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod schedule;
mod sim;

pub use schedule::{ScheduleError, Stage, System, SystemFn};
pub use sim::{
    COSMETIC_STREAM, GAMEPLAY_STREAM, GameplayRng, RestoreError, Sim, SimBuilder, SimConfig,
    SimInfo, SimTick, StepError, TickInputs, cosmetic_rng,
};
