//! `cargo xtask docs-check` (spec 7.4): AGENTS.md stays at most 150 lines,
//! no CLAUDE.md is committed beyond a pointer to AGENTS.md, and CODEOWNERS
//! gives the owner every protected path.

use std::fs;
use std::path::Path;

use crate::guard;

pub const AGENTS_MAX_LINES: usize = 150;

pub fn check(root: &Path) -> Result<String, String> {
    let mut errors = Vec::new();
    let agents = fs::read_to_string(root.join("AGENTS.md"))
        .map_err(|e| format!("[E-DOCS] AGENTS.md: {e}"))?;
    let lines = agents.lines().count();
    if lines > AGENTS_MAX_LINES {
        errors.push(format!(
            "[E-DOCS] AGENTS.md has {lines} lines (max {AGENTS_MAX_LINES})"
        ));
    }
    if let Ok(claude) = fs::read_to_string(root.join("CLAUDE.md"))
        && claude.trim() != "@AGENTS.md"
    {
        errors.push(
            "[E-DOCS] CLAUDE.md may only contain `@AGENTS.md`; put instructions in AGENTS.md"
                .into(),
        );
    }
    errors.extend(codeowners(root)?);
    if errors.is_empty() {
        Ok(format!(
            "docs-check: AGENTS.md {lines}/{AGENTS_MAX_LINES} lines; CODEOWNERS covers protected.toml"
        ))
    } else {
        Err(errors.join("\n"))
    }
}

/// The CODEOWNERS (gitignore-style) form of a protected glob.
pub fn codeowners_pattern(glob: &str) -> String {
    let base = glob
        .strip_suffix("/**")
        .map(|d| format!("{d}/"))
        .unwrap_or_else(|| glob.to_string());
    if base.starts_with("**/") {
        base
    } else {
        format!("/{base}")
    }
}

fn codeowners(root: &Path) -> Result<Vec<String>, String> {
    let rules = guard::protected()?;
    let text = fs::read_to_string(root.join(".github/CODEOWNERS")).unwrap_or_default();
    let owner = format!("@{}", rules.owner_github);
    let owned: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let pattern = parts.next()?;
            parts.any(|o| o == owner).then_some(pattern)
        })
        .collect();
    Ok(rules
        .paths
        .iter()
        .map(|p| codeowners_pattern(p))
        .filter(|p| !owned.contains(&p.as_str()))
        .map(|p| format!("[E-DOCS] .github/CODEOWNERS needs `{p} {owner}` (protected.toml)"))
        .collect())
}
