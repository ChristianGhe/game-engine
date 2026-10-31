# Rung 1a progress (append-only)

## 2026-10-24, session 1 (Claude Opus 5.5, main session plus one ADR subagent)

**Done**
- Repo initialized (`git init`, branch `main`, no commits yet). Toolchain
  1.99.1 pinned and installed.
- All rung 1a crates, the testbed game, `xtask`, CI workflows, rules files,
  ADRs 0001-0013, `AGENTS.md`, `docs/ARCHITECTURE.md`, this rung's plan and
  acceptance list. See `plan.md`.
- `eng-math` known-answer baseline computed on aarch64-apple-darwin and
  confirmed on Linux x86-64, Linux arm64 and RISC-V (QEMU).
- First committed hash streams (`cargo xtask replay --regen`), confirmed
  identical on the same four targets.

**Problems met**
- Writing `.claude/settings.json` mid-session activated its fail-closed
  guard before the guard binary existed, blocking every Bash and edit call.
  The owner installed the binary and created `.claude/guard-unlock` (`*`)
  for this bootstrap session. Fixed for the future: `install-hooks` now
  builds from the working tree when `main` does not exist yet.
- The harness refused writing `.claude/skills/add-crate/SKILL.md`; its
  steps are in `docs/ARCHITECTURE.md`.
- `syn` is at 3.x; `eng-derive` uses it (serde_derive does too, so one
  version in the tree).

**Timings**
- Docker CI jobs: test on Linux arm64 48 s, test on emulated Linux x86-64
  136 s, riscv under QEMU 62 s (cold caches, including the toolchain
  download).
- Local `ci --job test` on the M2 Pro: 7 s warm.

**Costs**
- AI: one interactive session on the owner's plan; token use was not
  measured inside the session (see `/cost` in Claude Code).
- CI minutes: none yet (no GitHub repo).

**Next**
- Owner actions in `plan.md`. Then the first PR, the first real CI run, and
  recording the measured PR-tier duration here (done-when 5).

## 2026-10-24, session 1 continued: review round

The `reviewer` subagent (fresh context) reviewed the whole tree. Verdict:
changes needed; the simulation core held up. Fixed:

- **Blockers.** `oracle-check` ran the PR's own code; it now runs from the
  base commit under `pull_request_target` and reads the head as git objects
  only. Renames hid protected files (`--no-renames` now). Harnesses and the
  scenario manifest were unprotected, and dropping a manifest line retired
  an assertion silently (now protected, and replay refuses unreferenced
  assertion files). Core crates could `extern crate std`, use a crate-level
  `clippy.toml`, or silence lints through groups or multi-line attributes
  (now `forbid` plus a token scan, build-script, `[patch]` and libm pin
  checks, and a complete std float ban list). The guard could be bypassed
  with `cd`, heredocs, variables, globs, hard links, `git checkout .`, and
  by writing its own unlock file or binary (all now caught and self-tested).
- **Should-fix.** A failed `Sim::restore` left a half-decoded world (now
  all-or-nothing); untrusted snapshots could force huge allocations (now
  refused before allocating); `replay verify` passed a recording with no
  hashes (now invalid); `BoxRoom` could report a blocked trace with
  fraction 1.0 (now always below 1); CI had no single required check (new
  `ci` job); `Authored::migrate` exposed `toml::Table` (now `JsonValue`).
- **Nits.** Huge `--ticks` panicked (now exit 64, and scripts stream
  lazily); `--events` did not create its directory; `quick` reported 0 s.

## 2026-10-24, session 1 continued: second review

A second fresh-context `reviewer` pass confirmed the first round's fixes
(the guard and core-purity checks only partly) and found:

- **Blockers.** Unprotected files that override protected ones
  (`.cargo/config`, `rust-toolchain`, a root `.clippy.toml`, and xtask's
  dependencies through the unprotected root `[workspace.dependencies]`).
  Now protected or refused by `arch-check` (`[E-ARCH-OVERRIDE]`), and
  xtask pins its own dependencies. Core-purity bypasses (`[lib] path`, a
  `mod target;` directory, char/raw literals fooling the scanner,
  `cfg_attr(path = ...)`), now caught.
- **Should-fix.** More guard bypasses (`x>file`, quoted targets, fd
  numbers, `cd` after keywords, `git -C`, `bash -ec`, `env`/`timeout`
  wrappers, awk, script files, `cp -l`), now caught; the guard's Bash
  analysis was rewritten (`guard_bash.rs`, `guard_tools.rs`,
  `guard_words.rs`) to follow `sh -c` code, literal variables, globs and
  git's options. Its false positives (crate names in inline code,
  `$TMPDIR`, `mktemp`, `target/*.json`, `git checkout -- .`, read-only
  `find`/`xargs`, commit messages that mention a bless flag) now pass. The
  hook binary is built from `origin/main` when a remote exists. The oracle
  label counts only if applied last, after the last push; `edited`
  re-runs the check on retargeting.
- **Nits.** `git diff -z`; `Sim::restore` refuses snapshots from other
  sessions or with non-finite state, and rolls back.
- **Left to the owner:** MCP write tools are not matched by the guard hook
  (`.claude/settings.json`), and the guard binary can be replaced by an
  agent with the owner's OS user (P7). See `plan.md`, owner action 6.
- The old bootstrap guard blocked two of this session's commands on that
  same false positive (test strings containing a bless flag); those edits
  went through the Edit tool instead. The new guard does not misfire there.


## 2026-10-24, first GitHub Actions run

- Repo published as `ChristianGhe/game-engine` (public). Commits go
  straight to `main` (ADR-0014); the owner pushed the first one, `cbfa4c2`.
- Run 40090932717: lint failed on one typo (a quote in ADR-0014, fixed in
  the next commit); test on ubuntu-24.04 and ubuntu-24.04-arm, riscv,
  wasm, selftest and summary passed.
- **PR-tier duration (done-when 5): 2.6 minutes**, against a 15-minute
  budget (measured by the summary job from the run's start).
- CI-computed acceptance: 11 of 12 features pass, including every
  done-when item; `lint-tier` failed only on the typo.
- Cross-architecture equality on GitHub runners: the 5 hash streams are
  identical on x86-64, arm64 and RISC-V (QEMU), and match the committed
  streams from macOS arm64.

## 2026-10-24, owner approvals

- Second CI run (40091566700, commit `8867967`): every job green; CI
  computed 12 of 12 acceptance features passing; the PR tier took 2.0
  minutes.
- The owner approved `acceptance.json` (criteria, 14-day time-box,
  15-session AI budget) and batch 1 of the crate allow-list (37 crates).
- Rung 1a is done. Next: rung 1b, the windowed dot.

## 2026-10-24, guard hardening for open-pulsar (ADR-0015)

- The owner wants to run the work through open-pulsar (`agent-loop.sh`),
  which drives `claude -p --dangerously-skip-permissions`. A test with
  those flags: `AGENTS.md` loads, and deny rules and hooks still block.
- Found while checking the guard:
  - A here-string (`<<<`) or an arithmetic shift (`$((1<<2))`) let the
    next line past the guard. Verified: a write to `acceptance.json`
    exited 0.
  - An unterminated heredoc panicked it (exit 101; the hook's `|| exit 2`
    still blocked the command).
  - `git revert` of the approvals commit passed, as did `reset` and
    `checkout` to an older commit.
  - Every `git push` passed, including `git -C . push origin main`, as
    did `jj restore` of a protected file and `gh api -X PUT`.
- A fresh-context review found over-blocking. With `main` ahead,
  `git switch main` from an older branch was refused. Two git errors let
  commands through. `gh run download` was on the read-only list. Its
  bypass hunt did not finish, so a second hunt was run by hand, with 32
  candidate commands. It found these, all passing the guard:
  - `$( )` inside double quotes with an apostrophe, e.g.
    `echo "it's $(...)"`, which is easy to hit by accident;
  - `$'...'` quoting and escapes, line continuations, and brace lists;
  - `curl -o`, `wget`, `sort -o`, `tar -x` and `unzip`;
  - scripts run by their path, piped into a shell, or written earlier in
    the same command;
  - git hooks and `.git/config`, which were unprotected;
  - `-c` settings, `GIT_*` variables (including `GIT_EXTERNAL_DIFF`, and
    `GIT_TRACE` set to a file), and `rebase --exec`;
  - `git fetch` into `main`, and jj `--config` or `--tool`.
- Fixed in `xtask/src/shell.rs`, `glob.rs`, `guard.rs`, `guard_bash.rs`
  and `guard_tools.rs`, and in four new modules: `guard_vcs.rs`
  (history), `guard_settings.rs` (git settings and variables),
  `guard_owner.rs` (push, gh, jj) and `guard_code.rs` (shells,
  interpreters, downloads, archives). Each case has unit tests in both
  directions, blocked and allowed. Four new selftest cases bring the total
  to 17. `settings.json` now denies `git push` and `jj git push`, and
  sends MCP tools through the guard.
- After the fixes, all 59 probe commands that should be refused are
  refused. The 9 allowed are ordinary commands, such as `jj log`,
  `gh run list`, `git commit` and merging an ancestor.
- End to end, `claude -p` ran with open-pulsar's flags in a scratch clone
  with the new guard. Blocked: the here-string write, both pushes,
  `git revert` of the approvals commit, and `jj restore deny.toml`. An
  ordinary Write went through.
- Two real open-pulsar runs on scratch clones (Haiku) completed every
  task, with no false blocks and the Stop hook's `quick` running:
  - creating a file, and reading jj: $0.0065, 42 s;
  - adding an `eng-math` unit test and running its tests: $0.013, 79 s.

  Their jj commits carried the no-reply address. The publishing steps
  in ADR-0015 were tried on the first (without pushing): they leave no
  empty commits.
- Test sessions cost $0.047 in all (nine Haiku sessions, counting the
  end-to-end runs below).
- jj commits in this repo use the GitHub no-reply address (a scoped entry
  in the owner's jj config), and `.agent-loop/` is git-ignored.
- A second fresh-context review found more that passed the guard:
  - a heredoc inside `$( )`, and an apostrophe in a comment or heredoc;
  - a quoted assignment before a command (`X="1" cmd`);
  - `rebase --onto` an older commit, `>|`, `curl -sSLo`, and `rm -rf` of
    a folder holding protected files;
  - zsh-only forms (`>!`, `=cmd`, `=( )`, `noglob`). The Bash tool runs
    the user's shell, which is zsh here, not bash;
  - Claude Code, git, jj and shell config files outside the repo.

  It also found ordinary commands refused, among them the usual
  `git commit -m "$(cat <<'EOF' ...)"`, `OUT="/tmp/x"`,
  `./target/debug/...`, `source "$HOME/.cargo/env"` and `node -v`.
- Fixed all of them; config files outside the repo are now refused too.
  `shell.rs`'s substitution scanner moved to `shell_subs.rs` to keep both
  files under 500 lines.
- Added a second line (`guard_snapshot.rs` and `guard_snapshot_files.rs`,
  ADR-0015), so a parser slip no longer decides the outcome. On every
  allowed call, the `PreToolUse` hook records the protected files. After
  the call, a new hook puts back any the call changed, unless it now
  matches `main`, and keeps what was written under
  `target/guard-quarantine/`.
- End to end, `claude -p` with open-pulsar's flags (Haiku) ran
  `make stamp`, a Makefile target that appends to `deny.toml`. The first
  line cannot see inside `make`. The second line put the file back, told
  the agent, and kept the write in quarantine. An ordinary Write in the
  same session was left alone. Cost $0.0048, 52 s.
- A third fresh-context review checked the second line and found:
  - It ran only after calls that succeeded. Verified: a `make` target
    that wrote `deny.toml` and then exited 1 left the change in place.
    Fixed by also running it as a `PostToolUseFailure` hook; the same
    test then put the file back ($0.0033).
  - It read `main` after the call, so a call that wrote a protected file
    and committed it approved its own change. It now uses `main` as it
    stood when the call began.
  - Putting a file back followed links, so a link in place of a
    protected file could make it write outside the repo. Links are now
    removed instead.
  - `.claude/settings.local.json`, which git ignores, was not watched.
    It is now, apart from permission rules, which Claude Code writes
    when the owner says "don't ask again".
  - An error stopped the restore half-way; an unreadable file or a name
    with a newline blocked every call. An approved change could wipe the
    owner's uncommitted work without a copy. All fixed.
  - The ADR said `.jj/repo/config.toml` was protected; now it is. MCP
    tools that only read (`read_file`, `list_dir`) were refused on
    protected paths; now they pass.

  The rest are written down in ADR-0015 as limits: parallel calls,
  commits that keep a put-back change, what the second line does not
  watch.
- Results: `cargo test -p xtask` (30), `quick`, `ci --job lint` and
  `selftest` (17) all green; the probe lists give the intended verdicts.
  With the final build, the end-to-end run of the failing `make` target
  plus an ordinary Write put `deny.toml` back and left the Write alone
  ($0.0041, 11 s).
- Still true: the guard will refuse some ordinary commands, because it
  refuses when it cannot tell what a word expands to; the message says
  what to do instead. ADR-0015 lists what remains beyond it. A separate
  macOS user for agents (P7) is the real fix.
- Built and tested in a scratch clone; the owner applies it as one patch,
  so the guard stays closed throughout.

## 2026-10-31, owner idea parked

- The owner floated an open "OASIS" for the engine (Ready Player One, the
  book; "as compatible as possible") and asked to park it and revisit
  later. Recorded in the spec, section 12; it comes back at each level
  start. No code or rung change.
