//! A CI job's report, `target/ci/report.json` and its copy in
//! `target/ci/reports/<job>-<target>.json`, which `acceptance` reads. A step
//! is ok, failed, or pending (`util::Done`): pending fails no job, and its
//! `ok` stays false, so no acceptance check counts it as a pass.

use std::fs;
use std::path::Path;
use std::time::Instant;

use serde_json::{Map, Value, json};

use crate::replay;
use crate::util::{self, StepResult, Steps};

/// The job's report (`report_version` 1), with `extra` keys added at the
/// top level.
pub(crate) fn report(
    job: &str,
    target: &str,
    steps: &Steps,
    seconds: f64,
    extra: Map<String, Value>,
) -> Value {
    let mut report = json!({
        "report_version": 1,
        "job": job,
        "target": target,
        "runner": std::env::var("RUNNER_NAME").unwrap_or_else(|_| "local".into()),
        "os": std::env::var("ImageOS").unwrap_or_else(|_| std::env::consts::OS.into()),
        "git_sha": std::env::var("GITHUB_SHA").unwrap_or_default(),
        "ok": steps.ok(),
        "seconds": seconds,
        "steps": steps.results.iter().map(|r| json!({
            "name": r.name, "ok": r.ok, "pending": r.pending, "seconds": r.seconds,
            "detail": util::tail(&r.detail, 5),
        })).collect::<Vec<_>>(),
    });
    if let Some(map) = report.as_object_mut() {
        map.extend(extra);
    }
    report
}

/// Writes the job's report and says whether every step passed; a pending
/// step fails no job.
pub(crate) fn finish(
    root: &Path,
    job: &str,
    steps: Steps,
    started: Instant,
    extra: Map<String, Value>,
) -> Result<String, String> {
    let target = replay::host_triple(root).unwrap_or_else(|_| "unknown".into());
    let seconds = started.elapsed().as_secs_f64();
    let report = report(job, &target, &steps, seconds, extra);
    let dir = root.join("target/ci");
    let text = serde_json::to_string_pretty(&report).expect("report serializes");
    let _ = fs::create_dir_all(dir.join("reports"));
    let _ = fs::write(dir.join("report.json"), &text);
    let _ = fs::write(
        dir.join("reports").join(format!("{job}-{target}.json")),
        &text,
    );
    let named = |pick: fn(&StepResult) -> bool| -> Vec<&str> {
        steps
            .results
            .iter()
            .filter(|r| pick(r))
            .map(|r| r.name.as_str())
            .collect()
    };
    if !steps.ok() {
        let failed = named(|r| !r.ok && !r.pending);
        return Err(format!("{job}: failed steps: {}", failed.join(", ")));
    }
    let pending = named(|r| r.pending);
    let pending = if pending.is_empty() {
        String::new()
    } else {
        format!("; pending, not passes: {}", pending.join(", "))
    };
    Ok(format!(
        "{job}: no step failed, {} of {} ok{pending} in {seconds:.0}s",
        steps.results.iter().filter(|r| r.ok).count(),
        steps.results.len()
    ))
}
