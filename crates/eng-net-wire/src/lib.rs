//! The wire codec (spec 6.8, contract 7): little-endian, fixed width,
//! capped counts, and decoders that never panic. Every message carries the
//! low 32 bits of the server tick, which the receiver extends against the
//! last acknowledged tick (contract 1).
//!
//! Rung 1b's messages are `Hello`, `Welcome`, `Denied`, `Commands` and
//! `Snapshot`. An empty skeleton until rung 1b's task T5
//! (`docs/rungs/1b/plan.md`).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
