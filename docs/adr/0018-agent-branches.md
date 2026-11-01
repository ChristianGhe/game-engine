# ADR-0018: Agents push to agent branches

- **Status:** Accepted
- **Date:** 2026-11-01
- **Decision:** amends ADR-0015 ("Agents never push or change GitHub");
  brings back part of spec 7.5 ("push to non-default branches")
- **Who:** Owner decided ("we need to allow the agent to push", "at least
  on a separate branch"); Claude wrote the rule, and a fresh reviewer
  agent checked it twice

## Context

Since ADR-0015 only the owner pushes, so every CI run waits for the
owner. Rung 1b's next steps need many CI runs: the native jobs on Windows
and macOS have never run, and only CI has those machines. Agents working
overnight could not see a single result until the morning.

The first draft taught the guard to allow `git push` to `agent/*`
branches. The reviewer broke it: an agent could move the ref the check
compared against, git could send a different commit from the one checked
(tags, branches named like commit ids, a race), and the guard's shell
parser has known gaps that hide a command completely. The `settings.json`
deny on `git push` still caught the forms that start with `git push`; the
draft removed it.

## Decision

- Agents push with `cargo xtask push <name>`, which:
  - reads GitHub's `main` with `git ls-remote`, outside the repository
    and without git's settings, so no `insteadOf` can answer for it, and
    fetches that commit if it is missing;
  - refuses when `HEAD`'s commit differs from it in a protected path or
    in the protected tables of the root `Cargo.toml` (one that changed
    and cannot be read counts as different);
  - sends the commit it checked, as `<id>^0`, which git cannot take for
    the name of a branch or tag, to `refs/heads/agent/<name>` on
    `https://github.com/ChristianGhe/game-engine.git` (a constant in
    `xtask/src/push.rs`), with `--no-follow-tags` and no force. GitHub
    refuses a commit that does not descend from the branch;
  - runs every git call with an empty environment plus a short list
    (`PATH`, `HOME`, the locale, `TMPDIR`, `SSH_AUTH_SOCK` and what Windows
    needs), so a `GIT_*` variable on the cargo command never reaches git.
  - `--dry-run` asks GitHub without changing it.
- `git push` itself stays denied to agents: `settings.json` keeps its deny
  and the guard keeps refusing it, now naming `cargo xtask push`. So do
  `jj git push` and every `gh` write. `main` and tags stay the owner's.
- Under the open-pulsar loop the loop keeps version control, and agents
  push nothing: git's `HEAD` there is jj's parent change, not the task's
  work.
- CI runs on pushes to `agent/**` as on `main`. Agent branches read
  `main`'s build caches but save none, so they cannot push `main`'s out of
  the repository's 10 GB.
- The owner still moves `main`: once an agent branch is green, `git push
  origin main` sends the same commits.

## Consequences

- Agents see CI without the owner: `cargo xtask push <name>`, then `gh run
  list --branch agent/<name>` and `gh run view`, which are reads.
- Branches pile up, because agents cannot delete them. The owner deletes
  old ones with `git push origin --delete agent/<name>`.
- This stops accidents and the obvious forms, like the guard (ADR-0015).
  It does not stop an agent set on pushing: agents run as the owner's OS
  user, `PATH` and `HOME` still come from the caller, the parser's gaps
  hide commands, and `git credential fill` hands over the owner's token.
- A branch with a changed workflow would run that workflow on GitHub with
  whatever token permissions it asks for, and such a token could update
  `main`. `cargo xtask push` refuses such a branch. Against the
  determined case the owner should add a GitHub ruleset on `main` and on
  tags that lets only repository admins update them (a workflow's token
  is not an admin). It does not exist yet.
- The parser gaps the reviewer found (process substitution, a program
  name in a variable, functions, `coproc`, `env -S`, wrappers such as
  `caffeinate`, `find -exec`, `xargs`, `git subtree`, and `<<` read as a
  heredoc) predate this ADR. Closing them is separate work.

## Revisit when

- Agents get their own GitHub identity and macOS user (ADR-0015's real
  fixes). A token that may push `agent/*` only, without the workflow
  permission, moves this rule to GitHub's side.
- Pull requests return (ADR-0014).
