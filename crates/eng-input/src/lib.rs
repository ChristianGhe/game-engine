//! Everything that becomes [`PlayerCommand`](eng_core::PlayerCommand)s:
//! authored input scripts, binary recordings, and live keyboard input.
//!
//! Quantization happens here, before the simulation, so the same input
//! produces identical commands on every platform:
//!
//! - scripts: degrees to binary angles ([`degrees_to_angle`]);
//! - live input: [`KeyEvent`]s mapped through [`Bindings`] to
//!   [`Action`]s, sampled once per tick ([`LiveInput`], [`quantize`]). The
//!   caller says which tick it samples; nothing here reads a clock.
//!
//! [`Key`] and [`KeyEvent`] are the platform contract's (`eng-platform-api`),
//! re-exported here. A platform reports `PlatformEvent`s, and
//! `PlatformEvent::key_event` gives the [`KeyEvent`] that
//! [`LiveInput::handle`] takes.
#![forbid(unsafe_code)]

mod bindings;
mod live;
mod recording;
mod script;

pub use bindings::{Action, ActionSet, Bindings, BindingsError, DEFAULT_BINDINGS};
pub use eng_platform_api::{Key, KeyEvent};
pub use live::{FULL_AXIS, LiveInput, TickInput, command_for, quantize};
pub use recording::{
    RECORDING_KIND, RECORDING_VERSION, Recording, RecordingHeader, TickHash, read_recording,
    write_recording,
};
pub use script::{InputScript, Segment, degrees_to_angle};
