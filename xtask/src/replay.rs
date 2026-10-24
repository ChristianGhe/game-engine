//! `cargo xtask replay`: run every testbed scenario and seed with its
//! assertions and budgets, write the hash streams to
//! `target/ci/hashes/<target>/`, and compare them with the committed streams
//! in `crates/game-testbed/hashes/`.
//!
//! Committed streams are change detectors, not oracles (spec 6.4): a
//! simulation-changing PR regenerates them with `--regen` and the reviewer
//! approves. Cross-architecture equality (`compare-hashes`) is always required.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::util;

const SCENARIOS: &str = "crates/game-testbed/scenarios";
const COMMITTED: &str = "crates/game-testbed/hashes";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    #[serde(rename = "version")]
    _version: u32,
    scenario: Vec<Scenario>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    name: String,
    input: String,
    assert: Option<String>,
    ticks: u64,
    seeds: Vec<u64>,
}

#[derive(Default)]
pub struct Options {
    pub regen: bool,
    pub target: Option<String>,
    pub out: Option<PathBuf>,
}

pub fn main(args: &[&str]) -> Result<String, String> {
    let mut opts = Options::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--regen" => opts.regen = true,
            "--target" => {
                opts.target = Some(it.next().ok_or("--target needs a triple")?.to_string())
            }
            "--out" => opts.out = Some(PathBuf::from(it.next().ok_or("--out needs a directory")?)),
            other => return Err(format!("replay: unknown argument {other:?}")),
        }
    }
    run(&util::root()?, &opts)
}

pub fn host_triple(root: &Path) -> Result<String, String> {
    let mut cmd = Command::new("rustc");
    cmd.current_dir(root).arg("-vV");
    let out = util::run_quiet(root, "rustc-version", &mut cmd)?;
    out.lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(String::from)
        .ok_or_else(|| "rustc -vV printed no host".into())
}

/// The `CARGO_TARGET_<TRIPLE>_RUNNER` command for cross targets, split into words.
fn runner(target: &str) -> Vec<String> {
    let var = format!(
        "CARGO_TARGET_{}_RUNNER",
        target.to_uppercase().replace('-', "_")
    );
    std::env::var(var)
        .map(|v| v.split_whitespace().map(String::from).collect())
        .unwrap_or_default()
}

pub fn run(root: &Path, opts: &Options) -> Result<String, String> {
    let manifest: Manifest = toml::from_str(
        &fs::read_to_string(root.join(SCENARIOS).join("scenarios.toml"))
            .map_err(|e| format!("scenarios.toml: {e}"))?,
    )
    .map_err(|e| format!("scenarios.toml: {e}"))?;
    // Every assertion file must be bound to a scenario, so dropping a
    // manifest line cannot silently retire a protected oracle.
    let referenced: Vec<&str> = manifest
        .scenario
        .iter()
        .filter_map(|s| s.assert.as_deref())
        .collect();
    let scenario_dir = root.join(SCENARIOS);
    let orphans: Vec<String> = fs::read_dir(&scenario_dir)
        .map_err(|e| format!("{}: {e}", scenario_dir.display()))?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".assert.toml") && !referenced.contains(&name.as_str()))
        .collect();
    if !orphans.is_empty() {
        return Err(format!(
            "[E-REPLAY] assertion files not referenced by scenarios.toml: {}",
            orphans.join(", ")
        ));
    }
    let label = match &opts.target {
        Some(t) => t.clone(),
        None => host_triple(root)?,
    };
    let mut build = util::cargo(root);
    build.args(["build", "-p", "game-testbed"]);
    if util::on_ci() {
        build.arg("--locked");
    }
    if let Some(t) = &opts.target {
        build.args(["--target", t]);
    }
    util::run_quiet(root, "replay-build", &mut build)?;
    let exe = format!(
        "game-testbed{}",
        if label.contains("windows") {
            ".exe"
        } else {
            ""
        }
    );
    let binary = match &opts.target {
        Some(t) => root.join("target").join(t).join("debug").join(exe),
        None => root.join("target/debug").join(exe),
    };
    let wrapper = opts.target.as_deref().map(runner).unwrap_or_default();
    let out_dir = opts
        .out
        .clone()
        .unwrap_or_else(|| root.join("target/ci/hashes").join(&label));
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;

    let scenarios = root.join(SCENARIOS);
    let mut failures = Vec::new();
    let mut runs = 0;
    for sc in &manifest.scenario {
        for seed in &sc.seeds {
            runs += 1;
            let stem = format!("{}.seed{seed}", sc.name);
            let mut cmd = match wrapper.split_first() {
                Some((program, args)) => {
                    let mut c = Command::new(program);
                    c.args(args).arg(&binary);
                    c
                }
                None => Command::new(&binary),
            };
            cmd.current_dir(root)
                .env("ENG_AGENT_LOG_DIR", root.join("target/agent-logs"))
                .args([
                    "run",
                    "--headless",
                    "--ticks",
                    &sc.ticks.to_string(),
                    "--seed",
                    &seed.to_string(),
                ])
                .arg("--input")
                .arg(scenarios.join(&sc.input))
                .arg("--budgets")
                .arg(root.join("budgets.toml"))
                .arg("--hashes")
                .arg(out_dir.join(format!("{stem}.txt")))
                .arg("--report")
                .arg(out_dir.join(format!("{stem}.report.json")));
            if let Some(a) = &sc.assert {
                cmd.arg("--assert").arg(scenarios.join(a));
            }
            if let Err(e) = util::run_quiet(root, &format!("replay-{stem}"), &mut cmd) {
                failures.push(format!("[E-REPLAY] {stem}: {e}"));
            }
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("\n"));
    }

    let committed = root.join(COMMITTED);
    if opts.regen {
        let _ = fs::remove_dir_all(&committed);
        fs::create_dir_all(&committed).map_err(|e| format!("{}: {e}", committed.display()))?;
        for (name, text) in streams(&out_dir)? {
            fs::write(committed.join(&name), text).map_err(|e| format!("{name}: {e}"))?;
        }
        return Ok(format!(
            "replay: {runs} runs; regenerated {COMMITTED}/. Say why in the commit; the reviewer approves."
        ));
    }
    let diffs = compare(
        &streams(&committed)?,
        &streams(&out_dir)?,
        "committed",
        &label,
    );
    if diffs.is_empty() {
        Ok(format!(
            "replay: {runs} runs on {label}, assertions and budgets ok, hash streams match {COMMITTED}/"
        ))
    } else {
        Err(format!(
            "{}\nIf the simulation change is intended, run `cargo xtask replay --regen` and explain it in the commit.",
            diffs.join("\n")
        ))
    }
}

/// Hash stream files (`*.txt`) in a directory, by file name.
fn streams(dir: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".txt") {
            let text = fs::read_to_string(entry.path()).map_err(|e| format!("{name}: {e}"))?;
            out.insert(name, text);
        }
    }
    Ok(out)
}

/// Differences between two sets of streams, naming the first divergent tick.
fn compare(
    a: &BTreeMap<String, String>,
    b: &BTreeMap<String, String>,
    la: &str,
    lb: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    for name in a
        .keys()
        .chain(b.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (a.get(name), b.get(name)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => {
                let first = x
                    .lines()
                    .zip(y.lines())
                    .find(|(l, r)| l != r)
                    .map(|(l, r)| {
                        if l.starts_with('#') {
                            format!("headers differ:\n  {la}: {l}\n  {lb}: {r}")
                        } else {
                            let tick = l.split_whitespace().next().unwrap_or("?");
                            format!("first divergence at tick {tick}: {la} {l} / {lb} {r}")
                        }
                    })
                    .unwrap_or_else(|| "streams have different lengths".into());
                out.push(format!("[E-HASH] {name}: {first}"));
            }
            (Some(_), None) => out.push(format!("[E-HASH] {name}: in {la} but not in {lb}")),
            (None, Some(_)) => out.push(format!("[E-HASH] {name}: in {lb} but not in {la}")),
            (None, None) => {}
        }
    }
    out
}

/// `compare-hashes DIR DIR...`: every directory must hold identical streams.
pub fn compare_dirs(root: &Path, dirs: &[&str]) -> Result<String, String> {
    let resolve = |d: &str| {
        let p = Path::new(d);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            root.join(p)
        }
    };
    let first = resolve(dirs[0]);
    let base = streams(&first)?;
    if base.is_empty() {
        return Err(format!(
            "[E-CROSS-ARCH] no hash streams in {}",
            first.display()
        ));
    }
    let mut diffs = Vec::new();
    for d in &dirs[1..] {
        let other = streams(&resolve(d))?;
        diffs.extend(
            compare(&base, &other, dirs[0], d)
                .into_iter()
                .map(|s| s.replace("[E-HASH]", "[E-CROSS-ARCH]")),
        );
    }
    if diffs.is_empty() {
        Ok(format!(
            "compare-hashes: {} streams identical across {} directories",
            base.len(),
            dirs.len()
        ))
    } else {
        Err(diffs.join("\n"))
    }
}
