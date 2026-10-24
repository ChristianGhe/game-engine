//! `replay verify` and `replay dump`.

use std::fs;
use std::path::Path;

use eng_core::Tick;
use eng_input::{Recording, read_recording};
use eng_schema_json::JsonValue;

use crate::run::{build_sim, hash, write_file};
use crate::{ENGINE_SHA, Exit, GameDef, RUSTC_VERSION, exit, log, report};

fn load(game: &GameDef, path: &Path) -> Result<Recording, Exit> {
    let bytes = fs::read(path)
        .map_err(|e| exit::usage(game.name, &format!("cannot read {}: {e}", path.display())))?;
    let recording = read_recording(&bytes)
        .map_err(|e| exit::usage(game.name, &format!("[E-RECORDING] {}: {e}", path.display())))?;
    recording
        .validate()
        .map_err(|e| exit::usage(game.name, &format!("[E-RECORDING] {}: {e}", path.display())))?;
    Ok(recording)
}

pub(crate) fn verify(game: &GameDef, path: &Path, report_path: Option<&Path>) -> Exit {
    let recording = match load(game, path) {
        Ok(r) => r,
        Err(exit) => return exit,
    };
    let h = &recording.header;
    if h.game != game.name {
        return exit::usage(
            game.name,
            &format!("[E-RECORDING] {} is a {} recording", path.display(), h.game),
        );
    }
    let mut sim = build_sim(game, h.players, h.seed);
    // In Level 1 a recording is valid only for the exact build in its header.
    let checks = [
        (
            "sim_protocol",
            format!("{:#018x}", h.sim_protocol),
            format!("{:#018x}", sim.protocol_version()),
        ),
        ("engine_sha", h.engine_sha.clone(), ENGINE_SHA.to_string()),
        ("game_sha", h.game_sha.clone(), game.build_sha.to_string()),
        ("rustc", h.rustc.clone(), RUSTC_VERSION.to_string()),
        (
            "tick_rate",
            h.tick_rate.to_string(),
            game.tick_rate.to_string(),
        ),
    ];
    let mismatches: Vec<String> = checks
        .iter()
        .filter(|(_, recorded, current)| recorded != current)
        .map(|(name, recorded, current)| {
            format!("{name} recorded {recorded}, this build {current}")
        })
        .collect();
    if !mismatches.is_empty() {
        return exit::usage(
            game.name,
            &format!(
                "[E-RECORDING-BUILD] recordings are valid only for the build that made them: {}",
                mismatches.join("; ")
            ),
        );
    }

    let mut expected = recording.hashes.iter().peekable();
    let mut checked = 0u64;
    let mut divergence = None;
    for t in 0..=recording.ticks() {
        let tick = Tick(t);
        exit::note_tick(tick);
        if t > 0 {
            sim.step(recording.commands_at(tick))
                .unwrap_or_else(|e| panic!("[E-STEP] {e}"));
        }
        if let Some(want) = expected.next_if(|h| h.tick == tick) {
            checked += 1;
            let got = hash(&sim);
            if got != want.hash {
                divergence = Some((tick, want.hash, got));
                break;
            }
        }
    }

    let exit = if divergence.is_some() {
        Exit::Desync
    } else {
        Exit::Ok
    };
    let mut lines = vec![format!(
        "{} replay verify: {} ticks, {} hashes checked, protocol {:#018x}",
        game.name,
        sim.tick().0,
        checked,
        sim.protocol_version()
    )];
    if let Some((tick, recorded, replayed)) = divergence {
        lines.push(format!(
            "[E-DESYNC] first divergence at {tick}: recorded {recorded:#018x}, replayed {replayed:#018x}"
        ));
        lines.push(format!(
            "inspect: `{} replay dump` lists the recorded commands; rerun the original `run` \
             with `dump-world --at-tick {}` on both builds and diff the JSON",
            game.name, tick.0
        ));
    }
    lines.push(format!("exit {} ({})", exit.code(), exit.label()));

    if let Some(report_path) = report_path {
        let doc = JsonValue::object([
            ("report_version", JsonValue::U64(report::REPORT_VERSION)),
            ("command", JsonValue::str("replay verify")),
            ("game", JsonValue::str(game.name)),
            ("build", report::build_info(game)),
            ("recording", JsonValue::str(path.display().to_string())),
            ("ticks", JsonValue::U64(recording.ticks())),
            ("hashes_checked", JsonValue::U64(checked)),
            (
                "first_divergence",
                divergence.map_or(JsonValue::Null, |(tick, recorded, replayed)| {
                    JsonValue::object([
                        ("tick", JsonValue::U64(tick.0)),
                        ("recorded", JsonValue::hex(recorded)),
                        ("replayed", JsonValue::hex(replayed)),
                    ])
                }),
            ),
            ("exit_code", JsonValue::U64(exit.code().into())),
            ("exit", JsonValue::str(exit.label())),
        ]);
        if let Err(message) = write_file(report_path, doc.to_pretty().as_bytes()) {
            return exit::usage(game.name, &message);
        }
    }
    for line in lines {
        println!("{line}");
        log::line(if exit == Exit::Ok { "info" } else { "error" }, &line);
    }
    exit
}

pub(crate) fn dump(game: &GameDef, path: &Path, out: Option<&Path>) -> Exit {
    let recording = match load(game, path) {
        Ok(r) => r,
        Err(exit) => return exit,
    };
    let text = JsonValue::of(&recording).to_pretty();
    match out {
        Some(out) => {
            if let Err(message) = write_file(out, text.as_bytes()) {
                return exit::usage(game.name, &message);
            }
            println!("{} replay dump: wrote {}", game.name, out.display());
        }
        None => println!("{text}"),
    }
    Exit::Ok
}
