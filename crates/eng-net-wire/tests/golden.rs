//! Golden vectors: the byte layout of each message kind, wire version 1
//! (spec 6.8, contract 7). PROTECTED ORACLE: builds of the engine talk to
//! each other in these bytes. Every byte below is written out by hand. The
//! header (kind, then the tick's low 32 bits), `Hello`'s first three fields
//! and `Denied` are how builds of different versions deny each other; rung
//! 5 fixes their final form with eng-net-proto's handshake.

use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_net_wire::{Commands, Denied, DenyReason, Hello, Message, Snapshot, Welcome};

fn check(message: &Message, baseline: Tick, bytes: &[u8]) {
    assert_eq!(message.encode().unwrap(), bytes, "encoding {message:?}");
    assert_eq!(
        &Message::decode(bytes, baseline).unwrap(),
        message,
        "decoding {bytes:02x?}"
    );
}

#[test]
fn hello_layout() {
    let hello = Message::Hello(Hello {
        wire_version: 1,
        sim_protocol: 0x0123_4567_89ab_cdef,
    });
    #[rustfmt::skip]
    let bytes = [
        0x01,                                           // kind: Hello
        0x00, 0x00, 0x00, 0x00,                         // tick: none known yet
        0x01, 0x00, 0x00, 0x00,                         // wire version 1
        0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01, // sim protocol
    ];
    check(&hello, Tick(0), &bytes);
}

#[test]
fn welcome_layout() {
    let welcome = Message::Welcome(Welcome {
        tick: Tick(0x2_0000_0010),
        slot: PlayerSlot(1),
        players: 2,
        seed: 0x0011_2233_4455_6677,
    });
    #[rustfmt::skip]
    let bytes = [
        0x02,                                           // kind: Welcome
        0x10, 0x00, 0x00, 0x00,                         // tick, low 32 bits
        0x02, 0x00, 0x00, 0x00,                         // tick, high 32 bits
        0x01,                                           // the client's player slot
        0x02,                                           // players in the session
        0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00, // session seed
    ];
    check(&welcome, Tick(0), &bytes);
}

#[test]
fn denied_layout() {
    let denied = Message::Denied(Denied {
        tick: Tick(0x1_8000_0000),
        reason: DenyReason::SIM_PROTOCOL,
        wire_version: 1,
        sim_protocol: 0xfedc_ba98_7654_3210,
    });
    #[rustfmt::skip]
    let bytes = [
        0x03,                                           // kind: Denied
        0x00, 0x00, 0x00, 0x80,                         // tick, low 32 bits
        0x01, 0x00, 0x00, 0x00,                         // tick, high 32 bits
        0x02,                                           // reason: sim protocol
        0x01, 0x00, 0x00, 0x00,                         // the server's wire version
        0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe, // the server's sim protocol
    ];
    check(&denied, Tick(0), &bytes);
}

#[test]
fn commands_layout() {
    let commands = Message::Commands(Commands {
        ack: Tick(0x0102),
        commands: vec![
            PlayerCommand {
                tick: Tick(0x0103),
                buttons: 0x8000_0001,
                forward: 127,
                side: -128,
                view_yaw: 0x4000,
                view_pitch: 0xfff0,
                select: 3,
            },
            PlayerCommand {
                tick: Tick(0x0104),
                buttons: 0,
                forward: -1,
                side: 1,
                view_yaw: 0xffff,
                view_pitch: 0,
                select: 0,
            },
        ],
    });
    #[rustfmt::skip]
    let bytes = [
        0x04,                   // kind: Commands
        0x02, 0x01, 0x00, 0x00, // acknowledged snapshot tick, low 32 bits
        0x02,                   // command count
        // The first command, in eng-core's canonical layout: tick (u64),
        // buttons (u32), forward, side, yaw (u16), pitch (u16), select.
        0x03, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x80,
        0x7f, 0x80,
        0x00, 0x40,
        0xf0, 0xff,
        0x03,
        // The second command.
        0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0xff, 0x01,
        0xff, 0xff,
        0x00, 0x00,
        0x00,
    ];
    check(&commands, Tick(0x0100), &bytes);
}

#[test]
fn snapshot_layout() {
    let snapshot = Message::Snapshot(Snapshot {
        tick: Tick(0x1_0000_0005),
        state: vec![0xaa, 0xbb, 0xcc],
    });
    #[rustfmt::skip]
    let bytes = [
        0x05,                   // kind: Snapshot
        0x05, 0x00, 0x00, 0x00, // tick, low 32 bits
        0x03, 0x00,             // state length
        0xaa, 0xbb, 0xcc,       // state
    ];
    // The receiver's baseline supplies the high bits.
    check(&snapshot, Tick(0x1_0000_0000), &bytes);
}
