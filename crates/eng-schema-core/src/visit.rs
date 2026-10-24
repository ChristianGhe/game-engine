/// Receives a value's structure in declaration order.
///
/// Calls nest the way the value does: a struct is `begin_struct`, then
/// `field` before each field's value, then `end_struct`. A sequence is
/// `begin_seq(len)`, `len` values, `end_seq`. An option is `none`, or `some`
/// followed by the value. A fieldless enum is a single `variant` call.
pub trait Visitor {
    fn bool(&mut self, v: bool);
    fn u8(&mut self, v: u8);
    fn u16(&mut self, v: u16);
    fn u32(&mut self, v: u32);
    fn u64(&mut self, v: u64);
    fn i8(&mut self, v: i8);
    fn i16(&mut self, v: i16);
    fn i32(&mut self, v: i32);
    fn i64(&mut self, v: i64);
    fn f32(&mut self, v: f32);
    fn f64(&mut self, v: f64);
    fn str(&mut self, v: &str);
    fn begin_struct(&mut self, name: &'static str);
    fn field(&mut self, name: &'static str);
    fn end_struct(&mut self);
    fn begin_seq(&mut self, len: u32);
    fn end_seq(&mut self);
    fn none(&mut self);
    fn some(&mut self);
    fn variant(&mut self, enum_name: &'static str, index: u32, name: &'static str);
}
