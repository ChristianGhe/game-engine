//! Bytes a session must not trust: a stranger's, a hostile client's, and
//! anything at all from the server's address. Nothing panics, and only a
//! slot's own client steers it. The proptests come from T6's review.

mod common;

use common::{SEED, receive, send, sim};
use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_net_transport::Transport;
use eng_net_transport::memory::MemoryNetwork;
use eng_net_wire::{Commands, Hello, Message, Snapshot, WIRE_VERSION, Welcome};
use eng_session::{Client, Server};
use proptest::prelude::*;

/// Ticks at the edges and anywhere.
fn tick() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(u64::MAX),
        Just(u64::from(u32::MAX)),
        0u64..200,
        any::<u64>(),
    ]
}

fn command() -> impl Strategy<Value = PlayerCommand> {
    let fields = (
        any::<u32>(),
        any::<i8>(),
        any::<i8>(),
        any::<u16>(),
        any::<u16>(),
        any::<u8>(),
    );
    (tick(), fields).prop_map(
        |(tick, (buttons, forward, side, view_yaw, view_pitch, select))| PlayerCommand {
            tick: Tick(tick),
            buttons,
            forward,
            side,
            view_yaw,
            view_pitch,
            select,
        },
    )
}

/// Any datagram: random bytes, or a well-formed `Commands`, `Snapshot` or
/// `Welcome` with arbitrary contents.
fn hostile() -> impl Strategy<Value = Vec<u8>> {
    let commands = (tick(), prop::collection::vec(command(), 0..=5)).prop_map(|(ack, mut list)| {
        list.sort_by_key(|c| c.tick);
        list.dedup_by_key(|c| c.tick);
        Message::Commands(Commands {
            ack: Tick(ack),
            commands: list,
        })
    });
    let snapshot = (tick(), prop::collection::vec(any::<u8>(), 0..300)).prop_map(|(t, state)| {
        Message::Snapshot(Snapshot {
            tick: Tick(t),
            state,
        })
    });
    let welcome =
        (tick(), any::<u8>(), any::<u8>(), any::<u64>()).prop_map(|(t, slot, players, seed)| {
            Message::Welcome(Welcome {
                tick: Tick(t),
                slot: PlayerSlot(slot),
                players,
                seed,
            })
        });
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..64),
        prop_oneof![commands, snapshot, welcome].prop_map(|m| m.encode().unwrap()),
    ]
}

fn cmd(tick: u64, forward: i8) -> PlayerCommand {
    PlayerCommand {
        forward,
        ..PlayerCommand::idle(Tick(tick))
    }
}

fn idle(tick: u64) -> PlayerCommand {
    PlayerCommand::idle(Tick(tick))
}

#[test]
fn datagrams_a_server_cannot_use_are_dropped() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(1), net.endpoint());
    let mut raw = net.endpoint();
    let to = server.local_id();
    for junk in [&[][..], &[0xff], &[1, 0, 0]] {
        raw.send(to, junk).unwrap();
    }
    let state = server.sim().snapshot();
    let welcome = Welcome {
        tick: Tick::ZERO,
        slot: PlayerSlot(0),
        players: 1,
        seed: SEED,
    };
    send(
        &mut raw,
        to,
        Message::Snapshot(Snapshot {
            tick: Tick::ZERO,
            state,
        }),
    );
    send(&mut raw, to, Message::Welcome(welcome));
    send(
        &mut raw,
        to,
        Message::Commands(Commands {
            ack: Tick::ZERO,
            commands: vec![cmd(1, 9)],
        }),
    );
    server.poll().unwrap();
    assert_eq!(server.ack(PlayerSlot(0)), None, "no client yet");
    assert_eq!(receive(&mut raw, Tick::ZERO), [], "no replies");
    assert_eq!(
        server.step().unwrap(),
        [idle(1)],
        "an unknown peer's command"
    );
    let mut client = Client::connect(sim(1), net.endpoint(), to).unwrap();
    server.poll().unwrap();
    client.poll().unwrap();
    assert_eq!(
        client.welcome().map(|w| w.slot),
        Some(PlayerSlot(0)),
        "slot 0 is free"
    );
}

#[test]
fn a_strangers_commands_never_reach_a_clients_slot() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(1), net.endpoint());
    let mut client = Client::connect(sim(1), net.endpoint(), server.local_id()).unwrap();
    let mut stranger = net.endpoint();
    server.poll().unwrap();
    client.poll().unwrap();
    // The stranger's commands come first, and a tick's first copy is kept.
    let forged = Commands {
        ack: Tick::ZERO,
        commands: (1..=3).map(|t| cmd(t, 99)).collect(),
    };
    send(&mut stranger, server.local_id(), Message::Commands(forged));
    client.send(cmd(1, 5)).unwrap();
    server.poll().unwrap();
    for t in 1..=3 {
        assert_eq!(server.step().unwrap(), [cmd(t, 5)], "tick {t}");
    }
}

type Rounds = Vec<(Vec<Vec<u8>>, Vec<Vec<u8>>)>;

fn rounds() -> impl Strategy<Value = Rounds> {
    let some = || prop::collection::vec(hostile(), 0..4);
    prop::collection::vec((some(), some()), 1..40)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Slot 0's client plays on while the client in slot 1 and a stranger
    /// send anything each tick: slot 0 gets exactly its client's commands,
    /// slot 1 a command for the tick, and slot 0's client every snapshot.
    #[test]
    fn hostile_peers_never_steer_another_slot(rounds in rounds()) {
        let net = MemoryNetwork::new();
        let mut server = Server::new(sim(2), net.endpoint());
        let mut client = Client::connect(sim(2), net.endpoint(), server.local_id()).unwrap();
        let mut attacker = net.endpoint();
        let mut stranger = net.endpoint();
        server.poll().unwrap();
        client.poll().unwrap();
        let hello = Message::Hello(Hello {
            wire_version: WIRE_VERSION,
            sim_protocol: server.sim().protocol_version(),
        });
        attacker.send(server.local_id(), &hello.encode().unwrap()).unwrap();
        server.poll().unwrap();
        prop_assert!(server.ack(PlayerSlot(1)).is_some(), "the attacker holds slot 1");
        for (from_attacker, from_stranger) in rounds {
            while attacker.poll_recv().is_some() {}
            while stranger.poll_recv().is_some() {}
            let tick = server.sim().tick().next();
            let mine = PlayerCommand {
                forward: (tick.0 % 100) as i8,
                ..PlayerCommand::idle(tick)
            };
            client.send(mine).unwrap();
            for bytes in &from_attacker {
                let _ = attacker.send(server.local_id(), bytes);
            }
            for bytes in &from_stranger {
                let _ = stranger.send(server.local_id(), bytes);
            }
            server.poll().unwrap();
            let applied = server.step().unwrap().to_vec();
            prop_assert_eq!(applied[0], mine);
            prop_assert_eq!(applied[1].tick, tick);
            client.poll().unwrap();
            prop_assert_eq!(client.latest(), Some(tick));
        }
    }

    /// After its welcome a client takes anything from the server's address
    /// without panicking; a snapshot its sim refuses is an error, not a
    /// crash.
    #[test]
    fn a_client_survives_anything_from_the_servers_address(
        messages in prop::collection::vec(hostile(), 0..20)
    ) {
        let net = MemoryNetwork::new();
        let mut server = net.endpoint();
        let mut client = Client::connect(sim(1), net.endpoint(), server.local_id()).unwrap();
        let peer = server.poll_recv().unwrap().0;
        let welcome = Message::Welcome(Welcome {
            tick: Tick(5),
            slot: PlayerSlot(0),
            players: 1,
            seed: SEED,
        });
        server.send(peer, &welcome.encode().unwrap()).unwrap();
        for bytes in &messages {
            let _ = server.send(peer, bytes);
        }
        // `poll` stops after a refused snapshot, so poll until all are read.
        for _ in 0..=messages.len() {
            let _ = client.poll();
        }
        prop_assert_eq!(client.welcome().map(|w| w.tick), Some(Tick(5)));
    }
}
