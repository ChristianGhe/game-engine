# ADR-0014: Direct commits to main

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** changes the workflow in spec 7.3 and 7.6 (D14, D15)
- **Who:** Owner decided

## Context

The spec has agents work on branches, open pull requests, and merge one
at a time after CI and a reviewer agent. Protected paths would change only
with the owner's approval, enforced by CI's `oracle` check on each PR and
by a separate GitHub identity for agents.

When the repo was created, the owner said there is "no need for pr": the
owner and their agents are the only ones committing. Until an agent
identity exists, every commit carries
the owner's GitHub account anyway, so the owner cannot approve a PR that
agents push.

## Decision

- Commits go straight to `main`. There are no pull requests.
- Before each commit, `cargo xtask quick` is green; for anything
  non-trivial, the `reviewer` subagent checks the diff against the rung's
  acceptance list.
- CI runs every job on every push to `main`. A red `main` is fixed first.
- The local Claude Code guard is the gate for protected paths. The
  `oracle` workflow stays in the repo but runs only on pull requests.

## Consequences

- Faster: no PR overhead for a team of one owner and their agents.
- Weaker: a protected change that slips past the guard lands on `main`
  without the owner's approval; CI cannot stop it after the push. Review
  of `git log -- <protected paths>` replaces it.
- `.claude/settings.json` still denies agents `git push origin main`; the
  owner pushes, or changes that rule.

## Revisit when

A second human contributes, an agent gets its own GitHub identity, or a
protected change lands on `main` without the owner noticing.
