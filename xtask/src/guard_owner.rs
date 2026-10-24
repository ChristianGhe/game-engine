//! The guard's checks for what only the owner does: push, change GitHub,
//! and write jj history (ADR-0015). Agents may read all three.

use crate::guard::Decision;
use crate::guard_bash::Check;
use crate::shell::Word;

pub(crate) const OWNER_ONLY: &str = "agents never push or change GitHub; the owner does (ADR-0015)";

/// jj commands that only read (`jj` alone runs `jj log`).
const JJ_READ: [&str; 12] = [
    "log",
    "status",
    "st",
    "diff",
    "show",
    "evolog",
    "obslog",
    "interdiff",
    "root",
    "help",
    "version",
    "cat",
];
/// jj global options that take a value.
const JJ_VALUE: [&str; 5] = ["-R", "--repository", "--at-op", "--at-operation", "--color"];
/// jj options and variables that bring in settings or tools, which can run
/// other programs (a pager, a diff tool).
const JJ_SETTINGS: [&str; 4] = ["--config", "--config-toml", "--config-file", "--tool"];
const JJ_VARS: [&str; 3] = ["JJ_CONFIG", "HOME", "XDG_CONFIG_HOME"];

/// gh commands that only read, with their reading subcommands (empty: all).
/// `download` subcommands are left out: they write files.
const GH_READ: [(&str, &[&str]); 18] = [
    ("run", &["list", "view", "watch"]),
    ("pr", &["list", "view", "status", "diff", "checks"]),
    ("issue", &["list", "view", "status"]),
    ("repo", &["view", "list"]),
    ("release", &["list", "view"]),
    ("workflow", &["list", "view"]),
    ("ruleset", &["list", "view", "check"]),
    ("cache", &["list"]),
    ("label", &["list"]),
    ("secret", &["list"]),
    ("variable", &["list", "get"]),
    ("gist", &["list", "view"]),
    ("auth", &["status"]),
    ("search", &[]),
    ("status", &[]),
    ("browse", &[]),
    ("help", &[]),
    ("version", &[]),
];

impl Check<'_> {
    /// jj: only commands that read, without settings or tools of their own.
    /// The loop that runs agents keeps the jj history itself.
    pub(crate) fn jj(&self, assigns: &[&Word], args: &[&Word]) -> Option<Decision> {
        let var = assigns
            .iter()
            .filter_map(|w| w.text.split_once('=').map(|(k, _)| k))
            .chain(self.vars.keys().map(String::as_str))
            .find(|k| JJ_VARS.contains(k));
        let option = args.iter().map(|w| w.text.as_str()).find(|t| {
            JJ_SETTINGS
                .iter()
                .any(|s| t == s || t.strip_prefix(s).is_some_and(|r| r.starts_with('=')))
        });
        if let Some(what) = var.or(option) {
            return Some(Decision::Block(format!(
                "jj with {what}: jj settings and tools can run other programs; the owner sets them"
            )));
        }
        let mut i = 0;
        while i < args.len() && args[i].text.starts_with('-') {
            i += if JJ_VALUE.contains(&args[i].text.as_str()) {
                2
            } else {
                1
            };
        }
        let word = |k: usize| args.get(k).map(|w| w.text.as_str());
        let (sub, action) = (word(i), word(i + 1));
        let reads = match sub {
            None => true,
            Some(s) if JJ_READ.contains(&s) => true,
            Some("file") => matches!(action, Some("show" | "list" | "annotate" | "search")),
            Some("op" | "operation") => matches!(action, Some("log" | "show" | "diff")),
            Some("config") => matches!(action, Some("get" | "list" | "path")),
            Some("bookmark" | "b" | "branch" | "tag" | "workspace") => {
                matches!(action, Some("list" | "l"))
            }
            Some("git") => action == Some("remote") && word(i + 2) == Some("list"),
            Some("resolve") => args[i..]
                .iter()
                .any(|w| matches!(w.text.as_str(), "-l" | "--list")),
            Some(_) => false,
        };
        let what = args[i.min(args.len())..]
            .iter()
            .take(2)
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let why = if action == Some("push") {
            OWNER_ONLY
        } else {
            "the loop that runs agents keeps the jj history; agents only read it (ADR-0015)"
        };
        (!reads).then(|| Decision::Block(format!("jj {what}: {why}")))
    }

    /// gh: only commands that read.
    pub(crate) fn gh(&self, args: &[&Word]) -> Option<Decision> {
        let mut words = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let t = args[i].text.as_str();
            if matches!(t, "-R" | "--repo") {
                i += 1;
            } else if !t.starts_with('-') {
                words.push(t);
            }
            i += 1;
        }
        let (command, sub) = (words.first().copied(), words.get(1).copied());
        let reads = match command {
            None => true,
            Some("api") => api_reads(args),
            Some(c) => GH_READ.iter().any(|(name, subs)| {
                *name == c && (subs.is_empty() || sub.is_some_and(|s| subs.contains(&s)))
            }),
        };
        let what = words.iter().take(2).copied().collect::<Vec<_>>().join(" ");
        (!reads).then(|| Decision::Block(format!("gh {what}: {OWNER_ONLY}")))
    }
}

/// `gh api` reads with GET: the default, unless fields or input are sent.
fn api_reads(args: &[&Word]) -> bool {
    let mut method = None;
    let mut fields = false;
    let mut i = 0;
    while i < args.len() {
        let t = args[i].text.as_str();
        match t {
            "-X" | "--method" => {
                method = args.get(i + 1).map(|w| w.text.to_ascii_uppercase());
                i += 1;
            }
            "-f" | "-F" | "--field" | "--raw-field" | "--input" => fields = true,
            _ if t.starts_with("--method=") => {
                method = Some(t["--method=".len()..].to_ascii_uppercase());
            }
            _ if t.starts_with("-X") => method = Some(t[2..].to_ascii_uppercase()),
            _ if ["--field=", "--raw-field=", "--input="]
                .iter()
                .any(|p| t.starts_with(p))
                || (t.len() > 2 && (t.starts_with("-f") || t.starts_with("-F"))) =>
            {
                fields = true;
            }
            _ => {}
        }
        i += 1;
    }
    match method.as_deref() {
        Some(m) => m == "GET",
        None => !fields,
    }
}
