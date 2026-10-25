//! Unit tests for the version stamp: the parsers, and both renderings
//! for builds that one environment cannot produce.

use super::*;

#[test]
fn seconds_parse() {
    assert_eq!(parse_seconds("0"), 0);
    assert_eq!(parse_seconds("1761350400"), 1_761_350_400);
    assert_eq!(parse_seconds("18446744073709551615"), u64::MAX);
}

#[test]
#[should_panic(expected = "whole Unix seconds")]
fn seconds_refuse_text() {
    parse_seconds("2026-10-25");
}

#[test]
#[should_panic(expected = "too large")]
fn seconds_refuse_overflow() {
    parse_seconds("18446744073709551616");
}

#[test]
fn short_sha_keeps_12_characters() {
    assert_eq!(
        short_sha("0123456789abcdef0123456789abcdef01234567"),
        "0123456789ab"
    );
    assert_eq!(short_sha(UNKNOWN), UNKNOWN);
    assert_eq!(short_sha(""), "");
}

#[test]
fn known_treats_empty_as_unknown() {
    assert_eq!(known(None), UNKNOWN);
    assert_eq!(known(Some("")), UNKNOWN);
    assert_eq!(known(Some("abc")), "abc");
}

#[test]
fn build_facts_come_from_build_rs() {
    assert!(TARGET.contains('-'), "a target triple: {TARGET}");
    assert!(PROFILE == "debug" || PROFILE == "release", "{PROFILE}");
    assert!(RUSTC_VERSION.starts_with("rustc "), "{RUSTC_VERSION}");
}

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const PROTOCOL: u64 = 0x69fd_4996_880a_6508;

/// A `cargo xtask` build: every value known.
fn xtask_build() -> Build<'static> {
    Build {
        game_version: "0.2.0",
        engine_version: "0.1.0",
        engine_sha: SHA,
        game_sha: SHA,
        dirty: Some(false),
        source_date_epoch: Some(1_761_350_400),
        ci_run: "github-18999/1",
        target: "x86_64-unknown-linux-gnu",
        rustc: "rustc 1.99.1 (58cf8f9c3 2026-10-13)",
        profile: "release",
        debug_assertions: false,
    }
}

/// A plain `cargo build`: only what the build itself knows.
fn plain_build() -> Build<'static> {
    Build {
        engine_sha: UNKNOWN,
        game_sha: UNKNOWN,
        dirty: None,
        source_date_epoch: None,
        ci_run: UNKNOWN,
        profile: "debug",
        debug_assertions: true,
        ..xtask_build()
    }
}

#[test]
fn build_json_with_every_value_known() {
    assert_eq!(
        xtask_build().to_json().to_compact(),
        concat!(
            r#"{"game_version":"0.2.0","engine_version":"0.1.0","#,
            r#""engine_sha":"0123456789abcdef0123456789abcdef01234567","#,
            r#""game_sha":"0123456789abcdef0123456789abcdef01234567","#,
            r#""dirty":false,"source_date_epoch":1761350400,"#,
            r#""ci_run":"github-18999/1","target":"x86_64-unknown-linux-gnu","#,
            r#""rustc":"rustc 1.99.1 (58cf8f9c3 2026-10-13)","#,
            r#""profile":"release","debug_assertions":false}"#,
        )
    );
}

#[test]
fn build_json_says_unknown_and_null_for_a_plain_build() {
    assert_eq!(
        plain_build().to_json().to_compact(),
        concat!(
            r#"{"game_version":"0.2.0","engine_version":"0.1.0","#,
            r#""engine_sha":"unknown","game_sha":"unknown","#,
            r#""dirty":null,"source_date_epoch":null,"ci_run":"unknown","#,
            r#""target":"x86_64-unknown-linux-gnu","#,
            r#""rustc":"rustc 1.99.1 (58cf8f9c3 2026-10-13)","#,
            r#""profile":"debug","debug_assertions":true}"#,
        )
    );
}

#[test]
fn line_for_each_kind_of_build() {
    let line = |b: Build<'_>| b.line("game-x", PROTOCOL);
    let tail = "x86_64-unknown-linux-gnu release, rustc 1.99.1, \
                sim protocol 0x69fd4996880a6508";
    // Clean: no dirty part.
    assert_eq!(
        line(xtask_build()),
        format!("game-x 0.2.0 (engine 0.1.0 0123456789ab) {tail}")
    );
    assert_eq!(
        line(Build {
            dirty: Some(true),
            ..xtask_build()
        }),
        format!("game-x 0.2.0 (engine 0.1.0 0123456789ab, dirty) {tail}")
    );
    // A game in its own repository shows its own commit.
    assert_eq!(
        line(Build {
            game_sha: "fedcba9876543210fedcba9876543210fedcba98",
            ..xtask_build()
        }),
        format!("game-x 0.2.0 (engine 0.1.0 0123456789ab, game fedcba987654) {tail}")
    );
    assert_eq!(
        line(plain_build()),
        "game-x 0.2.0 (engine 0.1.0 unknown, dirty unknown) \
         x86_64-unknown-linux-gnu debug, rustc 1.99.1, sim protocol 0x69fd4996880a6508"
    );
    // A compiler that could not be asked.
    assert!(
        line(Build {
            rustc: UNKNOWN,
            ..xtask_build()
        })
        .contains(", rustc unknown, "),
    );
    // The protocol is always 16 hex digits.
    assert!(
        xtask_build()
            .line("g", 1)
            .ends_with("sim protocol 0x0000000000000001")
    );
}
