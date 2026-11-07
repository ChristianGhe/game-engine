//! Level 1's collision (spec 6.11): a uniform grid of 1 m cells over static
//! primitives, vertical wall segments with a z range and horizontal convex
//! slabs for floors, ceilings and steps, built once from a map's sim data.
//! Sweeps of a `BoxHull` are deterministic (f64, IEEE basic operations only)
//! and end in contact (ADR-0024): exactly on faces square to an axis, within
//! `CONTACT_GAP` off the axes. It implements `eng-collision-api`'s
//! `CollisionWorld` and passes that crate's conformance suite.
//!
//! An empty skeleton until rung 2's task T5 (`docs/rungs/2/plan.md`).
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
