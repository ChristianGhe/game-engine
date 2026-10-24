use alloc::string::String;
use core::fmt;

use crate::Schema;

/// Hard cap on any decoded sequence or string length.
pub const MAX_SEQ_LEN: u32 = 1 << 24;

/// Reads the canonical encoding. Never panics on malformed input.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    UnexpectedEnd {
        at: usize,
        needed: usize,
    },
    InvalidBool {
        at: usize,
        value: u8,
    },
    InvalidOptionTag {
        at: usize,
        value: u8,
    },
    InvalidUtf8 {
        at: usize,
    },
    InvalidVariant {
        ty: &'static str,
        index: u32,
    },
    LengthTooLarge {
        at: usize,
        len: u32,
    },
    TrailingBytes {
        count: usize,
    },
    /// A named section did not match what the reader expected.
    Mismatch {
        expected: String,
        found: String,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd { at, needed } => {
                write!(
                    f,
                    "unexpected end of data at byte {at} (needed {needed} more)"
                )
            }
            Self::InvalidBool { at, value } => write!(f, "invalid bool {value} at byte {at}"),
            Self::InvalidOptionTag { at, value } => {
                write!(f, "invalid option tag {value} at byte {at}")
            }
            Self::InvalidUtf8 { at } => write!(f, "invalid UTF-8 in string at byte {at}"),
            Self::InvalidVariant { ty, index } => write!(f, "invalid variant {index} for {ty}"),
            Self::LengthTooLarge { at, len } => {
                write!(f, "length {len} at byte {at} exceeds the cap {MAX_SEQ_LEN}")
            }
            Self::TrailingBytes { count } => write!(f, "{count} trailing bytes after value"),
            Self::Mismatch { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
        }
    }
}

impl core::error::Error for DecodeError {}

macro_rules! read_le {
    ($name:ident, $ty:ty) => {
        pub fn $name(&mut self) -> Result<$ty, DecodeError> {
            let bytes = self.take(size_of::<$ty>())?;
            let mut buf = [0u8; size_of::<$ty>()];
            buf.copy_from_slice(bytes);
            Ok(<$ty>::from_le_bytes(buf))
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining() < n {
            return Err(DecodeError::UnexpectedEnd {
                at: self.pos,
                needed: n - self.remaining(),
            });
        }
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    read_le!(read_u8, u8);
    read_le!(read_u16, u16);
    read_le!(read_u32, u32);
    read_le!(read_u64, u64);
    read_le!(read_i8, i8);
    read_le!(read_i16, i16);
    read_le!(read_i32, i32);
    read_le!(read_i64, i64);

    pub fn read_f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    pub fn read_f64(&mut self) -> Result<f64, DecodeError> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    pub fn read_bool(&mut self) -> Result<bool, DecodeError> {
        let at = self.pos;
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(DecodeError::InvalidBool { at, value }),
        }
    }

    /// Reads an option tag: `false` for none, `true` for some.
    pub fn read_option_tag(&mut self) -> Result<bool, DecodeError> {
        let at = self.pos;
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(DecodeError::InvalidOptionTag { at, value }),
        }
    }

    /// Reads a sequence or string length, capped at [`MAX_SEQ_LEN`].
    pub fn read_len(&mut self) -> Result<u32, DecodeError> {
        let at = self.pos;
        let len = self.read_u32()?;
        if len > MAX_SEQ_LEN {
            return Err(DecodeError::LengthTooLarge { at, len });
        }
        Ok(len)
    }

    pub fn read_str(&mut self) -> Result<String, DecodeError> {
        let len = self.read_len()? as usize;
        let at = self.pos;
        let bytes = self.take(len)?;
        let text = core::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8 { at })?;
        Ok(String::from(text))
    }

    /// Fails unless every byte was consumed.
    pub fn finish(&self) -> Result<(), DecodeError> {
        match self.remaining() {
            0 => Ok(()),
            count => Err(DecodeError::TrailingBytes { count }),
        }
    }
}

/// Decodes exactly one value from `bytes`.
pub fn from_bytes<T: Schema>(bytes: &[u8]) -> Result<T, DecodeError> {
    let mut reader = Reader::new(bytes);
    let value = T::decode(&mut reader)?;
    reader.finish()?;
    Ok(value)
}
