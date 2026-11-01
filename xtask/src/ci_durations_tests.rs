//! The duration verdicts of `ci_durations.rs`, from made-up job lists in
//! the shape `run_jobs` gets from GitHub.

use super::*;

#[test]
fn unix_seconds_reads_github_times() {
    assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(unix_seconds("2000-03-01T00:00:00Z"), Some(951_868_800));
    assert_eq!(unix_seconds("2024-02-29T12:00:00Z"), Some(1_709_208_000));
    assert_eq!(unix_seconds("2026-11-01T00:12:34Z"), Some(1_793_491_954));
    assert_eq!(unix_seconds("2100-03-01T00:00:00Z"), Some(4_107_542_400));
    for bad in [
        "",
        "null",
        "2026-13-01T00:00:00Z",
        "2026-11-31T00:00:00Z",
        "2026-11-01 00:00:00",
        "2026-11-01T24:00:00Z",
        "2026-11-01T00:00:00",
        "1969-12-31T23:59:59Z",
        "2026-11-01T00:00:0xZ",
    ] {
        assert_eq!(unix_seconds(bad), None, "{bad}");
    }
}

const RUNNERS: [&str; 2] = ["ubuntu-24.04", "macos-26"];

#[test]
fn native_duration_reports_the_slowest_job_against_the_budget() {
    let jobs = "native (ubuntu-24.04)\t2026-11-01T10:00:00Z\t2026-11-01T10:12:00Z\n\
                native (macos-26)\t2026-11-01T10:00:30Z\t2026-11-01T10:18:30Z\n\
                lint\t2026-11-01T10:00:00Z\t2026-11-01T10:40:00Z\n";
    let detail = native_duration(jobs, &RUNNERS, 25).unwrap();
    for wanted in [
        "macos-26",
        "18.0 min",
        "ubuntu-24.04",
        "12.0 min",
        "within the 25 min",
    ] {
        assert!(detail.contains(wanted), "{wanted}: {detail}");
    }
    assert!(
        !detail.contains("lint") && !detail.contains("40.0"),
        "{detail}"
    );
}

#[test]
fn going_over_the_budget_is_reported_never_failed() {
    let jobs = "native (ubuntu-24.04)\t2026-11-01T10:00:00Z\t2026-11-01T10:31:00Z\n\
                native (macos-26)\t2026-11-01T10:00:00Z\t2026-11-01T10:10:00Z\n";
    let detail = native_duration(jobs, &RUNNERS, 25).unwrap();
    assert!(
        detail.contains("OVER") && detail.contains("31.0 min"),
        "{detail}"
    );
}

#[test]
fn native_duration_needs_every_native_job_finished() {
    let ubuntu = "native (ubuntu-24.04)\t2026-11-01T10:00:00Z\t2026-11-01T10:12:00Z\n";
    assert!(native_duration("", &RUNNERS, 25).is_err());
    let error = native_duration(ubuntu, &RUNNERS, 25).unwrap_err();
    assert!(error.contains("macos-26"), "{error}");
    let unfinished = format!("{ubuntu}native (macos-26)\t2026-11-01T10:00:00Z\tnull\n");
    let error = native_duration(&unfinished, &RUNNERS, 25).unwrap_err();
    assert!(error.contains("macos-26"), "{error}");
    let arm = "native (ubuntu-24.04-arm)\t2026-11-01T10:00:00Z\t2026-11-01T10:12:00Z\n";
    assert!(
        native_duration(arm, &["ubuntu-24.04"], 25).is_err(),
        "names match exactly"
    );
}

/// The jobs of a run whose PR tier ends at `last` (the slowest test job).
fn jobs_ending(last: &str) -> String {
    let test = format!("test (ubuntu-24.04)\t2026-11-01T10:00:00Z\t{last}");
    [
        "lint\t2026-11-01T10:00:00Z\t2026-11-01T10:05:00Z",
        &test,
        "test (ubuntu-24.04-arm)\t2026-11-01T10:00:00Z\t2026-11-01T10:11:00Z",
        "riscv\t2026-11-01T10:00:00Z\t2026-11-01T10:08:00Z",
        "wasm\t2026-11-01T10:00:00Z\t2026-11-01T10:03:00Z",
        "selftest\t2026-11-01T10:00:00Z\t2026-11-01T10:06:00Z",
        "native (macos-26)\t2026-11-01T10:00:00Z\t2026-11-01T10:40:00Z",
        "summary\t2026-11-01T10:40:00Z\tnull",
    ]
    .join("\n")
}

/// The summary now waits for the native jobs too, so the PR tier ends with
/// its own slowest job, not when the summary runs.
#[test]
fn pr_duration_ends_with_the_pr_tiers_last_job() {
    let jobs = jobs_ending("2026-11-01T10:12:00Z");
    let detail = pr_duration(&jobs, "2026-11-01T09:59:30Z", &PR_JOBS, 15).unwrap();
    for wanted in ["12.5 min", "within the 15 min", "test (ubuntu-24.04)"] {
        assert!(detail.contains(wanted), "{wanted}: {detail}");
    }
    assert!(
        !detail.contains("40.") && !detail.contains("native"),
        "{detail}"
    );
    let over = jobs_ending("2026-11-01T10:20:00Z");
    let detail = pr_duration(&over, "2026-11-01T09:59:30Z", &PR_JOBS, 15).unwrap();
    assert!(
        detail.contains("OVER") && detail.contains("20.5 min"),
        "{detail}"
    );
}

#[test]
fn pr_duration_needs_every_pr_job_finished_and_the_run_start() {
    let jobs = jobs_ending("2026-11-01T10:12:00Z");
    let without: String = jobs
        .lines()
        .filter(|line| !line.starts_with("riscv"))
        .collect::<Vec<_>>()
        .join("\n");
    let error = pr_duration(&without, "2026-11-01T09:59:30Z", &PR_JOBS, 15).unwrap_err();
    assert!(error.contains("riscv"), "{error}");
    let unfinished = jobs_ending("null");
    let error = pr_duration(&unfinished, "2026-11-01T09:59:30Z", &PR_JOBS, 15).unwrap_err();
    assert!(error.contains("test"), "{error}");
    assert!(pr_duration(&jobs, "null", &PR_JOBS, 15).is_err());
    // A job named like a PR job's prefix is not one.
    let testing = jobs.replace("test (ubuntu-24.04)\t", "testing\t");
    let testing = testing.replace("test (ubuntu-24.04-arm)\t", "tests\t");
    assert!(pr_duration(&testing, "2026-11-01T09:59:30Z", &PR_JOBS, 15).is_err());
}
