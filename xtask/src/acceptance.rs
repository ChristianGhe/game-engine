//! `cargo xtask acceptance DIR` (spec 7.3, rule 4): CI computes every
//! feature's `passes` from the CI job reports found under DIR. Agents never
//! set it by hand: the committed `acceptance.json` has no `passes` fields,
//! and the computed copy goes to `target/ci/acceptance.json`.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::util;

pub fn current_rung(root: &Path) -> Result<String, String> {
    fs::read_to_string(root.join("docs/rungs/current.txt"))
        .map(|t| t.trim().to_string())
        .map_err(|e| format!("docs/rungs/current.txt: {e}"))
}

fn reports(dir: &Path) -> Vec<Value> {
    util::walk(dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| serde_json::from_str::<Value>(&fs::read_to_string(p).ok()?).ok())
        .filter(|v| v.get("report_version").is_some() && v.get("job").is_some())
        .collect()
}

/// Whether a check passed, or why not.
fn check(reports: &[Value], check: &Value) -> Result<(), String> {
    let job = check["job"].as_str().ok_or("check without job")?;
    let step = check["step"].as_str().ok_or("check without step")?;
    let target = check["target"].as_str();
    let matching: Vec<&Value> = reports
        .iter()
        .filter(|r| r["job"] == job && target.is_none_or(|t| r["target"] == t))
        .collect();
    if matching.is_empty() {
        return Err(format!(
            "no report from job {job}{}",
            target.map(|t| format!(" on {t}")).unwrap_or_default()
        ));
    }
    for report in matching {
        let steps = report["steps"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        match steps.iter().find(|s| s["name"] == step) {
            Some(s) if s["ok"] == true => {}
            Some(_) => return Err(format!("{job}/{step} failed on {}", report["target"])),
            None => return Err(format!("{job} on {} has no step {step}", report["target"])),
        }
    }
    Ok(())
}

pub fn compute(root: &Path, dir: &str) -> Result<String, String> {
    let rung = current_rung(root)?;
    let path = root.join("docs/rungs").join(&rung).join("acceptance.json");
    let mut doc: Value = serde_json::from_str(
        &fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let found = reports(&root.join(dir));
    let mut lines = Vec::new();
    let (mut passing, mut total) = (0, 0);
    let features = doc["features"]
        .as_array_mut()
        .ok_or("acceptance.json has no features")?;
    for feature in features.iter_mut() {
        if feature.get("passes").is_some() {
            return Err(format!(
                "acceptance.json sets `passes` for {} by hand; CI computes it",
                feature["id"]
            ));
        }
        total += 1;
        let checks = feature["checks"].as_array().cloned().unwrap_or_default();
        let result = if checks.is_empty() {
            Err("no checks listed".to_string())
        } else {
            checks.iter().try_for_each(|c| check(&found, c))
        };
        let id = feature["id"].as_str().unwrap_or("?").to_string();
        match result {
            Ok(()) => {
                passing += 1;
                feature["passes"] = json!(true);
                lines.push(format!("PASS {id}"));
            }
            Err(reason) => {
                feature["passes"] = json!(false);
                feature["why_not"] = json!(reason);
                lines.push(format!("FAIL {id}: {reason}"));
            }
        }
    }
    doc["computed"] = json!({
        "from_reports": found.len(),
        "git_sha": std::env::var("GITHUB_SHA").unwrap_or_default(),
        "unix_time": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    });
    let out = root.join("target/ci/acceptance.json");
    let _ = fs::create_dir_all(out.parent().expect("has parent"));
    fs::write(
        &out,
        serde_json::to_string_pretty(&doc).expect("serializes"),
    )
    .map_err(|e| e.to_string())?;
    lines.push(format!(
        "rung {rung}: {passing}/{total} features pass; wrote {}",
        util::rel(root, &out)
    ));
    Ok(lines.join("\n"))
}
