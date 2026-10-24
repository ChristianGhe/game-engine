use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::{self, Write};

use crate::Visitor;

/// A NaN or infinity found in state that must stay finite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonFinite {
    /// JSON pointer to the value, matching the value's JSON dump,
    /// e.g. `/entities/0/components/kit_fps.MoveState/vel/x`.
    pub path: String,
    /// The offending value's bits (`f32` bits are zero-extended).
    pub bits: u64,
}

impl fmt::Display for NonFinite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "non-finite float at {} (bits {:#x})",
            self.path, self.bits
        )
    }
}

/// Walks a value and returns the first non-finite float, if any.
pub fn find_non_finite(walk: &dyn Fn(&mut dyn Visitor)) -> Option<NonFinite> {
    let mut checker = FiniteCheck::default();
    walk(&mut checker);
    checker.found
}

enum Frame {
    Struct { field: Option<&'static str> },
    Seq { index: u32 },
}

#[derive(Default)]
struct FiniteCheck {
    stack: Vec<Frame>,
    found: Option<NonFinite>,
}

impl FiniteCheck {
    fn value_done(&mut self) {
        if let Some(Frame::Seq { index }) = self.stack.last_mut() {
            *index += 1;
        }
    }

    fn report(&mut self, bits: u64) {
        if self.found.is_some() {
            return;
        }
        let mut path = String::new();
        for frame in &self.stack {
            match frame {
                Frame::Struct { field: Some(name) } => {
                    path.push('/');
                    for c in name.chars() {
                        match c {
                            '~' => path.push_str("~0"),
                            '/' => path.push_str("~1"),
                            c => path.push(c),
                        }
                    }
                }
                Frame::Struct { field: None } => {}
                Frame::Seq { index } => {
                    let _ = write!(path, "/{index}");
                }
            }
        }
        if path.is_empty() {
            path.push('/');
        }
        self.found = Some(NonFinite { path, bits });
    }
}

impl Visitor for FiniteCheck {
    fn bool(&mut self, _: bool) {
        self.value_done();
    }
    fn u8(&mut self, _: u8) {
        self.value_done();
    }
    fn u16(&mut self, _: u16) {
        self.value_done();
    }
    fn u32(&mut self, _: u32) {
        self.value_done();
    }
    fn u64(&mut self, _: u64) {
        self.value_done();
    }
    fn i8(&mut self, _: i8) {
        self.value_done();
    }
    fn i16(&mut self, _: i16) {
        self.value_done();
    }
    fn i32(&mut self, _: i32) {
        self.value_done();
    }
    fn i64(&mut self, _: i64) {
        self.value_done();
    }
    fn f32(&mut self, v: f32) {
        if !v.is_finite() {
            self.report(u64::from(v.to_bits()));
        }
        self.value_done();
    }
    fn f64(&mut self, v: f64) {
        if !v.is_finite() {
            self.report(v.to_bits());
        }
        self.value_done();
    }
    fn str(&mut self, _: &str) {
        self.value_done();
    }
    fn begin_struct(&mut self, _name: &'static str) {
        self.stack.push(Frame::Struct { field: None });
    }
    fn field(&mut self, name: &'static str) {
        if let Some(Frame::Struct { field }) = self.stack.last_mut() {
            *field = Some(name);
        }
    }
    fn end_struct(&mut self) {
        self.stack.pop();
        self.value_done();
    }
    fn begin_seq(&mut self, _len: u32) {
        self.stack.push(Frame::Seq { index: 0 });
    }
    fn end_seq(&mut self) {
        self.stack.pop();
        self.value_done();
    }
    fn none(&mut self) {
        self.value_done();
    }
    fn some(&mut self) {}
    fn variant(&mut self, _: &'static str, _: u32, _: &'static str) {
        self.value_done();
    }
}
