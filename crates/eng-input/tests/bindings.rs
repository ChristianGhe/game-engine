//! Key and action names, and the bindings file's other strictness rules.
//! Criterion 18's own checks are in `live_input.rs`.

use eng_input::*;

#[test]
fn key_names_are_w3c_codes_and_round_trip() {
    assert_eq!(Key::COUNT, Key::ALL.len());
    for (i, &key) in Key::ALL.iter().enumerate() {
        assert_eq!(key.index(), i);
        assert_eq!(Key::from_name(key.name()), Some(key));
        assert_eq!(key.to_string(), key.name());
    }
    let mut names: Vec<_> = Key::ALL.iter().map(|k| k.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Key::COUNT, "names are unique");
    // Spot checks against the W3C `code` table.
    for name in [
        "KeyA",
        "KeyZ",
        "Digit0",
        "Digit9",
        "ArrowUp",
        "ArrowLeft",
        "Escape",
        "Space",
        "Enter",
        "Tab",
        "Backspace",
        "ShiftLeft",
        "ControlRight",
        "AltLeft",
        "MetaLeft",
        "Backquote",
        "Quote",
        "IntlBackslash",
        "Numpad0",
        "NumpadEnter",
        "F1",
        "F12",
        "PageDown",
    ] {
        assert!(Key::from_name(name).is_some(), "{name}");
    }
    // Names are exact; there are no aliases.
    for not_a_key in ["keyw", "W", "Esc", "Up", "SuperLeft", "OSLeft", "F13", ""] {
        assert_eq!(Key::from_name(not_a_key), None, "{not_a_key}");
    }
}

#[test]
fn action_names_round_trip() {
    let names: Vec<_> = Action::ALL.iter().map(|a| a.name()).collect();
    assert_eq!(
        names,
        [
            "move_forward",
            "move_back",
            "move_left",
            "move_right",
            "quit"
        ]
    );
    for action in Action::ALL {
        assert_eq!(Action::from_name(action.name()), Some(action));
        assert_eq!(action.is_app(), action == Action::Quit);
    }
    assert_eq!(Action::from_name("Quit"), None);
}

#[test]
fn action_sets() {
    let set = ActionSet::EMPTY
        .with(Action::Quit)
        .with(Action::MoveLeft)
        .with(Action::Quit);
    assert!(ActionSet::EMPTY.is_empty() && !set.is_empty());
    assert!(set.contains(Action::Quit) && !set.contains(Action::MoveRight));
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        [Action::MoveLeft, Action::Quit]
    );
    assert_eq!(command_for(set, eng_core::Tick(3), 0, 0).side, -FULL_AXIS);
}

#[test]
fn unknown_keys_get_a_suggestion() {
    let cases = [
        ("w", Some("KeyW")),
        ("1", Some("Digit1")),
        ("up", Some("ArrowUp")),
        ("escape", Some("Escape")),
        ("Esc", Some("Escape")),
        ("space", Some("Space")),
        ("Jump", None),
    ];
    for (name, suggestion) in cases {
        let text = format!("version = 1\n[actions]\nquit = [\"{name}\"]\n");
        let err = Bindings::parse(&text, "b.toml").unwrap_err().to_string();
        assert!(err.contains(&format!("unknown key `{name}`")), "{err}");
        match suggestion {
            Some(key) => assert!(err.contains(&format!("did you mean `{key}`?")), "{err}"),
            None => assert!(!err.contains("did you mean"), "{err}"),
        }
        assert!(err.contains("W3C `code` names"), "{err}");
    }
}

/// Every problem is `[E-BINDINGS]`. Only TOML syntax (whose message names
/// its line), the version and a non-table `actions` have no `:line`.
#[test]
fn other_problems_fail_with_e_bindings_too() {
    let cases = [
        (
            "[actions]\nquit = [\"Escape\"]\n",
            None,
            "missing `version = 1`",
        ),
        ("version = 2\n", None, "version 2 is newer"),
        ("version = 1\nactions = 3\n", None, "expected a map"),
        (
            "version = 1\n[actions]\nquit = [\"Escape\"\n",
            None,
            "at line 3",
        ),
        (
            "version = 1\n[actions]\nquit = [\"Escape\"]\nquit = [\"KeyQ\"]\n",
            None,
            "duplicate key",
        ),
        (
            "version = 1\n[action]\nquit = [\"Escape\"]\n",
            Some(2),
            "unknown field `action`; a bindings file has `version` and `[actions]`",
        ),
        (
            "version = 1\n[actions]\nquit = \"Escape\"\n",
            Some(3),
            "`quit` must be a list of key names",
        ),
        (
            "version = 1\n[actions]\nquit = [1]\n",
            Some(3),
            "`quit` must be a list of key names",
        ),
        (
            "version = 1\n[actions]\nquit = [\"Escape\", \"Escape\"]\n",
            Some(3),
            "key `Escape` is already bound to `quit` on line 3",
        ),
        // The earlier problem wins, top level or not.
        (
            "version = 1\n[actions]\nquit = [\"Nope\"]\n\n[zzz]\n",
            Some(3),
            "unknown key `Nope`",
        ),
        (
            "version = 1\nzzz = 1\n[actions]\nquit = [\"Nope\"]\n",
            Some(2),
            "unknown field `zzz`",
        ),
        // Names from the file are escaped, so a message is one line.
        (
            "version = 1\n[actions]\nquit = [\"Key\\nW\"]\n",
            Some(3),
            "unknown key `Key\\nW`",
        ),
    ];
    for (text, line, needle) in cases {
        let err = Bindings::parse(text, "b.toml").unwrap_err();
        assert_eq!(err.line, line, "{err}");
        let shown = err.to_string();
        assert!(shown.starts_with("[E-BINDINGS] b.toml"), "{shown}");
        assert!(
            shown.contains(needle),
            "{shown:?} should mention {needle:?}"
        );
        if line.is_some() {
            assert_eq!(shown.lines().count(), 1, "{shown}");
        }
    }
}

#[test]
fn extensions_and_empty_files_load() {
    let text =
        "version = 1\nx_by = \"me\"\n[actions]\nx_later = [\"NotAKey\"]\nquit = [\"KeyQ\"]\n";
    let b = Bindings::parse(text, "b.toml").unwrap();
    assert_eq!(b.keys(Action::Quit).collect::<Vec<_>>(), [Key::KeyQ]);
    assert_eq!(
        Bindings::parse("version = 1\n", "b.toml").unwrap(),
        Bindings::empty()
    );
}

#[test]
fn bind_and_unbind() {
    let mut b = Bindings::empty();
    assert_eq!(b.bind(Key::Space, Action::MoveForward), None);
    assert_eq!(b.bind(Key::Space, Action::Quit), Some(Action::MoveForward));
    assert_eq!(b.action(Key::Space), Some(Action::Quit));
    assert_eq!(b.unbind(Key::Space), Some(Action::Quit));
    assert_eq!(b, Bindings::empty());
}
