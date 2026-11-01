use super::deny_verdict;

/// cargo-deny 0.20.2's lines for `raw-window-handle` in eng-render.
const DENIED: &str = "\
warning[unmatched-wrapper]: direct parent 'eng-render = 0.1.0' of banned crate 'raw-window-handle = 0.6.2' was not marked as a wrapper
error[banned]: crate 'raw-window-handle = 0.6.2' is explicitly banned
bans FAILED";

#[test]
fn the_ban_must_name_the_crate_and_its_user() {
    let ok = deny_verdict(false, DENIED, "raw-window-handle", "eng-render");
    assert_eq!(
        ok.unwrap(),
        "rejected: error[banned] raw-window-handle, used by eng-render"
    );
    let accepted = deny_verdict(true, "bans ok", "raw-window-handle", "eng-render");
    assert!(accepted.unwrap_err().contains("cargo-deny accepted"));
    // Refused, but for another crate or another user: not this case's rule.
    for (banned, parent) in [("wgpu", "eng-render"), ("raw-window-handle", "eng-app")] {
        let other = deny_verdict(false, DENIED, banned, parent).unwrap_err();
        assert!(other.contains("but not with error[banned]"), "{other}");
    }
}
