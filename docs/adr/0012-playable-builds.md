# ADR-0012: Playable builds

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D16 (spec section 8.4)
- **Who:** Claude's call

## Context

Nothing is sold (ADR-0001), so there is no store pipeline, no code signing and no notarization. Each milestone game still needs builds that the owner and invited playtesters can run for free. Builds start at rung 7.

## Decision

Builds for macOS arm64, Windows x64 and Linux x64, built locally or by CI. No stores.

- **macOS arm64:** a `.app` bundle, ad-hoc signed. A CI-built app needs `xattr -dr com.apple.quarantine <app>` before it opens. Apps built on the owner's Mac do not.
- **Windows x64:** `+crt-static`, the Windows subsystem, icon and version resources. Unsigned, so SmartScreen warns on first run and Smart App Control blocks it. Playtesters get written steps.
- **Linux x64:** a tarball built against an old glibc baseline.
- Builds come only from `v*` tags in the game repo. Only the owner creates tags.
- Stripped binaries. Symbols and `version.json` archived per build.
- Third-party notices come from cargo-about.

Other targets (RISC-V, Windows ARM64, Linux ARM64, Intel Mac) keep compiling in CI but are not packaged.

## Consequences

- Playtesters see OS warnings and need written steps (a tracked risk).
- No signing keys or store accounts to manage.
- Giving builds to friends for free is allowed. Taking money is not.
- macOS builds need `NSLocalNetworkUsageDescription`; rung 5 confirms the Local Network prompt still appears for an ad-hoc signed app.

## Revisit when

If a game ever needs to be sold (then signing, notarization and stores return, per ADR-0001). Otherwise no trigger planned; change only by a new ADR.

## Rung 2's early builds (Proposed, for the owner; T20 found, 2026-11-15)

- **Status:** Proposed (for the owner; written by T22). This ADR stands:
  rung 2's `cargo xtask dist` is a build folder that CI keeps as an
  artifact (plan difference 5, decision 17, taken at G1), not a release.
  No tags, signing or upload. What follows records what T20 found while
  building it, where it departs from decision 17's wording, and two
  defaults for the owner.
- **Stripping and debug info**, set per target with `--config
  profile.release.*` on cargo's command line, so no `[profile]` table
  changes; `debug = "limited"` (line tables and functions) symbolizes a
  backtrace:
  - macOS: `split-debuginfo = "packed"`, `strip = "symbols"`: a `.dSYM`
    beside the binary;
  - Windows: `split-debuginfo = "packed"` and `strip = "none"`. Any
    `strip` value drops the PDB too, and the linker never puts debug info
    in the `.exe`;
  - Linux: `split-debuginfo = "off"`, `strip = "none"`, then
    `objcopy --only-keep-debug` and `objcopy --strip-all
    --add-gnu-debuglink`. Split DWARF leaves the line tables in the
    binary, so a stripped binary could not use the `.dwp`. Decision 17
    said stripping comes from `--config` alone; this step is the
    departure.
- **cargo-about** 0.9.3 must be installed with `--features cli`; without
  it, `cargo install` installs no binary.
- **The notices** list every crate in the testbed's normal dependency
  graph on the three playable targets, under deny.toml's licence list
  (164 crates, five licences at T20). Two defaults for the owner:
  - they end with a section on Rust's standard library and its licence
    texts, which every binary contains and Cargo's tools cannot see;
  - about 24 crates ship no licence file, so their MIT text keeps the
    template's unfilled `<year> <copyright holders>` line. The owner
    accepts this, or adds per-crate clarifications later.
- **Not yet run:** Windows, and x86_64 Linux (linked with rust-lld, where
  GNU objcopy is untried); CI's first run after G5 shows them.
