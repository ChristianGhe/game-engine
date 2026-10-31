use std::path::PathBuf;

pub(crate) const USAGE: &str = "\
<game> run --headless --ticks N [--seed S] [--players P] [--input script.toml]
           [--assert assertions.toml] [--budgets budgets.toml]
           [--report report.json] [--events events.jsonl]
           [--hashes hashes.txt] [--hash-every N] [--record out.rec]
<game> replay verify <recording> [--report report.json]
<game> replay dump <recording> [--out recording.json]
<game> dump-world --at-tick N --out world.json [--seed S] [--players P] [--input script.toml]
<game> shot --at-tick N --out shot.png [--size 320x240] [--seed S] [--players P]
            [--input script.toml] [--budgets budgets.toml] [--report report.json]
<game> video --ticks N --out proof.apng [--every K] [--size 320x240] [--seed S] [--players P]
             [--input script.toml] [--budgets budgets.toml] [--report report.json]
<game> play [--ticks N] [--input script.toml] [--bindings bindings.toml] [--seed S]
            [--hashes hashes.txt] [--record out.rec] [--report report.json]
<game> --version [--json]
<game> help

exit codes: 0 ok, 10 assertion failed, 11 desync, 12 panic, 13 budget exceeded, 64 usage";

/// Keeps per-tick timings and recordings within memory.
pub(crate) const MAX_TICKS: u64 = 10_000_000;

/// The largest image side: Tier 0's largest texture (eng-gpu).
pub(crate) const MAX_SIDE: u32 = 8192;

/// The most frames a video holds, about 2 minutes 47 seconds at 60 Hz.
pub(crate) const MAX_VIDEO_FRAMES: u64 = 10_000;

/// Which ticks `shot` and `video` render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Frames {
    /// `shot`: one frame, the run's last tick.
    Last,
    /// `video`: tick 0 and every this many ticks after it, up to the last
    /// multiple not past the run's last tick.
    Every(u64),
}

/// `play`'s own flags: the window, the keyboard, and Esc to quit.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PlayFlags {
    /// Replaces the default bindings (`eng-input`'s `default.bindings.toml`).
    pub bindings: Option<PathBuf>,
    /// No `--ticks`: the run goes on until `quit` (at most [`MAX_TICKS`]).
    pub open_ended: bool,
}

/// `shot` and `video`: what to render and where it goes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CaptureArgs {
    pub frames: Frames,
    pub out: PathBuf,
    pub width: u32,
    pub height: u32,
}

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
    /// `shot` and `video`: render frames offscreen and write them.
    pub capture: Option<CaptureArgs>,
    /// `play`: a window, live input and Esc to quit (eng-app).
    pub play: Option<PlayFlags>,
}

impl RunArgs {
    /// `run`, `dump-world`, `shot`, `video` or `play`.
    pub fn command(&self) -> &'static str {
        if self.play.is_some() {
            return "play";
        }
        match (&self.dump, self.capture.as_ref().map(|c| c.frames)) {
            (Some(_), _) => "dump-world",
            (None, Some(Frames::Last)) => "shot",
            (None, Some(Frames::Every(_))) => "video",
            (None, None) => "run",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Command {
    Help,
    /// `--version`, or `--version --json`.
    Version {
        json: bool,
    },
    Run(Box<RunArgs>),
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
            Command::Version { .. } => "version",
            Command::Run(args) => args.command(),
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
        "--version" => parse_version(&mut flags),
        "run" => parse_run(&mut flags, Mode::Run).map(|a| Command::Run(Box::new(a))),
        "dump-world" => parse_run(&mut flags, Mode::DumpWorld).map(|a| Command::Run(Box::new(a))),
        "shot" => parse_run(&mut flags, Mode::Shot).map(|a| Command::Run(Box::new(a))),
        "video" => parse_run(&mut flags, Mode::Video).map(|a| Command::Run(Box::new(a))),
        "play" => parse_run(&mut flags, Mode::Play).map(|a| Command::Run(Box::new(a))),
        "replay" => parse_replay(&mut flags),
        other => Err(format!("unknown command {other:?}")),
    }
}

/// The commands that run the simulation, which share most of their flags.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Run,
    DumpWorld,
    Shot,
    Video,
    Play,
}

fn parse_run(flags: &mut Flags<'_>, mode: Mode) -> Result<RunArgs, String> {
    let mut a = RunArgs {
        players: 1,
        hash_every: 1,
        ..RunArgs::default()
    };
    let run = mode == Mode::Run;
    let play = mode == Mode::Play;
    // `dump-world` and `shot` stop at a tick; `run` and `video` run N ticks.
    let at_a_tick = matches!(mode, Mode::DumpWorld | Mode::Shot);
    let renders = matches!(mode, Mode::Shot | Mode::Video);
    let (mut ticks, mut at_tick, mut out) = (None, None, None);
    let (mut size, mut every, mut bindings) = ((320, 240), 1, None);
    while let Some(flag) = flags.rest.next() {
        let flag = flag.as_str();
        match flag {
            // The only mode `run` has; the window is `play`'s.
            "--headless" if run => {}
            "--ticks" if !at_a_tick => ticks = Some(flags.number(flag)?),
            "--at-tick" if at_a_tick => at_tick = Some(flags.number(flag)?),
            "--out" if !run && !play => out = Some(flags.path(flag)?),
            "--seed" => a.seed = flags.number(flag)?,
            "--players" if !play => a.players = flags.number(flag)?,
            "--input" => a.input = Some(flags.path(flag)?),
            "--assert" if run => a.assert = Some(flags.path(flag)?),
            "--budgets" if run || renders => a.budgets = Some(flags.path(flag)?),
            "--report" if run || renders || play => a.report = Some(flags.path(flag)?),
            "--events" if run => a.events = Some(flags.path(flag)?),
            "--hashes" if run || play => a.hashes = Some(flags.path(flag)?),
            "--hash-every" if run => a.hash_every = flags.number(flag)?,
            "--record" if run || play => a.record = Some(flags.path(flag)?),
            "--panic-at-tick" if run || play => a.panic_at_tick = Some(flags.number(flag)?),
            "--bindings" if play => bindings = Some(flags.path(flag)?),
            "--size" if renders => size = parse_size(flags.value(flag)?)?,
            "--every" if mode == Mode::Video => every = flags.number(flag)?,
            other => return Err(format!("unknown flag {other:?}")),
        }
    }
    if a.players == 0 {
        return Err("--players must be at least 1".into());
    }
    if a.hash_every == 0 {
        return Err("--hash-every must be at least 1".into());
    }
    if every == 0 {
        return Err("--every must be at least 1".into());
    }
    if ticks.or(at_tick).is_some_and(|t| t > MAX_TICKS) {
        return Err(format!(
            "at most {MAX_TICKS} ticks per run (46 hours at 60 Hz)"
        ));
    }
    let name = match mode {
        Mode::Run => "run",
        Mode::DumpWorld => "dump-world",
        Mode::Shot => "shot",
        Mode::Video => "video",
        Mode::Play => "play",
    };
    a.ticks = if at_a_tick {
        at_tick.ok_or(format!("{name} needs --at-tick N"))?
    } else if play {
        ticks.unwrap_or(MAX_TICKS)
    } else {
        ticks.ok_or(format!("{name} needs --ticks N"))?
    };
    if run {
        return Ok(a);
    }
    if play {
        a.play = Some(PlayFlags {
            bindings,
            open_ended: ticks.is_none(),
        });
        return Ok(a);
    }
    let out = out.ok_or(format!("{name} needs --out FILE"))?;
    if mode == Mode::DumpWorld {
        a.dump = Some((a.ticks, out));
        return Ok(a);
    }
    let frames = if mode == Mode::Shot {
        Frames::Last
    } else {
        let count = a.ticks / every + 1;
        if count > MAX_VIDEO_FRAMES {
            return Err(format!(
                "a video holds at most {MAX_VIDEO_FRAMES} frames, and --ticks {} --every \
                 {every} makes {count}; raise --every",
                a.ticks
            ));
        }
        Frames::Every(every)
    };
    a.capture = Some(CaptureArgs {
        frames,
        out,
        width: size.0,
        height: size.1,
    });
    Ok(a)
}

/// `WIDTHxHEIGHT`, each side 1 to [`MAX_SIDE`].
fn parse_size(raw: &str) -> Result<(u32, u32), String> {
    let side = |s: &str| {
        s.parse::<u32>()
            .ok()
            .filter(|n| (1..=MAX_SIDE).contains(n) && s.bytes().all(|b| b.is_ascii_digit()))
    };
    raw.split_once('x')
        .and_then(|(w, h)| Some((side(w)?, side(h)?)))
        .ok_or_else(|| format!("--size needs WIDTHxHEIGHT, each 1 to {MAX_SIDE}, got {raw:?}"))
}

fn parse_version(flags: &mut Flags<'_>) -> Result<Command, String> {
    let mut json = false;
    for flag in flags.rest.by_ref() {
        match flag.as_str() {
            "--json" if json => return Err("--json given twice".into()),
            "--json" => json = true,
            other => return Err(format!("unknown flag {other:?}; use --version [--json]")),
        }
    }
    Ok(Command::Version { json })
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
mod tests;
