//! The server loop: the handshake, command buffers, the missing-command
//! rule, acknowledged ticks and snapshots.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_ecs::World;
use eng_net_transport::{MAX_DATAGRAM, PeerId, Transport};
use eng_net_wire::{
    Commands, DecodeError, Denied, DenyReason, Hello, Message, Snapshot, WIRE_VERSION, Welcome,
};
use eng_sim::Sim;

use crate::{MAX_LEAD, SessionError};

/// What a client may see of the state, as the relevance hook answers it
/// for that client before each snapshot (spec 6.8, contract 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Relevance {
    /// The whole state. Rung 1b's snapshots carry nothing less; answers
    /// that narrow it arrive with delta snapshots in rung 5.
    Everything,
}

/// The default relevance hook: every client sees everything.
pub fn everything(_world: &World, _client: PlayerSlot) -> Relevance {
    Relevance::Everything
}

/// A `Snapshot` message's bytes besides the state: kind, tick and length
/// (eng-net-wire's layout). Used only for a state the codec cannot encode.
const SNAPSHOT_OVERHEAD: usize = 7;

/// A connected client.
struct Connection {
    peer: PeerId,
    /// The newest snapshot tick the client acknowledged, or the `Welcome`
    /// tick before its first ack: its baseline.
    ack: Tick,
    /// Commands for ticks after the server's, by tick.
    buffer: BTreeMap<Tick, PlayerCommand>,
}

/// The authoritative side of a session: it owns the [`Sim`] and steps it
/// with one command per player slot.
pub struct Server<T> {
    sim: Sim,
    transport: T,
    /// One entry per player slot.
    clients: Vec<Option<Connection>>,
    /// Each slot's command at the current tick: what [`Server::step`]
    /// applied last, and what a missing command repeats.
    applied: Vec<PlayerCommand>,
    relevance: fn(&World, PlayerSlot) -> Relevance,
}

impl<T: Transport> Server<T> {
    /// A server for `sim`'s session (its players and seed), reachable at
    /// `transport`. Every slot idles until a client sends commands for it.
    pub fn new(sim: Sim, transport: T) -> Self {
        let players = usize::from(sim.config().players);
        Self {
            clients: (0..players).map(|_| None).collect(),
            applied: vec![PlayerCommand::idle(sim.tick()); players],
            sim,
            transport,
            relevance: everything,
        }
    }

    /// Replaces the relevance hook, [`everything`] by default.
    pub fn with_relevance(mut self, hook: fn(&World, PlayerSlot) -> Relevance) -> Self {
        self.relevance = hook;
        self
    }

    /// The address clients connect to.
    pub fn local_id(&self) -> PeerId {
        self.transport.local_id()
    }

    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    /// The newest snapshot tick that the client in `slot` acknowledged, or
    /// its `Welcome` tick before its first ack; `None` without a client.
    pub fn ack(&self, slot: PlayerSlot) -> Option<Tick> {
        let client = self.clients.get(usize::from(slot.0))?.as_ref()?;
        Some(client.ack)
    }

    /// The commands applied at the server's tick, one per slot in slot
    /// order: what the last [`Server::step`] returned. They stay here when
    /// that step fails with [`SessionError::SnapshotTooLarge`], so a host
    /// that carries on records every tick. Before the first step, each
    /// slot's idle command at the starting tick, which nothing applied.
    pub fn applied(&self) -> &[PlayerCommand] {
        &self.applied
    }

    /// Handles every waiting datagram: welcomes or denies new clients and
    /// buffers commands. Fails only if a new client's first snapshot does
    /// not fit a datagram; its `Welcome` has gone out by then.
    pub fn poll(&mut self) -> Result<(), SessionError> {
        while let Some((peer, bytes)) = self.transport.poll_recv() {
            self.receive(peer, &bytes)?;
        }
        Ok(())
    }

    /// Advances one tick and sends the new state to every client. A slot
    /// with a client gets that client's command for the tick, else its last
    /// command restamped; a slot without one idles, keeping its view
    /// angles. Returns those commands, one per slot in slot order: the
    /// session recording's entry for this tick (spec 6.8).
    ///
    /// On [`SessionError::SnapshotTooLarge`] the sim has stepped, but no
    /// client got the new state; [`Server::applied`] still has the tick's
    /// commands.
    pub fn step(&mut self) -> Result<&[PlayerCommand], SessionError> {
        let tick = self.sim.tick().next();
        for (applied, client) in self.applied.iter_mut().zip(&mut self.clients) {
            *applied = match client {
                Some(client) => {
                    // The tick's command leaves the buffer, with anything older.
                    let later = client.buffer.split_off(&tick.next());
                    let mut due = core::mem::replace(&mut client.buffer, later);
                    due.remove(&tick).unwrap_or(applied.repeated_at(tick))
                }
                None => PlayerCommand {
                    view_yaw: applied.view_yaw,
                    view_pitch: applied.view_pitch,
                    ..PlayerCommand::idle(tick)
                },
            };
        }
        self.sim
            .step(&self.applied)
            .expect("one command per slot, each stamped for the next tick");
        self.send_snapshots(None)?;
        Ok(&self.applied)
    }

    fn receive(&mut self, peer: PeerId, bytes: &[u8]) -> Result<(), SessionError> {
        let slot = self
            .clients
            .iter()
            .position(|c| c.as_ref().is_some_and(|c| c.peer == peer));
        // A client's ticks extend against its ack. Nobody else sends a
        // message whose tick matters (a `Hello` has none).
        let baseline = slot
            .and_then(|s| self.clients[s].as_ref())
            .map_or(self.sim.tick(), |c| c.ack);
        match Message::decode(bytes, baseline) {
            Ok(Message::Hello(hello)) => self.hello(peer, slot, hello),
            Ok(Message::Commands(commands)) => {
                if let Some(slot) = slot {
                    self.commands(slot, commands);
                }
                Ok(())
            }
            Err(DecodeError::WireVersion { .. }) => {
                self.deny(peer, DenyReason::WIRE_VERSION);
                Ok(())
            }
            // Messages only a server sends, and bytes that do not decode.
            Ok(_) | Err(_) => Ok(()),
        }
    }

    /// Welcomes `peer` into its slot, or the lowest free one, or denies it.
    fn hello(
        &mut self,
        peer: PeerId,
        slot: Option<usize>,
        hello: Hello,
    ) -> Result<(), SessionError> {
        if hello.sim_protocol != self.sim.protocol_version() {
            self.deny(peer, DenyReason::SIM_PROTOCOL);
            return Ok(());
        }
        let Some(slot) = slot.or_else(|| self.clients.iter().position(Option::is_none)) else {
            self.deny(peer, DenyReason::FULL);
            return Ok(());
        };
        let tick = self.sim.tick();
        self.clients[slot].get_or_insert_with(|| Connection {
            peer,
            ack: tick,
            buffer: BTreeMap::new(),
        });
        let config = self.sim.config();
        let welcome = Message::Welcome(Welcome {
            tick,
            slot: slot_id(slot),
            players: config.players,
            seed: config.seed,
        });
        let bytes = welcome.encode().expect("a welcome always encodes");
        if self.transport.send(peer, &bytes).is_err() {
            self.clients[slot] = None;
            return Ok(());
        }
        self.send_snapshots(Some(slot))
    }

    /// Buffers a client's commands for the coming ticks and takes its ack.
    fn commands(&mut self, slot: usize, message: Commands) {
        let now = self.sim.tick();
        let Some(client) = self.clients[slot].as_mut() else {
            return;
        };
        // An ack of a snapshot this server has not sent yet: not a message
        // its client wrote.
        if message.ack > now {
            return;
        }
        // Datagrams may arrive out of order: the newest ack wins.
        client.ack = client.ack.max(message.ack);
        for command in message.commands {
            if command.tick > now && command.tick.0 - now.0 <= MAX_LEAD {
                client.buffer.entry(command.tick).or_insert(command);
            }
        }
    }

    fn deny(&mut self, peer: PeerId, reason: DenyReason) {
        let denied = Message::Denied(Denied {
            tick: self.sim.tick(),
            reason,
            wire_version: WIRE_VERSION,
            sim_protocol: self.sim.protocol_version(),
        });
        let bytes = denied.encode().expect("a denial always encodes");
        // A peer that has gone needs no answer.
        let _ = self.transport.send(peer, &bytes);
    }

    /// Sends the current state to every client, or to the one in `only`.
    /// A client the transport no longer knows is dropped, and its slot
    /// idles from then on.
    fn send_snapshots(&mut self, only: Option<usize>) -> Result<(), SessionError> {
        let mut message: Option<Vec<u8>> = None;
        for slot in 0..self.clients.len() {
            if only.is_some_and(|only| only != slot) {
                continue;
            }
            let Some(peer) = self.clients[slot].as_ref().map(|c| c.peer) else {
                continue;
            };
            match (self.relevance)(self.sim.world(), slot_id(slot)) {
                Relevance::Everything => {}
            }
            let bytes = match &mut message {
                Some(bytes) => bytes,
                none => none.insert(snapshot_message(&self.sim)?),
            };
            if self.transport.send(peer, bytes).is_err() {
                self.clients[slot] = None;
            }
        }
        Ok(())
    }
}

/// The current state as a `Snapshot` message, if it fits one datagram.
fn snapshot_message(sim: &Sim) -> Result<Vec<u8>, SessionError> {
    let tick = sim.tick();
    let state = sim.snapshot();
    let state_len = state.len();
    match Message::Snapshot(Snapshot { tick, state }).encode() {
        Ok(bytes) if bytes.len() <= MAX_DATAGRAM => Ok(bytes),
        Ok(bytes) => Err(SessionError::SnapshotTooLarge {
            tick,
            len: bytes.len(),
        }),
        // Past even the codec's cap of 65,535 state bytes.
        Err(_) => Err(SessionError::SnapshotTooLarge {
            tick,
            len: SNAPSHOT_OVERHEAD + state_len,
        }),
    }
}

fn slot_id(slot: usize) -> PlayerSlot {
    PlayerSlot(u8::try_from(slot).expect("a session has at most 255 slots"))
}
