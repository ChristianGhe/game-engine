//! The wire codec (spec 6.8, contract 7): explicit little-endian,
//! fixed-width messages with capped counts, and decoders that never panic.
//! It turns messages into bytes and back; the layers above do the I/O.
//!
//! Every message starts with the same 5-byte header: its kind (`u8`), then
//! the low 32 bits of a server tick (`u32`, contract 1). The body follows,
//! by kind:
//!
//! | Kind | Message | Header tick | Body |
//! |---|---|---|---|
//! | 1 | [`Hello`] | 0 | wire version `u32`, sim protocol `u64` |
//! | 2 | [`Welcome`] | the server's tick | the tick's high 32 bits `u32`, the client's player slot `u8`, players `u8`, seed `u64` |
//! | 3 | [`Denied`] | the server's tick | the tick's high 32 bits `u32`, reason `u8`, the server's wire version `u32` and sim protocol `u64` |
//! | 4 | [`Commands`] | the client's acknowledged snapshot tick | count `u8`, at most [`MAX_COMMANDS`]; each command in eng-core's canonical 19-byte layout, ticks strictly increasing |
//! | 5 | [`Snapshot`] | the snapshot's tick | length `u16`, at most [`MAX_STATE_LEN`]; the sim's canonical state |
//!
//! - **Ticks.** The receiver extends a 32-bit tick against its baseline,
//!   its last acknowledged tick ([`extend_tick`]). The handshake has no
//!   baseline yet, so `Welcome` and `Denied` also carry the high 32 bits.
//! - **Versions.** A `Hello` from another wire version decodes to
//!   [`DecodeError::WireVersion`] without reading past the version, and the
//!   server answers with `Denied`, which needs no baseline. That works
//!   across versions only while the header, `Hello`'s wire version and
//!   `Denied` stay readable by every build. They are not frozen yet: no two
//!   builds exchange messages before rung 5, whose handshake (magic, client
//!   salt, padding, with `eng-net-proto`) fixes their final form. Anything
//!   else may change with [`WIRE_VERSION`].
//! - **Canonical.** Bytes that decode re-encode to exactly the same bytes:
//!   nothing trails a message, commands are in order, a `Hello`'s tick is 0.
//! - Packet headers, acks, channels, fragmentation and the challenge cookie
//!   (`eng-net-proto`) come in rung 5.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod decode;
mod encode;

pub use decode::DecodeError;
pub use encode::EncodeError;

use alloc::vec::Vec;

use eng_core::{PlayerCommand, PlayerSlot, Tick};

/// The version of this encoding, sent in [`Hello`].
pub const WIRE_VERSION: u32 = 1;

/// The most commands one [`Commands`] message carries.
pub const MAX_COMMANDS: usize = 5;

/// The most state bytes one [`Snapshot`] carries.
pub const MAX_STATE_LEN: usize = 65_535;

// The count and the length go on the wire as a u8 and a u16.
const _: () = assert!(MAX_COMMANDS <= u8::MAX as usize);
const _: () = assert!(MAX_STATE_LEN <= u16::MAX as usize);

/// The first byte of each message.
mod kind {
    pub const HELLO: u8 = 1;
    pub const WELCOME: u8 = 2;
    pub const DENIED: u8 = 3;
    pub const COMMANDS: u8 = 4;
    pub const SNAPSHOT: u8 = 5;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Hello(Hello),
    Welcome(Welcome),
    Denied(Denied),
    Commands(Commands),
    Snapshot(Snapshot),
}

/// The client's first message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hello {
    pub wire_version: u32,
    pub sim_protocol: u64,
}

/// The server accepts the client, which controls `slot`. `players` and
/// `seed` are the session constants a client's sim must share to restore
/// the server's snapshots; the tick rate is part of the sim protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Welcome {
    pub tick: Tick,
    pub slot: PlayerSlot,
    pub players: u8,
    pub seed: u64,
}

/// The server refuses the client, and says which versions it speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Denied {
    pub tick: Tick,
    pub reason: DenyReason,
    pub wire_version: u32,
    pub sim_protocol: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DenyReason(pub u8);

impl DenyReason {
    pub const WIRE_VERSION: Self = Self(1);
    pub const SIM_PROTOCOL: Self = Self(2);
}

/// The client's newest commands, oldest first. Each packet repeats the
/// last few, so one lost packet costs no command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commands {
    /// The newest snapshot tick the client has, or the `Welcome` tick
    /// before its first snapshot: the server's baseline for this client
    /// (contract 5).
    pub ack: Tick,
    pub commands: Vec<PlayerCommand>,
}

/// The simulation state at `tick`. The bytes are opaque here: this crate
/// cannot reach `eng-sim`, so `eng-session` encodes and decodes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub tick: Tick,
    pub state: Vec<u8>,
}

/// The tick whose low 32 bits are `low`, nearest `baseline`: the one in
/// `[baseline - 2^31, baseline + 2^31 - 1]`. `None` if that tick would fall
/// before tick 0 or past `u64::MAX`.
pub fn extend_tick(baseline: Tick, low: u32) -> Option<Tick> {
    // The low 32 bits' wrapping distance, read as signed.
    let delta = low.wrapping_sub(low_bits(baseline)) as i32;
    baseline.0.checked_add_signed(i64::from(delta)).map(Tick)
}

fn low_bits(tick: Tick) -> u32 {
    tick.0 as u32
}

fn high_bits(tick: Tick) -> u32 {
    (tick.0 >> 32) as u32
}
