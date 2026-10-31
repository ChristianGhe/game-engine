//! The client's side of a session, against a hand-driven server endpoint:
//! what it sends, which snapshots it keeps, and what it refuses.

mod common;

use common::{SEED, receive, send, sim, sim_at, sim_with};
use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_net_transport::memory::{MemoryEndpoint, MemoryNetwork};
use eng_net_transport::{PeerId, Transport};
use eng_net_wire::{Commands, Denied, DenyReason, Hello, Message, Snapshot, WIRE_VERSION, Welcome};
use eng_session::{Client, SENT_COMMANDS, SessionError};
use eng_sim::Sim;

fn cmd(tick: u64, forward: i8) -> PlayerCommand {
    PlayerCommand {
        forward,
        ..PlayerCommand::idle(Tick(tick))
    }
}

/// The one-player test sim, stepped to `tick` with idle commands.
fn sim_at_tick(tick: u64) -> Sim {
    let mut sim = sim(1);
    for t in 1..=tick {
        sim.step(&[PlayerCommand::idle(Tick(t))]).unwrap();
    }
    sim
}

fn welcome(tick: u64) -> Message {
    Message::Welcome(Welcome {
        tick: Tick(tick),
        slot: PlayerSlot(0),
        players: 1,
        seed: SEED,
    })
}

fn snapshot(tick: u64) -> Message {
    Message::Snapshot(Snapshot {
        tick: Tick(tick),
        state: sim_at_tick(tick).snapshot(),
    })
}

/// A client of a raw server endpoint, which has read its hello.
fn connected() -> (Client<MemoryEndpoint>, MemoryEndpoint, PeerId) {
    let net = MemoryNetwork::new();
    let mut server = net.endpoint();
    let client = Client::connect(sim(1), net.endpoint(), server.local_id()).unwrap();
    let hellos = receive(&mut server, Tick::ZERO);
    let [(peer, Message::Hello(hello))] = hellos.as_slice() else {
        panic!("expected one hello, got {hellos:?}");
    };
    assert_eq!(
        *hello,
        Hello {
            wire_version: WIRE_VERSION,
            sim_protocol: sim(1).protocol_version(),
        }
    );
    (client, server, *peer)
}

#[test]
fn each_packet_carries_the_newest_commands_and_the_ack() {
    let (mut client, mut server, peer) = connected();
    send(&mut server, peer, welcome(5));
    send(&mut server, peer, snapshot(5));
    client.poll().unwrap();
    assert_eq!(client.latest(), Some(Tick(5)));
    for t in 6..=9 {
        client.send(cmd(t, t as i8)).unwrap();
    }
    let sent: Vec<(Tick, Vec<u64>)> = receive(&mut server, Tick(5))
        .into_iter()
        .map(|(_, message)| match message {
            Message::Commands(Commands { ack, commands }) => {
                (ack, commands.iter().map(|c| c.tick.0).collect())
            }
            other => panic!("expected commands, got {other:?}"),
        })
        .collect();
    assert_eq!(SENT_COMMANDS, 3);
    assert_eq!(
        sent,
        [
            (Tick(5), vec![6]),
            (Tick(5), vec![6, 7]),
            (Tick(5), vec![6, 7, 8]),
            (Tick(5), vec![7, 8, 9]),
        ]
    );
    send(&mut server, peer, snapshot(6));
    client.poll().unwrap();
    client.send(cmd(10, 0)).unwrap();
    let acks: Vec<Message> = receive(&mut server, Tick(6))
        .into_iter()
        .map(|(_, m)| m)
        .collect();
    assert!(
        matches!(acks.as_slice(), [Message::Commands(c)] if c.ack == Tick(6)),
        "{acks:?}"
    );
}

#[test]
fn the_welcome_tick_is_the_ack_before_the_first_snapshot() {
    let (mut client, mut server, peer) = connected();
    send(&mut server, peer, welcome(3));
    client.poll().unwrap();
    assert_eq!(client.latest(), None);
    client.send(cmd(4, 1)).unwrap();
    let sent = receive(&mut server, Tick(3));
    assert!(
        matches!(sent.as_slice(), [(_, Message::Commands(c))] if c.ack == Tick(3)),
        "{sent:?}"
    );
}

#[test]
fn a_command_before_the_welcome_is_refused() {
    let (mut client, _server, _) = connected();
    assert_eq!(client.send(cmd(1, 1)), Err(SessionError::NotWelcomed));
}

#[test]
fn commands_must_go_in_tick_order() {
    let (mut client, mut server, peer) = connected();
    send(&mut server, peer, welcome(0));
    client.poll().unwrap();
    client.send(cmd(2, 1)).unwrap();
    for t in [2, 1] {
        assert_eq!(
            client.send(cmd(t, 1)),
            Err(SessionError::CommandOrder {
                last: Tick(2),
                got: Tick(t)
            })
        );
    }
    client.send(cmd(3, 1)).unwrap();
}

#[test]
fn a_second_welcome_and_a_late_denial_change_nothing() {
    let (mut client, mut server, peer) = connected();
    send(&mut server, peer, welcome(3));
    let other = Welcome {
        tick: Tick(9),
        slot: PlayerSlot(7),
        players: 1,
        seed: SEED,
    };
    send(&mut server, peer, Message::Welcome(other));
    let denied = Denied {
        tick: Tick(4),
        reason: DenyReason::FULL,
        wire_version: WIRE_VERSION,
        sim_protocol: 0,
    };
    send(&mut server, peer, Message::Denied(denied));
    client.poll().unwrap();
    assert_eq!(
        client.welcome().map(|w| (w.tick, w.slot)),
        Some((Tick(3), PlayerSlot(0)))
    );
}

#[test]
fn the_baseline_follows_the_newest_snapshot_past_2_pow_31() {
    let (mut client, mut server, peer) = connected();
    let half = 1u64 << 31;
    send(&mut server, peer, welcome(0));
    // Each tick is within 2^31 of the snapshot before it, not of the
    // welcome's.
    for t in [half - 10, half + 10] {
        let state = sim_at(1, Tick(t)).snapshot();
        let snapshot = Snapshot {
            tick: Tick(t),
            state,
        };
        send(&mut server, peer, Message::Snapshot(snapshot));
    }
    client.poll().unwrap();
    assert_eq!(client.latest(), Some(Tick(half + 10)));
}

#[test]
fn old_and_repeated_snapshots_are_ignored() {
    let (mut client, mut server, peer) = connected();
    send(&mut server, peer, welcome(0));
    send(&mut server, peer, snapshot(2));
    send(&mut server, peer, snapshot(1));
    // Tick 2 again, with other state: the first copy stays.
    let other = sim_at_tick(3).snapshot();
    send(
        &mut server,
        peer,
        Message::Snapshot(Snapshot {
            tick: Tick(2),
            state: other,
        }),
    );
    client.poll().unwrap();
    assert_eq!(client.latest(), Some(Tick(2)));
    assert_eq!(client.sim().tick(), Tick(2));
    assert_eq!(client.sim().state_hash(), sim_at_tick(2).state_hash());
}

#[test]
fn datagrams_before_the_welcome_or_from_other_peers_are_dropped() {
    let net = MemoryNetwork::new();
    let mut server = net.endpoint();
    let mut stranger = net.endpoint();
    let mut client = Client::connect(sim(1), net.endpoint(), server.local_id()).unwrap();
    let peer = receive(&mut server, Tick::ZERO)[0].0;
    send(&mut server, peer, snapshot(1));
    send(&mut stranger, peer, welcome(9));
    server.send(peer, &[0xff, 1, 2]).unwrap();
    client.poll().unwrap();
    assert_eq!(client.welcome(), None);
    assert_eq!(client.latest(), None);
    send(&mut server, peer, welcome(0));
    send(&mut stranger, peer, snapshot(1));
    client.poll().unwrap();
    assert_eq!(client.welcome().map(|w| w.tick), Some(Tick::ZERO));
    assert_eq!(client.latest(), None, "only the server's snapshots count");
}

#[test]
fn a_snapshot_from_another_session_is_refused() {
    let (mut client, mut server, peer) = connected();
    send(&mut server, peer, welcome(0));
    send(&mut server, peer, snapshot(1));
    client.poll().unwrap();
    // The same game with another seed: the same protocol, another session.
    let state = sim_with(1, SEED + 1, Tick(2)).snapshot();
    send(
        &mut server,
        peer,
        Message::Snapshot(Snapshot {
            tick: Tick(2),
            state,
        }),
    );
    let Err(SessionError::Restore(err)) = client.poll() else {
        panic!("expected a refused snapshot");
    };
    assert!(err.to_string().contains("session"), "{err}");
    assert_eq!(client.latest(), Some(Tick(1)));
    assert_eq!(client.sim().state_hash(), sim_at_tick(1).state_hash());
}
