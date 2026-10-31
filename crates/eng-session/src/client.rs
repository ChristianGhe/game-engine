//! The client loop: the handshake, commands out, snapshots in.

use alloc::vec::Vec;

use eng_core::{PlayerCommand, Tick};
use eng_net_transport::{PeerId, Transport};
use eng_net_wire::{Commands, Denied, Hello, Message, Snapshot, WIRE_VERSION, Welcome};
use eng_sim::Sim;

use crate::{SENT_COMMANDS, SessionError};

enum State {
    /// `Hello` sent; waiting for the server's answer.
    Connecting,
    Welcomed(Welcome),
    Denied(Denied),
}

/// One player's side of a session: it sends that player's commands and
/// keeps the server's newest state in a sim of its own.
pub struct Client<T> {
    sim: Sim,
    transport: T,
    server: PeerId,
    state: State,
    /// The tick of the newest snapshot restored into `sim`.
    latest: Option<Tick>,
    /// The newest commands sent, oldest first: the next message repeats
    /// them.
    sent: Vec<PlayerCommand>,
}

impl<T: Transport> Client<T> {
    /// Says hello to the server at `server`. `sim` is this build's sim for
    /// the session (the same players and seed as the server's), which
    /// snapshots are restored into; until the first one it holds its own
    /// initial state.
    pub fn connect(sim: Sim, mut transport: T, server: PeerId) -> Result<Self, SessionError> {
        let hello = Message::Hello(Hello {
            wire_version: WIRE_VERSION,
            sim_protocol: sim.protocol_version(),
        });
        let bytes = hello.encode().expect("a hello always encodes");
        transport.send(server, &bytes).map_err(SessionError::Send)?;
        Ok(Self {
            sim,
            transport,
            server,
            state: State::Connecting,
            latest: None,
            sent: Vec::with_capacity(SENT_COMMANDS),
        })
    }

    /// Handles every waiting datagram from the server: its answer to the
    /// hello, then snapshots. Fails once the server has denied this client,
    /// and on a snapshot this client's sim refuses.
    pub fn poll(&mut self) -> Result<(), SessionError> {
        while let Some((peer, bytes)) = self.transport.poll_recv() {
            if peer == self.server {
                self.receive(&bytes)?;
            }
        }
        match self.state {
            State::Denied(denied) => Err(SessionError::Denied(denied)),
            State::Connecting | State::Welcomed(_) => Ok(()),
        }
    }

    /// The server's `Welcome`: this client's slot, the session constants
    /// and the tick it joined at. `None` until it arrives.
    pub fn welcome(&self) -> Option<&Welcome> {
        match &self.state {
            State::Welcomed(welcome) => Some(welcome),
            State::Connecting | State::Denied(_) => None,
        }
    }

    /// The tick of the newest snapshot in [`Client::sim`], `None` before
    /// the first.
    pub fn latest(&self) -> Option<Tick> {
        self.latest
    }

    /// The server's newest state that reached this client.
    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    /// Sends `command`, this client's for its tick, with the ones sent just
    /// before it ([`SENT_COMMANDS`] in all) and the newest snapshot tick as
    /// the ack. Commands go in tick order, from the `Welcome` on.
    pub fn send(&mut self, command: PlayerCommand) -> Result<(), SessionError> {
        let State::Welcomed(welcome) = &self.state else {
            return Err(SessionError::NotWelcomed);
        };
        if let Some(last) = self.sent.last()
            && command.tick <= last.tick
        {
            return Err(SessionError::CommandOrder {
                last: last.tick,
                got: command.tick,
            });
        }
        if self.sent.len() == SENT_COMMANDS {
            self.sent.remove(0);
        }
        self.sent.push(command);
        let message = Message::Commands(Commands {
            ack: self.latest.unwrap_or(welcome.tick),
            commands: self.sent.clone(),
        });
        let bytes = message
            .encode()
            .expect("at most SENT_COMMANDS commands, in tick order");
        self.transport
            .send(self.server, &bytes)
            .map_err(SessionError::Send)
    }

    fn receive(&mut self, bytes: &[u8]) -> Result<(), SessionError> {
        let connecting = matches!(self.state, State::Connecting);
        let welcomed = matches!(self.state, State::Welcomed(_));
        match Message::decode(bytes, self.baseline()) {
            Ok(Message::Welcome(welcome)) if connecting => self.state = State::Welcomed(welcome),
            Ok(Message::Denied(denied)) if connecting => self.state = State::Denied(denied),
            Ok(Message::Snapshot(snapshot)) if welcomed => return self.restore(snapshot),
            // Out of turn, or bytes that do not decode.
            _ => {}
        }
        Ok(())
    }

    /// What this client's next message from the server extends its tick
    /// against: the newest snapshot tick, else the `Welcome` tick. Before
    /// the `Welcome` none is needed: only `Welcome` and `Denied` count then,
    /// and both carry whole ticks.
    fn baseline(&self) -> Tick {
        match (&self.state, self.latest) {
            (_, Some(latest)) => latest,
            (State::Welcomed(welcome), None) => welcome.tick,
            (State::Connecting | State::Denied(_), None) => Tick::ZERO,
        }
    }

    /// Restores `snapshot` if it is newer than the state this client has.
    fn restore(&mut self, snapshot: Snapshot) -> Result<(), SessionError> {
        if self.latest.is_some_and(|latest| snapshot.tick <= latest) {
            return Ok(());
        }
        self.sim
            .restore(&snapshot.state)
            .map_err(SessionError::Restore)?;
        self.latest = Some(snapshot.tick);
        Ok(())
    }
}
