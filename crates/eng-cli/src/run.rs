//! `run`, `dump-world`, `shot` and `video`: drive the simulation headless
//! for N ticks, through the same session as every other mode (spec 6.8): a
//! server and player 0's client in-process over Memory, every message
//! encoded to bytes. The probe reads the client's sim, the state the
//! session delivered, and records the commands the server applied. The
//! ticks run on the headless platform, one per frame of its virtual clock
//! ([`Ticker`]). `shot` and `video` also render frames of that state
//! ([`Capture`]).

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use eng_core::{PlayerCommand, Tick};
use eng_input::{InputScript, Recording, RecordingHeader, TickHash, write_recording};
use eng_schema_json::JsonValue;
use eng_sim::{SIM_DATA_KEY, Sim, SimConfig};

use crate::args::RunArgs;
use crate::assertions::{self, Tracker};
use crate::budgets::BudgetTracker;
use crate::capture::Capture;
use crate::maps::{BuildFn, MapChoice};
use crate::session::{Session, TickStop, command_for};
use crate::ticker::{Flow, Ticker};
use crate::{
    ENGINE_SHA, Exit, GameDef, LookSwitches, RUSTC_VERSION, exit, log, report, shot_view, version,
};

pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

pub(crate) fn create_parent(path: &Path) -> Result<(), String> {
    match path.parent().filter(|d| !d.as_os_str().is_empty()) {
        Some(dir) => {
            fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))
        }
        None => Ok(()),
    }
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    create_parent(path)?;
    fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// The state hash; a non-finite float is a simulation bug, so it panics.
pub(crate) fn hash(sim: &Sim) -> u64 {
    sim.state_hash()
        .unwrap_or_else(|bad| panic!("[E-NAN] {bad} at {}", sim.tick()))
}

/// Builds a map's sim at tick 0 with `build` (`GameDef::build` for the
/// first map) and tells the panic hook its seed and tick.
pub(crate) fn build_sim(game: &GameDef, build: BuildFn, players: u8, seed: u64) -> Sim {
    exit::note_seed(seed);
    let config = SimConfig {
        tick_rate: game.tick_rate,
        players,
        seed,
    };
    let sim = build(config).unwrap_or_else(|e| panic!("[E-SCHEDULE] {e}"));
    exit::note_tick(sim.tick());
    sim
}

pub(crate) fn dump(sim: &Sim) -> JsonValue {
    JsonValue::from_walk(&|v| sim.world().visit_dump(v))
}

pub(crate) struct Inputs {
    pub script: Option<InputScript>,
    pub trackers: Vec<Tracker>,
    pub budgets: BudgetTracker,
    /// `shot`'s and `video`'s `--look` switches, if given (`play` reads its
    /// own `--look` with the same loader, `play/first_person.rs`).
    pub look: Option<LookSwitches>,
}

pub(crate) fn load_inputs(a: &RunArgs) -> Result<Inputs, String> {
    let script = match &a.input {
        Some(path) => Some(
            InputScript::parse(&read_text(path)?, &path.display().to_string())
                .map_err(|e| e.to_string())?,
        ),
        None => None,
    };
    let trackers = match &a.assert {
        Some(path) => assertions::load(&read_text(path)?, &path.display().to_string())
            .map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    let budgets = match &a.budgets {
        Some(path) => BudgetTracker::load(&read_text(path)?, &path.display().to_string())
            .map_err(|e| e.to_string())?,
        None => BudgetTracker::unlimited(),
    };
    let look = match &a.look {
        Some(path) => Some(
            LookSwitches::parse(&read_text(path)?, &path.display().to_string())
                .map_err(|e| e.to_string())?,
        ),
        None => None,
    };
    Ok(Inputs {
        script,
        trackers,
        budgets,
        look,
    })
}

/// Reports `message` and stops with `exit`, writing no other output.
pub(crate) fn fail(exit: Exit, message: &str) -> Exit {
    eprintln!("{message}");
    log::line("error", message);
    exit
}

fn observe(trackers: &mut [Tracker], sim: &Sim) {
    let tick = sim.tick();
    if trackers.iter().any(|t| t.wants(tick)) {
        let world = dump(sim);
        for tracker in trackers.iter_mut().filter(|t| t.wants(tick)) {
            tracker.observe(tick, &world);
        }
    }
}

fn write_events(out: &mut impl Write, sim: &Sim) -> Result<u64, String> {
    let JsonValue::Object(queues) = JsonValue::from_walk(&|v| sim.world().visit_events(v)) else {
        return Ok(0);
    };
    let mut count = 0;
    for (name, events) in queues {
        let JsonValue::Array(events) = events else {
            continue;
        };
        for event in events {
            let line = JsonValue::object([
                ("tick", JsonValue::U64(sim.tick().0)),
                ("event", JsonValue::str(name.clone())),
                ("data", event),
            ]);
            writeln!(out, "{}", line.to_compact()).map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(count)
}

pub(crate) fn run(game: &GameDef, map: MapChoice, a: &RunArgs) -> Exit {
    // Before anything runs or opens: a map without a view draws nothing,
    // and `--view`, `--pose` and `--look` must fit the view it has.
    let refused = a.capture.as_ref().and_then(|_| {
        map.no_view(game)
            .or_else(|| shot_view::refusal(game, map, a))
    });
    if let Some(message) = refused {
        return fail(Exit::Usage, &message);
    }
    let mut inputs = match load_inputs(a) {
        Ok(inputs) => inputs,
        Err(message) => return exit::usage(game.name, &message),
    };
    let mut events_out = match &a.events {
        Some(path) => match create_parent(path)
            .and_then(|()| File::create(path).map_err(|e| e.to_string()))
        {
            Ok(file) => Some(BufWriter::new(file)),
            Err(e) => {
                return exit::usage(game.name, &format!("cannot write {}: {e}", path.display()));
            }
        },
        None => None,
    };
    let (mut session, too_large) = Session::start(game, map, a.players, a.seed);
    // After the session, so a panic while the GPU opens has the seed, and
    // before the first tick, so a missing adapter costs no run. A
    // first-person world is built from the sim at tick 0.
    let mut capture = match &a.capture {
        Some(args) => match Capture::open(game, map.view(), session.sim(), args, a.ticks)
            .map(|capture| capture.framed(a.pose, inputs.look))
        {
            Ok(capture) => Some(capture),
            Err(message) => return fail(Exit::Usage, &message),
        },
        None => None,
    };
    if let Some(message) = too_large {
        inputs.budgets.exceeded.get_or_insert(message);
    }
    let mut hashes = vec![TickHash {
        tick: Tick::ZERO,
        hash: hash(session.sim()),
    }];
    let mut recorded = Vec::new();
    let mut tick_ns = Vec::with_capacity(a.ticks.min(1 << 20) as usize);
    let mut scripted = inputs.script.as_ref().map(InputScript::iter_commands);
    let mut event_count = 0;
    observe(&mut inputs.trackers, session.sim());
    let mut within_budget = inputs.budgets.observe(
        0,
        session.sim().state_len() as u64,
        session.sim().world().entity_count() as u64,
    );
    if let Some(capture) = capture.as_mut().filter(|c| c.wants(0)) {
        let counters = capture.frame(session.sim());
        within_budget = inputs
            .budgets
            .observe_view_frame(0, &counters, capture.first_person())
            && within_budget;
    }

    // One tick per frame of the headless platform.
    let ticker = Ticker::new(game.tick_rate, a.ticks, || {
        if !within_budget {
            return Flow::Stop;
        }
        let tick = session.server.sim().tick().next();
        exit::note_tick(tick);
        if a.panic_at_tick == Some(tick.0) {
            panic!("deliberate panic at {tick} (--panic-at-tick)");
        }
        // Player 0 follows the script, or idles without one.
        let command = scripted.as_mut().and_then(Iterator::next);
        let command = match command_for(tick, command.unwrap_or(PlayerCommand::idle(tick))) {
            Ok(command) => command,
            Err(message) => return Flow::Exit(exit::usage(game.name, &message)),
        };
        match session.tick(command, a.record.is_some().then_some(&mut recorded)) {
            Ok(step_ns) => tick_ns.push(step_ns),
            Err(TickStop::Limit(message)) => {
                inputs.budgets.exceeded.get_or_insert(message);
                return Flow::Stop;
            }
            Err(TickStop::Desync(message)) => return Flow::Exit(fail(Exit::Desync, &message)),
        }
        let sim = session.sim();
        if tick.0.is_multiple_of(a.hash_every) || tick.0 == a.ticks {
            hashes.push(TickHash {
                tick,
                hash: hash(sim),
            });
        }
        if let Some(out) = events_out.as_mut() {
            match write_events(out, sim) {
                Ok(n) => event_count += n,
                Err(e) => {
                    return Flow::Exit(exit::usage(game.name, &format!("writing events: {e}")));
                }
            }
        }
        observe(&mut inputs.trackers, sim);
        // A frame over a `[gpu]` budget stops the run here, as `[sim]` does.
        let frame_ok = match capture.as_mut().filter(|c| c.wants(tick.0)) {
            Some(capture) => {
                let counters = capture.frame(sim);
                let first_person = capture.first_person();
                inputs
                    .budgets
                    .observe_view_frame(tick.0, &counters, first_person)
            }
            None => true,
        };
        within_budget = inputs.budgets.observe(
            tick.0,
            sim.state_len() as u64,
            sim.world().entity_count() as u64,
        ) && frame_ok;
        Flow::Next
    });
    if let Some(exit) = ticker.run() {
        return exit;
    }
    let sim = session.sim();
    for tracker in &mut inputs.trackers {
        tracker.finish(sim.tick());
    }

    let exit = if inputs.budgets.exceeded.is_some() {
        Exit::BudgetExceeded
    } else if inputs.trackers.iter().any(|t| !t.ok()) {
        Exit::AssertionFailed
    } else {
        Exit::Ok
    };

    let stopped = inputs.budgets.exceeded.is_some();
    let frames = capture.as_ref().map(Capture::summary);
    let view = capture.as_ref().map(Capture::view_json);
    let outputs = flush_events(events_out)
        .and_then(|()| write_outputs(game, map, a, sim, &hashes, recorded, stopped))
        .and_then(|()| write_frames(a, sim, capture));
    if let Err(message) = outputs {
        return exit::usage(game.name, &message);
    }
    let summary = report::RunSummary {
        game,
        map: map.name(),
        args: a,
        sim,
        hashes: &hashes,
        tick_ns,
        events: event_count,
        trackers: &inputs.trackers,
        budgets: &inputs.budgets,
        frames: frames.as_ref(),
        platform: "headless",
        dropped_ticks: 0,
        window: None,
        exit,
    };
    let mut json = summary.to_json();
    if let Some(view) = view {
        json.insert("view", view);
    }
    if let Some(path) = &a.report
        && let Err(message) = write_file(path, json.to_pretty().as_bytes())
    {
        return exit::usage(game.name, &message);
    }
    for line in summary.console_lines() {
        println!("{line}");
        log::line("info", &line);
    }
    exit
}

/// Finishes `--events`' file.
fn flush_events(events_out: Option<BufWriter<File>>) -> Result<(), String> {
    match events_out {
        Some(mut out) => out.flush().map_err(|e| format!("writing events: {e}")),
        None => Ok(()),
    }
}

/// Writes the requested files. `stopped`: a budget stopped the run early,
/// so the summary explains a missing dump (exit 13).
pub(crate) fn write_outputs(
    game: &GameDef,
    map: MapChoice,
    a: &RunArgs,
    sim: &Sim,
    hashes: &[TickHash],
    recorded: Vec<PlayerCommand>,
    stopped: bool,
) -> Result<(), String> {
    if let Some(path) = &a.hashes {
        write_file(path, hash_stream(game, map, a, sim, hashes).as_bytes())?;
    }
    if let Some(path) = &a.record {
        let recording = Recording {
            header: RecordingHeader {
                game: game.name.to_string(),
                engine_sha: ENGINE_SHA.to_string(),
                game_sha: version::game_sha(game).to_string(),
                rustc: RUSTC_VERSION.to_string(),
                sim_protocol: sim.protocol_version(),
                sim_data_hash: sim.sim_data_hash(),
                seed: a.seed,
                players: a.players,
                tick_rate: game.tick_rate,
            },
            commands: recorded,
            hashes: hashes.to_vec(),
        };
        write_file(path, &write_recording(&recording))?;
    }
    if let Some((at, path)) = &a.dump {
        if sim.tick().0 != *at {
            // For dump-world only the snapshot limit stops a run early.
            if stopped {
                return Ok(());
            }
            return Err(format!(
                "the run stopped at {} before tick {at}",
                sim.tick()
            ));
        }
        let mut doc = vec![
            ("game", JsonValue::str(game.name)),
            ("tick", JsonValue::U64(sim.tick().0)),
            ("sim_protocol", JsonValue::hex(sim.protocol_version())),
            ("world", dump(sim)),
        ];
        // Only a map with sim data adds these, so the dot's dumps keep their
        // bytes and pointers (ADR-0023).
        if sim.has_sim_data() {
            doc.extend([
                ("map", JsonValue::str(map.name())),
                ("sim_data_hash", JsonValue::hex(sim.sim_data_hash())),
                (
                    SIM_DATA_KEY,
                    JsonValue::from_walk(&|v| sim.world().visit_constants(v)),
                ),
            ]);
        }
        write_file(path, JsonValue::object(doc).to_pretty().as_bytes())?;
    }
    Ok(())
}

/// Writes `shot`'s or `video`'s file if the run reached its last tick, so
/// every frame is in; a run stopped early by a budget writes none.
fn write_frames(a: &RunArgs, sim: &Sim, capture: Option<Capture>) -> Result<(), String> {
    match capture {
        Some(capture) if sim.tick().0 == a.ticks => {
            let (path, bytes) = capture.finish();
            write_file(&path, &bytes)
        }
        _ => Ok(()),
    }
}

/// The hash stream's text format, in its first line.
pub(crate) const HASH_STREAM_VERSION: u64 = 1;

/// The committed text form of a hash stream: a header, then `tick hash`.
/// A map with sim data ends the header's second line with
/// ` map=<name> sim_data=0x<16 hex>`, so `compare-hashes` holds the sim
/// data equal across targets too (ADR-0023). A map without keeps 1b's
/// header byte for byte.
pub(crate) fn hash_stream(
    game: &GameDef,
    map: MapChoice,
    a: &RunArgs,
    sim: &Sim,
    hashes: &[TickHash],
) -> String {
    let mut out = format!(
        "# eng hash stream v{HASH_STREAM_VERSION}\n# game={} seed={} players={} tick_rate={} hash_every={} sim_protocol={:#018x}",
        game.name,
        a.seed,
        a.players,
        game.tick_rate,
        a.hash_every,
        sim.protocol_version()
    );
    if sim.has_sim_data() {
        out.push_str(&format!(
            " map={} sim_data={:#018x}",
            map.name(),
            sim.sim_data_hash()
        ));
    }
    out.push('\n');
    for h in hashes {
        out.push_str(&format!("{} {:#018x}\n", h.tick.0, h.hash));
    }
    out
}
