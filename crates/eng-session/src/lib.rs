//! Server and client loops over a `Transport` (spec 6.6, 6.8). The server
//! buffers commands and repeats the last one when one is missing, keeps each
//! client's last acknowledged tick, and records the commands it applied. It
//! has no clock: the caller passes `now`.
//!
//! An empty skeleton until rung 1b's task T6 (`docs/rungs/1b/plan.md`).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
