//! Path globs for protected paths: `*` matches within one segment, `**`
//! matches any number of segments (including none). Paths are
//! repo-relative with forward slashes.

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
