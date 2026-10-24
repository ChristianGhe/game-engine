use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::encode::seq_len;
use crate::{DecodeError, Reader, Schema, TypeDesc, Visitor};

macro_rules! primitive {
    ($ty:ty, $name:literal, $desc:ident, $visit:ident, $read:ident) => {
        impl Schema for $ty {
            const NAME: &'static str = $name;
            const VERSION: u32 = 0;
            fn describe() -> TypeDesc {
                TypeDesc::$desc
            }
            fn visit(&self, v: &mut dyn Visitor) {
                v.$visit(*self);
            }
            fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
                r.$read()
            }
        }
    };
}

primitive!(bool, "bool", Bool, bool, read_bool);
primitive!(u8, "u8", U8, u8, read_u8);
primitive!(u16, "u16", U16, u16, read_u16);
primitive!(u32, "u32", U32, u32, read_u32);
primitive!(u64, "u64", U64, u64, read_u64);
primitive!(i8, "i8", I8, i8, read_i8);
primitive!(i16, "i16", I16, i16, read_i16);
primitive!(i32, "i32", I32, i32, read_i32);
primitive!(i64, "i64", I64, i64, read_i64);
primitive!(f32, "f32", F32, f32, read_f32);
primitive!(f64, "f64", F64, f64, read_f64);

impl Schema for String {
    const NAME: &'static str = "str";
    const VERSION: u32 = 0;
    fn describe() -> TypeDesc {
        TypeDesc::Str
    }
    fn visit(&self, v: &mut dyn Visitor) {
        v.str(self);
    }
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        r.read_str()
    }
}

impl<T: Schema> Schema for Vec<T> {
    const NAME: &'static str = "seq";
    const VERSION: u32 = 0;
    fn describe() -> TypeDesc {
        TypeDesc::Seq(Box::new(T::describe()))
    }
    fn visit(&self, v: &mut dyn Visitor) {
        v.begin_seq(seq_len(self.len()));
        for item in self {
            item.visit(v);
        }
        v.end_seq();
    }
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let len = r.read_len()? as usize;
        // Never trust a length for preallocation beyond the bytes present.
        let mut out = Vec::with_capacity(len.min(r.remaining()));
        for _ in 0..len {
            out.push(T::decode(r)?);
        }
        Ok(out)
    }
}

impl<T: Schema> Schema for Option<T> {
    const NAME: &'static str = "option";
    const VERSION: u32 = 0;
    fn describe() -> TypeDesc {
        TypeDesc::Option(Box::new(T::describe()))
    }
    fn visit(&self, v: &mut dyn Visitor) {
        match self {
            None => v.none(),
            Some(value) => {
                v.some();
                value.visit(v);
            }
        }
    }
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        if r.read_option_tag()? {
            Ok(Some(T::decode(r)?))
        } else {
            Ok(None)
        }
    }
}
