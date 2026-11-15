//! Rules for "proper" (rung 2, plan addition A2; acceptance `scene-rules`):
//! every first-person shot in `shots.toml`, rendered by the real binary as
//! the frame-record test renders it, keeps its scene's rules. The rules
//! are the owner's `scenes/<map>.rules.toml`, protected like assertions,
//! and eng-cli's `scene_rules` judges each shot's frame record by them:
//! the objects the shot must see, nothing showing where the world should
//! be, floor below and ceiling or sky above. A failure prints
//! `[E-SCENE-RULE]`, the check, the shot, the object and the values; every
//! finding, passed or not, goes to `target/scene-rules/rules.json`.
//!
//! - Every file in the rules directory is a `<map>.rules.toml` holding
//!   that map's rules, and each map's rules judge some first-person shot.
//! - A shot of a map with rules is rendered and judged; its rules need a
//!   `[shot.<name>]` table for it, and each table names such a shot.
//! - A shot of a map without rules must be of a map with no first-person
//!   view, which `shot --view first-person` refuses before any GPU opens:
//!   the top-down shots are skipped unrendered, and a first-person map
//!   without its rules fails.
//!
//! **Until the owner commits the first rules file** (gate G5, with
//! `docs/rungs/2/patches/ta2-scene-rules.patch`, which also replaces this
//! test with its protected, strict form), a tree without `scenes/` passes
//! with `[W-NO-RULES]`, as the golden shots pass before their goldens.
//!
//! The environment, as `tests/frame_record.rs` reads it: `ENG_REQUIRE_GPU=1`
//! fails without an adapter instead of skipping; `ENG_RULES_DIR` reads the
//! rules from another directory instead of `scenes/`: it must be a
//! directory holding rules. Each rule's selftest (`cargo xtask selftest
//! rule-1`, ...) plants a render bug in a workspace copy and requires this
//! test to fail on that rule.

// Shared with `frame_record.rs`, which uses the rest of it.
#[allow(dead_code)]
#[path = "frame_record/harness.rs"]
mod harness;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eng_cli::scene_rules::{Finding, Outcome, SceneRules};
use serde_json::{Value, json};

/// `ENG_RULES_DIR` if set (a directory, or it fails), else `scenes/`.
fn rules_dir() -> PathBuf {
    match std::env::var_os("ENG_RULES_DIR") {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            assert!(
                dir.is_dir(),
                "[E-SCENE-RULE] ENG_RULES_DIR {:?} is no directory",
                dir.display()
            );
            dir
        }
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("scenes"),
    }
}

/// Every `<map>.rules.toml` in `dir`, by map, read strictly: a file of
/// any other name fails, and so does a map other than its file's name.
/// `None` if `dir` does not exist.
fn read_rules(dir: &Path) -> Option<BTreeMap<String, SceneRules>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => panic!("[E-SCENE-RULE] {}: {e}", dir.display()),
    };
    let mut files: Vec<PathBuf> = entries
        .map(|e| {
            e.unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
                .path()
        })
        .collect();
    files.sort();
    let mut rules = BTreeMap::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let map = name.strip_suffix(".rules.toml").filter(|m| !m.is_empty());
        let Some(map) = map.filter(|_| path.is_file()) else {
            panic!(
                "[E-SCENE-RULE] {name} in {}: the rules directory holds <map>.rules.toml files \
                 alone",
                dir.display()
            );
        };
        let text = std::fs::read_to_string(&path).unwrap();
        let read = SceneRules::parse(&text, &name).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(read.map, map, "{name} must hold the rules of map {map}");
        rules.insert(map.to_owned(), read);
    }
    Some(rules)
}

/// Whether `map` (the game's first without one) has a first-person view:
/// `shot --view first-person` refuses a map without one with `[E-MAP]`
/// before a GPU opens, and draws an 8x8 frame of one. `None` without an
/// adapter.
fn first_person(map: Option<&str>, dir: &Path) -> Option<bool> {
    let mut args = vec!["shot", "--view", "first-person", "--at-tick", "0"];
    args.extend(["--size", "8x8", "--out", "view-probe.png"]);
    if let Some(map) = map {
        args.extend(["--map", map]);
    }
    let out = harness::probe(dir, &args)?;
    let text = harness::text(&out);
    match out.status.code() {
        Some(0) => Some(true),
        Some(64) if text.contains("[E-MAP]") && text.contains("no first-person view") => {
            Some(false)
        }
        _ => panic!("{args:?}: {text}"),
    }
}

fn finding_json(finding: &Finding) -> Value {
    let (outcome, scene_limit, why) = match &finding.outcome {
        Outcome::Pass => ("pass", None, None),
        Outcome::Fail => ("fail", None, None),
        Outcome::Excused { scene_limit, why } => ("excused", Some(*scene_limit), Some(why)),
        Outcome::Stale { scene_limit, why } => ("stale", Some(*scene_limit), Some(why)),
    };
    let measures: Vec<Value> = (finding.measures.iter())
        .map(|m| {
            json!({
                "name": m.name, "value": m.value, "limit": m.limit,
                "at_most": m.at_most, "ok": m.ok(),
            })
        })
        .collect();
    json!({
        "check": finding.check.name(), "tick": finding.tick, "object": finding.object,
        "outcome": outcome, "scene_limit": scene_limit, "excused_because": why,
        "measures": measures, "note": finding.note,
    })
}

#[test]
fn every_first_person_shot_keeps_its_scenes_rules() {
    let dir = rules_dir();
    let Some(rules) = read_rules(&dir) else {
        eprintln!(
            "[W-NO-RULES] no {}: no shot is judged until the owner commits the rules (G5)",
            dir.display()
        );
        return;
    };
    assert!(
        !rules.is_empty(),
        "[E-SCENE-RULE] {} holds no <map>.rules.toml",
        dir.display()
    );
    let shots = harness::shots();
    let tmp = harness::scratch("scene-rules");
    let mut failures = Vec::new();
    let mut judged: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut results = Vec::new();
    let mut config = None;
    // Each map without rules, once: no first-person view, so its shots
    // are skipped unrendered.
    let mut unruled: BTreeMap<Option<String>, Vec<&str>> = BTreeMap::new();
    for shot in &shots {
        let ruled = shot.map.as_ref().and_then(|map| rules.get(map));
        let (Some(map), Some(scene)) = (shot.map.clone(), ruled) else {
            unruled
                .entry(shot.map.clone())
                .or_default()
                .push(&shot.name);
            continue;
        };
        let Some(rendered) = harness::render(shot, &tmp) else {
            return;
        };
        config.get_or_insert_with(|| rendered.config.json());
        judged.entry(map).or_default().push(shot.name.clone());
        let findings = match scene.judge(&shot.name, &rendered.record) {
            Ok(findings) => findings,
            Err(e) => {
                eprintln!("{e}");
                failures.push(e);
                continue;
            }
        };
        for finding in &findings {
            let line = finding.message(&shot.name);
            eprintln!("{line}");
            if finding.fails() {
                failures.push(line);
            }
        }
        let findings: Vec<Value> = findings.iter().map(finding_json).collect();
        results.push(json!({ "name": shot.name, "map": shot.map, "findings": findings }));
    }
    for (map, names) in &unruled {
        let Some(first_person) = first_person(map.as_deref(), &tmp) else {
            return;
        };
        let map = map.as_deref().unwrap_or("the game's first map");
        if first_person {
            failures.push(format!(
                "[E-SCENE-RULE] {map} is drawn in first person, but {} has no rules for it \
                 (shots {names:?})",
                dir.display()
            ));
        } else {
            eprintln!("{map}: no first-person view, so no rules (shots {names:?} not judged)");
        }
    }
    // Each file judges some shots, and each of its tables names one.
    for (map, scene) in &rules {
        let shots = judged.get(map).cloned().unwrap_or_default();
        if shots.is_empty() {
            failures.push(format!(
                "[E-SCENE-RULE] {map}.rules.toml: no shot of map {map} in shots.toml"
            ));
        }
        for name in scene.shots.keys() {
            if !shots.contains(name) {
                failures.push(format!(
                    "[E-SCENE-RULE] {map}.rules.toml: [shot.{name}] is no shot of map {map} in \
                     shots.toml"
                ));
            }
        }
    }
    let summary = json!({
        "configuration": config, "rules_dir": dir.display().to_string(),
        "shots": results, "failures": failures,
    });
    let out = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .unwrap()
        .join("scene-rules");
    std::fs::create_dir_all(&out).unwrap();
    let text = serde_json::to_string_pretty(&summary).unwrap() + "\n";
    std::fs::write(out.join("rules.json"), text).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
