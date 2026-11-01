use serde_json::{Value, json};

use super::{evaluate, status};

/// A report from `job` on `target` whose steps are `(name, ok)`.
fn report(job: &str, target: &str, steps: &[(&str, bool)]) -> Value {
    let steps: Vec<Value> = steps
        .iter()
        .map(|(name, ok)| json!({ "name": name, "ok": ok }))
        .collect();
    json!({ "report_version": 1, "job": job, "target": target, "steps": steps })
}

fn ci_feature(id: &str, step: &str) -> Value {
    json!({ "id": id, "checks": [{ "job": "lint", "step": step }] })
}

/// Criterion 16 as the owner wrote it in rung 1b's acceptance.json.
fn owner_feature(id: &str) -> Value {
    json!({ "id": id, "owner": true, "command": "cargo run -p game-testbed -- play", "checks": [] })
}

fn feature<'a>(doc: &'a Value, id: &str) -> &'a Value {
    doc["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == id)
        .unwrap()
}

#[test]
fn a_ci_feature_passes_only_when_every_report_has_its_step_ok() {
    let mut doc =
        json!({ "features": [ci_feature("lint-tier", "fmt"), ci_feature("docs", "docs-check")] });
    let reports = [report("lint", "x", &[("fmt", true), ("docs-check", false)])];
    let (lines, totals) = evaluate(&mut doc, &reports).unwrap();
    assert_eq!(feature(&doc, "lint-tier")["passes"], true);
    assert_eq!(feature(&doc, "docs")["passes"], false);
    assert_eq!(
        lines,
        [
            "PASS lint-tier",
            "FAIL docs: lint/docs-check failed on \"x\""
        ]
    );
    assert_eq!(totals, "1/2 features pass");
}

#[test]
fn an_owner_feature_is_listed_and_never_computed() {
    let mut doc =
        json!({ "features": [ci_feature("lint-tier", "fmt"), owner_feature("owner-play")] });
    let reports = [report("lint", "x", &[("fmt", true)])];
    let (lines, totals) = evaluate(&mut doc, &reports).unwrap();
    let owner = feature(&doc, "owner-play");
    // CI writes no verdict: `passes` is null, never true or false.
    assert!(owner.get("passes").is_some_and(Value::is_null), "{owner}");
    assert!(owner.get("why_not").is_none(), "{owner}");
    assert_eq!(
        lines,
        [
            "PASS lint-tier",
            "OWNER owner-play: the owner checks this feature; CI does not compute it"
        ]
    );
    // The owner's feature is not among the computed ones, passing or failing.
    assert_eq!(
        totals,
        "1/1 computed features pass; 1 for the owner: owner-play"
    );
}

#[test]
fn an_owner_feature_that_lists_checks_is_refused() {
    let mut owner = owner_feature("owner-play");
    owner["checks"] = json!([{ "job": "lint", "step": "fmt" }]);
    let mut doc = json!({ "features": [owner] });
    let err = evaluate(&mut doc, &[report("lint", "x", &[("fmt", true)])]).unwrap_err();
    assert!(
        err.contains("owner-play") && err.contains("lists checks"),
        "{err}"
    );
}

#[test]
fn an_owner_feature_whose_checks_are_not_a_list_is_refused() {
    for checks in [json!({ "job": "lint", "step": "fmt" }), json!("fmt")] {
        let mut owner = owner_feature("owner-play");
        owner["checks"] = checks;
        let mut doc = json!({ "features": [owner] });
        let err = evaluate(&mut doc, &[]).unwrap_err();
        assert!(
            err.contains("owner-play") && err.contains("checks"),
            "{err}"
        );
    }
    // Leaving `checks` out is as good as an empty list.
    let mut owner = owner_feature("owner-play");
    owner.as_object_mut().unwrap().remove("checks");
    let mut doc = json!({ "features": [owner] });
    assert!(evaluate(&mut doc, &[]).is_ok());
}

#[test]
fn owner_must_be_true_or_false() {
    let mut owner = owner_feature("owner-play");
    owner["owner"] = json!("yes");
    let mut doc = json!({ "features": [owner] });
    let err = evaluate(&mut doc, &[]).unwrap_err();
    assert!(
        err.contains("owner-play") && err.contains("true or false"),
        "{err}"
    );
    // `false` is an ordinary feature: with no checks it fails.
    let mut doc = json!({ "features": [{ "id": "f", "owner": false, "checks": [] }] });
    let (lines, totals) = evaluate(&mut doc, &[]).unwrap();
    assert_eq!(lines, ["FAIL f: no checks listed"]);
    assert_eq!(totals, "0/1 features pass");
}

#[test]
fn passes_written_by_hand_is_refused_on_owner_features_too() {
    let mut owner = owner_feature("owner-play");
    owner["passes"] = json!(true);
    let mut doc = json!({ "features": [owner] });
    let err = evaluate(&mut doc, &[]).unwrap_err();
    assert!(err.contains("by hand"), "{err}");
}

/// `pending` is CI's too: written by hand, a failing feature would show as
/// pending (review of the integration round).
#[test]
fn pending_written_by_hand_is_refused() {
    let mut feature = ci_feature("lint-tier", "fmt");
    feature["pending"] = json!(true);
    let mut doc = json!({ "features": [feature] });
    let err = evaluate(&mut doc, &[]).unwrap_err();
    assert!(
        err.contains("`pending`") && err.contains("by hand"),
        "{err}"
    );
}

#[test]
fn the_session_status_counts_owner_features_apart() {
    let computed = json!({ "features": [
        { "id": "a", "passes": true },
        { "id": "b", "passes": false },
        { "id": "c", "passes": false, "pending": true },
        { "id": "owner-play", "owner": true, "passes": null },
    ] });
    assert_eq!(
        status(&computed),
        "1/3 pass; failing: b; pending: c; for the owner: owner-play"
    );
    let done = json!({ "features": [{ "id": "a", "passes": true }] });
    assert_eq!(status(&done), "1/1 pass");
}

/// A report whose one step is pending (patch X2's `util::Done::Pending`):
/// not ok, and failing no job.
fn pending_report(job: &str, target: &str, step: &str, detail: &str) -> Value {
    let step = json!({ "name": step, "ok": false, "pending": true, "detail": detail });
    json!({ "report_version": 1, "job": job, "target": target, "steps": [step] })
}

#[test]
fn a_pending_step_shows_as_pending_and_never_passes() {
    let check = json!({ "job": "native", "target": "x", "step": "shot-exact" });
    let mut doc = json!({ "features": [{ "id": "shot-exact", "checks": [check] }] });
    let detail = "pending, not a pass, until the goldens are committed (G6)\nmore";
    let reports = [pending_report("native", "x", "shot-exact", detail)];
    let (lines, totals) = evaluate(&mut doc, &reports).unwrap();
    let feature = feature(&doc, "shot-exact");
    assert_eq!(feature["passes"], false);
    assert_eq!(feature["pending"], true);
    assert_eq!(
        lines,
        [
            "PENDING shot-exact: native/shot-exact is pending on \"x\": \
          pending, not a pass, until the goldens are committed (G6)"
        ]
    );
    assert!(!lines[0].to_lowercase().contains("fail"), "{}", lines[0]);
    assert_eq!(totals, "0/1 features pass, 1 pending");
}

#[test]
fn a_failed_check_outranks_a_pending_one() {
    let pending = json!({ "job": "native", "target": "x", "step": "shot-exact" });
    let failed = json!({ "job": "lint", "step": "fmt" });
    let mut doc = json!({ "features": [{ "id": "f", "checks": [pending, failed] }] });
    let reports = [
        pending_report("native", "x", "shot-exact", "no golden yet"),
        report("lint", "x", &[("fmt", false)]),
    ];
    let (lines, totals) = evaluate(&mut doc, &reports).unwrap();
    assert_eq!(lines, ["FAIL f: lint/fmt failed on \"x\""]);
    assert!(feature(&doc, "f").get("pending").is_none());
    assert_eq!(totals, "0/1 features pass");
}
