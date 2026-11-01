//! How `cargo xtask selftest` reports a case that cannot run where it is
//! asked to, because cargo-deny or the pinned lavapipe is missing. As GPU
//! tests do (plan decision 4), that fails only a strict run, on CI or with
//! `ENG_REQUIRE_GPU=1` ([`NOT_RUN`]); elsewhere it is a warning
//! ([`NOT_RUN_HERE`]). It is never a pass: the step is not ok, and the last
//! line counts it apart.

use crate::util::{self, Done};

/// Starts the message of a case that could not run where it was asked to,
/// such as `golden-edit` without the pinned lavapipe: in a strict run, a
/// failure.
pub const NOT_RUN: &str = "[E-SELFTEST-NOT-RUN]";
/// The same outside a strict run: a warning, and still not a pass.
const NOT_RUN_HERE: &str = "[W-SELFTEST-NOT-RUN]";

/// Whether a case that cannot run fails the run: on CI (`CI` set to
/// anything, even empty) or with `ENG_REQUIRE_GPU=1`.
pub(super) fn strict() -> bool {
    let require_gpu = std::env::var("ENG_REQUIRE_GPU").ok();
    strict_from(util::on_ci(), require_gpu.as_deref())
}

/// Strict on CI (`util::on_ci`: `CI` set to anything, as for `--locked`)
/// or with `ENG_REQUIRE_GPU=1`.
fn strict_from(ci: bool, require_gpu: Option<&str>) -> bool {
    ci || require_gpu == Some("1")
}

/// A case's outcome as a step of a CI job (`ci --job selftest`): a case not
/// run here ([`NOT_RUN_HERE`]) is pending, which fails no job and passes no
/// acceptance check; a strict [`NOT_RUN`] stays a failure.
pub(super) fn judged(result: Result<String, String>) -> Result<Done, String> {
    match result {
        Ok(detail) => Ok(Done::Pass(detail)),
        Err(detail) if detail.starts_with(NOT_RUN_HERE) => Ok(Done::Pending(detail)),
        Err(detail) => Err(detail),
    }
}

/// A case's result as this run reports it: outside a strict run, "could not
/// run" becomes a warning, though the step is still not ok.
pub(super) fn outcome(result: Result<String, String>, strict: bool) -> Result<String, String> {
    match result {
        Err(detail) if !strict && detail.starts_with(NOT_RUN) => {
            Err(detail.replacen(NOT_RUN, NOT_RUN_HERE, 1))
        }
        other => other,
    }
}

/// The word a case's line shows.
pub(super) fn status(result: &Result<String, String>) -> &'static str {
    match result {
        Ok(_) => "ok",
        Err(detail) if detail.starts_with(NOT_RUN_HERE) => "not run",
        Err(_) => "FAILED",
    }
}

/// One case's step name and how it ended.
pub(super) type Outcome = (String, Result<String, String>);

/// The selftest's last line: how many cases caught their violation, how
/// many could not run here, and which let one through. A case not run
/// fails only a strict run, and the line never reads as all caught.
pub(super) fn last_line(outcomes: &[Outcome], strict: bool) -> Result<String, String> {
    let not_run_mark = |d: &str| d.starts_with(NOT_RUN) || d.starts_with(NOT_RUN_HERE);
    let names = |pick: &dyn Fn(&str) -> bool| -> Vec<&str> {
        outcomes
            .iter()
            .filter_map(|(name, result)| result.as_ref().err().map(|d| (name, d)))
            .filter(|(_, detail)| pick(detail))
            .map(|(name, _)| name.trim_start_matches("violation:"))
            .collect()
    };
    let not_run = names(&|d| not_run_mark(d));
    let missed = names(&|d| !not_run_mark(d));
    let caught = outcomes.len() - not_run.len() - missed.len();
    if !missed.is_empty() {
        return Err(format!(
            "selftest: a deliberate violation was not caught \
             ({caught} caught, {} not run, {} not caught: {})",
            not_run.len(),
            missed.len(),
            missed.join(", ")
        ));
    }
    if not_run.is_empty() {
        return Ok(format!("selftest: {caught} cases ok"));
    }
    let counts = format!(
        "selftest: {caught} caught, {} not run ({})",
        not_run.len(),
        not_run.join(", ")
    );
    if strict {
        Err(format!(
            "{counts}; with CI or ENG_REQUIRE_GPU=1 set, a case not run fails"
        ))
    } else {
        let it = if not_run.len() == 1 { "it" } else { "them" };
        Ok(format!(
            "{counts}; not run is not a pass: CI runs {it}, and CI=1 or \
             ENG_REQUIRE_GPU=1 makes this fail"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{NOT_RUN, NOT_RUN_HERE, judged, last_line, outcome, status, strict_from};
    use crate::util::Done;

    /// In a job, a case not run here is pending (it fails no job and passes
    /// no check); a case that fails a strict run is never pending.
    #[test]
    fn only_a_warning_becomes_pending() {
        let warned = format!("{NOT_RUN_HERE} golden-edit could not run here");
        assert_eq!(judged(Err(warned.clone())), Ok(Done::Pending(warned)));
        let strict = format!("{NOT_RUN} golden-edit could not run here");
        assert_eq!(judged(Err(strict.clone())), Err(strict));
        let miss = "clippy accepted the violation".to_string();
        assert_eq!(judged(Err(miss.clone())), Err(miss));
        assert_eq!(
            judged(Ok("rejected".into())),
            Ok(Done::Pass("rejected".into()))
        );
    }

    /// Strict as GPU tests are: on CI, or with `ENG_REQUIRE_GPU=1`.
    #[test]
    fn a_run_is_strict_on_ci_or_when_a_gpu_is_required() {
        // `util::on_ci`: CI set to anything, even empty (AGENTS.md).
        assert!(!strict_from(false, None));
        assert!(strict_from(true, None));
        assert!(strict_from(false, Some("1")));
        assert!(!strict_from(false, Some("0")));
        assert!(!strict_from(false, Some("")));
    }

    #[test]
    fn could_not_run_is_a_warning_only_outside_a_strict_run() {
        let not_run = || {
            Err(format!(
                "{NOT_RUN} golden-edit could not run here: needs the pinned lavapipe"
            ))
        };
        let warned = outcome(not_run(), false).unwrap_err();
        assert_eq!(
            warned,
            format!("{NOT_RUN_HERE} golden-edit could not run here: needs the pinned lavapipe")
        );
        assert_eq!(status(&Err(warned)), "not run");
        let strict = outcome(not_run(), true);
        assert_eq!(strict, not_run());
        assert_eq!(status(&strict), "FAILED");
        // A miss stays a miss, and a catch a catch.
        let miss = || Err("clippy accepted the violation".to_string());
        assert_eq!(outcome(miss(), false), miss());
        assert_eq!(status(&outcome(miss(), false)), "FAILED");
        assert_eq!(status(&outcome(Ok("rejected".into()), false)), "ok");
    }

    fn case(name: &str, result: Result<&str, &str>) -> (String, Result<String, String>) {
        let result = result.map(str::to_string).map_err(str::to_string);
        (format!("violation:{name}"), result)
    }

    #[test]
    fn the_last_line_counts_what_was_caught_and_what_was_not_run() {
        let caught = case("float-sin", Ok("rejected"));
        assert_eq!(
            last_line(std::slice::from_ref(&caught), false).unwrap(),
            "selftest: 1 cases ok"
        );
        let warned = format!("{NOT_RUN_HERE} no adapter");
        let not_run = case("golden-edit", Err(&warned));
        let both = [caught.clone(), not_run.clone()];
        assert_eq!(
            last_line(&both, false).unwrap(),
            "selftest: 1 caught, 1 not run (golden-edit); not run is not a pass: \
             CI runs it, and CI=1 or ENG_REQUIRE_GPU=1 makes this fail"
        );
        let failed = format!("{NOT_RUN} no adapter");
        let strict = [caught.clone(), case("golden-edit", Err(&failed))];
        assert_eq!(
            last_line(&strict, true).unwrap_err(),
            "selftest: 1 caught, 1 not run (golden-edit); \
             with CI or ENG_REQUIRE_GPU=1 set, a case not run fails"
        );
        let missed = case("float-min", Err("clippy accepted the violation"));
        assert_eq!(
            last_line(&[caught, not_run, missed], false).unwrap_err(),
            "selftest: a deliberate violation was not caught \
             (1 caught, 1 not run, 1 not caught: float-min)"
        );
    }
}
