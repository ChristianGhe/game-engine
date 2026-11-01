use eng_core::{PlayerCommand, Tick};
use eng_input::*;

const SCRIPT: &str = r#"
version = 1
x_comment = "extensions are ignored"

[[segment]]
ticks = 2
forward = 127
yaw_deg = 90.0

[[segment]]
ticks = 1
side = -5
buttons = 3
pitch_deg = -10.0
"#;

#[test]
fn script_expands_with_held_angles() {
    let script = InputScript::parse(SCRIPT, "test.toml").unwrap();
    assert_eq!(script.len_ticks(), 3);
    let cmds = script.commands(5);
    assert_eq!(cmds.len(), 5);
    assert_eq!(cmds[0].tick, Tick(1));
    assert_eq!((cmds[0].forward, cmds[0].view_yaw), (127, 0x4000));
    assert_eq!((cmds[1].forward, cmds[1].view_yaw), (127, 0x4000));
    assert_eq!(
        (
            cmds[2].forward,
            cmds[2].side,
            cmds[2].buttons,
            cmds[2].view_yaw
        ),
        (0, -5, 3, 0x4000)
    );
    assert_eq!(cmds[2].view_pitch, degrees_to_angle(-10.0));
    let streamed: Vec<_> = script.iter_commands().take(5).collect();
    assert_eq!(streamed, cmds);
    // After the script: idle, angles held.
    assert_eq!(
        cmds[4],
        PlayerCommand {
            view_yaw: 0x4000,
            view_pitch: degrees_to_angle(-10.0),
            ..PlayerCommand::idle(Tick(5))
        }
    );
}

/// `into_commands` keeps the script, so a run can hold its commands without
/// borrowing it (`play` leaked the script for that), and gives exactly
/// `iter_commands`'s, past the end too. A segment of 0 ticks, which only a
/// script built in code can have, gives nothing, not even its angles.
#[test]
fn an_owned_script_gives_the_same_commands() {
    let parsed = InputScript::parse(SCRIPT, "test.toml").unwrap();
    let mut built = parsed.clone();
    let mut empty = built.segment[1].clone();
    (empty.ticks, empty.forward, empty.yaw_deg) = (0, 9, Some(45.0));
    built.segment.insert(1, empty);
    assert_eq!(
        built.commands(7),
        parsed.commands(7),
        "0 ticks give nothing"
    );
    for script in [
        parsed,
        built,
        InputScript::parse("version = 1", "none").unwrap(),
    ] {
        let borrowed: Vec<PlayerCommand> = script.iter_commands().take(7).collect();
        let owned: Box<dyn Iterator<Item = PlayerCommand>> = Box::new(script.into_commands());
        assert_eq!(owned.take(7).collect::<Vec<_>>(), borrowed);
    }
}

#[test]
fn quantization() {
    assert_eq!(degrees_to_angle(0.0), 0);
    assert_eq!(degrees_to_angle(90.0), 0x4000);
    assert_eq!(degrees_to_angle(-90.0), 0xc000);
    assert_eq!(degrees_to_angle(360.0), 0);
    assert_eq!(degrees_to_angle(359.999), 0);
    assert_eq!(degrees_to_angle(1.0), 182);
}

#[test]
fn bad_scripts_name_the_problem() {
    let cases = [
        (
            "version = 1\n[[segment]]\nticks = 0\n",
            "ticks must be at least 1",
        ),
        (
            "version = 1\n[[segment]]\nticks = 1\nforward = 200\n",
            "forward",
        ),
        (
            "version = 1\n[[segment]]\nticks = 1\nyaw_deg = nan\n",
            "yaw_deg must be finite",
        ),
        (
            "version = 1\n[[segment]]\nticks = 1\nforwards = 1\n",
            "forwards",
        ),
        ("[[segment]]\nticks = 1\n", "version"),
    ];
    for (text, needle) in cases {
        let err = InputScript::parse(text, "bad.toml")
            .unwrap_err()
            .to_string();
        assert!(err.contains(needle), "{err:?} should mention {needle:?}");
    }
}

fn recording() -> Recording {
    let commands = InputScript::parse(SCRIPT, "t").unwrap().commands(3);
    Recording {
        header: RecordingHeader {
            game: "testbed".into(),
            engine_sha: "abc".into(),
            game_sha: "abc".into(),
            rustc: "1.99.1".into(),
            sim_protocol: 0x1234,
            sim_data_hash: 0,
            seed: 9,
            players: 1,
            tick_rate: 60,
        },
        commands,
        hashes: vec![
            TickHash {
                tick: Tick(0),
                hash: 1,
            },
            TickHash {
                tick: Tick(3),
                hash: 2,
            },
        ],
    }
}

#[test]
fn recording_roundtrips_and_validates() {
    let rec = recording();
    rec.validate().unwrap();
    let bytes = write_recording(&rec);
    assert_eq!(&bytes[..8], b"ENGFRECD");
    assert_eq!(read_recording(&bytes).unwrap(), rec);
    assert_eq!(rec.ticks(), 3);
    assert_eq!(rec.commands_at(Tick(2))[0].forward, 127);

    let mut bad = rec.clone();
    bad.commands[1].tick = Tick(7);
    assert!(bad.validate().is_err());
    let mut bad = rec.clone();
    bad.hashes.swap(0, 1);
    assert!(bad.validate().is_err());
    let mut bad = rec.clone();
    bad.hashes.clear();
    assert!(bad.validate().is_err(), "a recording must carry hashes");
    let mut bad = rec.clone();
    bad.hashes.pop();
    assert!(bad.validate().is_err(), "the last tick must be hashed");
    assert!(read_recording(&bytes[..bytes.len() - 1]).is_err());
}
