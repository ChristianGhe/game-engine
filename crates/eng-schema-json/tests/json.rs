use eng_schema_core::Schema;
use eng_schema_json::{Authored, JsonValue, load_toml};
use serde::Deserialize;

#[derive(Schema)]
#[schema(name = "test.Probe", version = 1)]
struct Probe {
    tenth: f32,
    big: u64,
    neg: i16,
    nan: f64,
    name: String,
    list: Vec<u8>,
    maybe: Option<u8>,
    mode: Mode,
}

#[derive(Schema)]
#[schema(name = "test.Mode", version = 1)]
enum Mode {
    _Off,
    On,
}

#[test]
fn json_is_stable_and_readable() {
    let value = JsonValue::of(&Probe {
        tenth: 0.1,
        big: u64::MAX,
        neg: -3,
        nan: f64::NAN,
        name: "a\"b\n".into(),
        list: vec![1, 2],
        maybe: None,
        mode: Mode::On,
    });
    assert_eq!(
        value.to_compact(),
        r#"{"tenth":0.1,"big":18446744073709551615,"neg":-3,"nan":null,"name":"a\"b\n","list":[1,2],"maybe":null,"mode":"On"}"#
    );
    assert_eq!(value.pointer("/list/1"), Some(&JsonValue::U64(2)));
    assert_eq!(
        value.pointer("/tenth").and_then(JsonValue::as_f64),
        Some(f64::from(0.1f32))
    );
    assert_eq!(value.pointer("/missing"), None);
    let pretty = JsonValue::object([("a", JsonValue::Array(vec![])), ("b", JsonValue::U64(1))]);
    assert_eq!(pretty.to_pretty(), "{\n  \"a\": [],\n  \"b\": 1\n}");
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Settings {
    version: u32,
    speed: u32,
    #[serde(default)]
    inner: Vec<Inner>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Inner {
    n: u8,
}

impl Authored for Settings {
    const KIND: &'static str = "settings";
    const VERSION: u32 = 2;
    fn migrate(from: u32, doc: &mut JsonValue) -> Result<(), String> {
        match from {
            1 => {
                let v = doc.remove("velocity").ok_or("v1 needs velocity")?;
                doc.insert("speed", v);
                Ok(())
            }
            _ => Err(format!("unknown version {from}")),
        }
    }
}

#[test]
fn toml_is_strict_with_extensions_and_migrations() {
    let ok: Settings = load_toml(
        "version = 2\nspeed = 3\nx_note = 'free'\n[[inner]]\nn = 1\nx_tag = 2\n",
        "ok.toml",
    )
    .unwrap();
    assert_eq!(ok.speed, 3);
    assert_eq!(ok.inner, [Inner { n: 1 }]);

    let migrated: Settings = load_toml("version = 1\nvelocity = 9\n", "old.toml").unwrap();
    assert_eq!((migrated.version, migrated.speed), (2, 9));

    let unknown = load_toml::<Settings>("version = 2\nspeed = 1\nsped = 2\n", "typo.toml");
    let err = unknown.unwrap_err().to_string();
    assert!(
        err.starts_with("[E-LOAD] typo.toml:") && err.contains("sped"),
        "{err}"
    );

    assert!(
        load_toml::<Settings>("speed = 1\n", "f").is_err(),
        "version is required"
    );
    assert!(load_toml::<Settings>("version = 3\nspeed = 1\n", "f").is_err());
    assert!(load_toml::<Settings>("version = 2\nspeed = \n", "f").is_err());
}
