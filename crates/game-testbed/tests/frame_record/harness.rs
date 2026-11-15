//! The frame-record tests' parts: the shots from `shots.toml`, rendering
//! one with the real binary, where it was rendered, the reference
//! directory and the per-shot tolerances.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use eng_cli::frame_record::{FrameRecord, RecordView, Tolerances, Worst};
use serde::Deserialize;
use serde_json::{Value, json};

/// How long one run may take.
const LIMIT: Duration = Duration::from_secs(180);

/// The pinned configuration (`tests/shots.rs`): Mesa 25.2.7's lavapipe
/// from gfx-rs/ci-build build26 with LLVM 18.1.3 and 256-bit vectors.
const PINNED_ADAPTER: &str = "llvmpipe (LLVM 18.1.3, 256 bits)";
const PINNED_DRIVER_INFO: &str = "Mesa 25.2.7 (LLVM 18.1.3)";

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// One shot of `shots.toml`, as far as these tests need it. The shots
/// harness reads the manifest strictly; this reads what it renders.
#[derive(Clone, Debug, Deserialize)]
pub struct Shot {
    pub name: String,
    pub map: Option<String>,
    pub tick: u32,
    pub players: Option<u32>,
    pub input: Option<String>,
    pub pose: Option<String>,
    pub look: Option<String>,
    /// The owner's marker, `"pending"`: no golden, and so no reference
    /// record, yet by choice.
    pub golden: Option<String>,
    /// The shot's `--pose` value, from `[pose]`.
    #[serde(skip)]
    pub pose_value: Option<String>,
}

impl Shot {
    /// Whether `shots.toml` marks it `golden = "pending"`.
    pub fn pending(&self) -> bool {
        self.golden.as_deref() == Some("pending")
    }
}

#[derive(Deserialize)]
struct Manifest {
    shot: Vec<Shot>,
    #[serde(default)]
    pose: BTreeMap<String, String>,
}

/// The shots `shots.toml` lists, in its order.
pub fn shots() -> Vec<Shot> {
    let text = std::fs::read_to_string(manifest_dir().join("shots.toml")).unwrap();
    let manifest: Manifest = toml::from_str(&text).unwrap();
    let mut shots = manifest.shot;
    for shot in &mut shots {
        shot.pose_value = shot.pose.as_ref().map(|name| manifest.pose[name].clone());
        let marker = shot.golden.as_deref();
        assert!(
            marker.is_none() || shot.pending(),
            "{}: golden is \"pending\" or absent",
            shot.name
        );
    }
    assert!(!shots.is_empty());
    shots
}

/// One table of `frame-tolerances.toml`; `[default]` gives every field.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    pixels: Option<u64>,
    edge_pixels: Option<f64>,
    class_fraction: Option<f64>,
    screen_box: Option<u32>,
    min_pixels: Option<u64>,
    luminance: Option<f64>,
    contrast: Option<f64>,
    histogram: Option<f64>,
}

impl Table {
    fn over(self, base: Tolerances) -> Tolerances {
        Tolerances {
            pixels: self.pixels.unwrap_or(base.pixels),
            edge_pixels: self.edge_pixels.unwrap_or(base.edge_pixels),
            class_fraction: self.class_fraction.unwrap_or(base.class_fraction),
            screen_box: self.screen_box.unwrap_or(base.screen_box),
            min_pixels: self.min_pixels.unwrap_or(base.min_pixels),
            luminance: self.luminance.unwrap_or(base.luminance),
            contrast: self.contrast.unwrap_or(base.contrast),
            histogram: self.histogram.unwrap_or(base.histogram),
        }
    }

    fn complete(&self) -> bool {
        self.pixels.is_some()
            && self.edge_pixels.is_some()
            && self.class_fraction.is_some()
            && self.screen_box.is_some()
            && self.min_pixels.is_some()
            && self.luminance.is_some()
            && self.contrast.is_some()
            && self.histogram.is_some()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TolerancesFile {
    version: u32,
    default: Table,
    #[serde(default)]
    shot: BTreeMap<String, Table>,
}

/// The per-shot tolerances: `[default]`, with a shot's own table over it.
pub struct ShotTolerances {
    default: Tolerances,
    shot: BTreeMap<String, Table>,
}

impl ShotTolerances {
    /// `[default]`.
    pub fn default(&self) -> Tolerances {
        self.default
    }

    pub fn for_shot(&self, name: &str) -> Tolerances {
        self.shot
            .get(name)
            .map_or(self.default, |t| t.over(self.default))
    }
}

/// `frame-tolerances.toml`, strictly: version 1, a complete `[default]`,
/// and a table only for a shot `shots` has.
pub fn tolerances(shots: &[Shot]) -> ShotTolerances {
    let path = manifest_dir().join("frame-tolerances.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let file: TolerancesFile =
        toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(file.version, 1, "{}", path.display());
    assert!(file.default.complete(), "[default] names every tolerance");
    for name in file.shot.keys() {
        assert!(
            shots.iter().any(|s| s.name == *name),
            "[shot.{name}] is no shot's"
        );
    }
    ShotTolerances {
        default: file.default.over(Tolerances::DEFAULT),
        shot: file.shot,
    }
}

/// `target/frame-candidates/`, emptied.
pub fn candidates() -> PathBuf {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let dir = tmp.parent().unwrap().join("frame-candidates");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A scratch directory of its own under the tests' temporary one, emptied.
pub fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("frame-record")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn gpu_required() -> bool {
    std::env::var("ENG_REQUIRE_GPU").as_deref() == Ok("1")
}

/// Runs the probe in `dir` with the llvmpipe environment fixed as the
/// shots fix it; `None` without an adapter, unless CI requires one.
pub fn probe(dir: &Path, args: &[&str]) -> Option<Output> {
    run(dir, args, true)
}

/// [`probe`] in this test's own environment, as a GPU opened in-process
/// sees it.
pub fn probe_as_is(dir: &Path, args: &[&str]) -> Option<Output> {
    run(dir, args, false)
}

fn run(dir: &Path, args: &[&str], pin_llvmpipe: bool) -> Option<Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_game-testbed"));
    // Mesa's on-disk shader cache off: on Debian's lavapipe (Mesa 25.0.7,
    // arm64) shot processes run side by side corrupted it, and every one
    // after then died with SIGSEGV or SIGABRT (16 of 16 ran with it off).
    // It changes what is compiled when, never what is drawn.
    command
        .args(args)
        .current_dir(dir)
        .env("ENG_AGENT_LOG_DIR", dir.join("logs"))
        .env("MESA_SHADER_CACHE_DISABLE", "true");
    if pin_llvmpipe {
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("LP_") || name.starts_with("GALLIVM_") {
                command.env_remove(&key);
            }
        }
        command.env("LP_NATIVE_VECTOR_WIDTH", "256");
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run game-testbed");
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > LIMIT {
            let _ = child.kill();
            panic!("{args:?} killed after {LIMIT:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output().unwrap();
    // Killed by a signal, say which (T12 and T16 saw Debian's lavapipe
    // lose shot processes so; the plan's T16 row has what to try).
    assert!(
        out.status.code().is_some(),
        "{args:?} ended without an exit code ({}): {}",
        out.status,
        text(&out)
    );
    let no_adapter = out.status.code() == Some(64) && text(&out).contains("[E-NO-ADAPTER]");
    if no_adapter {
        assert!(!gpu_required(), "ENG_REQUIRE_GPU=1: {}", text(&out));
        eprintln!("skipped: {}", text(&out));
        return None;
    }
    Some(out)
}

pub fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// A record file, read strictly.
pub fn read_record(path: &Path) -> FrameRecord {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    FrameRecord::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Where shots were rendered, from a report's `gpu` object.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    os: String,
    arch: String,
    adapter: String,
    backend: String,
    device_type: String,
    driver: String,
    driver_info: String,
}

impl Config {
    fn from_report(gpu: &Value) -> Self {
        let field = |key: &str| gpu[key].as_str().unwrap_or_default().to_owned();
        Self {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            adapter: field("adapter"),
            backend: field("backend"),
            device_type: field("device_type"),
            driver: field("driver"),
            driver_info: field("driver_info"),
        }
    }

    pub fn pinned(&self) -> bool {
        self.os == "linux"
            && self.arch == "x86_64"
            && self.backend == "vulkan"
            && self.device_type == "cpu"
            && self.driver == "llvmpipe"
            && self.adapter == PINNED_ADAPTER
            && self.driver_info == PINNED_DRIVER_INFO
    }

    pub fn json(&self) -> Value {
        json!({
            "os": self.os, "arch": self.arch, "adapter": self.adapter,
            "backend": self.backend, "device_type": self.device_type,
            "driver": self.driver, "driver_info": self.driver_info,
            "pinned": self.pinned(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Byte for byte, on the pinned configuration.
    Exact,
    /// Within the shot's tolerances, anywhere else.
    Tolerances,
}

/// How records rendered on `config` are compared, as `ENG_SHOT_MODE` asks.
pub fn mode(config: &Config) -> Result<Mode, String> {
    match std::env::var("ENG_SHOT_MODE").as_deref() {
        Err(_) | Ok("") if config.pinned() => Ok(Mode::Exact),
        Err(_) | Ok("") => Ok(Mode::Tolerances),
        Ok("exact") if config.pinned() => Ok(Mode::Exact),
        Ok("exact") => Err(format!(
            "[E-GOLDEN] ENG_SHOT_MODE=exact needs the pinned configuration, not {config:?}"
        )),
        Ok(other) => Err(format!(
            "[E-GOLDEN] ENG_SHOT_MODE is `exact` or unset, not `{other}`"
        )),
    }
}

/// `ENG_FRAME_DIR` if set, else `golden/frames/`, and whether it was asked
/// for; one asked for must exist outside `candidates`.
pub fn reference_dir(candidates: &Path) -> Result<(PathBuf, bool), String> {
    let requested: Option<OsString> = std::env::var_os("ENG_FRAME_DIR");
    let Some(dir) = requested.filter(|d| !d.is_empty()).map(PathBuf::from) else {
        return Ok((manifest_dir().join("golden").join("frames"), false));
    };
    let Ok(canonical) = dir.canonicalize() else {
        return Err(format!(
            "[E-GOLDEN] ENG_FRAME_DIR {} does not exist",
            dir.display()
        ));
    };
    if candidates
        .canonicalize()
        .is_ok_and(|c| canonical.starts_with(c))
    {
        return Err(format!(
            "[E-GOLDEN] ENG_FRAME_DIR {} lies in the candidates directory, which each run \
             empties",
            dir.display()
        ));
    }
    Ok((dir, true))
}

/// One shot's record as the binary wrote it, and where.
pub struct Rendered {
    pub text: String,
    pub record: FrameRecord,
    pub report: Value,
    pub config: Config,
}

/// Renders `shot` as the shots harness does (320x240, `--seed 1`) into
/// `dir`; `None` without an adapter.
pub fn render(shot: &Shot, dir: &Path) -> Option<Rendered> {
    std::fs::create_dir_all(dir).unwrap();
    let (tick, players) = (shot.tick.to_string(), shot.players.unwrap_or(1).to_string());
    let (png, report) = (format!("{}.png", shot.name), format!("{}.json", shot.name));
    let mut args: Vec<String> = ["shot", "--at-tick", &tick, "--seed", "1", "--players"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    args.extend([players, "--size".into(), "320x240".into(), "--out".into()]);
    args.extend([png.clone(), "--report".into(), report.clone()]);
    let from_manifest = |file: &str| manifest_dir().join(file).to_string_lossy().into_owned();
    let flags = [
        ("--map", shot.map.clone()),
        ("--input", shot.input.as_deref().map(from_manifest)),
        ("--pose", shot.pose_value.clone()),
        ("--look", shot.look.as_deref().map(from_manifest)),
    ];
    for (flag, value) in flags {
        if let Some(value) = value {
            args.extend([flag.to_owned(), value]);
        }
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = probe(dir, &args)?;
    assert_eq!(out.status.code(), Some(0), "{}: {}", shot.name, text(&out));
    let path = dir.join(format!("{png}.frame.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: no frame record: {e}", shot.name));
    let record = FrameRecord::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", shot.name));
    let report = read_json(&dir.join(&report));
    let config = Config::from_report(&report["gpu"]);
    Some(Rendered {
        text,
        record,
        report,
        config,
    })
}

/// What every shot's record must say, from its report and the room.
pub fn check_shot(shot: &Shot, rendered: &Rendered) {
    let name = &shot.name;
    let record = &rendered.record;
    let view = &rendered.report["view"];
    assert_eq!(record.view.name(), view["kind"], "{name}");
    assert_eq!((record.width, record.height), (320, 240), "{name}");
    let [frame] = &record.frames[..] else {
        panic!("{name}: a shot has one frame");
    };
    assert_eq!(frame.tick, u64::from(shot.tick), "{name}");
    assert_eq!(frame.invalid_info, 0, "{name}");
    assert!(frame.unlisted.is_empty(), "{name}: {:?}", frame.unlisted);
    if record.view == RecordView::TopDown {
        return;
    }
    // The camera drawn is the report's (serde_json reads floats to within
    // an ulp or so, so not to the bit).
    let camera = frame.camera.expect("first person");
    let drawn = &view["camera"];
    let near = |ours: f64, theirs: &Value| {
        let theirs = theirs.as_f64().unwrap_or(f64::NAN);
        let close = (ours - theirs).abs() <= 1e-9 * (1.0 + ours.abs());
        assert!(
            close,
            "{name}: the camera's {ours} against the report's {theirs}"
        );
    };
    for (axis, eye) in camera.eye.iter().enumerate() {
        near(*eye, &drawn["eye"][axis]);
    }
    near(camera.yaw_deg, &drawn["yaw_deg"]);
    near(camera.pitch_deg, &drawn["pitch_deg"]);
    near(camera.fov_y_deg, &drawn["fov_y_deg"]);
    // The room asks for its faces and three things, named by kind; at least
    // something is drawn, and every pixel is some object's or the sky's.
    let names: Vec<&str> = (frame.objects.iter())
        .filter_map(|o| o.name.as_deref())
        .collect();
    assert_eq!(names, ["marker", "crate", "barrel"], "{name}");
    assert!(frame.objects.len() > 40, "{name}: the room's faces");
    assert!(frame.objects.iter().any(|o| o.pixels > 0), "{name}");
}

/// How one shot's record fared against its reference, as `frames.json`
/// writes it: `pass`, `fail`, `no-reference`, or `pending` (a marked
/// shot's record measured against an `ENG_FRAME_DIR` reference, not
/// judged); the worst use of each tolerance when compared within them;
/// what failed it; and what was found that could not.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub status: &'static str,
    pub worst: Value,
    pub failures: Vec<String>,
    pub notes: Vec<String>,
}

/// Where a reference was looked for: whether `ENG_FRAME_DIR` named the
/// directory (`explicit`), and whether any shot has a reference there
/// (`complete`).
#[derive(Clone, Copy, Debug)]
pub struct References<'a> {
    pub at: &'a Path,
    pub explicit: bool,
    pub complete: bool,
}

/// Judges `ours` (a shot's record text, read) against `reference`, the
/// text at `refs.at` if there is one, as `tests/shots.rs` judges goldens:
/// - none: "no reference yet", unless the directory was asked for or
///   holds any shot's, when it is missing and fails;
/// - on the pinned configuration ([`Mode::Exact`]) byte for byte, else
///   within `tolerance`.
///
/// A shot `shots.toml` marks `golden = "pending"` (T16's marker) has no
/// reference yet by the owner's choice, whatever the others have: it fails
/// nothing, with `ENG_FRAME_DIR` too, where a reference it has is measured
/// and reported but cannot fail it. A reference for a marked shot in
/// `golden/frames/` itself fails: the marker should have been cleared in
/// the commit that added it.
pub fn judge(
    pending: bool,
    ours: (&str, &FrameRecord),
    reference: Option<&str>,
    refs: References<'_>,
    mode: Mode,
    tolerance: &Tolerances,
) -> Verdict {
    let marked = "golden = \"pending\" in shots.toml";
    let verdict = |status, worst, failures, notes| Verdict {
        status,
        worst,
        failures,
        notes,
    };
    let at = refs.at.display();
    let Some(text) = reference else {
        if pending {
            let note = format!("[W-NO-GOLDEN] no reference yet ({marked})");
            return verdict("no-reference", Value::Null, Vec::new(), vec![note]);
        }
        if refs.explicit || refs.complete {
            let why = if refs.explicit {
                "ENG_FRAME_DIR is set"
            } else {
                "other shots have theirs"
            };
            let failure = format!("[E-GOLDEN] {at} is missing; {why}");
            return verdict("fail", Value::Null, vec![failure], Vec::new());
        }
        return verdict("no-reference", Value::Null, Vec::new(), Vec::new());
    };
    if pending && !refs.explicit {
        let failure = format!(
            "[E-GOLDEN] {at} exists, but {marked}: clear the marker in the commit that adds \
             the reference"
        );
        return verdict("fail", Value::Null, vec![failure], Vec::new());
    }
    let (worst, problems) = if mode == Mode::Exact {
        let same = text == ours.0;
        let why = "[E-GOLDEN] not byte for byte the reference on the pinned configuration";
        (
            Value::Null,
            if same {
                Vec::new()
            } else {
                vec![why.to_owned()]
            },
        )
    } else {
        match FrameRecord::parse(text) {
            Ok(theirs) => {
                let comparison = ours.1.compare(&theirs, tolerance);
                (worst_json(&comparison.worst), comparison.failures)
            }
            Err(e) => (Value::Null, vec![format!("[E-GOLDEN] {at}: {e}")]),
        }
    };
    match (pending, problems.is_empty()) {
        (true, _) => verdict("pending", worst, Vec::new(), problems),
        (false, true) => verdict("pass", worst, Vec::new(), Vec::new()),
        (false, false) => verdict("fail", worst, problems, Vec::new()),
    }
}

/// A comparison's worst uses, as `frames.json` writes them.
pub fn worst_json(worst: &Worst) -> Value {
    serde_json::from_str(&worst.json().to_compact()).unwrap()
}
