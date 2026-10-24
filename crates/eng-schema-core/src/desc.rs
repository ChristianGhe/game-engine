use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::Sink;
use crate::encode::seq_len;

/// The shape of a [`Schema`](crate::Schema) type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDesc {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Str,
    Seq(Box<TypeDesc>),
    Option(Box<TypeDesc>),
    Struct {
        name: &'static str,
        version: u32,
        fields: Vec<(&'static str, TypeDesc)>,
    },
    Enum {
        name: &'static str,
        version: u32,
        variants: Vec<&'static str>,
    },
}

impl TypeDesc {
    /// Writes a canonical description, used to fingerprint schemas into the
    /// simulation protocol version.
    pub fn encode(&self, sink: &mut dyn Sink) {
        let tag: u8 = match self {
            Self::Bool => 0,
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 3,
            Self::U64 => 4,
            Self::I8 => 5,
            Self::I16 => 6,
            Self::I32 => 7,
            Self::I64 => 8,
            Self::F32 => 9,
            Self::F64 => 10,
            Self::Str => 11,
            Self::Seq(_) => 12,
            Self::Option(_) => 13,
            Self::Struct { .. } => 14,
            Self::Enum { .. } => 15,
        };
        sink.put(&[tag]);
        match self {
            Self::Seq(inner) | Self::Option(inner) => inner.encode(sink),
            Self::Struct {
                name,
                version,
                fields,
            } => {
                put_str(sink, name);
                sink.put(&version.to_le_bytes());
                sink.put(&seq_len(fields.len()).to_le_bytes());
                for (field, desc) in fields {
                    put_str(sink, field);
                    desc.encode(sink);
                }
            }
            Self::Enum {
                name,
                version,
                variants,
            } => {
                put_str(sink, name);
                sink.put(&version.to_le_bytes());
                sink.put(&seq_len(variants.len()).to_le_bytes());
                for variant in variants {
                    put_str(sink, variant);
                }
            }
            _ => {}
        }
    }
}

fn put_str(sink: &mut dyn Sink, s: &str) {
    sink.put(&seq_len(s.len()).to_le_bytes());
    sink.put(s.as_bytes());
}
