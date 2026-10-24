use alloc::vec::Vec;

use crate::{Schema, Visitor};

/// A byte destination for the canonical encoder: a buffer or a hasher.
pub trait Sink {
    fn put(&mut self, bytes: &[u8]);
}

impl Sink for Vec<u8> {
    fn put(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

/// The canonical encoder. Names are not encoded; lengths, option tags and
/// variant indices are.
pub struct Encoder<'a> {
    sink: &'a mut dyn Sink,
}

impl<'a> Encoder<'a> {
    pub fn new(sink: &'a mut dyn Sink) -> Self {
        Self { sink }
    }
}

impl Visitor for Encoder<'_> {
    fn bool(&mut self, v: bool) {
        self.sink.put(&[u8::from(v)]);
    }
    fn u8(&mut self, v: u8) {
        self.sink.put(&[v]);
    }
    fn u16(&mut self, v: u16) {
        self.sink.put(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.sink.put(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.sink.put(&v.to_le_bytes());
    }
    fn i8(&mut self, v: i8) {
        self.sink.put(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.sink.put(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.sink.put(&v.to_le_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.sink.put(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.sink.put(&v.to_bits().to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.sink.put(&v.to_bits().to_le_bytes());
    }
    fn str(&mut self, v: &str) {
        self.sink.put(&seq_len(v.len()).to_le_bytes());
        self.sink.put(v.as_bytes());
    }
    fn begin_struct(&mut self, _name: &'static str) {}
    fn field(&mut self, _name: &'static str) {}
    fn end_struct(&mut self) {}
    fn begin_seq(&mut self, len: u32) {
        self.sink.put(&len.to_le_bytes());
    }
    fn end_seq(&mut self) {}
    fn none(&mut self) {
        self.sink.put(&[0]);
    }
    fn some(&mut self) {
        self.sink.put(&[1]);
    }
    fn variant(&mut self, _enum_name: &'static str, index: u32, _name: &'static str) {
        self.sink.put(&index.to_le_bytes());
    }
}

/// Converts a length to the `u32` the format carries.
///
/// # Panics
/// If `len` exceeds `u32::MAX`; no simulation value is that large.
pub(crate) fn seq_len(len: usize) -> u32 {
    u32::try_from(len).expect("sequence longer than u32::MAX elements")
}

/// Writes `value`'s canonical encoding into `sink`.
pub fn encode<T: Schema>(value: &T, sink: &mut dyn Sink) {
    value.visit(&mut Encoder::new(sink));
}

/// Returns `value`'s canonical encoding.
pub fn to_bytes<T: Schema>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    encode(value, &mut out);
    out
}
