use eng_schema_core::{
    DecodeError, MAX_SEQ_LEN, Reader, Schema, TypeDesc, TypeRegistry, find_non_finite, from_bytes,
    to_bytes,
};
use proptest::prelude::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Schema)]
#[schema(name = "test.Marker", version = 1)]
struct Marker;

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

#[test]
fn roundtrip_derived_types() {
    let value = sample();
    assert_eq!(from_bytes::<Sample>(&to_bytes(&value)).unwrap(), value);
    assert_eq!(from_bytes::<Marker>(&to_bytes(&Marker)).unwrap(), Marker);
    assert!(to_bytes(&Marker).is_empty());
}

#[test]
fn describe_lists_fields_in_order() {
    let TypeDesc::Struct {
        name,
        version,
        fields,
    } = Sample::describe()
    else {
        panic!("expected a struct description");
    };
    assert_eq!((name, version), ("test.Sample", 3));
    let names: Vec<_> = fields.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names,
        [
            "flag", "small", "angle", "signed", "big", "ratio", "pos", "label", "list", "maybe",
            "mode", "inner"
        ]
    );
    assert_eq!(
        Mode::describe(),
        TypeDesc::Enum {
            name: "test.Mode",
            version: 1,
            variants: vec!["Idle", "Walk", "Run"]
        }
    );
}

#[test]
fn decode_rejects_malformed_input() {
    let bytes = to_bytes(&sample());
    for cut in 0..bytes.len() {
        assert!(
            from_bytes::<Sample>(&bytes[..cut]).is_err(),
            "truncated at {cut}"
        );
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert_eq!(
        from_bytes::<Sample>(&extra),
        Err(DecodeError::TrailingBytes { count: 1 })
    );
    assert!(matches!(
        from_bytes::<bool>(&[2]),
        Err(DecodeError::InvalidBool { .. })
    ));
    assert!(matches!(
        from_bytes::<Mode>(&[3, 0, 0, 0]),
        Err(DecodeError::InvalidVariant { index: 3, .. })
    ));
    let huge = (MAX_SEQ_LEN + 1).to_le_bytes();
    assert!(matches!(
        from_bytes::<Vec<u8>>(&huge),
        Err(DecodeError::LengthTooLarge { .. })
    ));
    assert!(matches!(
        from_bytes::<String>(&[1, 0, 0, 0, 0xff]),
        Err(DecodeError::InvalidUtf8 { .. })
    ));
}

#[test]
fn non_finite_reports_json_pointer() {
    let mut value = sample();
    assert_eq!(find_non_finite(&|v| value.visit(v)), None);
    value.inner.1 = f32::NAN;
    let found = find_non_finite(&|v| value.visit(v)).unwrap();
    assert_eq!(found.path, "/inner/1");
    let list = vec![Inner(0, 1.0), Inner(0, f32::INFINITY)];
    let found = find_non_finite(&|v| list.visit(v)).unwrap();
    assert_eq!(found.path, "/1/1");
}

#[test]
fn registry_rejects_conflicting_names() {
    #[derive(Schema)]
    #[schema(name = "test.Mode", version = 2)]
    struct Imposter {
        _x: u8,
    }
    let mut registry = TypeRegistry::new();
    registry.register::<Mode>().unwrap();
    registry.register::<Mode>().unwrap();
    assert!(registry.register::<Imposter>().is_err());
    assert_eq!(registry.len(), 1);
}

#[test]
fn reader_reports_positions() {
    let mut r = Reader::new(&[1, 2, 3]);
    assert_eq!(r.read_u16().unwrap(), 0x0201);
    assert_eq!(r.position(), 2);
    assert_eq!(
        r.read_u32(),
        Err(DecodeError::UnexpectedEnd { at: 2, needed: 3 })
    );
}

proptest! {
    #[test]
    fn roundtrip_any_sample(
        flag: bool, small: u8, angle: u16, signed: i32, big: u64,
        ratio: f32, pos: f64, label in ".{0,12}", list: Vec<i16>,
        maybe: Option<u32>, mode in 0u32..3, a: i8, b: f32,
    ) {
        let value = Sample {
            flag, small, angle, signed, big, ratio, pos, label, list, maybe,
            mode: [Mode::Idle, Mode::Walk, Mode::Run][mode as usize],
            inner: Inner(a, b),
        };
        let bytes = to_bytes(&value);
        let back = from_bytes::<Sample>(&bytes).unwrap();
        // Compare encodings, so NaN payloads count as equal only bit for bit.
        prop_assert_eq!(to_bytes(&back), bytes);
    }

    #[test]
    fn decode_never_panics(bytes: Vec<u8>) {
        let _ = from_bytes::<Sample>(&bytes);
    }
}

#[test]
fn container_checks_magic_kind_and_version() {
    use eng_schema_core::container::{read_container, write_container};
    let bytes = write_container(*b"TEST", 2, &sample());
    assert_eq!(&bytes[..12], b"ENGFTEST\x02\x00\x00\x00");
    assert_eq!(
        read_container::<Sample>(&bytes, *b"TEST", 2).unwrap(),
        sample()
    );
    assert!(read_container::<Sample>(&bytes, *b"RECD", 2).is_err());
    assert!(read_container::<Sample>(&bytes, *b"TEST", 1).is_err());
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert!(read_container::<Sample>(&bad, *b"TEST", 2).is_err());
}
