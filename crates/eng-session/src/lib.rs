//! Server and client loops over a
//! [`Transport`](eng_net_transport::Transport) (spec 6.6, 6.8). Every mode
//! runs them: single-player is the same server in-process over the Memory
//! transport, still encoding every message to bytes with `eng-net-wire`, so
//! there is one code path.
//!
//! A session in rung 1b, one tick at a time:
//!
//! ```text
//! client                                        server
//!   Hello { wire version, sim protocol } ──────▶  both must match; a free slot
//!   ◀── Welcome { tick, slot, players, seed }, then Snapshot (the state now)
//!       or Denied { reason }
//!   Commands { ack, newest 3 commands } ───────▶  buffered by tick
//!                                                step: one command per slot,
//!                                                Sim::step, then to each client
//!   ◀── Snapshot { tick, the whole state }
//! ```
//!
//! - **The server** ([`Server`]) owns the authoritative [`Sim`](eng_sim::Sim).
//!   [`Server::poll`] handles every waiting datagram; [`Server::step`]
//!   advances one tick and sends the new state to every client.
//! - **Missing commands repeat** (spec 6.8): at each tick a slot with a
//!   client gets that client's command for the tick, else its last command
//!   restamped
//!   ([`PlayerCommand::repeated_at`](eng_core::PlayerCommand::repeated_at)).
//!   A slot without a client, never joined or gone, idles, keeping its view
//!   angles. Commands up to [`MAX_LEAD`] ticks ahead wait in a buffer; once
//!   buffered, a tick's command is kept.
//! - **The record** (spec 6.8): [`Server::step`] returns the commands it
//!   applied, one per slot, in slot order, and [`Server::applied`] keeps
//!   the last tick's, also when sending its snapshot failed. Tick after
//!   tick they are the session recording, in the layout of `eng-input`'s
//!   recordings, which the probe writes. The server keeps no older ticks,
//!   so a caller that records nothing stores nothing.
//! - **Baselines** (contract 5): the server keeps each client's newest
//!   acknowledged snapshot tick ([`Server::ack`]), from the `Welcome` tick
//!   until the client's first ack. It is also the baseline that extends the
//!   32-bit ticks of that client's next message (contract 1).
//! - **Relevance** (contract 5): before each snapshot the server asks a
//!   per-client hook what the client may see ([`Server::with_relevance`]).
//!   Rung 1b sends whole states, so the one answer is
//!   [`Relevance::Everything`] ([`everything`], the default).
//! - **Snapshots** carry [`Sim::snapshot`](eng_sim::Sim::snapshot), the
//!   protocol version and then the canonical state, and the client puts it
//!   into its own sim with [`Sim::restore`](eng_sim::Sim::restore). A
//!   snapshot message must fit one datagram ([`MAX_DATAGRAM`], 1,200
//!   bytes); a larger one fails with [`SessionError::SnapshotTooLarge`]
//!   (`[E-SNAPSHOT-SIZE]`). Fragmentation arrives in rung 5.
//! - **The client** ([`Client`]) sends its commands with the
//!   [`SENT_COMMANDS`] newest in every message and its newest snapshot tick
//!   as the ack. It restores each snapshot newer than the last into its
//!   sim; that sim is what presentation reads. It extends ticks against its
//!   newest snapshot, or the `Welcome` tick before the first.
//! - **No clock.** Nothing here reads the time: the caller decides when a
//!   tick is due and calls [`Server::step`]. Rung 1b needs no time beyond
//!   that, so no call takes `now` yet; time sync (rung 5) adds it.
//! - **Nothing in the World.** The session registers nothing in the sim and
//!   only steps, snapshots and restores it, so the simulation protocol and
//!   every hash stream stay as they are.
//! - Datagrams a peer should not send, or that do not decode, are dropped.
//!   The one exception is a snapshot from the server that the client's sim
//!   refuses (another build or session): [`Client::poll`] returns it as
//!   [`SessionError::Restore`]. Every other error is this process's own
//!   mistake or limit.
//! - Rung 5 adds what loss and latency need: time sync, delta snapshots
//!   against the baselines, prediction, timeouts and reconnection.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod client;
mod server;

pub use client::Client;
pub use server::{Relevance, Server, everything};

use core::fmt;

use eng_core::Tick;
use eng_net_transport::{MAX_DATAGRAM, SendError};
use eng_net_wire::{Denied, DenyReason};
use eng_sim::RestoreError;

/// How many of its newest commands a client sends in each message, so a
/// lost datagram costs no command (spec 6.8: "the last 3-5").
pub const SENT_COMMANDS: usize = 3;

/// The furthest ahead of its own tick the server buffers a client's
/// commands. Later ones are dropped, so a client cannot fill the buffer.
pub const MAX_LEAD: u64 = 64;

// A client's commands must fit one message.
const _: () = assert!(SENT_COMMANDS <= eng_net_wire::MAX_COMMANDS);

/// What went wrong. Bad bytes from a peer are dropped, not reported; only
/// a snapshot the client's sim refuses comes back, as `Restore`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The snapshot of `tick` is `len` bytes as a message, more than one
    /// datagram carries. From [`Server::step`]: the sim has stepped to
    /// `tick` and [`Server::applied`] holds the tick's commands, but no
    /// client got the new state. From [`Server::poll`], a new client's
    /// first snapshot: nothing has stepped.
    SnapshotTooLarge { tick: Tick, len: usize },
    /// The server refused this client.
    Denied(Denied),
    /// A command sent before the server's `Welcome`.
    NotWelcomed,
    /// Commands go in tick order: one for `got` after one for `last`.
    CommandOrder { last: Tick, got: Tick },
    /// The transport refused a datagram to the server.
    Send(SendError),
    /// The client's sim refused a snapshot (another build or session).
    Restore(RestoreError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SnapshotTooLarge { tick, len } => write!(
                f,
                "[E-SNAPSHOT-SIZE] the snapshot of {tick} is {len} bytes; a datagram carries \
                 at most {MAX_DATAGRAM}, and fragmentation arrives in rung 5"
            ),
            Self::Denied(denied) => {
                let why = match denied.reason {
                    DenyReason::WIRE_VERSION => "another wire version",
                    DenyReason::SIM_PROTOCOL => "another simulation protocol",
                    DenyReason::FULL => "every player slot is taken",
                    _ => "an unknown reason",
                };
                write!(
                    f,
                    "the server refused this client: {why} (reason {}; the server speaks wire \
                     version {} and simulation protocol {:#018x})",
                    denied.reason.0, denied.wire_version, denied.sim_protocol
                )
            }
            Self::NotWelcomed => write!(f, "a command before the server's welcome"),
            Self::CommandOrder { last, got } => write!(
                f,
                "a command for {got} after one for {last}; commands go in tick order"
            ),
            Self::Send(err) => write!(f, "sending to the server: {err}"),
            Self::Restore(err) => write!(f, "a snapshot this client cannot use: {err}"),
        }
    }
}

impl core::error::Error for SessionError {}
