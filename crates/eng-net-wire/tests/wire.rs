//! The codec's behaviour: tick extension, round trips, caps and every
//! decode error. Byte layouts are pinned in `layout.rs`.

use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_net_wire::{
    Commands, DecodeError, Denied, DenyReason, EncodeError, Hello, MAX_COMMANDS, MAX_STATE_LEN,
    Message, Snapshot, WIRE_VERSION, Welcome, extend_tick,
};

fn command(tick: u64) -> PlayerCommand {
    PlayerCommand {
        buttons: 0x55,
        forward: -3,
        ..PlayerCommand::idle(Tick(tick))
    }
}

fn commands(ack: u64, ticks: &[u64]) -> Message {
    Message::Commands(Commands {
        ack: Tick(ack),
        commands: ticks.iter().map(|&t| command(t)).collect(),
    })
}

fn snapshot(tick: u64, len: usize) -> Message {
    Message::Snapshot(Snapshot {
        tick: Tick(tick),
        state: (0..len).map(|i| i as u8).collect(),
    })
}

/// One valid message of each kind.
fn samples() -> Vec<Message> {
    vec![
        Message::Hello(Hello {
            wire_version: WIRE_VERSION,
            sim_protocol: 0x1234,
        }),
        Message::Welcome(Welcome {
            tick: Tick(77),
            slot: PlayerSlot(0),
            players: 1,
            seed: 9,
        }),
        Message::Denied(Denied {
            tick: Tick(77),
            reason: DenyReason::WIRE_VERSION,
            wire_version: WIRE_VERSION,
            sim_protocol: 0x1234,
        }),
        commands(70, &[75, 76, 77]),
        snapshot(77, 4),
    ]
}

fn round_trip(message: &Message, baseline: Tick) {
    let bytes = message.encode().unwrap();
    assert_eq!(&Message::decode(&bytes, baseline).unwrap(), message);
}

#[test]
fn extension_finds_the_tick_nearest_the_baseline() {
    let cases: [(u64, u32, Option<u64>); 12] = [
        (0, 0, Some(0)),
        (0, 7, Some(7)),
        // Behind the baseline.
        (10, 3, Some(3)),
        // Forward and backward across a wrap of the low 32 bits.
        (0xffff_fff0, 0x0000_0005, Some(0x1_0000_0005)),
        (0x1_0000_0005, 0xffff_fff0, Some(0xffff_fff0)),
        // The window is [baseline - 2^31, baseline + 2^31 - 1].
        (0x1_0000_0000, 0x7fff_ffff, Some(0x1_7fff_ffff)),
        (0x1_0000_0000, 0x8000_0000, Some(0x8000_0000)),
        // A window that starts below tick 0: tick 0 itself, then nothing.
        (0x7fff_fff0, 0, Some(0)),
        (0x7fff_fff0, 0xffff_fff8, None),
        // The last tick there is.
        (u64::MAX - 5, 0xffff_ffff, Some(u64::MAX)),
        // Before tick 0, or past u64::MAX: no such tick.
        (0, 0xffff_ffff, None),
        (u64::MAX, 0, None),
    ];
    for (baseline, low, want) in cases {
        assert_eq!(
            extend_tick(Tick(baseline), low),
            want.map(Tick),
            "baseline {baseline:#x}, low {low:#x}"
        );
    }
}

#[test]
fn messages_at_their_caps_round_trip() {
    let five: Vec<u64> = (100..100 + MAX_COMMANDS as u64).collect();
    assert_eq!(five.len(), 5);
    round_trip(&commands(99, &five), Tick(99));
    // No commands: a pure acknowledgement.
    round_trip(&commands(99, &[]), Tick(99));
    assert_eq!(MAX_STATE_LEN, 65_535);
    round_trip(&snapshot(5, 65_535), Tick(5));
    round_trip(&snapshot(5, 0), Tick(5));
}

#[test]
fn encoding_refuses_more_than_the_caps() {
    let six: Vec<u64> = (100..106).collect();
    assert_eq!(
        commands(99, &six).encode(),
        Err(EncodeError::TooManyCommands(6))
    );
    assert_eq!(
        snapshot(5, 65_536).encode(),
        Err(EncodeError::StateTooLong(65_536))
    );
}

#[test]
fn encoding_refuses_commands_out_of_order() {
    assert_eq!(
        commands(1, &[5, 5]).encode(),
        Err(EncodeError::CommandOrder { index: 1 })
    );
    assert_eq!(
        commands(1, &[7, 6, 8]).encode(),
        Err(EncodeError::CommandOrder { index: 1 })
    );
}

#[test]
fn commands_carry_eng_cores_canonical_bytes() {
    let cmd = PlayerCommand {
        tick: Tick(0x0001_0203_0405_0607),
        buttons: 0xdead_beef,
        forward: -100,
        side: 42,
        view_yaw: 0x1234,
        view_pitch: 0xfedc,
        select: 9,
    };
    let bytes = Message::Commands(Commands {
        ack: Tick(0),
        commands: vec![cmd],
    })
    .encode()
    .unwrap();
    // After the kind, the tick and the count.
    assert_eq!(&bytes[6..], &eng_schema_core::to_bytes(&cmd)[..]);
}

#[test]
fn every_truncation_fails_with_unexpected_end() {
    assert_eq!(
        Message::decode(&[], Tick(0)),
        Err(DecodeError::UnexpectedEnd { at: 0, needed: 1 })
    );
    for message in samples() {
        let bytes = message.encode().unwrap();
        for cut in 0..bytes.len() {
            let got = Message::decode(&bytes[..cut], Tick(77));
            assert!(
                matches!(got, Err(DecodeError::UnexpectedEnd { .. })),
                "{message:?} cut to {cut} bytes gave {got:?}"
            );
        }
    }
    // A Hello cut after its version lacks the 8-byte sim protocol.
    let hello = samples()[0].encode().unwrap();
    assert_eq!(
        Message::decode(&hello[..9], Tick(0)),
        Err(DecodeError::UnexpectedEnd { at: 9, needed: 8 })
    );
    // A Snapshot that promises 3 state bytes and has 1.
    assert_eq!(
        Message::decode(&[0x05, 0, 0, 0, 0, 3, 0, 0xaa], Tick(0)),
        Err(DecodeError::UnexpectedEnd { at: 7, needed: 3 })
    );
}

#[test]
fn trailing_bytes_are_refused() {
    for message in samples() {
        let mut bytes = message.encode().unwrap();
        bytes.push(0);
        assert_eq!(
            Message::decode(&bytes, Tick(77)),
            Err(DecodeError::TrailingBytes { count: 1 }),
            "{message:?}"
        );
        bytes.extend([1, 2]);
        assert_eq!(
            Message::decode(&bytes, Tick(77)),
            Err(DecodeError::TrailingBytes { count: 3 }),
            "{message:?}"
        );
    }
}

#[test]
fn unknown_kinds_are_refused_before_anything_else() {
    for kind in [0x00, 0x06, 0xff] {
        assert_eq!(
            Message::decode(&[kind], Tick(0)),
            Err(DecodeError::UnknownKind(kind))
        );
    }
}

#[test]
fn a_hello_from_another_wire_version_reports_only_its_version() {
    // Whatever follows the version belongs to the other version's layout,
    // so neither the tick nor the rest is read.
    let cases: [&[u8]; 3] = [
        &[0x01, 0, 0, 0, 0, 0x02, 0, 0, 0, 0xde, 0xad],
        &[0x01, 9, 0, 0, 0, 0x07, 0, 0, 0],
        &[
            0x01, 0, 0, 0, 0, 0x00, 0x00, 0x01, 0x00, 1, 2, 3, 4, 5, 6, 7, 8,
        ],
    ];
    let found = [2, 7, 0x0001_0000];
    for (bytes, found) in cases.iter().zip(found) {
        assert_eq!(
            Message::decode(bytes, Tick(0)),
            Err(DecodeError::WireVersion { found })
        );
    }
}

#[test]
fn a_hello_has_no_tick() {
    let mut bytes = samples()[0].encode().unwrap();
    bytes[1] = 5;
    assert_eq!(
        Message::decode(&bytes, Tick(0)),
        Err(DecodeError::HelloTick(5))
    );
}

#[test]
fn decoding_refuses_more_commands_than_the_cap_before_reading_them() {
    // The count alone decides, with no commands present.
    assert_eq!(
        Message::decode(&[0x04, 0, 0, 0, 0, 0x06], Tick(0)),
        Err(DecodeError::TooManyCommands(6))
    );
    assert_eq!(
        Message::decode(&[0x04, 0, 0, 0, 0, 0xff], Tick(0)),
        Err(DecodeError::TooManyCommands(0xff))
    );
}

#[test]
fn decoding_refuses_commands_out_of_order() {
    // Two commands for tick 5, built from a valid message with ticks 5, 6.
    let mut bytes = commands(1, &[5, 6]).encode().unwrap();
    let second_tick = 6 + 19;
    assert_eq!(bytes[second_tick], 6);
    bytes[second_tick] = 5;
    assert_eq!(
        Message::decode(&bytes, Tick(1)),
        Err(DecodeError::CommandOrder { index: 1 })
    );
    // Each tick is compared with the one before it, not with the first:
    // 5, 7, 6 is out of order at the third command although 6 > 5.
    let mut bytes = commands(1, &[5, 7, 8]).encode().unwrap();
    let third_tick = 6 + 2 * 19;
    assert_eq!(bytes[third_tick], 8);
    bytes[third_tick] = 6;
    assert_eq!(
        Message::decode(&bytes, Tick(1)),
        Err(DecodeError::CommandOrder { index: 2 })
    );
}

#[test]
fn ticks_that_cannot_be_extended_are_refused() {
    let low = 0xffff_ffff;
    let snapshot = [0x05, 0xff, 0xff, 0xff, 0xff, 0, 0];
    let ack = [0x04, 0xff, 0xff, 0xff, 0xff, 0];
    for bytes in [&snapshot[..], &ack[..]] {
        assert_eq!(
            Message::decode(bytes, Tick(0)),
            Err(DecodeError::TickOutOfRange {
                low,
                baseline: Tick(0)
            })
        );
    }
}

#[test]
fn denied_keeps_reasons_it_does_not_know() {
    // A newer server's reason must still reach an older client.
    let denied = Message::Denied(Denied {
        tick: Tick(1),
        reason: DenyReason(0x7f),
        wire_version: 9,
        sim_protocol: 1,
    });
    round_trip(&denied, Tick(0));
}
