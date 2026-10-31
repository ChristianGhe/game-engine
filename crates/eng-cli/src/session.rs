//! The in-process session every mode runs through (spec 6.8): a server
//! that owns the authoritative sim and player 0's client, over Memory,
//! every message encoded to bytes.

use eng_core::{PlayerCommand, Tick};
use eng_net_transport::memory::{MemoryEndpoint, MemoryNetwork};
use eng_session::{Client, Server, SessionError};
use eng_sim::{RestoreError, Sim};

use crate::GameDef;
use crate::run::build_sim;

/// The in-process session: the server owns the authoritative sim, and the
/// client, player 0's, restores every snapshot into a sim of its own.
pub(crate) struct Session {
    pub server: Server<MemoryEndpoint>,
    pub client: Client<MemoryEndpoint>,
}

impl Session {
    /// A server and player 0's client, past the handshake. The other slots
    /// have no client, so the server idles them. Also returns
    /// `[E-SNAPSHOT-SIZE]` if the first snapshot does not fit a datagram.
    pub fn start(game: &GameDef, players: u8, seed: u64) -> (Self, Option<String>) {
        let net = MemoryNetwork::new();
        let mut server = Server::new(build_sim(game, players, seed), net.endpoint());
        let client = Client::connect(
            build_sim(game, players, seed),
            net.endpoint(),
            server.local_id(),
        );
        let mut client = client.unwrap_or_else(|e| session_bug(&e, Tick::ZERO));
        let first = server.poll().err().map(|e| snapshot_limit(e, Tick::ZERO));
        client
            .poll()
            .unwrap_or_else(|e| session_bug(&e, Tick::ZERO));
        (Self { server, client }, first)
    }

    /// The state the session delivered.
    pub fn sim(&self) -> &Sim {
        self.client.sim()
    }
}

/// A snapshot that outgrows a datagram stops the run as a budget does (exit
/// 13, `[E-SNAPSHOT-SIZE]`); fragmentation arrives in rung 5. Any other
/// session error is a bug in this process.
pub(crate) fn snapshot_limit(err: SessionError, tick: Tick) -> String {
    match err {
        SessionError::SnapshotTooLarge { .. } => err.to_string(),
        other => session_bug(&other, tick),
    }
}

pub(crate) fn session_bug(err: &SessionError, tick: Tick) -> ! {
    match err {
        // A release build checks floats only here, as the client restores
        // the state: still a simulation bug.
        SessionError::Restore(RestoreError::NonFinite(bad)) => panic!("[E-NAN] {bad} at {tick}"),
        _ => panic!("[E-SESSION] {err} at {tick}"),
    }
}

/// Player 0's command must be for the tick being stepped. The server would
/// otherwise keep it for its own tick, or drop it as late, and repeat the
/// last command, where `Sim::step` used to refuse it (`[E-STEP]`).
pub(crate) fn command_for(tick: Tick, command: PlayerCommand) -> Result<PlayerCommand, String> {
    if command.tick == tick {
        Ok(command)
    } else {
        Err(format!(
            "[E-STEP] player 0's command is for {}, but the sim is stepping to {tick}; \
             input scripts start at tick 1",
            command.tick
        ))
    }
}

/// The client must hold the state of the tick just stepped before the
/// probe reports it as that tick's.
pub(crate) fn restored(latest: Option<Tick>, tick: Tick) -> Result<(), String> {
    match latest {
        Some(latest) if latest == tick => Ok(()),
        _ => Err(format!(
            "[E-DESYNC] after the round trip for {tick} the client's newest snapshot is \
             {latest:?}: the session lost a snapshot"
        )),
    }
}

#[cfg(test)]
mod tests {
    use eng_core::{PlayerCommand, Tick};

    use super::{command_for, restored};

    #[test]
    fn a_command_for_another_tick_is_refused() {
        let at = |t| PlayerCommand::idle(Tick(t));
        assert_eq!(command_for(Tick(3), at(3)), Ok(at(3)));
        let err = command_for(Tick(6), at(1)).unwrap_err();
        assert!(
            err.starts_with("[E-STEP] player 0's command is for tick 1"),
            "{err}"
        );
    }

    #[test]
    fn the_client_must_hold_the_tick_just_stepped() {
        assert_eq!(restored(Some(Tick(4)), Tick(4)), Ok(()));
        for latest in [Some(Tick(3)), None] {
            let err = restored(latest, Tick(4)).unwrap_err();
            assert!(err.starts_with("[E-DESYNC] "), "{err}");
        }
    }
}
