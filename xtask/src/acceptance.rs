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

/// Why a check did not pass.
enum Miss {
    Failed(String),
    /// A step that cannot be judged yet (`"pending": true` in its report,
    /// patch X2), such as a shot without its golden: not a pass, and not a
    /// failure either.
    Pending(String),
}

/// Whether a check passed, or why not. A failure in any matching report
/// outranks a pending step in another.
fn check(reports: &[Value], check: &Value) -> Result<(), Miss> {
    let failed = |why: String| Err(Miss::Failed(why));
    let (Some(job), Some(step)) = (check["job"].as_str(), check["step"].as_str()) else {
        return failed("check without job or step".into());
    };
    let target = check["target"].as_str();
    let matching: Vec<&Value> = reports
        .iter()
        .filter(|r| r["job"] == job && target.is_none_or(|t| r["target"] == t))
        .collect();
    if matching.is_empty() {
        return failed(format!(
            "no report from job {job}{}",
            target.map(|t| format!(" on {t}")).unwrap_or_default()
        ));
    }
    let mut pending = None;
    for report in matching {
        let steps = report["steps"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        match steps.iter().find(|s| s["name"] == step) {
            Some(s) if s["ok"] == true => {}
            Some(s) if s["pending"] == true => {
                let why = s["detail"].as_str().and_then(|d| d.lines().next());
                let why = why.map(|line| format!(": {line}")).unwrap_or_default();
                let on = &report["target"];
                pending.get_or_insert(format!("{job}/{step} is pending on {on}{why}"));
            }
            Some(_) => return failed(format!("{job}/{step} failed on {}", report["target"])),
            None => return failed(format!("{job} on {} has no step {step}", report["target"])),
        }
    }
    pending.map_or(Ok(()), |why| Err(Miss::Pending(why)))
}

/// Whether the owner checks `feature` (`"owner": true`, rung 1b's patch X4):
/// the owner plays the rung and accepts it (spec 2.2, touchpoint 1), so CI
/// lists the feature and never computes it. Its `checks` must be `[]` or
/// absent.
fn owner_checked(feature: &Value) -> Result<bool, String> {
    let id = &feature["id"];
    let no_checks = match feature.get("checks") {
        None => true,
        Some(Value::Array(list)) => list.is_empty(),
        Some(_) => false,
    };
    match feature.get("owner") {
        None | Some(Value::Bool(false)) => Ok(false),
        Some(Value::Bool(true)) if no_checks => Ok(true),
        Some(Value::Bool(true)) => Err(format!(
            "acceptance.json: {id} is checked by the owner (`owner: true`) but lists checks \
             ({}); CI computes none for it, so `checks` must be `[]` or absent",
            feature["checks"]
        )),
        Some(other) => Err(format!(
            "acceptance.json: {id} has `owner: {other}`; it is true or false"
        )),
    }
}

/// Sets `passes` on every feature of `doc` from `reports`. Returns one line
/// per feature, then the totals. An owner-checked feature gets `passes:
/// null`: CI never decides it, and it counts neither way.
fn evaluate(doc: &mut Value, reports: &[Value]) -> Result<(Vec<String>, String), String> {
    let mut lines = Vec::new();
    let (mut passing, mut waiting, mut total) = (0, 0, 0);
    let mut owners = Vec::new();
    let features = doc["features"]
        .as_array_mut()
        .ok_or("acceptance.json has no features")?;
    for feature in features.iter_mut() {
        for computed in ["passes", "pending"] {
            if feature.get(computed).is_some() {
                return Err(format!(
                    "acceptance.json sets `{computed}` for {} by hand; CI computes it",
                    feature["id"]
                ));
            }
        }
        let id = feature["id"].as_str().unwrap_or("?").to_string();
        if owner_checked(feature)? {
            feature["passes"] = Value::Null;
            lines.push(format!(
                "OWNER {id}: the owner checks this feature; CI does not compute it"
            ));
            owners.push(id);
            continue;
        }
        total += 1;
        let checks = feature["checks"].as_array().cloned().unwrap_or_default();
        // Every check is looked at: a failure outranks a pending check.
        let (mut failed, mut pending) = (None, None);
        if checks.is_empty() {
            failed = Some("no checks listed".to_string());
        }
        for result in checks.iter().map(|c| check(reports, c)) {
            match result {
                Ok(()) => {}
                Err(Miss::Failed(why)) => _ = failed.get_or_insert(why),
                Err(Miss::Pending(why)) => _ = pending.get_or_insert(why),
            }
        }
        match (failed, pending) {
            (None, None) => {
                passing += 1;
                feature["passes"] = json!(true);
                lines.push(format!("PASS {id}"));
            }
            (Some(reason), _) => {
                feature["passes"] = json!(false);
                feature["why_not"] = json!(reason);
                lines.push(format!("FAIL {id}: {reason}"));
            }
            (None, Some(reason)) => {
                waiting += 1;
                feature["passes"] = json!(false);
                feature["pending"] = json!(true);
                feature["why_not"] = json!(reason);
                lines.push(format!("PENDING {id}: {reason}"));
            }
        }
    }
    let computed = if owners.is_empty() { "" } else { "computed " };
    let mut totals = format!("{passing}/{total} {computed}features pass");
    if waiting > 0 {
        totals.push_str(&format!(", {waiting} pending"));
    }
    if !owners.is_empty() {
        let (n, ids) = (owners.len(), owners.join(", "));
        totals.push_str(&format!("; {n} for the owner: {ids}"));
    }
    Ok((lines, totals))
}

/// A computed `acceptance.json` in one line, for the session-start hook:
/// owner-checked features are listed apart, neither passing nor failing.
pub fn status(doc: &Value) -> String {
    let features = doc["features"].as_array().cloned().unwrap_or_default();
    let (owners, computed): (Vec<&Value>, Vec<&Value>) =
        features.iter().partition(|f| f["owner"] == true);
    let ids = |list: Vec<&Value>| -> Vec<String> {
        list.iter()
            .map(|f| f["id"].as_str().unwrap_or("?").to_string())
            .collect()
    };
    let not_passing = |pending: bool| -> Vec<String> {
        let pick = |f: &&Value| f["passes"] != true && (f["pending"] == true) == pending;
        ids(computed.iter().copied().filter(pick).collect())
    };
    let (failing, pending) = (not_passing(false), not_passing(true));
    let passing = computed.len() - failing.len() - pending.len();
    let mut line = format!("{passing}/{} pass", computed.len());
    if !failing.is_empty() {
        line.push_str(&format!("; failing: {}", failing.join(", ")));
    }
    if !pending.is_empty() {
        line.push_str(&format!("; pending: {}", pending.join(", ")));
    }
    if !owners.is_empty() {
        line.push_str(&format!("; for the owner: {}", ids(owners).join(", ")));
    }
    line
}

pub fn compute(root: &Path, dir: &str) -> Result<String, String> {
    let rung = current_rung(root)?;
    let path = root.join("docs/rungs").join(&rung).join("acceptance.json");
    let mut doc: Value = serde_json::from_str(
        &fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let found = reports(&root.join(dir));
    let (mut lines, totals) = evaluate(&mut doc, &found)?;
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
        "rung {rung}: {totals}; wrote {}",
        util::rel(root, &out)
    ));
    Ok(lines.join("\n"))
}

#[cfg(test)]
#[path = "acceptance_tests.rs"]
mod tests;
