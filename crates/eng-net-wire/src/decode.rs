use alloc::vec::Vec;
use core::fmt;

use eng_core::{PlayerCommand, PlayerSlot, Tick};

use crate::{
    Commands, Denied, DenyReason, Hello, MAX_COMMANDS, Message, Snapshot, WIRE_VERSION, Welcome,
    extend_tick, kind,
};

/// Why bytes are not a message. Decoding never panics: every malformed
/// input ends in one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The `needed`-byte field at byte `at` runs past the end.
    UnexpectedEnd { at: usize, needed: usize },
    /// A whole message, then `count` more bytes.
    TrailingBytes { count: usize },
    /// The first byte names no message kind.
    UnknownKind(u8),
    /// A `Hello` from another wire version: its layout after the version is
    /// that version's, so only the version is read. The server answers with
    /// `Denied`.
    WireVersion { found: u32 },
    /// A `Hello`'s tick field is not zero.
    HelloTick(u32),
    /// A `Commands` count above [`MAX_COMMANDS`].
    TooManyCommands(u8),
    /// Command ticks must strictly increase; the one at `index` does not.
    CommandOrder { index: usize },
    /// No tick with these low 32 bits lies within 2^31 ticks of the
    /// baseline (see [`extend_tick`]).
    TickOutOfRange { low: u32, baseline: Tick },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd { at, needed } => {
                write!(f, "the {needed}-byte field at byte {at} runs past the end")
            }
            Self::TrailingBytes { count } => write!(f, "{count} bytes after the message"),
            Self::UnknownKind(kind) => write!(f, "unknown message kind {kind}"),
            Self::WireVersion { found } => write!(f, "a hello from wire version {found}"),
            Self::HelloTick(tick) => write!(f, "a hello with tick {tick}, not 0"),
            Self::TooManyCommands(n) => {
                write!(f, "{n} commands; a message carries at most {MAX_COMMANDS}")
            }
            Self::CommandOrder { index } => {
                write!(f, "command {index}'s tick is not after the one before it")
            }
            Self::TickOutOfRange { low, baseline } => {
                write!(f, "no tick with low bits {low:#x} near baseline {baseline}")
            }
        }
    }
}

impl core::error::Error for DecodeError {}

impl Message {
    /// Decodes one whole message.
    ///
    /// `baseline` supplies the high 32 bits of the ticks in `Commands` and
    /// `Snapshot` (see [`extend_tick`]). It is the receiver's last
    /// acknowledged tick: on the server, the client's previous `ack`; on the
    /// client, its newest snapshot's tick; on either, the `Welcome` tick
    /// until then. The handshake needs none: `Welcome` and `Denied` carry
    /// their ticks' high bits, and `Hello` has no tick.
    ///
    /// A wrong baseline cannot be detected. A tick more than 2^31 ticks
    /// from it decodes off by a multiple of 2^32, so a client drops a
    /// `Snapshot` that arrives before its `Welcome` instead of decoding it
    /// against tick 0.
    pub fn decode(bytes: &[u8], baseline: Tick) -> Result<Self, DecodeError> {
        let mut r = Reader { bytes, pos: 0 };
        let kind = r.u8()?;
        if !(kind::HELLO..=kind::SNAPSHOT).contains(&kind) {
            return Err(DecodeError::UnknownKind(kind));
        }
        let low = r.u32()?;
        let extend =
            |low| extend_tick(baseline, low).ok_or(DecodeError::TickOutOfRange { low, baseline });
        let message = match kind {
            kind::HELLO => {
                let wire_version = r.u32()?;
                if wire_version != WIRE_VERSION {
                    return Err(DecodeError::WireVersion {
                        found: wire_version,
                    });
                }
                if low != 0 {
                    return Err(DecodeError::HelloTick(low));
                }
                Self::Hello(Hello {
                    wire_version,
                    sim_protocol: r.u64()?,
                })
            }
            kind::WELCOME => Self::Welcome(Welcome {
                tick: full_tick(low, r.u32()?),
                slot: PlayerSlot(r.u8()?),
                players: r.u8()?,
                seed: r.u64()?,
            }),
            kind::DENIED => Self::Denied(Denied {
                tick: full_tick(low, r.u32()?),
                reason: DenyReason(r.u8()?),
                wire_version: r.u32()?,
                sim_protocol: r.u64()?,
            }),
            kind::COMMANDS => {
                let ack = extend(low)?;
                let count = r.u8()?;
                if usize::from(count) > MAX_COMMANDS {
                    return Err(DecodeError::TooManyCommands(count));
                }
                let mut commands: Vec<PlayerCommand> = Vec::with_capacity(usize::from(count));
                for index in 0..usize::from(count) {
                    let cmd = r.command()?;
                    if commands.last().is_some_and(|prev| cmd.tick <= prev.tick) {
                        return Err(DecodeError::CommandOrder { index });
                    }
                    commands.push(cmd);
                }
                Self::Commands(Commands { ack, commands })
            }
            _ => {
                let tick = extend(low)?;
                let len = r.u16()?;
                let state = r.take(usize::from(len))?.to_vec();
                Self::Snapshot(Snapshot { tick, state })
            }
        };
        r.finish()?;
        Ok(message)
    }
}

fn full_tick(low: u32, high: u32) -> Tick {
    Tick(u64::from(high) << 32 | u64::from(low))
}

/// Reads fields front to back. Every read checks the length first.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let rest = self.bytes.get(self.pos..).unwrap_or_default();
        let (field, _) = rest.split_at_checked(n).ok_or(DecodeError::UnexpectedEnd {
            at: self.pos,
            needed: n,
        })?;
        self.pos += n;
        Ok(field)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    fn i8(&mut self) -> Result<i8, DecodeError> {
        Ok(i8::from_le_bytes(self.array()?))
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    /// eng-core's canonical layout, the one recordings use.
    fn command(&mut self) -> Result<PlayerCommand, DecodeError> {
        Ok(PlayerCommand {
            tick: Tick(self.u64()?),
            buttons: self.u32()?,
            forward: self.i8()?,
            side: self.i8()?,
            view_yaw: self.u16()?,
            view_pitch: self.u16()?,
            select: self.u8()?,
        })
    }

    fn finish(&self) -> Result<(), DecodeError> {
        match self.bytes.len().saturating_sub(self.pos) {
            0 => Ok(()),
            count => Err(DecodeError::TrailingBytes { count }),
        }
    }
}
