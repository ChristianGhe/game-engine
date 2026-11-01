//! How long the PR tier and the native jobs took on GitHub Actions: the
//! summary's `pr-duration` and `native-duration` steps (rung 1b, patch X3).
//! The steps ask `gh` for this run's jobs; the verdicts are pure functions
//! of that list (`ci_durations_tests.rs`).

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::{ci_native, util};

/// The PR tier's jobs in the workflow (a matrix job is `<name> (<value>)`).
pub(crate) const PR_JOBS: [&str; 5] = ["lint", "test", "riscv", "wasm", "selftest"];

/// This run on GitHub Actions: its repository and id.
fn github_run(measured: &str) -> Result<(String, String), String> {
    match (
        std::env::var("GITHUB_REPOSITORY"),
        std::env::var("GITHUB_RUN_ID"),
    ) {
        (Ok(repo), Ok(run)) => Ok((repo, run)),
        _ => Err(format!(
            "not on GitHub Actions; {measured} is measured there"
        )),
    }
}

/// This run's jobs, a line each: name, start and end (`null` until set).
fn run_jobs(root: &Path, repo: &str, run: &str) -> Result<String, String> {
    let mut cmd = Command::new("gh");
    cmd.args([
        "api",
        "--paginate",
        &format!("repos/{repo}/actions/runs/{run}/jobs?per_page=100"),
        "--jq",
        r#".jobs[] | [.name, (.started_at // "null"), (.completed_at // "null")] | @tsv"#,
    ]);
    util::run_quiet(root, "gh-jobs", &mut cmd)
}

/// `budgets.toml [ci] <key>`.
fn ci_budget(root: &Path, key: &str) -> Option<i64> {
    let text = fs::read_to_string(root.join("budgets.toml")).ok()?;
    text.parse::<toml::Table>()
        .ok()?
        .get("ci")?
        .get(key)?
        .as_integer()
}

/// The PR tier's duration (spec 8.2): from the run's start to the end of its
/// last PR job, since the summary now also waits for the native jobs.
pub fn pr_tier(root: &Path) -> Result<String, String> {
    let (repo, run) = github_run("the PR-tier duration")?;
    let mut cmd = Command::new("gh");
    cmd.args([
        "api",
        &format!("repos/{repo}/actions/runs/{run}"),
        "--jq",
        ".run_started_at",
    ]);
    let started = util::run_quiet(root, "gh-run", &mut cmd)?;
    let budget = ci_budget(root, "pr_tier_minutes").unwrap_or(15);
    let jobs = run_jobs(root, &repo, &run)?;
    pr_duration(&jobs, &started, &PR_JOBS, budget)
}

/// The `native-duration` step (criterion 13): each native job's time on
/// GitHub, the slowest held to `budgets.toml [ci] native_job_minutes`.
pub fn native_tier(root: &Path) -> Result<String, String> {
    let (repo, run) = github_run("the native jobs' duration")?;
    let budget = ci_budget(root, "native_job_minutes")
        .ok_or("[E-BUDGET] budgets.toml has no [ci] native_job_minutes")?;
    let jobs = run_jobs(root, &repo, &run)?;
    let runners: Vec<&str> = ci_native::RUNNERS.iter().map(|r| r.0).collect();
    native_duration(&jobs, &runners, budget)
}

/// Seconds since 1970 for a UTC time as GitHub's API writes it,
/// `2026-11-01T00:12:34Z`; `None` for anything else.
pub fn unix_seconds(iso: &str) -> Option<u64> {
    let bytes = iso.as_bytes();
    let shape = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'Z'),
    ];
    if bytes.len() != 20 || shape.iter().any(|&(at, c)| bytes[at] != c) {
        return None;
    }
    let number = |from: usize, to: usize| -> Option<u64> {
        let digits = iso.get(from..to)?;
        digits.bytes().all(|b| b.is_ascii_digit()).then_some(())?;
        digits.parse().ok()
    };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let february = if leap(year) { 29 } else { 28 };
    let months = [31, february, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let days_in_month = *months.get(usize::try_from(month).ok()?.checked_sub(1)?)?;
    if year < 1970 || day == 0 || day > days_in_month || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let years: u64 = (1970..year).map(|y| if leap(y) { 366 } else { 365 }).sum();
    let before: u64 = months[..usize::try_from(month - 1).ok()?].iter().sum();
    let days = years + before + day - 1;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// The `native-duration` step's verdict (criterion 13): how long each
/// runner's native job took, from `jobs`, this run's jobs as lines of name,
/// start and end (tab-separated, as [`run_jobs`] asks `gh` for them),
/// with the slowest held to `budget_minutes`. Going over is reported,
/// never failed; a runner's job missing or unfinished fails, since then the
/// tier was not measured.
pub fn native_duration(
    jobs: &str,
    runners: &[&str],
    budget_minutes: i64,
) -> Result<String, String> {
    let (mut took, mut missing) = (Vec::new(), Vec::new());
    for &runner in runners {
        let name = format!("native ({runner})");
        let times = jobs.lines().find_map(|line| {
            let mut fields = line.split('\t');
            (fields.next()? == name).then(|| {
                let start = unix_seconds(fields.next()?)?;
                let end = unix_seconds(fields.next()?)?;
                end.checked_sub(start)
            })?
        });
        match times {
            Some(seconds) => took.push((runner, seconds)),
            None => missing.push(runner),
        }
    }
    if !missing.is_empty() || took.is_empty() {
        return Err(format!(
            "[E-DURATION] no finished native job for {}",
            missing.join(", ")
        ));
    }
    let minutes = |seconds: u64| format!("{:.1} min", seconds as f64 / 60.0);
    let list: Vec<String> = took
        .iter()
        .map(|&(runner, seconds)| format!("{runner} {}", minutes(seconds)))
        .collect();
    let &(slowest, seconds) = took
        .iter()
        .max_by_key(|&&(_, seconds)| seconds)
        .expect("took is not empty");
    let within = i64::try_from(seconds).is_ok_and(|s| s <= budget_minutes.saturating_mul(60));
    let verdict = if within { "within" } else { "OVER" };
    Ok(format!(
        "native jobs: {}; the slowest, {slowest}, took {} ({verdict} the {budget_minutes} min \
         budget, budgets.toml [ci] native_job_minutes)",
        list.join(", "),
        minutes(seconds)
    ))
}

/// The `pr-duration` step's verdict (spec 8.2, measured since rung 1a): from
/// the run's start (`run_started`) to the end of the PR tier's last job.
/// The summary now waits for the native jobs too, so its own clock would
/// count them. `jobs` is as for [`native_duration`]; a PR job is named as in
/// `pr_jobs`, or with a matrix value, as `test (ubuntu-24.04)`. Going over
/// is reported, never failed.
pub fn pr_duration(
    jobs: &str,
    run_started: &str,
    pr_jobs: &[&str],
    budget_minutes: i64,
) -> Result<String, String> {
    let start = unix_seconds(run_started.trim())
        .ok_or_else(|| format!("[E-DURATION] the run's start is not a time: {run_started:?}"))?;
    let (mut last, mut missing): (Option<(&str, u64)>, Vec<String>) = (None, Vec::new());
    for &job in pr_jobs {
        let matrix = format!("{job} (");
        let mut seen = false;
        for line in jobs.lines() {
            let mut fields = line.split('\t');
            let name = fields.next().unwrap_or_default();
            if name != job && !name.starts_with(&matrix) {
                continue;
            }
            seen = true;
            match fields.nth(1).and_then(unix_seconds) {
                Some(end) if last.is_none_or(|(_, latest)| end > latest) => {
                    last = Some((name, end))
                }
                Some(_) => {}
                None => missing.push(format!("{name} (unfinished)")),
            }
        }
        if !seen {
            missing.push(job.to_string());
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "[E-DURATION] the PR tier has no finished {}",
            missing.join(", ")
        ));
    }
    let (name, end) = last.ok_or("[E-DURATION] the PR tier has no job")?;
    let seconds = end.saturating_sub(start);
    let within = i64::try_from(seconds).is_ok_and(|s| s <= budget_minutes.saturating_mul(60));
    let verdict = if within { "within" } else { "OVER" };
    Ok(format!(
        "PR tier took {:.1} min, to the end of {name} ({verdict} the {budget_minutes} min budget)",
        seconds as f64 / 60.0
    ))
}

#[cfg(test)]
#[path = "ci_durations_tests.rs"]
mod tests;
