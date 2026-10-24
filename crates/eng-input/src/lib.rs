//! Everything that becomes [`PlayerCommand`](eng_core::PlayerCommand)s
//! outside a live window: authored input scripts and binary recordings.
//!
//! Quantization (degrees to binary angles) happens here, before the
//! simulation, so a script produces identical commands on every platform.
//!
//! Live input (actions, bindings, devices) arrives with the window in
//! rung 1b.
#![forbid(unsafe_code)]

mod recording;
mod script;

pub use recording::{
    RECORDING_KIND, RECORDING_VERSION, Recording, RecordingHeader, TickHash, read_recording,
    write_recording,
};
pub use script::{InputScript, Segment, degrees_to_angle};
