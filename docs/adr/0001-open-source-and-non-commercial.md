# ADR-0001: Open source and non-commercial

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D5, D20, G2 (spec sections 3, 5, 8.1, 10.4; resolves P3)
- **Who:** Owner decided (2026-10-24)

## Context

The first plan was to pay for the project by releasing games at each level. The owner then read that Anthropic's EU consumer terms do not allow commercial use. The owner appears to be in the EU.

- Those terms cover Free, Pro and other individual plans (effective 2025-10-08).
- They say: "You agree that you will not use our Services for any commercial or business purposes".
- The agents run on the owner's current consumer plan.

The owner replied: "ok, I get it. we will not sell anything. we will build the open source engine and test it with personal games that will not be selled".

## Decision

- **Not for sale (D5, G2).** The engine is open source. The games are personal test games and are never sold.
- **Repos (D20, P3).**
  - The engine and its agent tooling live in a public repo under MIT OR Apache-2.0.
  - Each personal game lives in a private repo.
  - The engine repo keeps a public testbed game with test levels.
- **What this rules out:** selling games, paid access or paid builds, and any other income from the project.
- **What it allows:** the open-source engine, personal games, and free builds for friends.
- **Before anything is ever sold:** agent work moves to commercial terms (Team, Enterprise or API). Only then does the v2 business plan come back (stores, signing, selling entity, revenue gates).
- Stores, code signing, notarization, revenue and a selling entity are out of Level 1.

## Consequences

- Public repos get free standard runners on every target we need, including macOS, ARM64 and native RISC-V.
- Branch protection and CODEOWNERS on a private repo need a paid GitHub plan. A private game repo is therefore optional (P11); the default stays private, per P3.
- Costs are the owner's own: the current Claude plan, free public CI, optionally a paid GitHub plan and test hardware. AI usage is logged per rung in `PROGRESS.md`.
- Dependency licences are limited to the SPDX allow-list in `deny.toml`. Never GPL or AGPL.
- Taking money for builds would make the project commercial. Drifting into commercial use is an anti-goal and a tracked risk.
- No platform-services contract exists in Level 1, since there is no store (6.12).

## Revisit when

If a game ever needs to be sold. Before that happens, agent work moves to commercial terms and this ADR is superseded by a new one.
