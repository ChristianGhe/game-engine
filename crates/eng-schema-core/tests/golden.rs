//! Golden vectors for the canonical binary format. PROTECTED ORACLE: every
//! state hash depends on these bytes, so only the owner approves changes.

use eng_schema_core::{Schema, to_bytes};

#[derive(Debug, Clone, PartialEq, Schema)]
#[schema(name = "test.Sample", version = 3)]
struct Sample {
    flag: bool,
    small: u8,
    angle: u16,
    signed: i32,
    big: u64,
    ratio: f32,
    pos: f64,
    label: String,
    list: Vec<i16>,
    maybe: Option<u32>,
    mode: Mode,
    inner: Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Inner", version = 1)]
struct Inner(i8, f32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "test.Mode", version = 1)]
enum Mode {
    Idle,
    Walk,
    Run,
}

fn sample() -> Sample {
    Sample {
        flag: true,
        small: 7,
        angle: 0x4000,
        signed: -2,
        big: 0x0102_0304_0506_0708,
        ratio: 0.5,
        pos: -1.25,
        label: "dot".into(),
        list: vec![1, -1],
        maybe: Some(9),
        mode: Mode::Walk,
        inner: Inner(-3, 2.0),
    }
}

/// Golden vector for the canonical format. Changing it changes every state
/// hash, so it is a protected oracle.
#[test]
fn canonical_encoding_golden_vector() {
    let bytes = to_bytes(&sample());
    let expected: Vec<u8> = [
        &[1u8][..],                                        // flag
        &[7],                                              // small
        &[0x00, 0x40],                                     // angle
        &(-2i32).to_le_bytes(),                            // signed
        &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01], // big
        &0.5f32.to_bits().to_le_bytes(),                   // ratio
        &(-1.25f64).to_bits().to_le_bytes(),               // pos
        &[3, 0, 0, 0, b'd', b'o', b't'],                   // label
        &[2, 0, 0, 0, 1, 0, 0xff, 0xff],                   // list
        &[1, 9, 0, 0, 0],                                  // maybe
        &[1, 0, 0, 0],                                     // mode = Walk
        &[0xfd],                                           // inner.0
        &2.0f32.to_bits().to_le_bytes(),                   // inner.1
    ]
    .concat();
    assert_eq!(bytes, expected);
}
