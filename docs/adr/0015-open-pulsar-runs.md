# ADR-0015: Unattended runs through open-pulsar

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** changes the Claude Code setup in spec 7.3 and 7.5 (D14)
- **Who:** Owner decided to run the work through open-pulsar and asked for
  whatever it needs; Claude made the guard changes

## Context

The owner wants agents to run through open-pulsar, the owner's own task
runner (`ChristianGhe/open-pulsar`, script `agent-loop.sh`). It feeds a
markdown task list to `claude -p --dangerously-skip-permissions`, one
session per group of tasks, and records each task as a Jujutsu (jj)
change, which it abandons when the task fails.

A test on 2026-10-24 with exactly those flags showed that `AGENTS.md`
still loads, and that deny rules and `PreToolUse` hooks in
`.claude/settings.json` still block. The flag removes every permission
prompt and Claude Code's own check on risky actions. With direct commits
to `main` (ADR-0014), that leaves the guard as the only local gate.

Checking the guard found gaps, each verified with a write to a protected
file:

- **Shell syntax it misread:** here-strings; arithmetic shifts; `$'...'`;
  line continuations; brace lists; and `$( )` inside double quotes that
  also hold an apostrophe. This one is easy to hit by accident.
- **Programs it did not know:** `curl -o`, `wget`, `sort -o`, archive
  extraction, and scripts run by path or piped into a shell.
- **Things only the owner should do:** pushes and GitHub writes; any jj
  command; `git revert` of an approval commit; and git settings, variables
  or hooks that make git run another program.

## Decision

- **Agents never push or change GitHub.** The owner pushes, with
  `! git push` from the prompt or from a terminal. `settings.json` denies
  `git push` and `jj git push`. The guard refuses `git push`, `send-pack`
  and `http-push`, `jj git push`, and every `gh` command except reads
  (`gh run view`, `gh api` with GET and the like; downloads write files,
  so they are refused).
- **History may only bring `main`'s version of a protected file.** The
  guard asks git which files `revert`, `cherry-pick`, `reset`, `checkout`
  or `switch` to a commit, `merge`, `rebase`, `pull` and `stash pop` or
  `apply` would change. It refuses when a protected file would end up
  different from both now and `main`. Only the owner moves `main`;
  `git fetch` into `main` is refused too.
- **git runs nothing the agent sets up.** These are refused:
  - `-c`, `--config-env` and `git config` writes, except for a short list
    of harmless settings (the user's name and email, colours, `pull.rebase`,
    a pager or editor of `cat` or `true`, and similar);
  - `GIT_*` variables other than author and committer details, plus `HOME`
    and `XDG_CONFIG_HOME`;
  - `--git-dir`, `--work-tree` and `--exec-path=`;
  - `difftool`, `mergetool` and `filter-branch`.

  `.git/hooks/`, `.git/config` and `.jj/repo/config.toml` are protected.
  The commands given to `rebase --exec`, `bisect run` and
  `submodule foreach` are checked like any other.
- **The loop owns jj.** Agents may only read jj (`jj log`, `diff`,
  `show`, `status`, `op log` and the like), without `--config` or
  `--tool`. Where `AGENTS.md` says to commit, an agent under the loop
  commits nothing, as the loop's system prompt says.
- **The parser reads shell as bash and zsh do.** Claude Code's Bash tool
  runs the user's login shell, which is zsh on the owner's Mac. That
  covers:
  - here-strings and arithmetic;
  - quotes, including `$'...'` and `$( )` inside double quotes;
  - line continuations, comments and brace lists;
  - unterminated heredocs, and heredocs inside `$( )`;
  - the commands inside `<( )`, `>( )` and zsh's `=( )`;
  - zsh's `>!` and `>>!` redirects, `=cmd`, and the `noglob`, `nocorrect`
    and `repeat` prefixes.

  Heredoc text that is data (not code, not a git message) is also read as
  commands, looking only for named protected paths, so a parser slip
  cannot hide a command.
- **Settings outside the repo are protected too.** The guard refuses writes
  to files through which an agent could switch it off or make a tool run
  its programs:
  - Claude Code's user settings, `.claude.json`, plugins and hooks (in
    `$CLAUDE_CONFIG_DIR` and `~/.claude`);
  - git's and jj's user config (`~/.gitconfig`, `~/.config/git`,
    `~/.config/jj`, where jj also keeps each repo's config);
  - shell start-up files (`~/.zshrc`, `~/.zshenv`, `~/.bashrc`,
    `~/.profile` and the like).
- **A second line puts back what the first missed.** Before each allowed
  tool call, the `PreToolUse` hook records the protected files and
  `main`'s commit. For each file it keeps the content (stored with
  `git hash-object -w`), whether it is executable, or that it is absent.
  After the call, whether it worked or failed (`PostToolUse` and
  `PostToolUseFailure`, both running `hook verify`), the hook compares.
  - Any protected file the call changed, created or deleted is put back,
    whatever did it: a command the parser misread, a test, a build
    script. What the call wrote is kept under
    `target/guard-quarantine/<time>-<call>/`, and the message tells the
    agent.
  - A file that now matches its version on `main`, as `main` stood when
    the call began, is left alone, so a call that also commits approves
    nothing. The version it replaced is kept in the same folder under
    `replaced/`, in case it was the owner's uncommitted work.
  - Links are never followed. A link put in place of a protected file or
    one of its folders is removed, and the file is written back in the
    repo.
  - It watches the protected files git lists (tracked, or new and not
    ignored), the unlock file, git's hooks, and
    `.claude/settings.local.json`. In that last file, changes to the
    permission rules alone are kept: Claude Code writes them when the
    owner answers a prompt with "don't ask again", and they cannot
    switch the guard off.
  - The records live in `.git/eng-hooks/snapshots/`, beside the guard
    binary. Nothing is recorded while `.claude/guard-unlock` exists.

  The second line covers files in the repo only. It cannot undo a push, a
  GitHub change or anything sent over the network, so the first line
  still refuses code it cannot see.
- **Code the guard cannot see is refused.** That covers code piped into a
  shell or interpreter, and a script that the same command writes before
  running it. A script run by its path is checked with its `#!`
  interpreter. Downloads (`curl -o` and `-O`, `wget`) and
  `sort -o` are checked for their output file. Extracting an archive
  inside the repo is refused.
- `PreToolUse` and the second line also match MCP tools (`mcp__.*`). The
  guard checks their `file_path`, `notebook_path` and `path` arguments,
  unless the tool's name says it only reads (`read_file`, `list_dir`,
  `search_*` and the like).
- jj ignores git's repo-local email. A repository-scoped setting in the
  owner's jj config makes jj commits here use the GitHub no-reply address
  too.
- The loop's state (`.agent-loop/`) is git-ignored.

## Running it

```sh
~/work/pythonProjects/agent-loop/agent-loop.sh \
  --dir ~/work/projectResources/game-engine \
  --task-timeout-sec 3600 --max-cost-usd <cap> docs/rungs/<rung>/tasks.md
```

Use the task runner only, not open-pulsar's Telegram or "thinker" shell,
which can call other model providers; the AI budget is the Claude plan
(P1). Each task group counts as one agent session against the rung's
budget, and the loop records each task's cost.

To publish a run, the owner reviews it (`jj log -r 'main..@'`, then
`jj diff -r <change>`), and runs:

```sh
jj new                            # a fresh empty change above the last task
jj abandon 'main..@- & empty()'   # drop empty changes: jj's start-up one, read-only tasks
jj bookmark set main -r @-        # main moves to the last task
git push origin main
```

Each task becomes one commit named after its task text; rename one with
`jj describe -r <change>` first if that makes a poor message. jj leaves
git on a detached HEAD; `git switch main` returns to plain git.

## Consequences

- Spec 7.5's "push to non-default branches" and the `gh pr` permission
  are withdrawn. An agent cannot push even when asked; the owner runs
  the push.
- Taking an older version of a protected file is the owner's job, or
  needs the owner's unlock. So is naming a protected file in
  `git checkout <commit> -- <path>` or `git restore`, even for `main`'s
  version.
- Some legitimate commands are refused: running code from a pipe,
  extracting archives in the repo, unusual git settings, and heredoc data
  with a line that reads as a write to a protected path. The parser also
  errs towards refusing when it cannot tell what a word expands to. The
  refusal says what to do instead.
- While an agent's tool call runs, the second line also puts back an edit
  the owner makes to a protected file at the same moment, and deletes an
  unlock file created then; the owner's version is in the quarantine
  folder. The owner should edit protected files, or write
  `.claude/guard-unlock`, while no agent is working.
- A commit the call made keeps what it committed: the second line puts
  the file back, not the commit. `git status` then shows the protected
  file as changed against `HEAD`. Before pushing, the owner should check
  that `git status` is clean and read the agent's commits.
- What the second line does not watch, so only the first line guards:
  - `.git/config`, since git writes it itself for allowed commands
    (`git config user.email`, a branch's upstream);
  - the guard's own binary and its build folders (`.git/eng-hooks/`,
    `target/hooks-*`); `install-hooks` rebuilds those from `origin/main`;
  - git's hooks seen from a linked worktree.
- Two tool calls running at once can confuse it: one call's record can
  catch the other's write, then put that write back after the other call
  removed it. That needs two calls touching the same protected file at
  the same moment, which is rare.
- Still beyond the guard: the second line puts back the protected files
  these change, but not what they do elsewhere.
  - git plumbing (`read-tree`, `update-index --index-info`, `commit-tree`),
    which can change what is committed;
  - programs that run code from files the guard does not read: `make`,
    `cargo run` and tests, build scripts, `npm` scripts;
  - git aliases and hooks already in the owner's global config;
  - code that builds a protected path, or a push, at run time;
  - a background process a call starts, which can write after the call
    ends (the next call then records its change as the starting point);
  - MCP tools that act outside the repo;
  - calls to GitHub's API made with the owner's credentials by other
    means.

  The guard runs as the owner's macOS user, so a determined agent can
  still defeat it. The real fixes are a separate macOS user for agents
  (P7) and an agent GitHub identity without push rights.
- Talk items for the owner, since they change open-pulsar itself:
  `agent-loop.sh` could pass `--strict-mcp-config` (its `thinker_query.py`
  already does), and agents under the loop can write the loop's own
  source folder.
