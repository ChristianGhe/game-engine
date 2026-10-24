//! Path globs for protected paths: `*` matches within one segment, `**`
//! matches any number of segments (including none). Paths are
//! repo-relative with forward slashes. Also the shell's own expansion of
//! globs and braces, as the guard follows it.

use std::fs;
use std::path::{Path, PathBuf};

pub fn matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match_segments(&pat, &segs)
}

fn match_segments(pat: &[&str], segs: &[&str]) -> bool {
    match (pat.first(), segs.first()) {
        (None, None) => true,
        (Some(&"**"), _) => {
            match_segments(&pat[1..], segs) || (!segs.is_empty() && match_segments(pat, &segs[1..]))
        }
        (Some(p), Some(s)) => {
            match_segment(p.as_bytes(), s.as_bytes()) && match_segments(&pat[1..], &segs[1..])
        }
        _ => false,
    }
}

fn match_segment(p: &[u8], s: &[u8]) -> bool {
    match (p.first(), s.first()) {
        (None, None) => true,
        (Some(b'*'), _) => {
            match_segment(&p[1..], s) || (!s.is_empty() && match_segment(p, &s[1..]))
        }
        (Some(b'?'), Some(_)) => match_segment(&p[1..], &s[1..]),
        (Some(a), Some(b)) => a == b && match_segment(&p[1..], &s[1..]),
        _ => false,
    }
}

/// Paths under `base` matching a `*`/`?` glob, at most a few thousand.
pub fn expand_paths(base: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut found = vec![PathBuf::new()];
    for segment in pattern.split('/').filter(|s| !s.is_empty() && *s != ".") {
        let mut next = Vec::new();
        for prefix in &found {
            if !segment.contains(['*', '?']) {
                next.push(prefix.join(segment));
                continue;
            }
            let Ok(entries) = fs::read_dir(base.join(prefix)) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if matches(segment, &name) {
                    next.push(prefix.join(name));
                }
            }
        }
        next.truncate(4096);
        found = next;
    }
    found
        .into_iter()
        .filter(|p| base.join(p).exists())
        .collect()
}

/// `x{1,2}.toml` as `x1.toml` and `x2.toml`; `None` without a comma list.
pub fn braces(text: &str) -> Option<Vec<String>> {
    let open = text.find('{')?;
    let close = open + text[open..].find('}')?;
    let inner = &text[open + 1..close];
    if !inner.contains(',') || inner.contains('{') {
        return None;
    }
    let (before, after) = (&text[..open], &text[close + 1..]);
    Some(
        inner
            .split(',')
            .map(|alt| format!("{before}{alt}{after}"))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn globs() {
        assert!(matches("deny.toml", "deny.toml"));
        assert!(!matches("deny.toml", "crates/deny.toml"));
        assert!(matches("xtask/**", "xtask/src/main.rs"));
        assert!(matches("xtask/**", "xtask"));
        assert!(matches(
            "**/*.assert.toml",
            "crates/game-testbed/scenarios/a.assert.toml"
        ));
        assert!(matches("**/*.assert.toml", "x.assert.toml"));
        assert!(!matches("**/*.assert.toml", "x.input.toml"));
        assert!(matches(
            "docs/rungs/*/acceptance.json",
            "docs/rungs/1a/acceptance.json"
        ));
        assert!(!matches(
            "docs/rungs/*/acceptance.json",
            "docs/rungs/1a/x/acceptance.json"
        ));
        assert!(matches(
            "crates/*/tests/golden.rs",
            "crates/eng-core/tests/golden.rs"
        ));
    }
}
