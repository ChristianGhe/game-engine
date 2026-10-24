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
