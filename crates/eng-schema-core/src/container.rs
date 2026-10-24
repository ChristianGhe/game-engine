//! The engine's one versioned binary file format (spec 6.11), used for
//! recordings now and cooked data later:
//!
//! | bytes  | content                                   |
//! |--------|-------------------------------------------|
//! | 0..4   | magic `ENGF`                              |
//! | 4..8   | kind, four ASCII bytes such as `RECD`     |
//! | 8..12  | format version of that kind, `u32` LE     |
//! | 12..   | canonical encoding of the payload         |

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{DecodeError, Reader, Schema, encode};

pub const MAGIC: [u8; 4] = *b"ENGF";

pub fn write_container<T: Schema>(kind: [u8; 4], version: u32, value: &T) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&kind);
    out.extend_from_slice(&version.to_le_bytes());
    encode(value, &mut out);
    out
}

/// Reads a container of `kind` written at exactly `version`.
pub fn read_container<T: Schema>(
    bytes: &[u8],
    kind: [u8; 4],
    version: u32,
) -> Result<T, DecodeError> {
    let mut r = Reader::new(bytes);
    let magic = r.take(4)?;
    if magic != MAGIC {
        return Err(DecodeError::Mismatch {
            expected: String::from("an ENGF file"),
            found: format!("magic {magic:02x?}"),
        });
    }
    let found_kind = r.take(4)?;
    if found_kind != kind {
        return Err(DecodeError::Mismatch {
            expected: format!("kind {}", String::from_utf8_lossy(&kind)),
            found: format!("kind {}", String::from_utf8_lossy(found_kind)),
        });
    }
    let found_version = r.read_u32()?;
    if found_version != version {
        return Err(DecodeError::Mismatch {
            expected: format!("version {version}"),
            found: format!("version {found_version}"),
        });
    }
    let value = T::decode(&mut r)?;
    r.finish()?;
    Ok(value)
}
