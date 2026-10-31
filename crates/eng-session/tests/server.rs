//! The server's side of a session: the handshake, command buffering, the
//! missing-command rule, acknowledged ticks, snapshots and their size limit.

mod common;

use std::sync::Mutex;

use common::{SEED, blob_sim, receive, send, sim, sim_at, walked};
use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_ecs::World;
use eng_net_transport::MAX_DATAGRAM;
use eng_net_transport::memory::{MemoryEndpoint, MemoryNetwork};
use eng_net_wire::{Commands, Denied, DenyReason, Hello, Message, WIRE_VERSION, Welcome};
use eng_session::{Client, MAX_LEAD, Relevance, Server, SessionError, everything};
use eng_sim::Sim;

fn cmd(tick: u64, forward: i8) -> PlayerCommand {
    PlayerCommand {
        forward,
        ..PlayerCommand::idle(Tick(tick))
    }
}

fn idle(tick: u64) -> PlayerCommand {
    PlayerCommand::idle(Tick(tick))
}

/// A server over `server_sim` and one client, past the handshake.
fn pair(server_sim: Sim, client_sim: Sim) -> (Server<MemoryEndpoint>, Client<MemoryEndpoint>) {
    let net = MemoryNetwork::new();
    let mut server = Server::new(server_sim, net.endpoint());
    let mut client = Client::connect(client_sim, net.endpoint(), server.local_id()).unwrap();
    server.poll().unwrap();
    client.poll().unwrap();
    (server, client)
}

/// One tick: the client sends `command` (if any), the server polls and
/// steps, the client polls. Returns slot 0's applied command.
fn tick(
    server: &mut Server<MemoryEndpoint>,
    client: &mut Client<MemoryEndpoint>,
    command: Option<PlayerCommand>,
) -> PlayerCommand {
    if let Some(command) = command {
        client.send(command).unwrap();
    }
    server.poll().unwrap();
    let applied = server.step().unwrap()[0];
    client.poll().unwrap();
    applied
}

/// A raw endpoint that says hello with the right protocol and reads the
/// welcome, for sending hand-made messages.
fn raw_client(
    net: &MemoryNetwork,
    server: &mut Server<MemoryEndpoint>,
) -> (MemoryEndpoint, Welcome) {
    let mut raw = net.endpoint();
    let hello = Hello {
        wire_version: WIRE_VERSION,
        sim_protocol: server.sim().protocol_version(),
    };
    send(&mut raw, server.local_id(), Message::Hello(hello));
    server.poll().unwrap();
    let replies = receive(&mut raw, server.sim().tick());
    let Some((_, Message::Welcome(welcome))) = replies.first() else {
        panic!("expected a welcome, got {replies:?}");
    };
    (raw, *welcome)
}

#[test]
fn welcome_gives_each_client_a_slot_and_the_session_constants() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(2), net.endpoint());
    let mut a = Client::connect(sim(2), net.endpoint(), server.local_id()).unwrap();
    let mut b = Client::connect(sim(2), net.endpoint(), server.local_id()).unwrap();
    assert_eq!(a.welcome(), None);
    assert_eq!(server.ack(PlayerSlot(0)), None);
    server.poll().unwrap();
    a.poll().unwrap();
    b.poll().unwrap();
    for (client, slot) in [(&a, 0), (&b, 1)] {
        let welcome = Welcome {
            tick: Tick::ZERO,
            slot: PlayerSlot(slot),
            players: 2,
            seed: SEED,
        };
        assert_eq!(client.welcome(), Some(&welcome));
        assert_eq!(
            client.latest(),
            Some(Tick::ZERO),
            "the state comes with the welcome"
        );
        assert_eq!(client.sim().state_hash(), server.sim().state_hash());
        assert_eq!(
            server.ack(PlayerSlot(slot)),
            Some(Tick::ZERO),
            "the welcome tick"
        );
    }
}

#[test]
fn a_full_server_denies_the_next_client() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(1), net.endpoint());
    let mut first = Client::connect(sim(1), net.endpoint(), server.local_id()).unwrap();
    let mut second = Client::connect(sim(1), net.endpoint(), server.local_id()).unwrap();
    server.poll().unwrap();
    first.poll().unwrap();
    let denied = SessionError::Denied(Denied {
        tick: Tick::ZERO,
        reason: DenyReason::FULL,
        wire_version: WIRE_VERSION,
        sim_protocol: server.sim().protocol_version(),
    });
    assert_eq!(second.poll(), Err(denied.clone()));
    assert_eq!(second.poll(), Err(denied), "it stays denied");
    assert_eq!(second.welcome(), None);
    assert_eq!(first.welcome().map(|w| w.slot), Some(PlayerSlot(0)));
}

#[test]
fn a_client_of_another_sim_protocol_is_denied() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(1), net.endpoint());
    let mut client = Client::connect(blob_sim(0, 0), net.endpoint(), server.local_id()).unwrap();
    server.poll().unwrap();
    let Err(SessionError::Denied(denied)) = client.poll() else {
        panic!("expected a denial");
    };
    assert_eq!(denied.reason, DenyReason::SIM_PROTOCOL);
    assert_eq!(denied.sim_protocol, server.sim().protocol_version());
    assert_eq!(server.ack(PlayerSlot(0)), None, "the slot stays free");
}

#[test]
fn a_hello_from_another_wire_version_is_denied() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(1), net.endpoint());
    let mut raw = net.endpoint();
    let hello = Hello {
        wire_version: WIRE_VERSION + 1,
        sim_protocol: server.sim().protocol_version(),
    };
    send(&mut raw, server.local_id(), Message::Hello(hello));
    server.poll().unwrap();
    let denied = Denied {
        tick: Tick::ZERO,
        reason: DenyReason::WIRE_VERSION,
        wire_version: WIRE_VERSION,
        sim_protocol: server.sim().protocol_version(),
    };
    assert_eq!(
        receive(&mut raw, Tick::ZERO),
        [(server.local_id(), Message::Denied(denied))]
    );
}

#[test]
fn a_repeated_hello_keeps_its_slot() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(2), net.endpoint());
    let (mut raw, welcome) = raw_client(&net, &mut server);
    assert_eq!(welcome.slot, PlayerSlot(0));
    let hello = Hello {
        wire_version: WIRE_VERSION,
        sim_protocol: server.sim().protocol_version(),
    };
    send(&mut raw, server.local_id(), Message::Hello(hello));
    server.poll().unwrap();
    let replies = receive(&mut raw, Tick::ZERO);
    assert!(
        matches!(replies.as_slice(), [(_, Message::Welcome(w)), (_, Message::Snapshot(_))] if *w == welcome),
        "{replies:?}"
    );
    let (_other, other) = raw_client(&net, &mut server);
    assert_eq!(other.slot, PlayerSlot(1));
}

#[test]
fn missing_commands_repeat_the_last_one() {
    let (mut server, mut client) = pair(sim(1), sim(1));
    let sent = [None, Some(cmd(2, 10)), None, Some(cmd(4, -3)), None];
    let applied: Vec<PlayerCommand> = sent
        .into_iter()
        .map(|command| tick(&mut server, &mut client, command))
        .collect();
    assert_eq!(
        applied,
        [idle(1), cmd(2, 10), cmd(3, 10), cmd(4, -3), cmd(5, -3)],
        "idle before the first command, then the last one restamped"
    );
    assert_eq!(walked(client.sim(), 0), 10 + 10 - 3 - 3);
}

#[test]
fn a_slot_without_a_client_idles() {
    let (mut server, mut client) = pair(sim(2), sim(2));
    for t in 1..=3 {
        client.send(cmd(t, 5)).unwrap();
        server.poll().unwrap();
        assert_eq!(server.step().unwrap(), [cmd(t, 5), idle(t)]);
        client.poll().unwrap();
    }
}

#[test]
fn commands_sent_ahead_wait_for_their_tick() {
    let (mut server, mut client) = pair(sim(1), sim(1));
    for t in 1..=3 {
        client.send(cmd(t, t as i8)).unwrap();
    }
    server.poll().unwrap();
    for t in 1..=3 {
        assert_eq!(server.step().unwrap(), [cmd(t, t as i8)]);
    }
}

#[test]
fn stale_far_ahead_and_repeated_commands_are_dropped() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(1), net.endpoint());
    let (mut raw, _) = raw_client(&net, &mut server);
    let to = server.local_id();
    let commands = |ack, commands: Vec<PlayerCommand>| {
        Message::Commands(Commands {
            ack: Tick(ack),
            commands,
        })
    };
    send(&mut raw, to, commands(0, vec![cmd(1, 1)]));
    server.poll().unwrap();
    assert_eq!(server.step().unwrap(), [cmd(1, 1)]);
    // Tick 1 is past; 1 + MAX_LEAD is the furthest the server buffers.
    let edge = 1 + MAX_LEAD;
    send(
        &mut raw,
        to,
        commands(
            1,
            vec![cmd(1, 99), cmd(2, 2), cmd(edge, 7), cmd(edge + 1, 8)],
        ),
    );
    send(&mut raw, to, commands(1, vec![cmd(2, 50)]));
    server.poll().unwrap();
    assert_eq!(
        server.step().unwrap(),
        [cmd(2, 2)],
        "the first copy is kept"
    );
    for t in 3..edge {
        assert_eq!(server.step().unwrap(), [cmd(t, 2)]);
    }
    assert_eq!(server.step().unwrap(), [cmd(edge, 7)]);
    assert_eq!(server.step().unwrap(), [cmd(edge + 1, 7)], "too far ahead");
}

#[test]
fn the_server_keeps_each_clients_newest_ack() {
    let (mut server, mut client) = pair(sim(1), sim(1));
    tick(&mut server, &mut client, Some(cmd(1, 1)));
    tick(&mut server, &mut client, Some(cmd(2, 1)));
    assert_eq!(
        server.ack(PlayerSlot(0)),
        Some(Tick(1)),
        "acked with tick 2's command"
    );
    client.send(cmd(3, 1)).unwrap();
    server.poll().unwrap();
    assert_eq!(server.ack(PlayerSlot(0)), Some(Tick(2)));

    // Hand-made acks: an older one changes nothing, one the server has not
    // reached drops its whole message.
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(1), net.endpoint());
    let (mut raw, _) = raw_client(&net, &mut server);
    let to = server.local_id();
    server.step().unwrap();
    server.step().unwrap();
    let commands = |ack, command| {
        Message::Commands(Commands {
            ack: Tick(ack),
            commands: vec![command],
        })
    };
    send(&mut raw, to, commands(2, cmd(3, 3)));
    send(&mut raw, to, commands(1, cmd(4, 4)));
    send(&mut raw, to, commands(5, cmd(5, 5)));
    server.poll().unwrap();
    assert_eq!(server.ack(PlayerSlot(0)), Some(Tick(2)));
    assert_eq!(server.step().unwrap(), [cmd(3, 3)]);
    assert_eq!(
        server.step().unwrap(),
        [cmd(4, 4)],
        "an older ack's commands count"
    );
    assert_eq!(server.step().unwrap(), [cmd(5, 4)], "the bad ack's do not");
}

#[test]
fn ticks_extend_across_the_32_bit_boundary() {
    let start = Tick(u64::from(u32::MAX) - 2);
    let (mut server, mut client) = pair(sim_at(1, start), sim(1));
    assert_eq!(client.welcome().map(|w| w.tick), Some(start));
    assert_eq!(client.latest(), Some(start));
    for t in start.0 + 1..=start.0 + 6 {
        let command = cmd(t, (t % 100) as i8);
        assert_eq!(tick(&mut server, &mut client, Some(command)), command);
        assert_eq!(client.latest(), Some(Tick(t)));
        assert_eq!(server.ack(PlayerSlot(0)), Some(Tick(t - 1)));
        assert_eq!(client.sim().state_hash(), server.sim().state_hash());
    }
}

#[test]
fn a_snapshot_larger_than_a_datagram_fails() {
    let overhead = 7 + blob_sim(0, 0).snapshot().len();
    // Exactly one datagram is fine.
    let (_server, client) = pair(
        blob_sim(MAX_DATAGRAM - overhead, 0),
        blob_sim(MAX_DATAGRAM - overhead, 0),
    );
    assert_eq!(client.latest(), Some(Tick::ZERO));

    // One byte more fails the welcome's snapshot.
    let net = MemoryNetwork::new();
    let mut server = Server::new(blob_sim(MAX_DATAGRAM + 1 - overhead, 0), net.endpoint());
    let _client = Client::connect(blob_sim(0, 0), net.endpoint(), server.local_id()).unwrap();
    let err = server.poll().unwrap_err();
    assert_eq!(
        err,
        SessionError::SnapshotTooLarge {
            tick: Tick::ZERO,
            len: MAX_DATAGRAM + 1
        }
    );
    assert!(err.to_string().starts_with("[E-SNAPSHOT-SIZE] "), "{err}");

    // A state that grows fails the step that makes it too large.
    let (mut server, mut client) = pair(blob_sim(MAX_DATAGRAM - overhead - 40, 4), blob_sim(0, 4));
    let failed = loop {
        client.send(cmd(server.sim().tick().0 + 1, 7)).unwrap();
        server.poll().unwrap();
        match server.step() {
            Ok(_) => client.poll().unwrap(),
            Err(err) => break err,
        }
    };
    let SessionError::SnapshotTooLarge { tick, len } = failed else {
        panic!("{failed:?}");
    };
    assert_eq!(tick, server.sim().tick());
    assert_eq!(
        server.applied(),
        [cmd(tick.0, 7)],
        "the tick's record stays"
    );
    assert_eq!(len, 7 + server.sim().snapshot().len());
    assert!(len > MAX_DATAGRAM);
    client.poll().unwrap();
    assert_eq!(
        client.latest(),
        Some(Tick(tick.0 - 1)),
        "the last one that fit"
    );
    assert!(7 + client.sim().snapshot().len() <= MAX_DATAGRAM);
}

#[test]
fn a_vanished_clients_slot_idles_with_its_view() {
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(1), net.endpoint());
    let mut client = Client::connect(sim(1), net.endpoint(), server.local_id()).unwrap();
    server.poll().unwrap();
    client.poll().unwrap();
    let walking = PlayerCommand {
        buttons: 1,
        view_yaw: 0x4000,
        view_pitch: 0x0100,
        select: 2,
        ..cmd(1, 3)
    };
    assert_eq!(tick(&mut server, &mut client, Some(walking)), walking);
    drop(client);
    // Tick 2's send finds the client gone; until then its command repeats.
    assert_eq!(server.step().unwrap(), [walking.repeated_at(Tick(2))]);
    assert_eq!(server.ack(PlayerSlot(0)), None, "dropped");
    let left = PlayerCommand {
        view_yaw: 0x4000,
        view_pitch: 0x0100,
        ..idle(3)
    };
    assert_eq!(server.step().unwrap(), [left], "idle, looking the same way");
    let mut next = Client::connect(sim(1), net.endpoint(), server.local_id()).unwrap();
    server.poll().unwrap();
    next.poll().unwrap();
    assert_eq!(
        next.welcome().map(|w| (w.slot, w.tick)),
        Some((PlayerSlot(0), Tick(3)))
    );
}

static ASKED: Mutex<Vec<u8>> = Mutex::new(Vec::new());

fn asking(world: &World, slot: PlayerSlot) -> Relevance {
    ASKED.lock().unwrap().push(slot.0);
    everything(world, slot)
}

#[test]
fn the_relevance_hook_is_asked_for_every_snapshot_of_every_client() {
    assert_eq!(
        everything(sim(1).world(), PlayerSlot(0)),
        Relevance::Everything
    );
    let net = MemoryNetwork::new();
    let mut server = Server::new(sim(2), net.endpoint()).with_relevance(asking);
    let mut a = Client::connect(sim(2), net.endpoint(), server.local_id()).unwrap();
    let mut b = Client::connect(sim(2), net.endpoint(), server.local_id()).unwrap();
    server.poll().unwrap();
    for _ in 0..2 {
        server.step().unwrap();
    }
    a.poll().unwrap();
    b.poll().unwrap();
    assert_eq!(*ASKED.lock().unwrap(), [0, 1, 0, 1, 0, 1]);
    assert_eq!(a.latest(), Some(Tick(2)));
}
