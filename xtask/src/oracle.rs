//! `cargo xtask oracle-check --base REV --head REV` (spec 7.3, rule 3): on a
//! pull request, changes to protected paths pass only if the owner approved
//! the PR's head commit, or the owner applied the `oracle-update` label.
//!
//! It must run as the *base* commit's code, with the base's protected list
//! compiled in (`oracle.yml` uses `pull_request_target`): the head is only
//! read as git objects, never built or run, so a PR cannot loosen its own check.

use std::fs;
use std::process::Command;

use serde_json::Value;

use crate::{guard, util};

pub const LABEL: &str = "oracle-update";

fn git(root: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(root).args(args);
    util::run_quiet(root, "git", &mut cmd)
}

fn gh_json(root: &std::path::Path, endpoint: &str) -> Result<Value, String> {
    let mut cmd = Command::new("gh");
    cmd.current_dir(root)
        .args(["api", "--paginate", "--slurp", endpoint]);
    let text = util::run_quiet(root, "gh-api", &mut cmd)?;
    let pages: Value =
        serde_json::from_str(&text).map_err(|e| format!("gh api {endpoint}: {e}"))?;
    Ok(Value::Array(
        pages
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|p| p.as_array().cloned().unwrap_or_default())
            .collect(),
    ))
}

pub fn check(base: &str, head: &str) -> Result<String, String> {
    let root = util::root()?;
    let rules = guard::protected()?;
    // --no-renames: a rename lists both the old (protected) and new names.
    let changed = git(
        &root,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            // NUL-separated: otherwise git quotes non-ASCII names.
            "-z",
            &format!("{base}...{head}"),
        ],
    )?;
    let mut hits: Vec<String> = changed
        .split('\0')
        .filter(|path| rules.matches(path))
        .map(String::from)
        .collect();
    let old_manifest = git(&root, &["show", &format!("{base}:Cargo.toml")]).unwrap_or_default();
    let new_manifest = git(&root, &["show", &format!("{head}:Cargo.toml")]).unwrap_or_default();
    let tables = |text: &str| -> Vec<Option<toml::Value>> {
        let table: toml::Table = text.parse().unwrap_or_default();
        rules
            .cargo_toml_tables
            .iter()
            .map(|dotted| {
                dotted
                    .split('.')
                    .try_fold(toml::Value::Table(table.clone()), |v, k| v.get(k).cloned())
            })
            .collect()
    };
    if tables(&old_manifest) != tables(&new_manifest) {
        hits.push(format!(
            "Cargo.toml [{}]",
            rules.cargo_toml_tables.join("], [")
        ));
    }
    if hits.is_empty() {
        return Ok("oracle-check: no protected paths changed".into());
    }
    let listing = hits.join("\n  ");
    let event: Value = std::env::var("GITHUB_EVENT_PATH")
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let pr = &event["pull_request"];
    let (Some(number), Ok(repo)) = (pr["number"].as_u64(), std::env::var("GITHUB_REPOSITORY"))
    else {
        return Ok(format!(
            "oracle-check: not a pull request; protected changes:\n  {listing}"
        ));
    };
    let owner = &rules.owner_github;
    let head = pr["head"]["sha"].as_str().unwrap_or_default();
    let reviews = gh_json(&root, &format!("repos/{repo}/pulls/{number}/reviews"))?;
    let approved = reviews.as_array().into_iter().flatten().any(|r| {
        r["user"]["login"] == owner.as_str() && r["state"] == "APPROVED" && r["commit_id"] == head
    });
    let labelled_now = pr["labels"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|l| l["name"] == LABEL);
    // The label counts only if the owner applied it last and nothing was
    // pushed after it.
    let timeline = gh_json(&root, &format!("repos/{repo}/issues/{number}/timeline"))?;
    let mut labelled_by_owner = false;
    for event in timeline.as_array().into_iter().flatten() {
        match event["event"].as_str().unwrap_or_default() {
            "labeled" if event["label"]["name"] == LABEL => {
                labelled_by_owner = event["actor"]["login"] == owner.as_str();
            }
            "unlabeled" if event["label"]["name"] == LABEL => labelled_by_owner = false,
            "committed" | "head_ref_force_pushed" => labelled_by_owner = false,
            _ => {}
        }
    }
    if approved || (labelled_now && labelled_by_owner) {
        Ok(format!(
            "oracle-check: protected changes accepted by {owner}:\n  {listing}"
        ))
    } else {
        Err(format!(
            "[E-ORACLE] this PR changes protected paths:\n  {listing}\nIt needs {owner}'s approval of commit {head}, \
             or the `{LABEL}` label applied by {owner}."
        ))
    }
}
