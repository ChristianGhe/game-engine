//! Planted render bugs against the frame record (rung 2, plan addition
//! A1; acceptance `frame-record`): a thing not drawn, a wall face not
//! drawn and a thing drawn black, drawn in-process as `shot` draws the
//! room-spawn shot, each change the record where they should and fail the
//! check within the shot's tolerances (`frame_record/planted.rs`).
//!
//! A test binary of its own, apart from `frame_record.rs`'s shots: on
//! Debian's lavapipe (Mesa 25.0.7, arm64) a device in the test process
//! crashed it with SIGSEGV while that file's other tests ran shot
//! processes beside it. Without an adapter the tests skip, unless
//! `ENG_REQUIRE_GPU=1`.

// Shared with `frame_record.rs`, which uses the rest of it.
#[allow(dead_code)]
#[path = "frame_record/harness.rs"]
mod harness;
#[path = "frame_record/planted.rs"]
mod planted;
