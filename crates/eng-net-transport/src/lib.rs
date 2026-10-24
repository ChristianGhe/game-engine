//! The transport contract (spec 6.8): unreliable, unordered datagrams of at
//! most [`MAX_DATAGRAM`] bytes between opaque peers.
//!
//! Implementations: [`memory`] (here, rung 1a), Sim (Memory plus seeded
//! latency, loss and reordering, rung 5) and UDP (`eng-net-udp`, rung 5).
//! Every implementation must pass [`conformance::check`].
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod conformance;
pub mod memory;

use alloc::vec::Vec;
use core::fmt;

/// The largest datagram any transport carries.
pub const MAX_DATAGRAM: usize = 1200;

/// An opaque peer address. Only transports interpret the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId(u64);

impl PeerId {
    /// For transport implementations only.
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// For transport implementations only.
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    TooLarge { len: usize },
    UnknownPeer(PeerId),
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { len } => {
                write!(f, "datagram of {len} bytes exceeds {MAX_DATAGRAM}")
            }
            Self::UnknownPeer(peer) => write!(f, "unknown peer {}", peer.raw()),
        }
    }
}

impl core::error::Error for SendError {}

/// Sends and receives datagrams. Delivery may be lossy, duplicated or
/// reordered; protocols above must cope.
pub trait Transport {
    /// This endpoint's own address, as other peers see it.
    fn local_id(&self) -> PeerId;

    fn send(&mut self, to: PeerId, bytes: &[u8]) -> Result<(), SendError>;

    /// The next received datagram, if any. Never blocks.
    fn poll_recv(&mut self) -> Option<(PeerId, Vec<u8>)>;
}
