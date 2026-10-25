# Rung 1b progress (append-only)

Rung 1b, the windowed dot: see `plan.md`. Each session appends what it
did, costs and timings. Changes to protected paths are written here as
proposals for the owner, never made by agents.

## 2026-10-25, session 1: plan draft (Claude Opus 5.5, open-pulsar task)

**Done**
- Read `AGENTS.md`, `docs/ARCHITECTURE.md`, the 1a plan and progress log,
  `architecture.toml`, `deny.toml`, `budgets.toml`, `protected.toml`,
  ADRs 0002, 0003, 0007 (after the review), 0010-0012, 0014 and 0015, and
  spec sections 6, 8.2, 8.3 and 9.2.
- Wrote `plan.md`: crates, the frame, probe additions, decisions,
  differences from the spec, a draft acceptance list (17 features),
  budgets and time-box, 17 tasks, owner gates G1-G7, risks.
- Worked out allow-list batch 2 from a scratch project outside the repo
  (`wgpu =30.0.1` without `gles`/`webgpu`, `winit =0.30.13` without
  Wayland decorations, `raw-window-handle 0.6.2`, `png 0.18.1`,
  `nv-flip 0.1.2`; toolchain 1.99.1): 132 crates not yet allowed, all
  under licences already on the allow-list (proposal 4).
- No code, no new crates, no protected file touched;
  `docs/rungs/current.txt` stays at 1a; no `acceptance.json`.

**Review**
- One fresh-context `reviewer` pass: no layering blockers; the proposed
  `architecture.toml` entries cover every dependency in the plan. It found
  16 issues, all fixed in the draft:
  - the windowed hash check used frames, not ticks (now 240 ticks,
    seed 1, against `dot-square.seed1.txt`);
  - the golden tasks looped (now T12 harness, T16 candidates after G5,
    with "no golden yet" keeping `main` green until G6);
  - network contracts 3-5 and the protocol version in `Hello` were not
    addressed (now in the crate table and difference 10);
  - G4 missing from three tasks' needs;
  - `golden.rs` for `eng-net-wire` had no owner gate;
  - wasm32 in cargo-deny's targets would need web-only crates;
  - nothing enforced ADR-0016 (now a wrapper ban and a selftest);
  - no mouse or gamepad events in the platform contract;
  - bind-group switches missing from the counters;
  - golden comparison by PNG bytes (now decoded pixels);
  - `crash.json` in `play` had no task;
  - the conditions for unchanged hash streams were unstated (World
    registrations, snapshot within one datagram);
  - the panic hook's crate differed from spec 6.1 without saying so;
  - FLIP's reference was unclear (now the lavapipe golden);
  - patch names clashed with the spec's P1-P11 (now X1-X6);
  - xdotool under xvfb without a window manager, and two unprotected
    harnesses.
- `cargo xtask quick`: all 7 steps green. `typos docs/rungs/1b`: clean.

**Costs and timings**
- One open-pulsar session, about 40 minutes, including the reviewer
  (about 7 minutes and 119k tokens). The main session's token use is not
  visible inside the session (see open-pulsar's log).

**Next**
- The owner's G1-G4 (`plan.md`, "Owner actions"). T1 and T2 need only G1.

## 2026-10-25, session 2: plan revision (Claude Opus 5.5, open-pulsar task)

**Done**
- Rewrote `plan.md` to the queue item's outline. It now has:
  - a "DRAFT FOR OWNER" banner;
  - a tag on every item: *[spec X]*, *[spec X + proposal]* or
    *[proposal]*;
  - new and changed crates in separate tables, each with its proposed
    `architecture.toml` layer, core flag and `same_layer` edges;
  - the acceptance criteria as a numbered list, plus a table mapping
    spec 9.2's done-when items to criteria;
  - a time-box and AI budget labelled as proposals for the owner;
  - "Owner gates": every protected change, by `protected.toml` pattern,
    and the protected paths 1b leaves alone;
  - the empty `## Crate batch 2` placeholder for the next queue item.
- Mapped this window's queue to the rung (`plan.md`, "This window's
  queue"). All three items are 1b, and none needs a gate to start:
  - `crash.json`: T2, criterion 6 (done-when 4);
  - `--version` and `--version --json`: T1, criterion 8;
  - live actions and bindings: T4, new criterion 18 `live-input`, also
    serving 3 and 16 (done-when 2).
- Two plan changes follow from that:
  - T4 no longer waits for `eng-platform-api`. `eng-input` defines the
    plain-data types `Key` and `KeyEvent` now, and T3 moves them into
    the contract.
  - Plain `--version` is added as a proposal (spec 6.13 has only
    `--version --json`).
- Found one protected item the first draft missed. The Intel-Mac
  `cargo check` needs the `x86_64-apple-darwin` target, which
  `rust-toolchain.toml` does not list. The proposal is that X6 installs
  it in the macOS job only, as the RISC-V job does, so the file stays
  unchanged (proposal 11).
- No code, no new crates, no protected file touched; `current.txt` stays
  at 1a; no `acceptance.json`.

**Review**
- One fresh-context `reviewer` pass found 1 blocker, 9 should-fix issues
  and 8 nits. All are fixed in `plan.md`, and proposal 11 records the
  changes to the proposals.
  - **Blocker:** G2 as written turns `main` red, because `arch-check`
    fails on crates listed but absent. G2 now lands in one commit with
    T0's skeleton crates.
  - **Dependencies:** T3 needs G3 (it uses `raw-window-handle`), and T11
    needs T2 and T9.
  - **CI steps:** the `test`-job steps behind criteria 6 and 10 had no
    patch (now X7). Criteria 11 and 18 relied on unnamed, unprotected
    tests (now named steps, and the files join proposal 8).
  - **Completeness:**
    - `.config/nextest.toml` was missing from the protected-paths
      audit, and so were the other names `protected.toml` lists and the
      two existing `golden.rs` files.
    - `exit_codes.rs` constrains more than exit codes: recordings, usage
      errors and the summary length. "This window's queue" now lists it.
  - **Tags:**
    - The shader check is spec 6.7, not a proposal.
    - Exact pixels on Linux with FLIP elsewhere is spec 8.3, and
      physical keys are spec 6.10.
    - `eng-platform-api` follows from spec 6.1's contract pair.
    - Difference 1 now quotes 6.7's own "two-pass" sentence.
  - **Gates:** G6 needs the CI run after G5, so the third sitting is
    G6 with G7, not G5 with G6.
  - **Smaller fixes:** golden candidates go under `target/`, and
    `Snapshot` carries opaque bytes.

**Checks**
- `typos docs/rungs/1b/`: clean.
- `cargo xtask quick`: all 7 steps green, before and after the review
  fixes.

**Costs and timings**
- One open-pulsar session, about 55 minutes, including the reviewer
  (about 7 minutes and 105k tokens). The main session's token use is not
  visible inside the session.

## 2026-10-25, session 3: crate batch 2 scratch project (Claude Opus 5.5, open-pulsar task)

**Done**
- Recorded the repo's `Cargo.toml` and `Cargo.lock` before starting:
  clean against `HEAD`, git blobs `36c2d08` and `7288f53`. Both end
  unchanged, and no crate was added to the repo.
- Created a new scratch project outside the repo with `mktemp -d`:
  `$TMPDIR/eng-1b-batch2.obsitl`. It has its own `[workspace]` table and
  a `rust-toolchain.toml` at 1.99.1, because this machine's default
  toolchain outside the repo is 1.93.1.
- Versions: the latest stable releases on crates.io today are wgpu
  30.0.1 and winit 0.30.13, the ones the plan names. Both are pinned with
  `=`. winit 0.31.0-beta.4 (2026-10-24) is a pre-release and is not used.
- Features (decision 5), with `default-features = false`:
  - wgpu: `std`, `dx12`, `metal`, `vulkan`, `wgsl`. Besides `gles` and
    `webgpu`, this also leaves out the default `parking_lot` feature,
    which decision 5 does not list.
  - winit: `rwh_06`, `x11`, `wayland`, `wayland-dlopen`, which is the
    default set without `wayland-csd-adwaita`.
- The project depends on wgpu and winit only, not on `png`, `nv-flip`
  or `naga`'s translation back-ends.
- `cargo check` passes on 1.99.1 (aarch64-apple-darwin, 13.5 s cold),
  with 236 packages locked. `cargo tree -e features` shows exactly the
  features above.
- Crates on the seven targets of proposal 4: Linux (x64, arm64, RISC-V)
  101 each, Windows 78, macOS 75, and 138 in all. None is a web, Android,
  GL or Wayland-decoration crate. 123 are not yet on `[bans] allow`:
  proposal 4's 132, less `png`, `nv-flip` and their 7 dependencies. No
  crate is new compared with proposal 4.

**Costs and timings**
- One open-pulsar session, about 5 minutes (01:45 to 01:50), most of
  it the cold `cargo check`. No reviewer pass: the only repo change is
  this entry.

**Next**
- The next queue item fills `plan.md`, "Crate batch 2", from this
  project: licences and per-target groups. `$TMPDIR` is temporary; if the
  directory is gone, recreate it from the versions and features above.

## 2026-10-25, session 4: crate batch 2 dependencies and licences (Claude Opus 5.5, open-pulsar task)

**Done**
- In the session 3 scratch project, for `aarch64-apple-darwin`,
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` and
  `x86_64-pc-windows-msvc`: `cargo tree --target <t> -e normal,build`
  and `cargo metadata --filter-platform <t>` (versions and licences).
  As a third view, `cargo deny check bans licenses` (0.20.2) ran in the
  scratch project on a copy of the repo policy with a `[graph] targets`
  line added. All commands ran with `--locked --offline`. The repo's
  `Cargo.toml` and `Cargo.lock` and the scratch lock are unchanged.
- Outputs are in `$TMPDIR/eng-1b-batch2-tree.DW32ue/`. `final.tsv` is
  the list below with each crate's licence, `tree-<target>.txt` the
  trees, and `deny-*.jsonl` cargo-deny's diagnostics.
- Third-party crates, counted by name and version, with build
  dependencies: macOS 80, Linux 107 on each architecture, Windows 80.
  Of these, 65, 90 and 66 are not on `[bans] allow`, 129 across the
  four targets. These counts differ from session 3's because they count
  versions and include build dependencies; by name the totals agree.
- The three views differ:
  - cargo-deny on one target at a time sees exactly what `cargo tree`
    sees.
  - With all four targets in one `[graph] targets` list, cargo-deny
    sees 6 more: 135 crates by version, 126 by name. It tests every
    platform-gated edge against every listed target, so Windows-only
    edges of Linux crates count too (for example `polling` on
    `concurrent-queue`, and `windows-targets` on `windows_x86_64_gnu`,
    whose `cfg` also matches x86-64 Linux). None of these is built, but
    `cargo deny` fails unless they are allowed.
  - `cargo metadata --filter-platform` sees more again: the Vulkan crates
    and five more objc2 crates on macOS, `errno` and `jobserver` on
    Linux, and `serde` everywhere. It merges features across platforms.
    Neither cargo nor cargo-deny uses these, so they need no entry.
  - The seven targets of proposal 4 add one crate,
    `windows_aarch64_msvc 0.52.6` (MIT OR Apache-2.0): 127 names.
    Without `[graph] targets`, as `deny.toml` is today, cargo-deny sees
    184 names (Android, web, Redox and `tokio` crates among them).
- Not on `[bans] allow`, grouped by the targets that build them (135
  crates by version, 126 by name):
  - All four (42): arrayvec 0.7.8, bit-set 0.10.0, bit-vec 0.9.1,
    bytemuck 1.25.2, bytemuck_derive 1.12.1, cfg_aliases 0.2.2,
    codespan-reporting 0.13.1, cursor-icon 1.2.0, document-features
    0.2.12, dpi 0.1.2, foldhash 0.2.0, half 2.7.1, libloading 0.8.9,
    litrs 1.0.1, lock_api 0.4.14, log 0.4.34, naga 30.0.1, naga-types
    30.0.1, once_cell 1.21.4, parking_lot 0.12.5, parking_lot_core
    0.9.12, pin-project-lite 0.2.17, profiling 1.0.18, raw-window-handle
    0.6.2, rustc-hash 1.1.0, scopeguard 1.2.0, smallvec 1.16.4, smol_str
    0.2.2, static_assertions 1.1.0, thiserror 2.0.21, thiserror-impl
    2.0.21, tracing 0.1.44, tracing-core 0.1.36, unicode-width 0.2.2,
    wgpu 30.0.1, wgpu-core 30.0.1, wgpu-hal 30.0.1, wgpu-naga-bridge
    30.0.1, wgpu-types 30.0.1, winit 0.30.13, zerocopy 0.8.63,
    zerocopy-derive 0.8.63.
  - Linux and Windows, for Vulkan (8): allocator-api2 0.2.21, ash
    0.38.0+1.3.281, gpu-allocator 0.28.0, ordered-float 5.5.0, presser
    0.3.1, renderdoc-sys 1.1.0, spirv 0.4.0+sdk-1.4.341.0,
    wgpu-core-deps-windows-linux-android 30.0.1.
  - Linux only, for X11 and Wayland (40): ahash 0.8.12,
    as-raw-xcb-connection 1.0.1, calloop 0.13.0, calloop-wayland-source
    0.3.0, cc 1.10.0, dlib 0.5.3, downcast-rs 1.2.1, find-msvc-tools
    0.1.15, gethostname 1.1.0, linux-raw-sys 0.12.1 and 0.4.15, memmap2
    0.9.12, percent-encoding 2.3.2, pkg-config 0.3.34, polling 3.11.0,
    quick-xml 0.41.0, rustix 0.38.44 and 1.1.5, scoped-tls 1.0.1, shlex
    2.0.1, slab 0.4.12, smithay-client-toolkit 0.19.2, thiserror 1.0.69,
    thiserror-impl 1.0.69, version_check 0.9.5, wayland-backend 0.3.17,
    wayland-client 0.31.15, wayland-csd-frame 0.3.0, wayland-cursor
    0.31.14, wayland-protocols 0.32.13, wayland-protocols-plasma 0.3.12,
    wayland-protocols-wlr 0.3.12, wayland-scanner 0.31.11, wayland-sys
    0.31.11, x11-dl 2.21.0, x11rb 0.13.2, x11rb-protocol 0.13.2, xcursor
    0.3.11, xkbcommon-dl 0.4.2, xkeysym 0.2.1.
  - Windows only (16): range-alloc 0.1.5, unicode-segmentation 1.13.3,
    windows 0.62.2, windows-collections 0.3.2, windows-core 0.62.2,
    windows-future 0.3.2, windows-implement 0.60.2, windows-interface
    0.59.3, windows-link 0.2.1, windows-numerics 0.3.1, windows-result
    0.4.1, windows-strings 0.5.1, windows-sys 0.52.0, windows-targets
    0.52.6, windows-threading 0.2.1, windows_x86_64_msvc 0.52.6.
  - macOS only (23): block2 0.5.1 and 0.6.2, core-foundation 0.9.4,
    core-foundation-sys 0.8.7, core-graphics 0.23.2, core-graphics-types
    0.1.3, dispatch 0.2.0, foreign-types 0.5.0, foreign-types-macros
    0.2.4, foreign-types-shared 0.3.1, objc-sys 0.3.5, objc2 0.5.3 and
    0.6.5, objc2-app-kit 0.2.2, objc2-core-foundation 0.3.2,
    objc2-core-graphics 0.3.2, objc2-encode 4.1.0, objc2-foundation
    0.2.2 and 0.3.2, objc2-metal 0.3.2, objc2-quartz-core 0.3.2,
    raw-window-metal 1.1.0, wgpu-core-deps-apple 30.0.1.
  - Only in cargo-deny's graph when several targets are listed (6):
    concurrent-queue 2.5.0, crossbeam-utils 0.8.23, errno 0.3.14,
    windows-sys 0.59.0 and 0.61.2, windows_x86_64_gnu 0.52.6.
- Licences of these 135. "Either licence" counts the old `MIT/Apache-2.0`
  form too.
  - MIT or Apache-2.0, either licence (81): every crate not named below.
  - MIT only (32): block2 (both), calloop, calloop-wayland-source,
    cfg_aliases, dispatch, dlib, objc-sys, objc2 (both), objc2-app-kit,
    objc2-encode, objc2-foundation (both), ordered-float, quick-xml,
    slab, smithay-client-toolkit, tracing, tracing-core, the nine
    `wayland-*` crates, x11-dl, xcursor, xkbcommon-dl.
  - MIT, Apache-2.0 or Zlib (9): bytemuck, bytemuck_derive, cursor-icon,
    objc2-core-foundation, objc2-core-graphics, objc2-metal,
    objc2-quartz-core, raw-window-handle, xkeysym.
  - Apache-2.0 only (4): codespan-reporting, gethostname, spirv, winit.
  - Apache-2.0 WITH LLVM-exception, Apache-2.0 or MIT (4): rustix and
    linux-raw-sys (both versions of each).
  - BSD-2-Clause, Apache-2.0 or MIT (2): zerocopy, zerocopy-derive.
  - Apache-2.0 AND MIT (1): dpi. Both licences apply.
  - Zlib (1): foldhash. ISC (1): libloading.
- **Licence flags: none.** Every licence ID in the 135 is on
  `[licenses] allow`. Every crate has an SPDX `license` field, so none
  relies on a licence file. cargo-deny reports no licence error with four
  targets, with seven, or with none. No `exceptions` or `clarify` entries
  are needed. The only ID not on the list is `Unlicense`, and it is
  always the other choice in "Unlicense OR MIT", where MIT suffices.
  It appears in `memchr` (allowed in batch 1), and, only without
  `[graph] targets`, in `same-file`, `walkdir` and `winapi-util` (Android
  only, through `jni`). Outside this project, proposal 4's `adler2` (a
  `png` dependency) is "0BSD OR MIT OR Apache-2.0", where MIT suffices.
- Proposal 4 against cargo-deny with its seven targets: its 132 names
  are the 127 needed here, less the 4 below, plus `png`, `nv-flip` and
  their 7 dependencies. With seven targets, cargo-deny also needs
  `concurrent-queue`, `crossbeam-utils`, `errno` and
  `windows_x86_64_gnu` (all MIT or Apache-2.0). That makes 136 names.
  Added as amendment 12 below.
- Duplicate versions (`multiple-versions = "warn"`, so warnings, not
  errors): block2, objc2 and objc2-foundation on macOS (winit 0.30 uses
  objc2 0.5, wgpu-hal uses 0.6), rustix, linux-raw-sys, thiserror and
  thiserror-impl on Linux, and windows-sys in the multi-target graph.
  Batch-1 crates are duplicated too: bitflags 1 and 2 (macOS),
  hashbrown 0.16 and 0.17 (Linux, Windows) and syn 2 and 3 (all four).

**Costs and timings**
- One open-pulsar session, about 8 minutes (01:49 to 01:57). Each
  `cargo tree`, `cargo metadata` and `cargo deny` run took under 2 s;
  nothing was compiled. No reviewer pass: the only repo change is this
  file.
- The guard refuses a Python command when the script text names
  `deny.toml` (or `_typos.toml`), even when it only reads. Copying the
  policy into `$TMPDIR` as `policy.toml` and leaving the name out of the
  script works. Redirections need literal paths, not `$VAR/$1`.

**Next**
- Fill `plan.md`, "Crate batch 2", from the groups above and amendment
  12. The allow list depends on `[graph] targets`, so the plan should
  say that the list and the targets line are one change.

## 2026-10-25, session 5: crate batch 2 table in the plan (Claude Opus 5.5, open-pulsar task)

**Done**
- Filled `plan.md`, "Crate batch 2". It has one row per crate version
  with the version, licence, targets and a flag for licence IDs not on
  `[licenses] allow`, plus notes on the method, counts and duplicates.
  The rest of `plan.md` is unchanged: the diff removes only the
  placeholder line.
- Sessions 3 and 4 had only `wgpu` and `winit`, so this session also
  resolved `png =0.18.1` and `nv-flip =0.1.2`. It used a copy of the
  session 3 project, `$TMPDIR/eng-1b-batch2-png.Y0U6Ur`, so session 3's
  lock is unchanged (`66aa33cf…`). The repo's `Cargo.toml` and
  `Cargo.lock` are unchanged too.
- The copy's lock added 13 packages: `png`, `nv-flip`, `nv-flip-sys`,
  `adler2`, `crc32fast`, `fdeflate`, the deflate crate, `miniz_oxide`
  (0.8.9 and 0.9.1), `simd-adler32` and `zlib-rs`. `zlib-rs` is locked
  but never built (see below).
- In the copy, for all seven targets of proposal 4:
  - `cargo tree --target <t> -e normal,build`;
  - `cargo metadata`;
  - `cargo deny check bans licenses` (0.20.2) with each policy copy
    from session 4.
- Outputs are in `$TMPDIR/eng-1b-batch2-out.LKRlli/`: `table.tsv`,
  `table.md` and `deny-*.jsonl`. The script is
  `$TMPDIR/eng-1b-batch2-scripts5.8GF3RA/table.py`.
- Results:
  - 146 crate versions, 136 names. These are exactly the names of
    proposal 4 with amendment 12, checked by the script.
  - cargo-deny on one target still lists exactly what `cargo tree`
    builds there: macOS 78, Linux 100, Windows 79.
  - Counts by target (versions): all seven 55, Linux 37, macOS 23,
    Windows 17, Linux and Windows 8, cargo-deny graph only 6.
  - Without `[graph] targets`, cargo-deny sees 193 names.
- **Licence flag: one, `adler2`** ("0BSD OR MIT OR Apache-2.0"). 0BSD is
  not on `[licenses] allow`; MIT is, so the crate passes and nothing
  changes. Session 4 found no flag because `png` was not in its project.
  cargo-deny reports no licence error with the seven targets.
- **New finding: locked but never built.** The deflate crate's default
  `runtime_detection` feature names `zlib-rs?/std`. That weak feature
  makes cargo lock `zlib-rs 0.6.8` (Zlib). `cargo tree` (including
  `-e features`) and cargo-deny never include it, so it needs no entry.
  But the comment in the policy file ("only these crates may appear in
  Cargo.lock") would then be wrong. See amendment 13.
- **Guard workarounds:**
  - `cargo deny` takes `--config` before `check`, not after it.
  - The guard refused a Python command that only wrote this file,
    because its text named the typos config. The Edit tool works.
  - typos still flags the deflate crate's bare name (typos-cli 1.51.1),
    so the plan writes it as a crates.io link, as proposal 4 does.
    `typos docs/rungs/1b/` is clean.

**Costs and timings**
- One open-pulsar session, about 10 minutes (01:59 to 02:09). Resolving
  the two new crates took 2 s. Each `cargo tree`, `cargo metadata` and
  `cargo deny` run took under 2 s, and nothing was compiled.
- No reviewer pass: the changes are documentation only, `plan.md` and
  this file, with nothing applied to a protected path.

**Next**
- The queue in "This window's queue" (T1, T2, T4). Crate batch 2 waits
  for the owner at G3, together with `[graph] targets`, the wrapper ban
  and the typos-config entry.

## 2026-10-25, session 6: `crash.json` from the panic hook (T2) (Claude Opus 5.5, open-pulsar task)

**Done**
- `crates/eng-cli/src/exit.rs`: on any panic the hook prints
  `[E-PANIC]` as before, then writes `crash-<game>-<pid>.json` beside the
  agent log (`$ENG_AGENT_LOG_DIR`, else `target/agent-logs/`). It prints
  `crash: <path>` and `log: <path>` and exits 12. This follows the
  plan's `crash.json` item (spec 6.13 + proposal). The fields are:
  - `crash_version` (1) and `game`;
  - `version`: the build stamp that reports already carry
    (`game_version`, `engine_sha`, `game_sha`, `rustc`, `target`,
    `profile`), because T1's full `version.json` stamp does not exist
    yet;
  - `args` (the command line) and `seed`, or `null` before a sim is
    built;
  - `tick`: 0 for the initial state, N while the commands for tick N
    are applied, `null` before a sim exists;
  - `thread` (`name`, `id`);
  - `panic` (`message`, `location`);
  - `backtrace`: one string per line, forced on whatever
    `RUST_BACKTRACE` says, and cut at 400 lines;
  - `last_log_lines`: the last 50 log records, oldest first, ending with
    the `[E-PANIC]` line;
  - `log_file`.
- The hook must not panic or hang, because a panic inside a panic hook
  aborts instead of exiting 12. So it writes to stderr with `writeln!`
  and ignores errors (`eprintln!` panics when stderr is closed), and
  never unwraps. It takes the log and context locks with `try_lock` and
  gives up after about 100 ms (`lock_on_panic`). A thread that panics
  while holding the log lock therefore still exits 12. If the crash
  file cannot be written, it prints `[E-CRASH-JSON] <reason>` and still
  exits 12. Only the first panic writes the file (a static
  `CRASHING` flag). A panic on another thread meanwhile prints its
  `[E-PANIC]` line and waits for the exit, so two threads never
  truncate each other's file. The one known way the hook can still
  abort is `std::thread::current()` for a panic in a thread-local
  destructor. Stable Rust has no safe alternative, and the doc comment
  says so.
- `log.rs` keeps the last 50 records in memory beside the file, even when
  the log file could not be opened. `run_args` logs a first line,
  `start: <game> <args>`, so the tail always shows the command.
  `build_sim` (used by `run`, `dump-world` and `replay verify`) records
  the seed and tick 0, and the `run` and `replay verify` loops record
  each tick.
- New harness `crates/game-testbed/tests/crash_json.rs` (T2's "done
  when"). It runs the binary with `run --seed 7 --panic-at-tick 3` and
  parses the file, checking that it exits 12 and every field's value or
  type. It also checks that the console names the file, that the log
  tail matches the log file, and that the panic still exits 12 with
  `[E-CRASH-JSON]` when the log directory cannot be created. It parses
  with `serde_json`, added as a dev-dependency of `game-testbed` only.
  The crate is already on `deny.toml`'s allow-list and in `Cargo.lock`
  (xtask), so the lock gains one dependency line and no package.
- Unit tests: the 50-line tail, the backtrace cut, and `lock_on_panic`
  giving up on a held lock and using a poisoned one.
- `AGENTS.md` (133 lines) and `docs/ARCHITECTURE.md` say where crash
  files go.
- Kept, as "This window's queue" requires: the protected `exit_codes.rs`
  passes unchanged. No sim code changed, so the hash streams are
  unchanged and `replay` is green.

**Checks**
- `cargo xtask quick` green (8 s); `cargo xtask ci --job lint` green,
  including cargo-deny and typos (2 s).
- Release build by hand: the backtrace has function names
  (`eng_cli::run::run`) but no line numbers. The whole hook took about
  0.5 s, mostly symbolizing.
- Two threads panicking at once, with a throwaway game binary outside
  the repo (`$TMPDIR/eng-crash-race.*`, deleted afterwards). Its build
  function makes two named threads panic on a barrier. Over 8 runs:
  exit 12 every time, both `[E-PANIC]` lines printed, exactly one valid
  crash file naming the thread `worker-1`. It also showed `seed` set
  and `tick` `null` for a panic during the build.
- `reviewer` subagent: approve, nothing blocking. Its own `quick` run
  was green, and `crash_json` passed 3 runs in a row. Exit 12 and the
  crash file held with stderr closed and with a broken pipe. Findings
  and what happened to each:
  1. `version` is 1a's build info, not T1's stamp (medium): recorded
     below and in amendment 14.
  2. Two threads panicking at once could leave a half-written file:
     fixed (`CRASHING`), tested by hand as above.
  3. `thread::current()` can still panic in a thread-local destructor:
     documented on the hook; no stable alternative.
  4. If the log lock stays busy for 100 ms, `log_file` is `null` too:
     left as it is. The lock is held only inside `State::push`, which
     has no reachable panic, and other threads hold it for
     microseconds, which the retries cover.
  5. No test of `replay verify`'s seed and tick, or of `null` before
     the sim exists: no CLI path panics in either case (the scratch
     binary above covers the second by hand). `--panic-at-tick` exists
     only on `run`, and the plan asks only for `run`.
  6. About 11 frames of hook and std noise at the top of the backtrace:
     kept. Trimming by frame name is fragile across targets and
     inlining, and the 400-line cap leaves plenty of room.
  7. A reused pid overwrites an older crash file: the plan names the
     file `crash-<game>-<pid>.json`. Adding the time, as log names
     already do, is an option for the owner at G1.
  - Nit: the tick doc now says "the tick of the latest state".

**Not done (waits for others)**
- `play` (criterion 6's second half) waits for `eng-app` (T11).
- The `version` object becomes T1's full stamp when T1 lands. The
  field name stays, and `crash_version` goes to 2 if the stamp's fields
  change meaning. T1 must also update `crash_json.rs`'s version checks,
  before the file is protected (amendment 14).

**Costs and timings**
- One open-pulsar session, about 40 minutes (02:14 to about 02:55). The
  `reviewer` pass took about 8 minutes and 123k subagent tokens. Builds
  and test runs took seconds each.

**Next**
- T1 (`--version`, `--version --json`) and T4 (live input).

## 2026-10-25, session 7: `crash.json` integration test (T2 follow-up) (Claude Opus 5.5, open-pulsar task)

The task: an integration test that panics through `--panic-at-tick` and
checks `crash.json` and exit 12, and an ARCHITECTURE.md update if it
describes the probe.

**Done**
- Session 6 had already written the test,
  `crates/game-testbed/tests/crash_json.rs`. Its first case runs
  `run --headless --ticks 5 --seed 7 --panic-at-tick 3` and checks exit
  12, every field, the console's `crash:` line, and that the log tail
  matches the log file. Its second case checks exit 12 and
  `[E-CRASH-JSON]` when the log directory cannot be created. No new code
  was needed.
- Checked: `crash_json` passes 2/2, and the protected `exit_codes` passes
  7/7. The crash file's `tick` (3) matches `note_tick`'s documented
  meaning: N from the moment tick N's commands start being applied.
- Mutation check by hand, with the sources restored byte-for-byte
  afterwards. With the hook exiting 13 instead of 12, both cases fail.
  With the seed recorded as `seed ^ 1`, the field case fails on `seed`
  (6 != 7).
- `docs/ARCHITECTURE.md` (Agent tooling, the probe): lists the crash
  file's contents and names the two tests that pin the probe's exit
  codes and crash file.

**Not done**
- The same as session 6: the `play` case waits for T11. Protecting the
  test and adding the `crash-json` CI step wait for the owner
  (amendment 14).

**Costs and timings**
- One short open-pulsar session (about 10 minutes); no subagents.

## 2026-10-25, session 8: `crash.json` test and `quick` re-run (T2 follow-up) (Claude Opus 5.5, open-pulsar task)

The task: run the new test and `cargo xtask quick`, both green, and
record the results and open questions. No code or docs other than this
entry changed. `docs/rungs/current.txt` stays at `1a`, and nothing was
pushed.

**Results** (aarch64-apple-darwin, Rust 1.99.1, `cargo test` because
cargo-nextest is not installed here)
- `cargo test -p game-testbed --test crash_json`: 2/2 ok in 0.24 s.
  `run_panic_writes_crash_json_with_every_field` and
  `panic_exits_12_when_crash_json_cannot_be_written` both pass.
- `cargo xtask quick`: all 7 steps ok in 8 s (fmt, clippy, arch-check,
  size-check, docs-check, tests 6.9 s, replay 0.3 s). The tests step ran
  `cargo test --workspace`. Its log (`target/agent-logs/xtask-tests.log`)
  shows that `crash_json` ran and passed.
- `cargo xtask ci --job test`: all 4 steps ok in 9 s (tests,
  known-answers, exit-codes, replay). The protected `exit_codes` passes
  7/7.
- Is it stable?
  - 20 runs in a row: 20 passed, 0 failed.
  - It also passes with `RUST_BACKTRACE` and `RUST_LIB_BACKTRACE` set to
    `0`, `1` and `full`, so the backtrace check does not depend on the
    environment.
  - It passes with `--test-threads=1`.
  - It passes on the release profile (`cargo test --release`, 0.44 s).
    So the `eng_cli::run::run` frame check survives opt-level 3 here.
- Hash streams unchanged (replay ok), no protected path touched, and
  `jj st` was clean before this entry.

**Open questions**
1. **Linux and nextest are not checked yet.** CI's test job runs on
   `ubuntu-24.04` and `ubuntu-24.04-arm` with cargo-nextest 0.9.148,
   where each test runs in its own process. This Mac has neither. The
   two cases use separate scratch directories
   (`$CARGO_TARGET_TMPDIR/crash-json/{run,unwritable}`), so
   per-process runs should not clash. The first CI run after the owner
   pushes is the real check, and the backtrace frame check is the part
   most likely to differ on Linux.
2. **A named CI step (amendment 14, item 2).** `crash_json` already runs
   in CI inside the workspace `tests` step. A separate `crash-json` step
   after `exit-codes` would only give the failure its own label in the
   report, as `exit-codes` has. Is that label worth a second run of
   0.3 s? I think it is, because `exit_codes` already sets the pattern.
   The owner decides, since `xtask/` is protected.
3. **Two `cargo test` runs at once.** `scratch()` deletes and recreates
   its directory, as `exit_codes.rs` does. Two cargo runs of the same
   test at the same moment (say, `quick` in two terminals) could delete
   each other's files. CI never does this, and the loop runs one task at
   a time. A per-pid subdirectory would fix it if it ever matters, but
   the test should not change before it is protected.
4. **When to protect `crash_json.rs` (amendment 14, item 1).** T1 will
   change the `version` checks. Protecting the file now means the owner
   unlocks it for T1; protecting it after T1 leaves it open to agents
   until then. My suggestion is still after T1.
5. **The `play` half of criterion 6** waits for `eng-app` (T11). The
   test's panic case will need a twin that runs `play`.
6. **Pid reuse** (session 6, item 7): a reused pid overwrites an older
   `crash-<game>-<pid>.json`. The test checks for exactly one crash file
   per fresh scratch directory, so it does not see this. That stays for
   the owner to decide at G1.

**Costs and timings**
- One short open-pulsar session (about 10 minutes); no subagents. Every
  build was already cached except the release one (2.3 s to build).

**Next**
- T1 (`--version`, `--version --json`, plus `crash_json.rs`'s version
  checks), then T4 (live input).

## 2026-10-25, session 9: `--version` and `--version --json` (T1) (Claude Opus 5.5, open-pulsar task)

The task: `--version` (for people) and `--version --json` on the probe,
with spec 6.13's fields plus the crate, protocol and schema versions, the
target triple and the build profile. The JSON must stay stable and be
documented. No new crates and no protected paths.

**Done**
- `crates/eng-cli/src/version.rs` builds the stamp, and `args.rs` parses
  `--version [--json]`. Anything else after `--version`, and a bare
  `--json` or `version`, exits 64 with `[E-USAGE]`. `help` lists it. The
  command opens its log file as every other command does.
- The JSON (`stamp_version` 1) has four parts. They are documented key by
  key in `docs/ARCHITECTURE.md`, "The version stamp":
  - `build`: spec 6.13's `version.json` fields. They are
    `game_version` (the spec's "semver"), `engine_version`, `engine_sha`,
    `game_sha`, `dirty`, `source_date_epoch`, `ci_run`, `target`,
    `rustc`, `profile` and `debug_assertions`.
  - `sim`: `tick_rate`, `protocol`, the resolved `systems` order, and
    `schemas`. `schemas` lists every named type the world encodes, both
    registered and nested, with its version and kind, sorted.
  - `formats`: the versions of the report, crash file, recording, hash
    stream, input script, assertions and budgets files.
  - The line for people, for example: `game-testbed 0.1.0 (engine 0.1.0
    unknown, dirty unknown) aarch64-apple-darwin debug, rustc 1.99.1,
    sim protocol 0x69fd4996880a6508`.
- Where the values come from:
  - New `crates/eng-cli/build.rs`: the target triple (Cargo's `TARGET`),
    the profile (Cargo's `PROFILE`, `debug` or `release`) and
    `rustc -V`. eng-cli is L4 and not core, so `arch-check` allows a
    build script (`[E-ARCH-BUILD]` is for core crates).
  - The source tree, at compile time, through `option_env!`:
    `ENG_ENGINE_SHA`, `ENG_DIRTY` (`0` or `1`), `SOURCE_DATE_EPOCH` and
    `ENG_CI_RUN`. An empty variable counts as unset. A malformed
    `ENG_DIRTY` or `SOURCE_DATE_EPOCH` fails the build with
    `[E-VERSION]`, through const evaluation. CI's workflow already sets
    `ENG_ENGINE_SHA` (`.github/workflows/ci.yml`, top-level `env`), so CI
    builds carry the engine and game SHAs. Nothing sets the other three
    yet (patch X1, amendment 15). Without them, strings read `unknown`
    and `dirty` and `source_date_epoch` read `null`.
  - `ENG_RUSTC_VERSION` is no longer read, because `build.rs` measures
    the compiler.
- The same `build` object is now reports' `build` and `crash.json`'s
  `version`, replacing 1a's `build_info`. It keeps that object's six
  keys and their types, so `report_version` and `crash_version` stay 1.
  Two values change:
  - `target` is now the triple (`aarch64-apple-darwin`, the name CI
    reports already use) instead of `aarch64-macos`.
  - `rustc` is now the real version instead of `unknown`. Recording
    headers therefore carry it too, so `replay verify` refuses a
    recording from another compiler, as the header check intends.
- The hash stream's `v1` header line now comes from a constant
  (`HASH_STREAM_VERSION`), so `formats` cannot drift from it. The bytes
  are unchanged.
- Tests:
  - New `crates/game-testbed/tests/version.rs` runs the binary. It pins
    every key, their order and their types. For the order it reads the
    JSON in document order with a small serde visitor, because
    `serde_json::Value` sorts keys without `preserve_order`. It checks
    that `sim.protocol` equals the committed hash streams' `sim_protocol`,
    that the line matches the JSON, that a report's `build` equals the
    stamp's, and the usage errors.
  - `crash_json.rs`: `version` must now equal `--version --json`'s
    `build`, which replaces the old non-empty checks on five keys.
  - Unit tests: the seconds parser (valid, text, overflow), the short
    SHA, empty-as-unknown, and that `build.rs` facts arrived.
  - `game-testbed` gains `serde` as a dev-dependency. It was already in
    the workspace and on the allow-list, so `Cargo.lock` gains one line
    and no package.
- By hand, with the four variables set (release build):
  `engine 0.1.0 0123456789ab, dirty`, `"dirty": true`,
  `"source_date_epoch": 1761350400`, `"ci_run": "github-18999/1"` and
  `"profile": "release"`. Changing a variable rebuilt eng-cli, because
  rustc tracks `option_env!`. `ENG_DIRTY=yes` and
  `SOURCE_DATE_EPOCH=2026-10-25` each failed to compile with their
  `[E-VERSION]` message.
- Docs: `docs/ARCHITECTURE.md` has the new section, and the probe
  paragraph says what `crash.json`'s `version` is. `AGENTS.md` gains the
  command (134 lines).

**Where this differs from the spec and the plan**
- Spec 6.13 says `cargo xtask` computes `version.json`. Here the build
  itself supplies what it knows best (target, profile, rustc), and xtask
  only has to supply the four source-tree values. X1 can then write
  `version.json` by running `<game> --version --json` (amendment 15), so
  there is one source for it.
- The plan's example line had no engine version, rustc or protocol. The
  line now has them, and it says `unknown` explicitly.
- `profile` is Cargo's base profile, `debug` or `release`. A custom
  profile such as `ci` reports its base. Cargo has no documented way for
  a build script to learn the custom name.

**Checks**
- `cargo xtask quick` green (10 s). `cargo xtask ci --job lint` green,
  including cargo-deny and typos (2 s). `ci --job test` green (9 s),
  with the protected `exit_codes` unchanged at 7/7. `ci --job wasm`
  green. After the review fixes: `quick` green again (12 s), and the
  `lint` and `test` jobs green.
- `replay` green: the hash streams are unchanged. No sim code changed.
- RISC-V under QEMU was not run here. Cargo sets `TARGET` to
  `riscv64gc-unknown-linux-gnu` for that cross build, and the build
  script runs on the host. The reviewer confirmed the same mechanism with
  `--target wasm32-unknown-unknown`.
- `reviewer` subagent: "changes needed (small)", and approve once
  finding 1 is fixed. Its own `quick` run was green. It checked that
  `exit_codes.rs`, `hashes/`, `xtask/`, `protected.toml` and
  `architecture.toml` are byte-identical, and that setting and unsetting
  the variables rebuilds both ways. Findings and what happened to each:
  1. The docs said an empty variable counts as unset, but an empty
     `ENG_DIRTY` or `SOURCE_DATE_EPOCH` failed the build (medium). A
     GitHub `env:` entry with an empty expression would hit this. Fixed:
     empty now means unset for all four. Checked by hand: all four empty
     give `unknown` and `null`.
  2. "Nothing sets these four" was wrong, since CI's workflow sets
     `ENG_ENGINE_SHA` (low). Fixed here, in `ARCHITECTURE.md` and in
     `version.rs`. Amendment 15 now says which value wins.
  3. The `target` and `profile` sources changed without a version bump
     (low). Kept at 1, because nothing reads `build` (xtask only writes
     reports). `ARCHITECTURE.md` now records the pre-T1 form of version
     1.
  4. An undocumented `registered` fallback kind (low). Fixed: the list
     is now built from the world's components, resources and events, so
     only documented kinds can appear.
  5. Recordings and `replay verify` used the raw `build_sha`, but the
     stamp turned `""` into `unknown` (low). Fixed: all three use
     `version::game_sha`. The testbed is unaffected.
  6. Protecting `tests/version.rs` also freezes additive keys, and the
     test's header contradicted the docs (low). Fixed: the header now
     matches, and amendment 15 says what protection implies.
  7. The plan puts protected changes in `docs/rungs/1b/patches/`, but X1
     is prose only (low). Recorded under "Not done".
  8. Nits:
     - "as git abbreviates them" was wrong: fixed.
     - A repeated `--json` said "unknown flag": it now says
       `--json given twice`.
     - `--version` opens a log file and builds the sim, so a broken
       game build exits 12: kept, and noted in amendment 15 for X1.

**Not done (waits for others)**
- X1 as a tested patch file in `docs/rungs/1b/patches/`, as the plan's
  task table describes. It is in prose only (amendment 15), because this
  task asked for proposals in this file. The owner can ask for the patch.
- Criterion 8's "no `unknown` in a CI build" needs X1 and the native
  `version` step (X2, X6).

**Costs and timings**
- One open-pulsar session, about 03:00 to 04:10. The `reviewer` pass
  took about 9 minutes and 145k subagent tokens. Builds and test runs
  took seconds each.

## 2026-10-25, session 10: tests for both `--version` forms (T1 follow-up) (Claude Opus 5.5, open-pulsar task)

The task: tests for both forms, plain and `--json`, including the JSON's
keys and values. Update `crash_json.rs`'s version checks if the shape
changed, but only if that file is not protected. It is not protected
yet, so it was updated.

**Done**
- `crates/eng-cli/src/version.rs`: the build facts are now one `Build`
  struct, and both the JSON `build` object and the line render from it.
  So the two forms cannot disagree, and tests can render builds that
  one environment cannot produce. Output is unchanged: the old
  integration checks passed before the rewrite, and `replay` is green.
- New unit tests (16 in eng-cli, 3 new), now in
  `crates/eng-cli/src/version/tests.rs`. They moved there because
  `version.rs` reached 495 lines; it is 339 now, and the tests still see
  the private `Build`. They check:
  - the exact compact JSON for an xtask-style build (every value known)
    and for a plain build (`unknown` and `null`);
  - the exact line when clean, dirty, dirty unknown, with a game commit
    that differs from the engine's, with an unknown rustc, and with a
    small protocol (zero-padded to 16 hex digits).
- The integration tests are now two files, both under the 500-line
  warning:
  - `crates/game-testbed/tests/version.rs` (4 tests, 446 lines) is the
    format contract, the file to protect later.
  - `version_testbed.rs` (3 tests, 157 lines) holds what changes with
    the testbed's sim.
- `version.rs` checks:
  - `build` equals an expected object built from the same compile-time
    variables the binary saw. The test is compiled in the same cargo run,
    and rustc tracks `option_env!` in both crates.
  - `rustc` equals `rustc -V` of `$RUSTC` if that was set, else of the
    `rustc` beside `env!("CARGO")` (rustup's layout, as on CI), else of
    the `rustc` on `PATH`.
  - `target` must match the running machine's architecture, OS and
    `target_env` (variants such as `gnueabihf` included, and `x86` as
    `i686` or `i586`), with 3 or 4 parts. It cannot be compared with the
    host, because an x86_64 macOS build may run under Rosetta.
  - `sim`: `tick_rate` equals `game_testbed_sim::TICK_RATE`, and
    `protocol` equals the committed hash streams'. The systems and
    schemas are checked for their form: `<Stage> <name>`; sorted by name,
    then version; each schema with `name`, `version` and `kind` in that
    order, and a documented kind.
  - `formats` is pinned exactly, and `hash_stream` is cross-checked
    against the committed `# eng hash stream v1` line.
  - The line equals the one the JSON implies, as documented.
  - Both forms exit 0, write nothing to stderr, and end in exactly one
    newline.
  - Both forms print byte-identical stdout in two runs. The first has
    `ENG_ENGINE_SHA`, `ENG_DIRTY`, `SOURCE_DATE_EPOCH`, `ENG_CI_RUN` and
    `RUST_BACKTRACE` removed. The second has them set at run time, in
    another directory. This checks the docs' "never from the environment
    at run time".
  - The five usage errors each give their exact `[E-USAGE]` message and
    print nothing on stdout.
- `version_testbed.rs` checks:
  - the exact systems and the exact list of 17 schemas;
  - that `dump-world` agrees: the same resources and event channels,
    and every component on an entity at tick 0 is a stamp component;
  - that reports agree: `build`, `sim_protocol` and `report_version`;
  - that a recording agrees. Its container version is read from the
    file's bytes 8..12, and its header's game, SHAs, rustc, tick rate
    and protocol are compared.
- Dropped from the draft: a `<=` check of the `version` lines in the
  committed authored files (input scripts, assertions, budgets). It said
  little.
- `crash_json.rs`: the version shape did not change in this task, and
  `version` already had to equal the stamp's `build`, which compares
  every key and value. It now also checks that `crash_version` equals the
  stamp's `formats.crash`.
- `docs/ARCHITECTURE.md` says what each of the two test files pins.

**Checks**
- `cargo xtask quick` green (11 s), and the `lint` and `test` jobs green.
  The protected `exit_codes` is unchanged and passes.
- The new tests also pass in three other modes:
  - with all four variables set at build time (`dirty` true, an epoch,
    a CI run; `crash_json` too);
  - with `ENG_DIRTY=0` and an empty `ENG_ENGINE_SHA` (clean and
    unknown);
  - with `--release`.
- Mutation check by hand, with `version.rs` restored byte-for-byte
  afterwards (`cmp`). Each mutation failed a test:
  - `dirty unknown` changed to `dirty?` fails the line unit test;
  - `profile` hard-coded to `release` fails `version_json_keys_and_values`;
  - no nested schemas failed `version_json_keys_and_values` before the
    split. Re-run after it, it fails `testbed_systems_and_schemas`, while
    the contract file passes. That file checks only the list's form, and
    the protocol does not change with the stamp.
- CI's toolchain comes from rustup with `rust-toolchain.toml`. Its bin
  directory holds `cargo` and `rustc` side by side, so the rustc check
  finds the right compiler. RISC-V runs only core-crate tests, so these
  tests never run under QEMU.
- After the review fixes: `quick` green (10 s), and the `lint` and
  `test` jobs green. The three build modes above pass again: all four
  variables set, clean with an empty SHA, and `--release`.
- `reviewer` subagent: approve, nothing blocking. Its own `quick` run was
  green. It built the parent change and this one and compared both forms
  byte for byte under five build setups: identical, so the refactor
  changes no output. It also confirmed that the variables trigger
  rebuilds both ways. Findings and what happened to each:
  1. CI checks `ENG_DIRTY`, `SOURCE_DATE_EPOCH` and `ENG_CI_RUN` only as
     unset, since its workflow sets only `ENG_ENGINE_SHA`. A misspelt
     `option_env!` name for one of them would pass CI (medium). Needs
     xtask: amendment 16 asks the proposed X7 `version` step to build
     with all four set. Checked correct by hand today (above).
  2. The pinned schema list would need the owner for every testbed
     schema change once the file is protected (medium). Fixed by the
     split: the pins live in `version_testbed.rs`, and amendment 16 says
     to protect `version.rs` only.
  3. The `dump-world` component check would fail for a component no
     entity has at tick 0 (medium). Fixed: components are checked as a
     subset; resources and events stay exact, because the dump lists
     all of them.
  4. The quiet run inherited the shell's variables (low). Fixed: it
     removes them.
  5. The `RECORDING_VERSION` check compared the constant with itself
     (low). Fixed: the version is read from a real recording's bytes,
     and the header's build fields are compared too.
  6. `check_target` failed on `gnueabihf`-style variants and on i686
     (low). Fixed, with a comment on what it covers.
  7. The rustc check assumed rustup's layout, and `nth(1).unwrap()`
     could panic (low). Fixed: it falls back to `rustc` on `PATH`, and an
     unknown rustc renders as `unknown`.
  8. `version.rs` was at 495 lines, and `ARCHITECTURE.md` still said the
     test pins only keys, order and types (low). Fixed: the unit tests
     moved to `version/tests.rs`, and the docs now say what each test
     file pins.

**Costs and timings**
- One open-pulsar session, about 02:55 to 03:25 EET. Summer time ended
  at 04:00 EEST tonight, so these times are an hour behind the clock
  session 9 used. Correction to session 9: it ended at about 03:55 EEST
  (02:55 EET), not 04:10; that end time was an estimate.
- The `reviewer` pass took about 8 minutes and 100k subagent tokens.
  Builds and test runs took seconds each.

## 2026-10-25, session 11: `--version` tests and `quick` re-run (T1 follow-up) (Claude Opus 5.5, open-pulsar task)

The task: run the new tests and `cargo xtask quick`, both green, and
record the results and open questions. No code or docs other than this
entry changed. `docs/rungs/current.txt` stays at `1a`, and nothing was
pushed.

**Results** (aarch64-apple-darwin, Rust 1.99.1, `cargo test` because
cargo-nextest is not installed here)
- `cargo test -p eng-cli`: 16/16 ok in 0.15 s. That includes the 9
  `version::tests` and `args::tests::parses_version`.
- `cargo test -p game-testbed --test version --test version_testbed
  --test crash_json`: 9/9 ok.
  - `version` (the format contract): 4/4 in 0.02 s.
  - `version_testbed`: 3/3 in 0.01 s.
  - `crash_json`: 2/2 in 0.26 s.
- `cargo xtask quick`: all 7 steps ok in 9 s (fmt, clippy, arch-check,
  size-check, docs-check, tests 8.3 s, replay 0.3 s). Its tests step ran
  all three files. A separate `cargo test --workspace` run gives 132
  passed, 0 failed and 1 ignored over 34 test binaries. The ignored one
  is an `ignore` doc example in `eng-derive`, as before.
- The other CI jobs that run on this Mac:
  - `cargo xtask ci --job lint`: all 7 steps ok in 2 s, deny and typos
    included.
  - `ci --job test`: all 4 steps ok in 9 s (tests, known-answers,
    exit-codes, replay).
  - `ci --job wasm`: ok.
- Is it stable?
  - The three files ran 20 times in a row: 20 passed, 0 failed, in 6 s
    in all.
  - They pass with `--test-threads=1`.
  - On the release profile, the eng-cli unit tests and the three files
    pass (16 + 9).
- They also pass in two other build setups. Each was built in a scratch
  target directory (`target/version-modes`, deleted afterwards), so the
  main target kept its cache:
  - All four variables set: `ENG_ENGINE_SHA` from `git rev-parse HEAD`,
    `ENG_DIRTY=1`, `SOURCE_DATE_EPOCH=1792886400` and
    `ENG_CI_RUN=local-session11`. 16 + 9 ok. The line was
    `game-testbed 0.1.0 (engine 0.1.0 aed83cb78f69, dirty)
    aarch64-apple-darwin debug, rustc 1.99.1, sim protocol
    0x69fd4996880a6508`.
  - Clean with no SHA: `ENG_DIRTY=0`, the other three empty. 16 + 9 ok.
    The line read `engine 0.1.0 unknown`, with no dirty part.
- A plain `cargo run` build prints `engine 0.1.0 unknown, dirty
  unknown`. Its JSON has `unknown` or `null` for the four values that
  come from the source tree. That is expected until X1.
- Hash streams unchanged (replay ok; protocol `0x69fd4996880a6508`). No
  protected path touched, and `jj st` was clean before this entry.

**Open questions**
1. **Linux and nextest are not checked yet.** CI's test job runs on
   `ubuntu-24.04` and `ubuntu-24.04-arm` under nextest, which runs each
   test in its own process. This Mac has neither. Each test uses its
   own scratch directory (`$CARGO_TARGET_TMPDIR/version/<name>` and
   `.../version-testbed/<name>`), so those processes do not clash. Two
   parts could still behave differently on Linux:
   - `check_target` on the `*-unknown-linux-gnu` triples;
   - finding `rustc` beside `env!("CARGO")`. CI uses rustup's layout,
     where they sit side by side.

   The first CI run after the owner pushes is the real check.
2. **Under jj, X1's `git rev-parse HEAD` names the parent change.** Here
   it gave `aed83cb7`, the commit of `@-`, not of the working copy `@`.
   In a colocated repo git's `HEAD` is `@-`, and `git status
   --porcelain` lists `@`'s edits against it. So, with amendment 15's
   recipe, a build inside the loop is stamped with `@-` and `dirty`
   true whenever `@` has changes. That is accurate: the build is `@-`
   plus edits not yet recorded. But the stamp cannot name the jj change
   being worked on. If the owner wants that name, X1 could set
   `ENG_CI_RUN` to something like `local-jj-<change id>` on local runs.
   eng-cli would not need to change. This is the owner's call when
   writing X1.
3. **Two `cargo test` runs at once** (as in session 8, item 3).
   `scratch()` deletes and recreates its directory, so two runs at the
   same moment could delete each other's files. CI and the loop never
   do this.
4. **Criterion 8 (`version-json`) cannot pass yet.** It needs X1 for the
   values and the CI `version` step (X2, X6) to check them. Until then CI
   checks `ENG_DIRTY`, `SOURCE_DATE_EPOCH` and `ENG_CI_RUN` only as
   unset (amendment 16). Today's run with all four set was by hand.
5. **Both harness files are still unprotected.** Amendments 14 to 16
   ask to protect `crash_json.rs` and `version.rs` (not
   `version_testbed.rs`). Until then an agent can weaken them. The
   agent side of T1 is done (feature, tests and a green run), and no
   further T1 work needs to change either file. So now is a good time
   to protect them.
6. **`current.txt` still says `1a`** while 1b tasks run (proposal 10,
   G1). The SessionStart hook still points agents at 1a's files. Loop
   tasks name 1b explicitly, so this has caused no harm so far.

**Costs and timings**
- One short open-pulsar session, about 03:22 to 03:30 EET; no subagents.
  Most builds were cached. The scratch build with all four variables
  took 11 s (52 s of CPU), and the release test build took 3.4 s.

**Next**
- What is left of T1 needs the owner: amendments 15 and 16 (X1, the
  `version` step in X2 and X7, and protection). Then T4 (live input).

## 2026-10-25, session 12: live input (T4) (Claude Opus 5.5, open-pulsar task)

The task: in `eng-input`, plain-data `Key` and `KeyEvent`, actions,
strict-TOML bindings (WASD, the arrows, Esc for `quit` by default) and
quantization of a key timeline into one `PlayerCommand` per tick, with
the caller passing the tick. The rules are criterion 18's. Script and
recording formats and the hash streams stay unchanged; no new crates; no
protected paths.

**Done**
- `crates/eng-input/src/key.rs`:
  - `Key` is a closed enum of 110 physical keys, named by their W3C UI
    Events `code`. They are the writing-system, functional, control-pad,
    arrow, numpad and function-section keys of 104-key, 105-key and
    Japanese layouts. It has `ALL`, `COUNT`, `index`, `name`, an exact
    `from_name` and `Display`.
  - `KeyEvent` is `Down(Key)`, `Up(Key)` or `FocusLost`.
  - The module uses only `core` and derives only std traits (no
    `Schema`: it is not simulation data). T3 can move it into
    `eng-platform-api` by cut and paste.
- `src/bindings.rs`:
  - `Action`: `move_forward`, `move_back`, `move_left`, `move_right` and
    `quit`. `is_app()` is true for `quit` only. `ActionSet` is a small
    bit set.
  - `Bindings` maps each key to at most one action, in an array indexed
    by key. It has `bind`, `unbind`, `action` and `keys`.
  - The format is `version = 1` plus `[actions] name = ["Code", ...]`,
    loaded through `eng_schema_json::load_toml`, so it is strict, ignores
    `x_` keys and has a version with migrations. An action the file
    leaves out is unbound: a file replaces the defaults.
  - The defaults are `src/default.bindings.toml`, compiled in as
    `DEFAULT_BINDINGS`. `Bindings::default()` parses that text.
  - Every error is `BindingsError`, shown as
    `[E-BINDINGS] origin:line: message`.
    - These carry their line: an unknown action or key name, an action
      whose value is not a list of names, an unknown top-level key, and
      a key bound twice. The first problem in the file is the one
      reported, not the first by name. To make that hold, serde accepts
      any action value and any extra top-level key (an untagged enum
      over `IgnoredAny`, and a flattened map), and `parse` rejects them
      with their lines.
    - An unknown key gets a hint: `w` gives "did you mean `KeyW`?", and
      so do `1`, `up`, `escape` and `Esc` for their keys. The message
      gives examples of real names.
    - Names from the file are escaped (`escape_debug`), so every message
      with a line is one line.
    - Only three problems have no `:line`, and they keep `load_toml`'s
      message: a TOML syntax error (whose message names its line), a
      missing or newer `version`, and an `actions` that is not a table.
- `src/live.rs`:
  - `LiveInput` has `handle(KeyEvent)`, `sample(Tick) -> TickInput`,
    `set_view(yaw, pitch)`, `bindings_mut` and `is_held`.
  - `TickInput` is `{ command, actions }`, and `quit()` reads `quit` from
    `actions`.
  - `quantize(&Bindings, &[(Tick, KeyEvent)], ticks)` is the same for a
    whole timeline. Each event carries the tick whose sample first sees
    it.
  - The rules:
    - A key counts for a tick if it is held at the sample, or if it went
      from up to down since the previous sample (a latch). So a tap
      shorter than a tick counts once, however many taps there were.
    - A `Down` for a key already held (OS key repeat) changes nothing,
      so releasing a held key never adds a tick.
    - `FocusLost` releases every key.
    - Opposite actions cancel, and movement is full speed (±127).
    - `quit` is reported in `actions` and never changes the command.
    - No clock and no floats.
  - `sample` must get increasing ticks; sampling a tick twice would lose
    its taps. Debug builds panic with `[E-INPUT]` if a tick does not
    increase.
- `eng-schema-json`: new `TomlSpans` and `PathStep` (`src/spans.rs`).
  They parse the text again with toml's span-keeping `DeTable` and
  answer "on which line is this key or array element". No toml type is
  in the public API. They belong in the authored-TOML crate, so
  `eng-input` needs no direct `toml` dependency.
- Tests:
  - `crates/eng-input/tests/live_input.rs` holds criterion 18's own
    checks, 12 tests (467 lines):
    - the default bindings load from the TOML, and a file replaces them;
    - `[E-BINDINGS]` with the line for tables, inline tables, dotted
      keys, `[actions.<name>]` headers and multi-line arrays, for an
      unknown action whatever its value, in file order, and for a key
      bound twice;
    - held keys (OS key repeat included), taps, focus loss and `quit`;
    - a known answer: a fixed 2,000-event timeline (made with an integer
      LCG) over 1,014 ticks. It pins an FNV-1a fingerprint of every
      field (`0x4d12_793a_10d0_b98f`) and six counts, so another target
      that differs fails. RISC-V does not run it, since `eng-input` is
      not core;
    - four proptests:
      - `quantize` against a model of the rules written out
        independently, under random rebinding;
      - same timeline, same commands, including streaming and a clone
        taken part-way;
      - removing the events of every key bound to `quit` leaves the
        commands unchanged;
      - everything is idle after a focus loss.
  - `tests/bindings.rs` (7 tests): key and action names round-trip, the
    hints, the other errors with their lines (or none) and escaping,
    `x_` keys, bind and unbind.
  - `tests/quantize.rs` (4 tests, beyond the criterion): cancelling,
    keys sharing an action, the held view and rebinding, a timeline out
    of order, and a tick sampled twice (debug builds).
  - `crates/eng-schema-json/tests/spans.rs` (2 tests).
- Docs: `docs/ARCHITECTURE.md` gains "Live input". The `eng-input` crate
  docs and its `Cargo.toml` description now cover live input.
- `eng-input` gains `proptest` as a dev-dependency. It was already in
  `Cargo.lock` (1.12.0, a dev-dependency of five crates), so the lock
  gains one line and no package, and no table-only fallback was needed.

**Decisions** (Claude's calls; the owner can overrule any)
1. `FocusLost` acts as an `Up` for every key. A key that went down since
   the last sample still counts once, as with any release. The other
   choice would be to drop pending taps as well.
2. A key triggers one action, so a key given twice in a file is an
   error. An action may have any number of keys.
3. An action left out of a file is unbound, and `quit = []` unbinds
   Escape. The window's close button still quits.
4. Diagonals send `forward` and `side` at 127 each. kit-fps clamps the
   wish length to 1, so a diagonal is not faster.
5. The view angles are values the caller holds with `set_view`, 0 by
   default, so `move_forward` goes along +X. For the top-down view, T11
   should probably hold yaw `0x4000` (90°, +Y), so that W moves the dot
   up the screen and D to the right.
6. Five actions only, and `buttons` and `select` stay 0. Game-defined
   actions (fire, use, jump) arrive when a game has buttons (rung 2).
7. `quantize` panics if the timeline's ticks decrease. Events for tick 0
   land in tick 1, and events after `ticks` are dropped.
8. Not in the key set: IME keys (`Lang1`-`Lang5`, `Convert`, `KanaMode`,
   `NonConvert`), media and browser keys, `F13` and up, and `Fn`. A
   platform drops keys that `Key` cannot name. winit calls `MetaLeft`
   `SuperLeft`, so T10 maps between them.

**Checks**
- After the review fixes: `cargo xtask quick` all 7 steps ok in 13 s.
  `ci --job lint` (deny and typos included): 7/7 ok in 2 s. `ci --job
  test`: 4/4 ok in 8 s, the protected `exit_codes` included. `ci --job
  wasm`: ok. All of them were green before the review too.
- `cargo test --workspace`: 157 passed, 0 failed, 1 ignored (the
  `eng-derive` doc example, as before). Session 11 had 132; the 25 new
  ones are 12 + 7 + 4 + 2. `PROPTEST_CASES=5000` also passes.
- Each rule bites. Four deliberate bugs were tried and reverted:
  - `FocusLost` also drops pending taps: 3 tests fail;
  - `quit` leaks into `buttons`: 4 fail;
  - `Up` cancels a pending tap: 3 fail;
  - after the review, a repeat `Down` re-arms the latch again (the bug
    the reviewer found): 3 fail.

  Each mutant had the model proptest and the known answer among the
  failures. (The first three ran against the first version of the
  tests.)
- rustdoc for both crates builds with `-D warnings`.
- Hash streams unchanged (`replay` ok). `script.rs` and `recording.rs`
  are untouched. No protected path changed. The diff is `Cargo.lock`
  (one line), two `Cargo.toml`/`lib.rs` pairs and `docs/ARCHITECTURE.md`,
  plus the new files.

**Found: proptest's regression files miss the protected pattern.**
While the mutants ran, proptest wrote
`crates/eng-input/tests/live_input.proptest-regressions`, beside the test
file. proptest 1.12's default persistence, `SourceParallel`, looks for a
`lib.rs` or `main.rs` in the test file's directory and then in each
parent. Only if it finds one does it write into a
`proptest-regressions/` directory next to that directory (for
`src/lib.rs`, `<crate>/proptest-regressions/`). A file in `tests/` has no
such parent here, so proptest falls back to writing
`<file>.proptest-regressions` next to the test. Every proptest in the
repo is in a `tests/` directory, so `**/proptest-regressions/**`
matches none of the files they would write. Those mutant-only files
were deleted, so none is left. Proposal: amendment 17.

**Review**
- One fresh-context `reviewer` pass. Verdict: "changes needed", for one
  medium finding; it said criterion 18's six rules were met and tested.
  Its own `quick` and `replay` runs were green, and so was
  `PROPTEST_CASES=5000`. It also fuzzed the error lines from a scratch
  crate in `/tmp`, outside the repo:
  - 100,000 generated files: 86,831 errors carried a line and none was
    wrong. They covered tables, inline tables, dotted and quoted keys,
    multi-line arrays, CRLF, a BOM, comments in arrays and TOML 1.1
    multi-line inline tables.
  - 200,000 random token strings: no panic.

  Findings and what happened to each:
  1. **OS key repeat added a tick (medium).** `handle` set the tap latch
     on every `Down`, including a repeat for a key already held.
     `[(1, Down W), (30, Up W)]` gave 29 ticks of forward, but a repeat
     `(30, Down W)` just before the `Up` gave 30. With key repeat on,
     releasing a held key could add a tick, depending on where the
     repeat fell. The proptest model had the same flaw, so it could not
     catch it. Fixed: the latch is set only when the key was up, the
     model is fixed the same way, and a repeat test was added. Reverting
     only the fix fails 3 tests.

     The known answer changed for this reason, from
     `0x5ef3_6683_a7aa_bbb5` to `0x4d12_793a_10d0_b98f`. The fixed
     timeline often sends `Down` for held keys. The counts went from
     `[202, 197, 240, 199, 388, 114]` to `[202, 196, 239, 199, 388, 116]`:
     one tick fewer each of back and right, and two more idle ticks. The
     file is not protected yet, and the old value never left this
     session.
  2. **"With the line" and "first in file order" failed when serde
     rejected the file first (low).** `fly = "KeyW"` or `[actions.fly]`
     gave an unknown action with no line. `fly = []` on line 3 followed
     by `quit = "Escape"` reported `quit`. Fixed as described under
     "Done": serde now accepts any action value and any top-level key,
     and `parse` rejects them with their lines, in file order. Tests
     cover each scenario. The docs (rustdoc, `ARCHITECTURE.md`) list the
     three problems that have no line.
  3. **Names went into messages unescaped (nit).** `quit = ["Key\nW"]`
     split an `[E-BINDINGS]` line in two. Fixed with `escape_debug`, and
     a test checks that every message with a line is one line.
  4. **Increasing ticks were not checked (nit).** Sampling a tick twice
     silently lost its taps. Fixed with a `debug_assert!` and
     `[E-INPUT]`, and a debug-only test.
  5. **`live_input.rs` was near the 500-line warning (nit).** Fixed: the
     three tests outside the criterion moved to `tests/quantize.rs`. It
     is 467 lines with the new tests.
  6. **Placeholders in this entry (nit).** Filled in here.

**Not done (waits for others)**
- Nothing calls live input yet. T11 (`eng-app`) wires `play`,
  `--bindings` and Esc. When the probe reads bindings files, the
  version stamp's `formats` should gain `bindings`, and
  `crates/game-testbed/tests/version.rs` pins those keys.
- T3 moves `Key` and `KeyEvent` into `eng-platform-api` (after G2).
- The `live-input` CI step and protecting `live_input.rs` need the owner
  (amendment 17).

**Costs and timings**
- One open-pulsar session, about 03:30 to 04:00 EET, including one
  `reviewer` pass (about 8 minutes, 115k subagent tokens, 41 tool
  calls). Builds and test runs took seconds; `quick` takes 13 s.

## 2026-10-25, session 13: review of the overnight window, report version 2 (Claude Opus 5.5, with the owner)

The owner had the window's work (sessions 1 to 12, queue items 1 to 5)
reviewed before publishing it. A fresh-context `reviewer` pass checked
T1, T2 and T4 against criteria 8, 6 and 18 and the plan's task table.
Verdict: approve, no blockers. `docs/rungs/current.txt` stays at `1a`.

**T4: the two tasks that failed**
- Item 5's open-pulsar run had three tasks. Session 12 was the first, and
  it wrote all the tests along with the code. The other two ("add
  tests", "run the checks and record") each failed all 5 attempts
  between 03:58 and 04:21 EET, after the plan's session limit was
  reached, and left only empty jj changes. Nothing is missing from T4:
  `live_input` 12, `bindings` 7, `quantize` 4 and `input` 4 pass, and so
  do eng-schema-json's `spans` 2.
- Correction to session 12: only eng-input's `Cargo.toml` changed;
  eng-schema-json changed `lib.rs` only.

**Changed here**
- `report_version` is now 2 (`crates/eng-cli/src/report.rs`). T1 gave
  reports the stamp's `build` object: `target` became the full target
  triple and keys were added. `docs/ARCHITECTURE.md`'s rule (a change to
  `build` bumps `report_version`) applies, and version 1 reports exist
  from rung 1a. This replaces session 9's "stay 1".
- `crash_version` stays 1: `crash.json` is first published with the
  stamp's `build`, so no version 1 crash file with the older object
  exists. `docs/ARCHITECTURE.md` says so in place of the exception, and
  `crates/game-testbed/tests/version.rs` expects `"report": 2`.

**Review notes not acted on**
- `crates/game-testbed/tests/version.rs:241` works out the expected
  `profile` from `cfg!(debug_assertions)`, while the build reads Cargo's
  `PROFILE`. They differ only for a release profile with debug
  assertions on, which no profile here uses.
- `crates/eng-input/tests/live_input.rs:414` calls `quantize` twice on
  the same input. That only catches hidden state between calls.
  Equality across targets rests on the known answer (line 301), and the
  RISC-V job does not run it because eng-input is not core.
- X1 is still prose (amendments 15 and 16). The plan wants it as a
  tested patch in `docs/rungs/1b/patches/`.

**Window costs and timings** (from the open-pulsar runtime's logs)
- The window ran from 00:49 EEST to 06:00 EET, 6 h 11 min (the clocks
  went back at 04:00). The plan's session limit was reached twice, at
  02:46 EEST (reset 03:20) and at 04:18 EET (reset 07:20).
- Lost to the limit: item 4's first run (0 of 3 tasks, 02:43 to 02:52
  EEST), item 5's last two tasks, and item 6 (the closing report), which
  never ran. This entry stands in for it.
- Session 12 reported $9.95 at API prices; the plan is a subscription,
  so nothing was billed per token.
- This session's `reviewer` pass: about 6 minutes, 184k subagent tokens,
  36 tool calls. `quick` passes.
- The runtime's own problems (a retry that reset the wrong state
  folder, false dead-run alarms, no usage cap) went to open-pulsar.

## Proposals for the owner (protected paths)

Nothing below is applied. Each item is what an agent would otherwise have
to change; the owner applies it, or unlocks the path.

1. **`docs/rungs/1b/acceptance.json`** (G1). Make it from the table in
   `plan.md`, "Acceptance", in 1a's format: rung `1b`, spec section 9.2,
   time-box 14 calendar days from approval, AI budget at most 30 agent
   sessions. Feature 16 (`owner-play`) cannot be computed; proposal 7
   (X4) teaches `cargo xtask acceptance` an owner-checked feature.

2. **`architecture.toml`** (G2). Add, and change the two existing entries
   marked below:

   ```toml
   [crates.eng-platform-api]
   layer = "L1"

   [crates.eng-platform-winit]
   layer = "L1"
   same_layer = ["eng-platform-api"]

   # changed: implements the platform contract
   [crates.eng-platform-headless]
   layer = "L1"
   same_layer = ["eng-platform-api"]

   # changed: maps platform key codes to actions
   [crates.eng-input]
   layer = "L1"
   same_layer = ["eng-schema-json", "eng-platform-api"]

   [crates.eng-net-wire]
   layer = "L1"
   core = true

   [crates.eng-session]
   layer = "L2"
   core = true
   same_layer = ["eng-sim"]

   [crates.eng-gpu]
   layer = "L3"

   [crates.eng-render]
   layer = "L3"
   same_layer = ["eng-gpu"]

   [crates.eng-app]
   layer = "L4"
   same_layer = ["eng-cli"]
   ```

   `[floor]` already maps wgpu to `eng-gpu` and winit to
   `eng-platform-winit`; no change there.

3. **Root `Cargo.toml`, `[profile]` table** (G2), for the two new core
   crates:

   ```toml
   [profile.release.package.eng-net-wire]
   overflow-checks = true
   [profile.release.package.eng-session]
   overflow-checks = true
   ```

4. **`deny.toml`, allow-list batch 2** (G3).
   - Limit cargo-deny to the targets whose whole workspace we build, so
     crates for Android, iOS and the web stay out of the list:

     ```toml
     [graph]
     all-features = false
     targets = [
       "x86_64-unknown-linux-gnu",
       "aarch64-unknown-linux-gnu",
       "x86_64-pc-windows-msvc",
       "aarch64-pc-windows-msvc",
       "aarch64-apple-darwin",
       "x86_64-apple-darwin",
       "riscv64gc-unknown-linux-gnu",
     ]
     ```

     `wasm32-unknown-unknown` is left out on purpose: CI builds only the
     core set for it (which reaches nothing but `libm`), while winit would
     add web-only crates to the full graph. RISC-V adds no crates beyond
     the Linux ones (checked). The comment above `allow` ("only these
     crates may appear in Cargo.lock") then becomes "in the dependency
     graph of these targets". Batch-1 entries reached only on other
     targets (`r-efi`) may become unused; keep or drop them.
   - Keep the window-handle exception (proposal 5) inside its seam with a
     wrapper ban next to the floor-crate bans:

     ```toml
     { crate = "raw-window-handle", wrappers = [
       "wgpu", "wgpu-core", "wgpu-hal", "wgpu-types", "winit",
       "eng-platform-api", "eng-platform-winit", "eng-gpu", "eng-app",
     ] },
     ```

     The five third-party names are the crates that use it directly
     today (checked with `cargo tree -i`).
   - Add these 132 crates to `[bans] allow` (computed 2026-10-25; T7 and
     T10 recompute against the real lock and report any difference here):
     - every OS (54): adler2, arrayvec, bit-set, bit-vec, bytemuck,
       bytemuck_derive, cc, cfg_aliases, codespan-reporting, crc32fast,
       cursor-icon, document-features, dpi, fdeflate, find-msvc-tools,
       <https://crates.io/crates/flate2> (see the `_typos.toml` note
       below), foldhash, half, libloading, litrs, lock_api, log,
       miniz_oxide, naga, naga-types, nv-flip, nv-flip-sys, once_cell,
       parking_lot, parking_lot_core, pin-project-lite, png, profiling,
       raw-window-handle, rustc-hash, scopeguard, shlex, simd-adler32,
       smallvec, smol_str, static_assertions, thiserror, thiserror-impl,
       tracing, tracing-core, unicode-width, wgpu, wgpu-core, wgpu-hal,
       wgpu-naga-bridge, wgpu-types, winit, zerocopy, zerocopy-derive
     - Linux and Windows (8, Vulkan): allocator-api2, ash, gpu-allocator,
       ordered-float, presser, renderdoc-sys, spirv,
       wgpu-core-deps-windows-linux-android
     - Linux only (33, X11 and Wayland): ahash, as-raw-xcb-connection,
       calloop, calloop-wayland-source, dlib, downcast-rs, gethostname,
       linux-raw-sys, memmap2, percent-encoding, pkg-config, polling,
       quick-xml, rustix, scoped-tls, slab, smithay-client-toolkit,
       version_check, wayland-backend, wayland-client, wayland-csd-frame,
       wayland-cursor, wayland-protocols, wayland-protocols-plasma,
       wayland-protocols-wlr, wayland-scanner, wayland-sys, x11-dl,
       x11rb, x11rb-protocol, xcursor, xkbcommon-dl, xkeysym
     - Windows only (17): range-alloc, unicode-segmentation, windows,
       windows-collections, windows-core, windows-future,
       windows-implement, windows-interface, windows-link,
       windows-numerics, windows-result, windows-strings, windows-sys,
       windows-targets, windows-threading, windows_aarch64_msvc,
       windows_x86_64_msvc
     - macOS only (20): block2, core-foundation, core-foundation-sys,
       core-graphics, core-graphics-types, dispatch, foreign-types,
       foreign-types-macros, foreign-types-shared, objc-sys, objc2,
       objc2-app-kit, objc2-core-foundation, objc2-core-graphics,
       objc2-encode, objc2-foundation, objc2-metal, objc2-quartz-core,
       raw-window-metal, wgpu-core-deps-apple
   - Licences: every one is MIT, Apache-2.0, Zlib, ISC, BSD-2/3-Clause or
     0BSD as one choice of an OR, or a combination of allowed ones
     (`dpi`: Apache-2.0 AND MIT; `nv-flip-sys`: (MIT OR Apache-2.0 OR
     Zlib) AND BSD-3-Clause). No exceptions needed.
   - Test-only: `nv-flip` and `nv-flip-sys` (FLIP image comparison, as the
     spec chose; builds C++ through `cc`). `png` serves both goldens and
     the proof video.
   - **`_typos.toml`** (protected) needs the deflate crate's name, the one
     given above as a link, under `[default.extend-identifiers]` in the
     same change. typos reads it as a misspelt "flat", so the `lint` job
     fails once that name is in `deny.toml`.

5. **ADR-0016 (G4), an exception to rule 4.** Allow the `raw-window-handle`
   0.6 traits `HasWindowHandle` and `HasDisplayHandle` in public APIs
   between `eng-platform-api`, `eng-platform-winit`, `eng-gpu` and
   `eng-app`, and nowhere else. Why: wgpu needs a window handle from winit;
   this crate is the neutral interop standard both implement (as do SDL3's
   Rust bindings), so the swap path in spec 6.3 still holds. The
   alternative, our own raw-handle enum, needs `unsafe` and copies that
   crate. The ADR is written in T14 if the owner agrees; `AGENTS.md` and
   `docs/ARCHITECTURE.md` then name both exceptions. The wrapper ban in
   proposal 4 and a selftest case (X5) enforce the seam.

6. **`budgets.toml`** (G5):

   ```toml
   # Runner-independent per-frame counters for the testbed's top-down
   # view (spec 6.13), checked by `shot` and `play --report`.
   [gpu]
   max_draws_per_frame = 8
   max_pipeline_switches_per_frame = 4
   max_bind_group_switches_per_frame = 8
   max_upload_bytes_per_frame = 16384

   [ci]
   pr_tier_minutes = 15
   native_job_minutes = 25
   ```

   Starting values, about four times what the dot needs; tightened at the
   first deload.

7. **xtask, workflows and `.claude/`** (G5), prepared as patch files in
   `docs/rungs/1b/patches/` during T1 and T13:
   (Named X1-X6 so they are not confused with the spec's pending
   decisions P1-P11.)
   - X1 version stamps: `cargo xtask` computes `version.json` (6.13) and
     passes its values to builds as `ENG_*` variables.
   - X2 `cargo xtask ci --job native`: build, tests (with
     `ENG_REQUIRE_GPU=1`), replay with hash upload, crash-json, version,
     shot-exact or shot-flip (reporting "no golden yet" without failing
     while a golden is missing), window, gpu-budgets, esc (Linux x64),
     proof-video (Linux x64), check-intel-mac (macOS); every step in
     `target/ci/report.json` with the adapter and driver.
   - X3 summary: compare hash streams across the native jobs too, and
     measure `native-duration`.
   - X4 acceptance: features with `"owner": true` are listed, never
     computed.
   - X5 selftest cases: `wgpu-outside-gpu`, `winit-outside-platform`,
     `rwh-outside-seam`, `core-depends-on-gpu`, `golden-edit`.
   - X6 `.github/workflows/ci.yml`: the five native jobs; Mesa 25.2.7
     lavapipe (gfx-rs/ci-build build26) with a fixed
     `LP_NATIVE_VECTOR_WIDTH` on `ubuntu-24.04`; WARP from its pinned
     NuGet package on Windows; xvfb and xdotool on Linux; artifacts for
     shots, golden candidates, FLIP maps, `crash.json` and `proof.apng`.
   - `.claude/`: the `golden-update` skill (how candidates reach the
     owner) and the advisory `visual-qa` subagent (spec 7.5; 1a deferred
     both to the rung that needs them).

8. **`protected.toml` and `.github/CODEOWNERS`** (G5), once the files
   pass review: add the harnesses that acceptance features rely on (as 1a
   protected `exit_codes.rs`):
   - `crates/game-testbed/tests/shots.rs` (features 4, 5; T12)
   - `crates/game-testbed/tests/crash_json.rs` (feature 6; T2, T11)
   - `eng-gpu`'s shader translation test (feature 10; T7)
   - the headless-twin Esc test and the windowed-run test in `eng-app` or
     `game-testbed` (features 2, 3; T11)

   Exact paths are given when those tasks finish. Goldens and
   `golden/thresholds.toml` are already covered by `**/golden/**`, and
   `eng-net-wire`'s vectors by `crates/*/tests/golden.rs`.

9. **Goldens** (G6): one golden per shot, from the pinned lavapipe
   configuration, committed to `crates/game-testbed/golden/` with
   `thresholds.toml`; every other adapter is compared against it with
   FLIP. Proposed start: mean FLIP at most 0.01 and 99th percentile at
   most 0.2 per image. `eng-net-wire`'s `tests/golden.rs` arrives as a
   patch in T5 and is applied at G5.

10. **`docs/rungs/current.txt`** (G1): set to `1b` when the owner approves.

11. **Amendments from session 2** (to proposals 1, 2, 3, 4, 7 and 8; the
    text above is kept as written):
    - Proposal 1: the acceptance draft is now the numbered list in
      `plan.md`, "Proposed acceptance criteria". Numbers 1-17 are
      unchanged. Criterion 18 `live-input` is new (test x64, arm64:
      live-input; native x5: tests). Criteria 6, 10 and 11 now name the
      `test`-job steps they rely on (`crash-json`, `shaders`,
      `wire-session`).
    - Proposals 2 and 3 (G2) must land in one commit together with
      skeletons of the seven new crates. `arch-check` fails on a listed
      crate that is not in the workspace (`[E-ARCH-UNKNOWN]`) as well as
      on an unlisted one. So entries added before the crates, or crates
      added before the entries, turn `main` red. New task T0 prepares
      the combined patch (skeletons, entries, `[profile]` lines) and
      tests it in a scratch clone.
    - Proposal 4: the crate list moves into `plan.md`, "Crate batch 2",
      in the next queue item. Until then the list above is the working
      copy. T3 also needs this batch, since the window seam uses
      `raw-window-handle`.
    - Proposal 7, X6 (not X2): the macOS native job in `ci.yml` runs
      `rustup target add x86_64-apple-darwin` before `check-intel-mac`,
      as the RISC-V job already does for its target.
      `rust-toolchain.toml` stays unchanged, so other machines do not
      download the Intel-Mac standard library.
    - Proposal 7, new X7: `cargo xtask ci --job test` gains the named
      steps `crash-json` (`run` only; the test job has no display),
      `shaders`, `wire-session` and `live-input`. Today it has only
      tests, known-answers, exit-codes and replay.
    - Proposal 8, more harnesses to protect: `crates/eng-input/tests/live_input.rs`
      (criterion 18), and the `eng-net-wire` never-panic and
      `eng-session` hash-equality tests (criterion 11). Without that,
      deleting them would leave the criteria green. Exact paths are
      given when T4-T6 finish.
    - **`**/proptest-regressions/**`** (as needed, with G5). When a
      proptest in `eng-input`, `eng-net-wire` or `eng-session` fails, it
      writes a regression file that agents may not commit. The file
      reaches the owner as a candidate together with the fix, and the
      owner commits it.
    - **`.config/nextest.toml`** (as needed, with G5): a test group that
      runs the GPU and window tests one at a time, and a longer
      slow-timeout for them, if T7, T11 or T13 shows they need it.
    - Golden candidates are written under `target/golden-candidates/`,
      never into `golden/`, where the guard would restore and quarantine
      them.

12. **Amendment from session 4** (to proposal 4; the text above is kept as
    written). With proposal 4's seven `[graph] targets`, `cargo deny
    check bans` also needs `concurrent-queue`, `crossbeam-utils`, `errno`
    and `windows_x86_64_gnu`, all MIT or Apache-2.0. That makes 136 names.
    cargo-deny tests each platform-gated edge against every listed
    target, so these four count although no target builds them.
    Checked with cargo-deny 0.20.2 on the wgpu and winit scratch
    project; see session 4. The licence summary stands: nothing needs an
    exception.

13. **Amendment from session 5** (to proposal 4; the text above is kept as
    written).
    - The list to apply is now `plan.md`, "Crate batch 2": 136 names,
      with each version, licence and target. These are the same names as
      proposal 4 with amendment 12. `png 0.18.1` and `nv-flip 0.1.2` are
      now resolved and checked as well.
    - Licences: 0BSD is not on `[licenses] allow`. `adler2` ("0BSD OR MIT
      OR Apache-2.0") passes through MIT, so no change is proposed to the
      licence list, and no `exceptions` or `clarify` entry is needed.
    - The comment above `allow`: use proposal 4's wording ("in the
      dependency graph of these targets"). The deflate crate's default
      feature makes cargo lock `zlib-rs` without building it, and
      cargo-deny does not check it. So "only these crates may appear in
      Cargo.lock" would no longer be true.
    - Recompute against the real lock in T9 (`png`) and T12 (`nv-flip`)
      too, not only in T7 and T10.

14. **Amendment from session 6** (to proposals 7 and 8; the text above is
    kept as written). T2 is built, so two of those items can be applied
    now, without waiting for T11:
    - Proposal 8: `crates/game-testbed/tests/crash_json.rs` exists at
      that path. Add it to `protected.toml` beside `exit_codes.rs`, with
      the matching `.github/CODEOWNERS` line. Until then, deleting it or
      weakening it would leave criterion 6 unchecked. Best applied after
      T1: T1 swaps `version` for the full stamp and updates the
      harness's version checks. If it is protected first, that update
      needs the owner too.
    - Proposal 7, X7: the `crash-json` step in `cargo xtask ci --job
      test`. In `xtask/src/ci.rs`, `fn test`, after the `exit-codes`
      step:

      ```rust
      steps.run("crash-json", || {
          tests(root, Some(("game-testbed", "crash_json")))
      });
      ```

      Until then the harness runs only inside the `tests` step, so a
      failure shows up as `tests` and not as `crash-json`. Keep the step
      on the dev profile: the harness looks for the `eng_cli::run::run`
      frame, and a release build without debug info can inline it away.
    - `deny.toml` (G3, optional): the allow-list comment gives
      `serde_json`'s uses as "authored data, xtask". It is now also a
      dev-dependency of `game-testbed`, for parsing `crash.json` in a
      test, so the comment could add "tests". The crate list is
      unchanged.
    - When T11 adds `play`, the harness gains a `play` case. It is
      protected by then, so that change also needs the owner.

15. **Amendment from session 9** (to proposals 7 and 8, and amendment 14;
    the text above is kept as written). T1 is built, so these can be
    applied:
    - Proposal 7, X1 (`xtask/**`), now concrete. Every cargo command that
      xtask runs sets four variables, read by
      `crates/eng-cli/src/version.rs`. "Every" includes the tests, the
      replay build and the CI jobs. If only some commands set them,
      eng-cli and every game rebuild each time a run with them follows
      one without them.
      - `ENG_ENGINE_SHA`: `git rev-parse HEAD`. CI's workflow already
        sets `ENG_ENGINE_SHA: ${{ github.sha }}` at the top level, so
        X1 should keep a value that is already set and compute one only
        when none is. On GitHub `github.sha` is the checked-out `HEAD`, so
        the two agree, and the workflow line can stay or go with X6.
      - `ENG_DIRTY`: `1` if `git status --porcelain` prints anything,
        else `0`. Any other non-empty value fails the build.
      - `SOURCE_DATE_EPOCH`: `git log -1 --format=%ct`, in whole
        seconds. Any other non-empty value fails the build.
      - `ENG_CI_RUN`: on GitHub `github-$GITHUB_RUN_ID/$GITHUB_RUN_ATTEMPT`,
        locally `local`.

      An empty value counts as unset for all four.

      xtask does not need to pass the target, profile or rustc, because
      `build.rs` measures them. `ENG_RUSTC_VERSION` is no longer read.
      For spec 6.13's "`cargo xtask` computes `version.json`", the
      simplest patch writes `target/version.json` from
      `<game> --version --json` after the build, so the file and the
      binary cannot disagree. A new commit changes `ENG_ENGINE_SHA`, so
      eng-cli and the game rebuild once per commit, which takes seconds.
      Running `--version --json` builds the game's sim and writes one
      small log file under `$ENG_AGENT_LOG_DIR`. A game whose sim build
      panics makes it exit 12, which is the right failure for such a
      build.
    - Proposal 7, X2 (`ci --job native`), the `version` step for
      criterion 8: run `--version` and `--version --json`, and fail if a
      `build` value is the string `unknown` or `null`.
    - Proposal 7, X7: a named step in `ci --job test`, after
      `exit-codes`:

      ```rust
      steps.run("version", || tests(root, Some(("game-testbed", "version"))));
      ```

    - Proposal 8 (`protected.toml` and `.github/CODEOWNERS`):
      - Protect `crates/game-testbed/tests/version.rs`, criterion 8's
        harness. It pins the stamp's keys, order and types, so weakening
        it would let the JSON drift silently. Once it is protected, every
        change to the stamp needs the owner, including a new key, which
        does not bump `stamp_version`. That keeps the format the owner's
        decision. If additions should stay open to agents, the test could
        instead require only that the pinned keys come first and in
        order.
      - Amendment 14 asked for `crates/game-testbed/tests/crash_json.rs`
        to be protected after T1. T1 has now made its version change
        (`version` must equal the stamp's `build`), so it can be
        protected now.
    - `deny.toml` (optional, G3): `serde` is now also a dev-dependency of
      `game-testbed`, for reading JSON keys in document order in a test.
      Its allow-list comment could say so. The crate list is unchanged.

16. **Amendment from session 10** (to amendment 15; the text above is kept
    as written):
    - Proposal 8: protect `crates/game-testbed/tests/version.rs` only,
      which is the format contract. Leave `version_testbed.rs`
      unprotected. It pins the testbed's systems and schema list, which
      change with the sim the way hash streams do. Protecting it would
      put every testbed schema change through the owner.
    - Proposal 7, X7: the `version` step should also build and test
      once with all four variables set, for example `ENG_DIRTY=1`, a
      `SOURCE_DATE_EPOCH` and an `ENG_CI_RUN`. CI's workflow sets only
      `ENG_ENGINE_SHA`, so today CI sees the other three only as unset,
      and a misspelt variable name in `crates/eng-cli/src/version.rs`
      would pass. The tests compute their expected values from the same
      compile-time variables, so they pass in both builds and fail only
      when the binary and the variables disagree. After X1 sets all four
      everywhere, this extra build can go.

      ```rust
      steps.run("version", || {
          tests(root, Some(("game-testbed", "version")))?;
          // Once more, as a dirty xtask build on CI would be.
          cargo_step_env(
              root,
              "version-set",
              &["test", "-p", "game-testbed", "--test", "version"],
              &[("ENG_DIRTY", "1"), ("SOURCE_DATE_EPOCH", "1761350400"), ("ENG_CI_RUN", "version-step")],
          )
      });
      ```

      `cargo_step_env` does not exist yet. It would be `cargo_step` with
      `Command::envs`. A separate `CARGO_TARGET_DIR` for that run would
      avoid rebuilding the main target twice.

17. **Amendment from session 12** (to proposals 7 and 8, and amendment 11;
    the text above is kept as written). T4 is built, so these can be
    applied:
    - Proposal 8: protect `crates/eng-input/tests/live_input.rs`,
      criterion 18's harness, with the matching `.github/CODEOWNERS`
      line. It pins a known-answer fingerprint of the commands from a
      fixed timeline (`0x4d12_793a_10d0_b98f`). Protecting the file also
      makes that answer the owner's, which is what keeps "the same
      timeline gives the same commands on every target" honest. Leave
      `crates/eng-input/tests/bindings.rs` and `tests/quantize.rs`
      unprotected: they cover names, hints, messages and details beyond
      the criterion.
    - Proposal 7, X7: the `live-input` step in `cargo xtask ci --job
      test`. In `xtask/src/ci.rs`, `fn test`, after `exit-codes`:

      ```rust
      steps.run("live-input", || {
          tests(root, Some(("eng-input", "live_input")))
      });
      ```

    - **The protected-paths list and `.github/CODEOWNERS`: proptest
      regression files.** Add `"**/*.proptest-regressions"` beside
      `"**/proptest-regressions/**"`. For a test in `tests/` (where
      every proptest here is), proptest 1.12 finds no `lib.rs` or
      `main.rs` above the file, so it writes
      `<test file>.proptest-regressions` next to it. The present
      pattern does not match that. Session 12 saw it happen. Until then a
      failing proptest's regression file is not protected, and the guard
      would let an agent commit or delete one. Amendment 11's plan (the
      file reaches the owner with the fix) still applies.
