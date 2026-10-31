//! `run` and `dump-world`: drive the simulation headless for N ticks,
//! through the same session as every other mode (spec 6.8): a server and
//! player 0's client in-process over Memory, every message encoded to
//! bytes. The probe reads the client's sim, the state the session
//! delivered, and records the commands the server applied.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

use eng_core::{PlayerCommand, Tick};
use eng_input::{InputScript, Recording, RecordingHeader, TickHash, write_recording};
use eng_net_transport::memory::{MemoryEndpoint, MemoryNetwork};
use eng_platform_headless::HeadlessPlatform;
use eng_schema_json::JsonValue;
use eng_session::{Client, Server, SessionError};
use eng_sim::{RestoreError, Sim, SimConfig};

use crate::args::RunArgs;
use crate::assertions::{self, Tracker};
use crate::budgets::BudgetTracker;
use crate::{ENGINE_SHA, Exit, GameDef, RUSTC_VERSION, exit, log, report, version};

pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn create_parent(path: &Path) -> Result<(), String> {
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

/// Builds the sim at tick 0 and tells the panic hook its seed and tick.
pub(crate) fn build_sim(game: &GameDef, players: u8, seed: u64) -> Sim {
    exit::note_seed(seed);
    let config = SimConfig {
        tick_rate: game.tick_rate,
        players,
        seed,
    };
    let sim = (game.build)(config).unwrap_or_else(|e| panic!("[E-SCHEDULE] {e}"));
    exit::note_tick(sim.tick());
    sim
}

pub(crate) fn dump(sim: &Sim) -> JsonValue {
    JsonValue::from_walk(&|v| sim.world().visit_dump(v))
}

struct Inputs {
    script: Option<InputScript>,
    trackers: Vec<Tracker>,
    budgets: BudgetTracker,
}

fn load_inputs(a: &RunArgs) -> Result<Inputs, String> {
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
    Ok(Inputs {
        script,
        trackers,
        budgets,
    })
}

/// The in-process session: the server owns the authoritative sim, and the
/// client, player 0's, restores every snapshot into a sim of its own.
struct Session {
    server: Server<MemoryEndpoint>,
    client: Client<MemoryEndpoint>,
}

impl Session {
    /// A server and player 0's client, past the handshake. The other slots
    /// have no client, so the server idles them. Also returns
    /// `[E-SNAPSHOT-SIZE]` if the first snapshot does not fit a datagram.
    fn start(game: &GameDef, players: u8, seed: u64) -> (Self, Option<String>) {
        let net = MemoryNetwork::new();
        let mut server = Server::new(build_sim(game, players, seed), net.endpoint());
        let client = Client::connect(
            build_sim(game, players, seed),
            net.endpoint(),
            server.local_id(),
        );
        let mut client = client.unwrap_or_else(|e| session_bug(&e, Tick::ZERO));
        let first = server.poll().err().map(|e| snapshot_limit(e, Tick::ZERO));
        client
            .poll()
            .unwrap_or_else(|e| session_bug(&e, Tick::ZERO));
        (Self { server, client }, first)
    }

    /// The state the session delivered.
    fn sim(&self) -> &Sim {
        self.client.sim()
    }
}

/// A snapshot that outgrows a datagram stops the run as a budget does (exit
/// 13, `[E-SNAPSHOT-SIZE]`); fragmentation arrives in rung 5. Any other
/// session error is a bug in this process.
fn snapshot_limit(err: SessionError, tick: Tick) -> String {
    match err {
        SessionError::SnapshotTooLarge { .. } => err.to_string(),
        other => session_bug(&other, tick),
    }
}

fn session_bug(err: &SessionError, tick: Tick) -> ! {
    match err {
        // A release build checks floats only here, as the client restores
        // the state: still a simulation bug.
        SessionError::Restore(RestoreError::NonFinite(bad)) => panic!("[E-NAN] {bad} at {tick}"),
        _ => panic!("[E-SESSION] {err} at {tick}"),
    }
}

/// Player 0's command must be for the tick being stepped. The server would
/// otherwise keep it for its own tick, or drop it as late, and repeat the
/// last command, where `Sim::step` used to refuse it (`[E-STEP]`).
fn command_for(tick: Tick, command: PlayerCommand) -> Result<PlayerCommand, String> {
    if command.tick == tick {
        Ok(command)
    } else {
        Err(format!(
            "[E-STEP] player 0's command is for {}, but the sim is stepping to {tick}; \
             input scripts start at tick 1",
            command.tick
        ))
    }
}

/// The client must hold the state of the tick just stepped before the
/// probe reports it as that tick's.
fn restored(latest: Option<Tick>, tick: Tick) -> Result<(), String> {
    match latest {
        Some(latest) if latest == tick => Ok(()),
        _ => Err(format!(
            "[E-DESYNC] after the round trip for {tick} the client's newest snapshot is \
             {latest:?}: the session lost a snapshot"
        )),
    }
}

/// Reports `message` and stops with `exit`, writing no other output.
fn fail(exit: Exit, message: &str) -> Exit {
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

pub(crate) fn run(game: &GameDef, a: &RunArgs) -> Exit {
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
    let (mut session, too_large) = Session::start(game, a.players, a.seed);
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

    let mut platform = HeadlessPlatform::new(game.tick_rate, a.ticks);
    while within_budget && platform.next_frame().is_some() {
        let tick = session.server.sim().tick().next();
        exit::note_tick(tick);
        if a.panic_at_tick == Some(tick.0) {
            panic!("deliberate panic at {tick} (--panic-at-tick)");
        }
        // Player 0 follows the script, or idles without one.
        let command = scripted.as_mut().and_then(Iterator::next);
        let command = match command_for(tick, command.unwrap_or(PlayerCommand::idle(tick))) {
            Ok(command) => command,
            Err(message) => return exit::usage(game.name, &message),
        };
        let client = &mut session.client;
        client
            .send(command)
            .unwrap_or_else(|e| session_bug(&e, tick));
        session
            .server
            .poll()
            .unwrap_or_else(|e| session_bug(&e, tick));
        let started = Instant::now();
        let applied = match session.server.step() {
            Ok(applied) => applied,
            Err(e) => {
                inputs
                    .budgets
                    .exceeded
                    .get_or_insert(snapshot_limit(e, tick));
                break;
            }
        };
        tick_ns.push(started.elapsed().as_nanos() as u64);
        if a.record.is_some() {
            recorded.extend_from_slice(applied);
        }
        client.poll().unwrap_or_else(|e| session_bug(&e, tick));
        if let Err(message) = restored(client.latest(), tick) {
            return fail(Exit::Desync, &message);
        }
        let sim = client.sim();
        if tick.0.is_multiple_of(a.hash_every) || tick.0 == a.ticks {
            hashes.push(TickHash {
                tick,
                hash: hash(sim),
            });
        }
        if let Some(out) = events_out.as_mut() {
            match write_events(out, sim) {
                Ok(n) => event_count += n,
                Err(e) => return exit::usage(game.name, &format!("writing events: {e}")),
            }
        }
        observe(&mut inputs.trackers, sim);
        within_budget = inputs.budgets.observe(
            tick.0,
            sim.state_len() as u64,
            sim.world().entity_count() as u64,
        );
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
    let outputs = write_outputs(game, a, sim, &hashes, recorded, events_out, stopped);
    if let Err(message) = outputs {
        return exit::usage(game.name, &message);
    }
    let summary = report::RunSummary {
        game,
        args: a,
        sim,
        hashes: &hashes,
        tick_ns,
        events: event_count,
        trackers: &inputs.trackers,
        budgets: &inputs.budgets,
        exit,
    };
    if let Some(path) = &a.report
        && let Err(message) = write_file(path, summary.to_json().to_pretty().as_bytes())
    {
        return exit::usage(game.name, &message);
    }
    for line in summary.console_lines() {
        println!("{line}");
        log::line("info", &line);
    }
    exit
}

/// Writes the requested files. `stopped`: a budget stopped the run early,
/// so the summary explains a missing dump (exit 13).
fn write_outputs(
    game: &GameDef,
    a: &RunArgs,
    sim: &Sim,
    hashes: &[TickHash],
    recorded: Vec<PlayerCommand>,
    events_out: Option<BufWriter<File>>,
    stopped: bool,
) -> Result<(), String> {
    if let Some(mut out) = events_out {
        out.flush().map_err(|e| format!("writing events: {e}"))?;
    }
    if let Some(path) = &a.hashes {
        write_file(path, hash_stream(game, a, sim, hashes).as_bytes())?;
    }
    if let Some(path) = &a.record {
        let recording = Recording {
            header: RecordingHeader {
                game: game.name.to_string(),
                engine_sha: ENGINE_SHA.to_string(),
                game_sha: version::game_sha(game).to_string(),
                rustc: RUSTC_VERSION.to_string(),
                sim_protocol: sim.protocol_version(),
                sim_data_hash: 0,
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
        let doc = JsonValue::object([
            ("game", JsonValue::str(game.name)),
            ("tick", JsonValue::U64(sim.tick().0)),
            ("sim_protocol", JsonValue::hex(sim.protocol_version())),
            ("world", dump(sim)),
        ]);
        write_file(path, doc.to_pretty().as_bytes())?;
    }
    Ok(())
}

/// The hash stream's text format, in its first line.
pub(crate) const HASH_STREAM_VERSION: u64 = 1;

/// The committed text form of a hash stream: a header, then `tick hash`.
fn hash_stream(game: &GameDef, a: &RunArgs, sim: &Sim, hashes: &[TickHash]) -> String {
    let mut out = format!(
        "# eng hash stream v{HASH_STREAM_VERSION}\n# game={} seed={} players={} tick_rate={} hash_every={} sim_protocol={:#018x}\n",
        game.name,
        a.seed,
        a.players,
        game.tick_rate,
        a.hash_every,
        sim.protocol_version()
    );
    for h in hashes {
        out.push_str(&format!("{} {:#018x}\n", h.tick.0, h.hash));
    }
    out
}

#[cfg(test)]
mod tests {
    use eng_core::{PlayerCommand, Tick};

    use super::{command_for, restored};

    #[test]
    fn a_command_for_another_tick_is_refused() {
        let at = |t| PlayerCommand::idle(Tick(t));
        assert_eq!(command_for(Tick(3), at(3)), Ok(at(3)));
        let err = command_for(Tick(6), at(1)).unwrap_err();
        assert!(
            err.starts_with("[E-STEP] player 0's command is for tick 1"),
            "{err}"
        );
    }

    #[test]
    fn the_client_must_hold_the_tick_just_stepped() {
        assert_eq!(restored(Some(Tick(4)), Tick(4)), Ok(()));
        for latest in [Some(Tick(3)), None] {
            let err = restored(latest, Tick(4)).unwrap_err();
            assert!(err.starts_with("[E-DESYNC] "), "{err}");
        }
    }
}
