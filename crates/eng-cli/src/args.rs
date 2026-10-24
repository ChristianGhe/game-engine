use std::path::PathBuf;

pub(crate) const USAGE: &str = "\
<game> run --headless --ticks N [--seed S] [--players P] [--input script.toml]
           [--assert assertions.toml] [--budgets budgets.toml]
           [--report report.json] [--events events.jsonl]
           [--hashes hashes.txt] [--hash-every N] [--record out.rec]
<game> replay verify <recording> [--report report.json]
<game> replay dump <recording> [--out recording.json]
<game> dump-world --at-tick N --out world.json [--seed S] [--players P] [--input script.toml]
<game> help

exit codes: 0 ok, 10 assertion failed, 11 desync, 12 panic, 13 budget exceeded, 64 usage";

/// Keeps per-tick timings and recordings within memory.
pub(crate) const MAX_TICKS: u64 = 10_000_000;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunArgs {
    pub ticks: u64,
    pub seed: u64,
    pub players: u8,
    pub input: Option<PathBuf>,
    pub assert: Option<PathBuf>,
    pub budgets: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub events: Option<PathBuf>,
    pub hashes: Option<PathBuf>,
    pub hash_every: u64,
    pub record: Option<PathBuf>,
    /// Test-only: panic at this tick, to exercise the panic hook.
    pub panic_at_tick: Option<u64>,
    /// `dump-world`: write the world at this tick.
    pub dump: Option<(u64, PathBuf)>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Command {
    Help,
    Run(RunArgs),
    ReplayVerify {
        recording: PathBuf,
        report: Option<PathBuf>,
    },
    ReplayDump {
        recording: PathBuf,
        out: Option<PathBuf>,
    },
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Command::Help => "help",
            Command::Run(args) if args.dump.is_some() => "dump-world",
            Command::Run(_) => "run",
            Command::ReplayVerify { .. } => "replay verify",
            Command::ReplayDump { .. } => "replay dump",
        }
    }
}

struct Flags<'a> {
    rest: std::slice::Iter<'a, String>,
}

impl<'a> Flags<'a> {
    fn value(&mut self, flag: &str) -> Result<&'a str, String> {
        self.rest
            .next()
            .map(String::as_str)
            .ok_or_else(|| format!("{flag} needs a value"))
    }

    fn number<T: std::str::FromStr>(&mut self, flag: &str) -> Result<T, String> {
        let raw = self.value(flag)?;
        raw.parse()
            .map_err(|_| format!("{flag} needs a whole number, got {raw:?}"))
    }

    fn path(&mut self, flag: &str) -> Result<PathBuf, String> {
        self.value(flag).map(PathBuf::from)
    }
}

pub(crate) fn parse(args: &[String]) -> Result<Command, String> {
    let Some(first) = args.first() else {
        return Err("no command given".into());
    };
    let mut flags = Flags {
        rest: args[1..].iter(),
    };
    match first.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "run" => parse_run(&mut flags, false).map(Command::Run),
        "dump-world" => parse_run(&mut flags, true).map(Command::Run),
        "replay" => parse_replay(&mut flags),
        other => Err(format!("unknown command {other:?}")),
    }
}

fn parse_run(flags: &mut Flags<'_>, dump_world: bool) -> Result<RunArgs, String> {
    let mut a = RunArgs {
        players: 1,
        hash_every: 1,
        ..RunArgs::default()
    };
    let (mut ticks, mut at_tick, mut out) = (None, None, None);
    while let Some(flag) = flags.rest.next() {
        let flag = flag.as_str();
        match flag {
            // The only mode until the window arrives in rung 1b.
            "--headless" if !dump_world => {}
            "--ticks" if !dump_world => ticks = Some(flags.number(flag)?),
            "--at-tick" if dump_world => at_tick = Some(flags.number(flag)?),
            "--out" if dump_world => out = Some(flags.path(flag)?),
            "--seed" => a.seed = flags.number(flag)?,
            "--players" => a.players = flags.number(flag)?,
            "--input" => a.input = Some(flags.path(flag)?),
            "--assert" if !dump_world => a.assert = Some(flags.path(flag)?),
            "--budgets" if !dump_world => a.budgets = Some(flags.path(flag)?),
            "--report" if !dump_world => a.report = Some(flags.path(flag)?),
            "--events" if !dump_world => a.events = Some(flags.path(flag)?),
            "--hashes" if !dump_world => a.hashes = Some(flags.path(flag)?),
            "--hash-every" if !dump_world => a.hash_every = flags.number(flag)?,
            "--record" if !dump_world => a.record = Some(flags.path(flag)?),
            "--panic-at-tick" if !dump_world => a.panic_at_tick = Some(flags.number(flag)?),
            other => return Err(format!("unknown flag {other:?}")),
        }
    }
    if a.players == 0 {
        return Err("--players must be at least 1".into());
    }
    if a.hash_every == 0 {
        return Err("--hash-every must be at least 1".into());
    }
    if ticks.or(at_tick).is_some_and(|t| t > MAX_TICKS) {
        return Err(format!(
            "at most {MAX_TICKS} ticks per run (46 hours at 60 Hz)"
        ));
    }
    if dump_world {
        let at = at_tick.ok_or("dump-world needs --at-tick N")?;
        let out = out.ok_or("dump-world needs --out FILE")?;
        a.ticks = at;
        a.dump = Some((at, out));
    } else {
        a.ticks = ticks.ok_or("run needs --ticks N")?;
    }
    Ok(a)
}

fn parse_replay(flags: &mut Flags<'_>) -> Result<Command, String> {
    let sub = flags.value("replay")?;
    let recording = flags.path(sub)?;
    let mut extra = None;
    while let Some(flag) = flags.rest.next() {
        match (sub, flag.as_str()) {
            ("verify", "--report") | ("dump", "--out") => extra = Some(flags.path(flag)?),
            (_, other) => return Err(format!("unknown flag {other:?}")),
        }
    }
    match sub {
        "verify" => Ok(Command::ReplayVerify {
            recording,
            report: extra,
        }),
        "dump" => Ok(Command::ReplayDump {
            recording,
            out: extra,
        }),
        other => Err(format!(
            "unknown replay command {other:?}; use verify or dump"
        )),
    }
}

#[cfg(test)]
mod tests {
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
        let Command::Run(dump) = parse(&args("dump-world --at-tick 9 --out w.json")).unwrap()
        else {
            panic!("expected dump-world");
        };
        assert_eq!(dump.dump, Some((9, PathBuf::from("w.json"))));
        assert_eq!(dump.ticks, 9);
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
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?} should be refused");
        }
    }
}
