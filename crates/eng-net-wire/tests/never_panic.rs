//! Property tests for criterion 11 (`wire-session`): decoding never panics
//! on any input, accepted bytes are canonical, and every valid message
//! survives a round trip.

use eng_core::{PlayerCommand, PlayerSlot, Tick};
use eng_net_wire::{
    Commands, Denied, DenyReason, Hello, MAX_COMMANDS, Message, Snapshot, WIRE_VERSION, Welcome,
    extend_tick,
};
use proptest::collection::{btree_set, vec};
use proptest::prelude::*;
use proptest::sample::Index;

fn command() -> impl Strategy<Value = PlayerCommand> {
    (
        any::<u32>(),
        any::<i8>(),
        any::<i8>(),
        any::<u16>(),
        any::<u16>(),
        any::<u8>(),
    )
        .prop_map(
            |(buttons, forward, side, view_yaw, view_pitch, select)| PlayerCommand {
                tick: Tick(0),
                buttons,
                forward,
                side,
                view_yaw,
                view_pitch,
                select,
            },
        )
}

fn message() -> impl Strategy<Value = Message> {
    let tick = || any::<u64>().prop_map(Tick);
    prop_oneof![
        any::<u64>().prop_map(|sim_protocol| Message::Hello(Hello {
            wire_version: WIRE_VERSION,
            sim_protocol,
        })),
        (tick(), any::<u8>(), any::<u8>(), any::<u64>()).prop_map(|(tick, slot, players, seed)| {
            Message::Welcome(Welcome {
                tick,
                slot: PlayerSlot(slot),
                players,
                seed,
            })
        }),
        (tick(), any::<u8>(), any::<u32>(), any::<u64>()).prop_map(
            |(tick, reason, wire_version, sim_protocol)| Message::Denied(Denied {
                tick,
                reason: DenyReason(reason),
                wire_version,
                sim_protocol,
            })
        ),
        (
            tick(),
            btree_set(any::<u64>(), 0..=MAX_COMMANDS),
            vec(command(), MAX_COMMANDS),
        )
            .prop_map(|(ack, ticks, cmds)| Message::Commands(Commands {
                ack,
                // A set iterates in order: the ticks strictly increase.
                commands: ticks
                    .into_iter()
                    .zip(cmds)
                    .map(|(t, c)| c.repeated_at(Tick(t)))
                    .collect(),
            })),
        (tick(), vec(any::<u8>(), 0..64))
            .prop_map(|(tick, state)| Message::Snapshot(Snapshot { tick, state })),
    ]
}

/// The tick a receiver must extend, if the message has one.
fn extended_tick(message: &Message) -> Option<Tick> {
    match message {
        Message::Commands(m) => Some(m.ack),
        Message::Snapshot(m) => Some(m.tick),
        _ => None,
    }
}

/// A valid message's bytes with one byte flipped, inserted or removed, or
/// cut short: inputs that reach deep into the decoder.
fn damaged() -> impl Strategy<Value = Vec<u8>> {
    (message(), 0..4u8, any::<Index>(), any::<u8>()).prop_map(|(m, how, at, byte)| {
        let mut bytes = m.encode().unwrap();
        let i = at.index(bytes.len());
        match how {
            0 => bytes[i] ^= byte | 1,
            1 => bytes.insert(i, byte),
            2 => {
                bytes.remove(i);
            }
            _ => bytes.truncate(i),
        }
        bytes
    })
}

proptest! {
    #[test]
    fn decode_never_panics(bytes: Vec<u8>, baseline: u64) {
        let _ = Message::decode(&bytes, Tick(baseline));
    }

    #[test]
    fn damaged_messages_never_panic_and_stay_canonical(bytes in damaged(), baseline: u64) {
        // Whatever the decoder accepts, it reads back to the same bytes.
        if let Ok(message) = Message::decode(&bytes, Tick(baseline)) {
            prop_assert_eq!(message.encode().unwrap(), bytes);
        }
    }

    #[test]
    fn valid_messages_round_trip(message in message(), ahead: i32, other: u64) {
        let baseline = match extended_tick(&message) {
            // A baseline whose window holds the tick:
            // tick - baseline = ahead, in [-2^31, 2^31 - 1].
            Some(t) => t.0.checked_add_signed(-i64::from(ahead)).unwrap_or(t.0),
            // The handshake ignores the baseline.
            None => other,
        };
        let bytes = message.encode().unwrap();
        prop_assert_eq!(Message::decode(&bytes, Tick(baseline)), Ok(message));
    }

    #[test]
    fn extension_recovers_every_tick_in_the_window(tick: u64, ahead: i32) {
        let Some(baseline) = tick.checked_add_signed(-i64::from(ahead)) else {
            return Ok(());
        };
        prop_assert_eq!(extend_tick(Tick(baseline), tick as u32), Some(Tick(tick)));
    }

    #[test]
    fn extension_keeps_the_low_bits_within_the_window(baseline: u64, low: u32) {
        if let Some(Tick(t)) = extend_tick(Tick(baseline), low) {
            prop_assert_eq!(t as u32, low);
            let distance = i128::from(t) - i128::from(baseline);
            prop_assert!((-(1i128 << 31)..(1i128 << 31)).contains(&distance));
        }
    }
}
