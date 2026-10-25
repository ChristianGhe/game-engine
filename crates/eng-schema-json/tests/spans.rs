use eng_schema_json::PathStep::{Index, Key};
use eng_schema_json::TomlSpans;

const TEXT: &str = "\
version = 1
x_note = \"kept for positions\"

[actions]
quit = [\"Escape\"]
move = [
  \"KeyW\",
  \"KeyS\",
]
inline = { a = [1, 2] }

[[segment]]
ticks = 1

[[segment]]
ticks = 2
";

#[test]
fn lines_of_keys_and_elements() {
    let spans = TomlSpans::parse(TEXT);
    let cases: [(&[_], _); 11] = [
        (&[Key("version")], Some(1)),
        (&[Key("x_note")], Some(2)),
        (&[Key("actions")], Some(4)),
        (&[Key("actions"), Key("quit")], Some(5)),
        (&[Key("actions"), Key("quit"), Index(0)], Some(5)),
        (&[Key("actions"), Key("move"), Index(1)], Some(8)),
        (
            &[Key("actions"), Key("inline"), Key("a"), Index(1)],
            Some(10),
        ),
        (&[Key("segment"), Index(1), Key("ticks")], Some(16)),
        (&[Key("actions"), Key("move"), Index(2)], None),
        (&[Key("actions"), Key("absent")], None),
        (&[Key("version"), Key("deeper")], None),
    ];
    for (path, line) in cases {
        assert_eq!(spans.line_of(path), line, "{path:?}");
    }
    assert_eq!(spans.offset(&[]), None);
    assert_eq!(spans.offset(&[Key("version")]), Some(0));
    assert_eq!(spans.line(usize::MAX), 17, "clamped to the end");
}

#[test]
fn text_that_does_not_parse_has_no_positions() {
    let spans = TomlSpans::parse("version = [\n");
    assert_eq!(spans.line_of(&[Key("version")]), None);
    assert_eq!((spans.line(5), spans.line(12)), (1, 2), "lines still count");
}
