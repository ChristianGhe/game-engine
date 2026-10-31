use alloc::vec::Vec;
use core::fmt;

use eng_core::PlayerCommand;

use crate::{MAX_COMMANDS, MAX_STATE_LEN, Message, high_bits, kind, low_bits};

/// A message the format cannot carry. Each is a bug in the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// More than [`MAX_COMMANDS`] commands.
    TooManyCommands(usize),
    /// Command ticks must strictly increase; the one at `index` does not.
    CommandOrder { index: usize },
    /// More than [`MAX_STATE_LEN`] state bytes.
    StateTooLong(usize),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCommands(n) => {
                write!(f, "{n} commands; a message carries at most {MAX_COMMANDS}")
            }
            Self::CommandOrder { index } => {
                write!(f, "command {index}'s tick is not after the one before it")
            }
            Self::StateTooLong(n) => {
                write!(
                    f,
                    "{n} state bytes; a snapshot carries at most {MAX_STATE_LEN}"
                )
            }
        }
    }
}

impl core::error::Error for EncodeError {}

impl Message {
    /// The message as bytes, for any transport.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut w = Writer(Vec::new());
        match self {
            Self::Hello(m) => {
                w.header(kind::HELLO, 0);
                w.u32(m.wire_version);
                w.u64(m.sim_protocol);
            }
            Self::Welcome(m) => {
                w.header(kind::WELCOME, low_bits(m.tick));
                w.u32(high_bits(m.tick));
                w.u8(m.slot.0);
                w.u8(m.players);
                w.u64(m.seed);
            }
            Self::Denied(m) => {
                w.header(kind::DENIED, low_bits(m.tick));
                w.u32(high_bits(m.tick));
                w.u8(m.reason.0);
                w.u32(m.wire_version);
                w.u64(m.sim_protocol);
            }
            Self::Commands(m) => {
                let count = m.commands.len();
                if count > MAX_COMMANDS {
                    return Err(EncodeError::TooManyCommands(count));
                }
                let pairs = m.commands.windows(2);
                if let Some(at) = pairs.into_iter().position(|p| p[1].tick <= p[0].tick) {
                    return Err(EncodeError::CommandOrder { index: at + 1 });
                }
                w.header(kind::COMMANDS, low_bits(m.ack));
                w.u8(count as u8);
                for cmd in &m.commands {
                    w.command(cmd);
                }
            }
            Self::Snapshot(m) => {
                let len = m.state.len();
                if len > MAX_STATE_LEN {
                    return Err(EncodeError::StateTooLong(len));
                }
                w.header(kind::SNAPSHOT, low_bits(m.tick));
                w.u16(len as u16);
                w.0.extend_from_slice(&m.state);
            }
        }
        Ok(w.0)
    }
}

struct Writer(Vec<u8>);

impl Writer {
    /// Every message starts with its kind and its tick's low 32 bits.
    fn header(&mut self, kind: u8, tick_low: u32) {
        self.u8(kind);
        self.u32(tick_low);
    }

    /// eng-core's canonical layout, the one recordings use.
    fn command(&mut self, cmd: &PlayerCommand) {
        let PlayerCommand {
            tick,
            buttons,
            forward,
            side,
            view_yaw,
            view_pitch,
            select,
        } = *cmd;
        self.u64(tick.0);
        self.u32(buttons);
        self.0.extend_from_slice(&forward.to_le_bytes());
        self.0.extend_from_slice(&side.to_le_bytes());
        self.u16(view_yaw);
        self.u16(view_pitch);
        self.u8(select);
    }

    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
}
