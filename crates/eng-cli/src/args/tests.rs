//! Unit tests for the command line: every command and its refusals.

use super::*;

fn args(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

#[test]
fn parses_run_and_dump_world() {
    let Command::Run(run) =
        parse(&args("run --headless --ticks 5 --seed 2 --hash-every 3")).unwrap()
    else {
        panic!("expected run");
    };
    assert_eq!(
        (run.ticks, run.seed, run.hash_every, run.players),
        (5, 2, 3, 1)
    );
    let Command::Run(dump) = parse(&args("dump-world --at-tick 9 --out w.json")).unwrap() else {
        panic!("expected dump-world");
    };
    assert_eq!(dump.dump, Some((9, PathBuf::from("w.json"))));
    assert_eq!(dump.ticks, 9);
}

#[test]
fn parses_version() {
    assert_eq!(
        parse(&args("--version")),
        Ok(Command::Version { json: false })
    );
    assert_eq!(
        parse(&args("--version --json")),
        Ok(Command::Version { json: true })
    );
}

#[test]
fn parses_shot() {
    let command = parse(&args(
        "shot --at-tick 120 --out s.png --seed 1 --players 2 --input i.toml \
         --report r.json --budgets b.toml",
    ))
    .unwrap();
    assert_eq!(command.name(), "shot");
    let Command::Run(shot) = command else {
        panic!("expected shot");
    };
    assert_eq!((shot.ticks, shot.seed, shot.players), (120, 1, 2));
    assert_eq!(shot.input, Some(PathBuf::from("i.toml")));
    assert_eq!(shot.report, Some(PathBuf::from("r.json")));
    assert_eq!(shot.budgets, Some(PathBuf::from("b.toml")));
    assert_eq!(
        shot.capture,
        Some(CaptureArgs {
            frames: Frames::Last,
            out: PathBuf::from("s.png"),
            width: 320,
            height: 240,
        })
    );
    assert_eq!(shot.dump, None);
    let Command::Run(sized) = parse(&args("shot --at-tick 0 --out s.png --size 64x48")).unwrap()
    else {
        panic!("expected shot");
    };
    let capture = sized.capture.unwrap();
    assert_eq!((sized.ticks, capture.width, capture.height), (0, 64, 48));
}

#[test]
fn parses_video() {
    let command = parse(&args(
        "video --ticks 240 --out p.apng --every 2 --size 8192x1",
    ))
    .unwrap();
    assert_eq!(command.name(), "video");
    let Command::Run(video) = command else {
        panic!("expected video");
    };
    assert_eq!(video.ticks, 240);
    assert_eq!(
        video.capture,
        Some(CaptureArgs {
            frames: Frames::Every(2),
            out: PathBuf::from("p.apng"),
            width: 8192,
            height: 1,
        })
    );
    let Command::Run(every_tick) = parse(&args("video --ticks 9999 --out p.apng")).unwrap() else {
        panic!("expected video");
    };
    assert_eq!(every_tick.capture.unwrap().frames, Frames::Every(1));
    // The largest video: 10,000 frames, ticks 0 to 19,998.
    assert!(parse(&args("video --ticks 19999 --every 2 --out p.apng")).is_ok());
}

#[test]
fn rejects_bad_shot_and_video_usage() {
    for bad in [
        "shot",
        "shot --out s.png",
        "shot --at-tick 1",
        "shot --at-tick 1 --out s.png --ticks 3",
        "shot --at-tick 1 --out s.png --every 2",
        "shot --at-tick 1 --out s.png --hashes h.txt",
        "shot --at-tick 1 --out s.png --record r.rec",
        "shot --at-tick 1 --out s.png --size 0x240",
        "shot --at-tick 1 --out s.png --size 320x0",
        "shot --at-tick 1 --out s.png --size 8193x240",
        "shot --at-tick 1 --out s.png --size 320",
        "shot --at-tick 1 --out s.png --size 320x",
        "shot --at-tick 1 --out s.png --size x240",
        "shot --at-tick 1 --out s.png --size 32ax24",
        "shot --at-tick 1 --out s.png --size 320x240x1",
        "shot --at-tick 18446744073709551615 --out s.png",
        "video",
        "video --ticks 5",
        "video --out p.apng",
        "video --ticks 5 --out p.apng --at-tick 3",
        "video --ticks 5 --out p.apng --every 0",
        "video --ticks 5 --out p.apng --every x",
        "video --ticks 10000 --out p.apng",
        "video --ticks 20000 --every 2 --out p.apng",
        "video --ticks 5 --out p.apng --assert a.toml",
        "run --ticks 1 --size 4x4",
        "run --ticks 1 --every 2",
        "dump-world --at-tick 1 --out w.json --size 4x4",
    ] {
        assert!(parse(&args(bad)).is_err(), "{bad:?} should be refused");
    }
}

#[test]
fn parses_play() {
    let command = parse(&args("play")).unwrap();
    assert_eq!(command.name(), "play");
    let Command::Run(bare) = command else {
        panic!("expected play");
    };
    assert_eq!((bare.ticks, bare.seed, bare.players), (MAX_TICKS, 0, 1));
    assert_eq!(
        bare.play,
        Some(PlayFlags {
            bindings: None,
            open_ended: true,
            first_person: FirstPersonFlags::default(),
        })
    );
    let Command::Run(full) = parse(&args(
        "play --ticks 240 --seed 1 --input i.toml --bindings b.toml --hashes h.txt \
         --record r.rec --report p.json --panic-at-tick 3",
    ))
    .unwrap() else {
        panic!("expected play");
    };
    assert_eq!((full.ticks, full.seed), (240, 1));
    assert_eq!(full.input, Some(PathBuf::from("i.toml")));
    assert_eq!(full.hashes, Some(PathBuf::from("h.txt")));
    assert_eq!(full.record, Some(PathBuf::from("r.rec")));
    assert_eq!(full.report, Some(PathBuf::from("p.json")));
    assert_eq!(full.panic_at_tick, Some(3));
    assert_eq!(full.hash_every, 1);
    assert_eq!(
        full.play,
        Some(PlayFlags {
            bindings: Some(PathBuf::from("b.toml")),
            open_ended: false,
            first_person: FirstPersonFlags::default(),
        })
    );
    assert_eq!((full.dump, full.capture), (None, None));
}

#[test]
fn parses_plays_first_person_flags() {
    let Command::Run(run) = parse(&args(
        "play --map room --windowed --fov 90 --look l.toml --sensitivity 0.022 \
         --pitch-limit 80 --invert-pitch",
    ))
    .unwrap() else {
        panic!("expected play");
    };
    let flags = run.play.unwrap().first_person;
    assert_eq!(
        flags,
        FirstPersonFlags {
            windowed: true,
            fov_y: Some(eng_math::BAngle::QUARTER),
            look: Some(PathBuf::from("l.toml")),
            sensitivity_microdeg: Some(22_000),
            pitch_limit: Some(14_563),
            invert_pitch: true,
        }
    );
    for bad in [
        "play --fov",
        "play --fov 0",
        "play --fov 180",
        "play --fov 1e2",
        "play --look",
        "play --sensitivity 0.0000001",
        "play --sensitivity -1",
        "play --pitch-limit 91",
        "play --pitch-limit 80.5.1",
        "run --ticks 1 --windowed",
        "shot --at-tick 1 --out s.png --fov 90",
        "video --ticks 1 --out v.apng --sensitivity 1",
        "run --ticks 1 --invert-pitch",
    ] {
        assert!(parse(&args(bad)).is_err(), "{bad:?} should be refused");
    }
}

#[test]
fn rejects_bad_play_usage() {
    for bad in [
        "play --headless",
        "play --players 2",
        "play --out x.png",
        "play --at-tick 3",
        "play --assert a.toml",
        "play --budgets b.toml",
        "play --events e.jsonl",
        "play --hash-every 2",
        "play --size 4x4",
        "play --every 2",
        "play --ticks",
        "play --ticks 18446744073709551615",
        "play --bindings",
        "run --ticks 1 --bindings b.toml",
        "shot --at-tick 1 --out s.png --bindings b.toml",
    ] {
        assert!(parse(&args(bad)).is_err(), "{bad:?} should be refused");
    }
}

#[test]
fn rejects_bad_usage() {
    for bad in [
        "",
        "fly",
        "run",
        "run --ticks",
        "run --ticks x",
        "run --ticks 1 --bogus",
        "run --ticks 1 --players 0",
        "run --ticks 18446744073709551615",
        "dump-world --out w.json",
        "dump-world --at-tick 1 --out w.json --record r",
        "replay",
        "replay verify",
        "replay jump r.rec",
        "--json",
        "--version --json --json",
        "--version --verbose",
        "--version run",
        "version",
    ] {
        assert!(parse(&args(bad)).is_err(), "{bad:?} should be refused");
    }
}

#[test]
fn every_command_that_runs_a_sim_takes_a_map() {
    for line in [
        "run --headless --ticks 5",
        "dump-world --at-tick 5 --out w.json",
        "shot --at-tick 5 --out s.png",
        "video --ticks 5 --out v.apng",
        "play --ticks 5",
    ] {
        let Command::Run(without) = parse(&args(line)).unwrap() else {
            panic!("{line}: expected a run");
        };
        assert_eq!(without.map, None, "{line}: the first map by default");
        let with = format!("{line} --map room");
        let Command::Run(with) = parse(&args(&with)).unwrap() else {
            panic!("{with}: expected a run");
        };
        assert_eq!(with.map.as_deref(), Some("room"), "{line}");
        assert_eq!(with.command(), without.command());
        assert!(
            parse(&args(&format!("{line} --map"))).is_err(),
            "{line} --map"
        );
    }
    // `replay verify` picks the map by the recording's hash; the stamp is
    // the first map's.
    for bad in ["replay verify r.rec --map room", "--version --map room"] {
        assert!(parse(&args(bad)).is_err(), "{bad:?} should be refused");
    }
}

#[test]
fn shot_and_video_take_a_view_a_pose_and_a_look() {
    for line in [
        "shot --at-tick 5 --out s.png",
        "video --ticks 5 --out v.apng",
    ] {
        let flags = "--view first-person --pose 9,3.5,1.6,-90,-15 --look off.toml";
        let Command::Run(with) = parse(&args(&format!("{line} {flags}"))).unwrap() else {
            panic!("{line}: expected a run");
        };
        assert_eq!(with.view, Some("first-person"), "{line}");
        assert_eq!(with.pose, Some(Pose::parse("9,3.5,1.6,-90,-15").unwrap()));
        assert_eq!(with.look, Some(PathBuf::from("off.toml")));
        let Command::Run(without) = parse(&args(line)).unwrap() else {
            panic!("{line}: expected a run");
        };
        assert_eq!(
            (without.view, without.pose, without.look),
            (None, None, None)
        );
        let Command::Run(top) = parse(&args(&format!("{line} --view top-down"))).unwrap() else {
            panic!("{line}: expected a run");
        };
        assert_eq!(top.view, Some("top-down"));
        for bad in [
            "--view",
            "--view sideways",
            "--pose",
            "--pose 1,2,3",
            "--pose 1,2,3,0,95",
            "--look",
        ] {
            let bad = format!("{line} {bad}");
            assert!(parse(&args(&bad)).is_err(), "{bad:?} should be refused");
        }
    }
    // Only the commands that render take a view or a pose; `play` takes
    // `--look` as its own first-person flag, the same file format.
    for line in [
        "run --headless --ticks 5",
        "dump-world --at-tick 5 --out w.json",
        "play --ticks 5",
    ] {
        let play = line.starts_with("play");
        for flag in ["--view first-person", "--pose 0,0,1,0,0", "--look l.toml"] {
            let line = format!("{line} {flag}");
            if play && flag.starts_with("--look") {
                let Command::Run(run) = parse(&args(&line)).unwrap() else {
                    panic!("{line}: expected a run");
                };
                let look = run.play.unwrap().first_person.look;
                assert_eq!((run.look, look), (None, Some(PathBuf::from("l.toml"))));
                continue;
            }
            let err = parse(&args(&line)).unwrap_err();
            assert!(err.starts_with("unknown flag"), "{line}: {err}");
        }
    }
}
