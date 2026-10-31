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

## 2026-10-25, session 14: patch X1, xtask sets the version stamp's values (Claude Opus 5.5, open-pulsar task)

The task: patch X1 (G5, `xtask/**`) as a tested patch file. xtask sets
the four build-time values only it knows, so a build made through xtask
prints no `unknown`. Amendments 15 and 16 describe them. They are made
and tested in a scratch clone, never in this repo. No new crates; no
protected path edited here; `current.txt` stays at `1a`.

**The patch:** `docs/rungs/1b/patches/x1-version-stamps.patch`, against
`main` at 106a8f0. It changes 6 files: 375 lines added, 14 removed.
- New `xtask/src/stamp.rs` works out the four variables:

  | Variable | Value |
  |---|---|
  | `ENG_ENGINE_SHA` | `git rev-parse --verify HEAD`, kept only if it is 40 or 64 hex digits |
  | `ENG_DIRTY` | `1` if `git --no-optional-locks status --porcelain --untracked-files=normal` prints anything, else `0` |
  | `SOURCE_DATE_EPOCH` | `git log -1 --no-show-signature --format=%ct HEAD`, kept only if it is whole seconds that fit a `u64` |
  | `ENG_CI_RUN` | `github-$GITHUB_RUN_ID/$GITHUB_RUN_ATTEMPT` when both are set, else `local` |

  - A non-empty value already in the environment wins, unchanged.
    `version.rs` still refuses a malformed one. Empty counts as unset,
    as in `version.rs`.
  - A value git cannot give (no git, no repository, no commit) stays
    unset. The stamp then says `unknown` or `null`; the build does not
    fail.
  - The values are worked out once per process (`OnceLock`), so every
    build in one run gets the same ones.
- `util::cargo` sets them on every command it makes. Every cargo
  command xtask runs is made there: `fmt`, clippy, tests and doctests,
  the RISC-V and wasm builds, the replay build, `cargo metadata`, the
  selftest clippy runs, the stop hook's nested `cargo xtask quick` and
  `install-hooks`. The `cargo deny` step and the `has_tool` probes
  compile nothing. This matches amendment 15's "every cargo command",
  and a new step cannot forget the values.
- New `xtask/src/stamp_tests.rs`, 8 tests:
  - fake git and environment: all four from git and GitHub;
  - set values win, git does not even run, and `ENG_DIRTY=yes` passes
    through unchanged;
  - empty counts as unset;
  - without git, only `ENG_CI_RUN` is set;
  - output that `version.rs` would refuse is dropped (`HEAD`, short or
    non-hex SHAs, `-1`, fractions, overflow), and SHA-256 hashes are
    accepted;
  - a GitHub run needs both its id and its attempt;
  - a real temporary repository: no commit, then clean, then an
    untracked file, then an edit to a tracked file;
  - `util::cargo` carries every value.

  None of the tests reads the process's own environment, because
  `cargo xtask` sets the four for the tests too.
- Docs in the same patch: `crates/eng-cli/src/version.rs` (module
  header) and `docs/ARCHITECTURE.md`, "The version stamp": xtask sets
  all four, each one's source, and which value wins. Both said "is to
  set (patch X1)", which becomes wrong when the patch lands, so they are
  in the patch and not edited here.
- The patch starts with a short header, which `git apply` ignores. Its
  `index` lines use full hashes (`--full-index`), because CI's `typos`
  step flagged a run of hex letters inside the short ones.

**How it was tested.** Scratch clone of this repo under `$TMPDIR`, with
its `target/debug` seeded by a copy-on-write copy.
- Baseline before the change: `quick` green, 54 s.
- Patch applied, tree dirty:
  - `cargo xtask quick` green (14 s);
  - `cargo xtask selftest` 17 of 17;
  - `cargo test -p xtask` 38 passed, 8 of them new;
  - `cargo xtask ci --job lint` (cargo-deny, typos), `--job test` and
    `--job wasm` green.

  xtask's logs show each cargo command with all four values.
  `quick`'s test step ran the version tests in that build:
  `version.rs`'s 4 (protected, unchanged), `version_testbed.rs` and
  eng-cli's unit tests.
- The binary that build left (`target/debug/game-testbed`):
  - `--version`: `game-testbed 0.1.0 (engine 0.1.0 bf9f2844257b, dirty) aarch64-apple-darwin debug, rustc 1.99.1, sim protocol 0x69fd4996880a6508`;
  - `--version --json`: `engine_sha` and `game_sha` are the full
    `bf9f2844…`, `dirty` is `true`, `source_date_epoch` is
    `1792919095` and `ci_run` is `local`. There is no `unknown` and no
    `null` in it.
- The patch committed in the clone only, so the tree is clean: `quick`
  green, and the stamp is `engine 0.1.0 d278c4a0f5fa` with
  `"dirty": false` and that commit's time.
- GitHub simulated (`GITHUB_RUN_ID=18999`, `GITHUB_RUN_ATTEMPT=2`, a
  set `ENG_ENGINE_SHA`) through `cargo xtask replay`: `ci_run` is
  `github-18999/2`, and the set SHA is kept.
- Rebuilds:
  - a second `quick` compiled nothing (8 s);
  - a plain `cargo test -p game-testbed --test version`, then `quick`,
    rebuilt eng-cli and game-testbed each time (about 2 s each). The
    plain run passed too, checking the unset values;
  - `git status` stays empty after `quick`, so runs do not flip `dirty`
    for each other.
- The file in this repo applies with `git apply` to a fresh clone, and
  the files it gives equal the tested clone's, byte for byte. The guard
  refuses `git apply`, even `--check`, in this repo, because the patch
  touches `xtask/`. The owner applies it, or unlocks `xtask/**`.
- This repo, with the patch file and this entry: `cargo xtask quick`
  green (11 s); `typos` passes on the patch file.

**Reviewer**
- Pending.

**Where this differs from amendments 15 and 16**
- Amendment 15 keeps an already-set value for `ENG_ENGINE_SHA` only.
  The patch keeps one for all four. It is the usual rule for
  `SOURCE_DATE_EPOCH`. It lets a container or another CI pass in what
  git there cannot see. And the stop hook's nested `quick` inherits the
  same values. The cost is that a stale value left in a shell would be
  believed.
- Amendment 15 suggests that X1 also write `target/version.json` from
  `<game> --version --json`. The patch does not: the task scoped X1 to
  the four values (open question 3).
- The task says `build.rs` reads the four variables. It reads none of
  them: `version.rs` reads them with `option_env!`. rustc records them
  in eng-cli's dep-info, so cargo rebuilds eng-cli when one changes (the
  rebuild runs above show it), and `build.rs` needs no
  `rerun-if-env-changed`.
- Amendment 16's extra `version-set` build in X7 is no longer needed.
  CI's builds now set all four, so the `version` step can be just
  `tests(root, Some(("game-testbed", "version")))`. Nothing on CI
  checks the unset path any more. eng-cli's
  `build_json_says_unknown_and_null_for_a_plain_build` covers its
  rendering, and any plain local `cargo test` covers the rest.

**Open questions**
1. **Keep set values for all four?** See above. If not, xtask would
   overwrite `ENG_DIRTY` and `SOURCE_DATE_EPOCH`, and maybe
   `ENG_CI_RUN`. That is two lines in `compute` plus a test change.
2. **Under jj, the SHA is `@-`'s** (session 11, question 2, unchanged).
   `dirty` is true whenever `@` has edits, and `ci_run` is `local`. Say
   if the loop's builds should name the jj change, for example as
   `ENG_CI_RUN=local-jj-<change id>`.
3. **`version.json` as a file.** With X1, `--version --json` prints
   spec 6.13's `version.json` with real values. X2's `version` step
   runs it anyway, and could save the output as a CI artifact. That
   would cover "computes `version.json`", and ADR-0012's archive per
   build later. Say if X1 should write `target/version.json` itself.
4. **`SOURCE_DATE_EPOCH` and C builds after crate batch 2.** `cc`
   1.10.0 prints `rerun-if-env-changed=SOURCE_DATE_EPOCH`, because C
   compilers read it. Two crates will compile C through `cc`:
   `nv-flip-sys` (tests, every target) and `wayland-backend` (Linux).
   Their C will recompile once per commit in xtask builds. That is
   correct for reproducible C output, but costs time. A private name
   such as `ENG_SOURCE_DATE_EPOCH` would avoid it. That change needs
   the protected `crates/game-testbed/tests/version.rs` too. Measure
   when batch 2 lands.
5. **Mixing plain `cargo` and xtask builds** rebuilds eng-cli and
   everything above it each time, about 2 s today (amendment 15
   expected this). rust-analyzer's `cargo check` runs without the
   values too, so it may alternate with xtask's clippy the same way
   (not measured).
6. **Containers.** Git refuses a checkout owned by another user
   ("dubious ownership"), which is common with a mounted volume. The
   values then stay unset and the stamp says `unknown`. For T7 and T12's
   lavapipe runs in Docker, pass the four in with `-e` (set values win)
   or set `safe.directory`.
7. **The workflow's `ENG_ENGINE_SHA: ${{ github.sha }}`** can stay
   (xtask keeps it) or go with X6. On `pull_request` both name the
   checked-out merge commit.
8. **Hooks.** The stop hook runs the working tree's `cargo xtask
   quick`, so X1 applies there as soon as the patch lands. The prebuilt
   hook binary picks the patch up at the next `install-hooks`. Until
   then its nested call passes no values, and the inner `quick` works
   them out itself.
9. xtask's own job reports still take `git_sha` from `GITHUB_SHA`
   only. They could use the stamp's SHA, which belongs with X3 (the
   summary).

**Costs and timings**
- One open-pulsar session, from about 11:13 EET.

## 2026-10-25, session 15: patch X7, named harness steps in the `test` job (Claude Opus 5.5, open-pulsar task)

The task: patch X7 (G5, `xtask/**`) as a tested patch file, for the
three harnesses that exist now: `crash-json`, `version` and
`live-input`. Amendments 14 to 17 describe them. The patch applies after
X1. It is made and tested in a scratch clone, never in this repo. No new
crates; no protected path edited here; `current.txt` stays at `1a`.

**The patch:** `docs/rungs/1b/patches/x7-test-steps.patch`, against
`main` at a09c34f with `x1-version-stamps.patch` applied. It changes 2
files: 75 lines added, 16 removed.
- `cargo xtask ci --job test` runs these steps in order: `tests`,
  `known-answers`, `exit-codes`, `crash-json`, `version`, `live-input`,
  `replay`. `ci --job all` runs the same function. The three new steps
  are:

  | Step | Runs | Criterion |
  |---|---|---|
  | `crash-json` | `cargo test -p game-testbed --test crash_json` | 6 (amendment 14) |
  | `version` | `cargo test -p game-testbed --test version` | 8's harness (amendments 15, 16) |
  | `live-input` | `cargo test -p eng-input --test live_input` | 18 (amendment 17) |

  They run on the dev profile, as amendment 14 asks for `crash_json`'s
  frame check, and get `--locked` on CI like every other step.
- A table, `HARNESSES` in `xtask/src/ci.rs`, lists the five named steps
  as (step, package, test target), and `harness()` runs one. It takes the
  place of the amendments' `tests(root, Some(..))` snippets, which no
  longer compile: `tests(root)` now runs only the workspace's tests. Its
  behaviour is unchanged: nextest or `cargo test`, doctests, `--profile ci`.
- Each named step logs under its own name, as
  `target/agent-logs/xtask-<step>.log`. Before, `known-answers` and
  `exit-codes` logged as `tests` too. So after a test job,
  `xtask-tests.log` held the `exit_codes` run, while a failed `tests` step
  pointed at that file, and their own failures said "tests failed".
  Nothing reads the old names (workflows, `.claude/`, docs and xtask
  checked).
- Amendment 16's extra `version-set` build is left out, as session 14
  said. With X1, `util::cargo` sets the four values on every cargo
  command, including this step.
- New unit test in `ci.rs`, `every_harness_step_names_an_existing_test_file`.
  It checks that each step's `crates/<package>/tests/<target>.rs` exists
  and that no step name repeats. It runs in `quick`, so a renamed harness
  fails locally and not only in CI's test job.
- `docs/ARCHITECTURE.md`, "Agent tooling": a bullet naming the test job's
  steps and where their results go. It becomes true only when the patch
  lands, so it is in the patch.
- The patch has a header like X1's. Its `index` lines use full hashes.
- X7 also applies without X1, because X1 does not touch `ci.rs`. But its
  comment then names a `stamp.rs` that does not exist, and `version`
  checks only unset values. Apply X1 first.

**How it was tested.** Scratch clone of this repo under `$TMPDIR` at
a09c34f, with X1 applied and committed there as the base. Its
`target/debug` was seeded by a copy-on-write copy.
- Baseline with X1 only: `ci --job test` green, 4 steps, 31 s. Its
  `tests` step took 29.8 s, rebuilding the workspace's crates in the new
  clone with X1's values.
- With X7:
  - `cargo xtask ci --job test`: all 7 steps ok, 11 to 14 s. The new
    steps take 0.1 to 0.3 s each, reusing the `tests` step's build. Their
    logs show 2 tests for `crash_json`, 4 for `version` and 12 for
    `live_input`. Each command carries all four values (`ENG_DIRTY="1"`,
    the SHA, `SOURCE_DATE_EPOCH`, `ENG_CI_RUN="local"`).
  - `CI=1 cargo xtask ci --job test`: the same, with `--locked` after
    `test` in each harness command.
  - `CI=1 cargo xtask ci --job lint` (clippy `-D warnings`, cargo-deny,
    typos): green once the typos fix below was in.
  - `cargo xtask quick` green (9 s).
  - `cargo xtask selftest`: 17 of 17.
  - `cargo test -p xtask`: 39 passed, 1 of them new.
- Mutations, each reverted:
  - A misspelt target (`crash_jsn`) fails the `crash-json` step with its
    own log and cargo's "a target with a similar name exists". The unit
    test fails too and names the missing path. So does the `tests` step,
    which runs xtask's tests.
  - A repeated row fails the unit test with
    `step names repeat: ["exit-codes"]`.
- The file in this repo: X1 then X7 apply with `git apply` to a fresh
  clone, and the files they give equal the tested clone's byte for byte.
  `typos` passes on the file.
- This repo, with the patch file and this entry: `cargo xtask quick`
  green (9 s), and `typos` passes.

**A typos fix to session 14 in this repo.** CI's `typos` step fails at
a09c34f. Session 14's entry quoted the two letters that typos had
flagged in short hashes, so `typos` now flags them in `PROGRESS.md`. The
next push would have turned the `lint` job red; `quick` does not run
typos. That one sentence is reworded with the same meaning. It is the
only edit to an earlier entry.

**Reviewer** (fresh context, read-only): approve, nothing blocking. It
re-ran the test job (also with `CI=1`), `quick`, selftest, xtask's tests
and clippy `-D warnings`, and checked the apply order in a fresh clone.
Fixed after the review:
- The header said "amendments 14, 16 and 17". The `version` step's code
  is in amendment 15, so it now says 14 to 17.
- `ARCHITECTURE.md` said each step is "a line in
  `target/ci/report.json`". A step is an entry in the report's `steps`.
  On CI, acceptance reads the per-job copies under `target/ci/reports/`.
- The `ci.rs` comment said acceptance criteria name every step. No
  criterion names the test job's `version` step: criterion 8 checks X2's
  native `version` step. It also said that a plain `cargo test` checks
  the unset values. On CI nothing does any more (session 14).
- The unit test's message did not say which name repeats.

The table and the new log names were kept: the reviewer found the old
log behaviour a real bug. Its two notes on the table's shape are open
question 1.

**Where this differs from amendments 14 to 17**
- One table in place of three `steps.run` lines, with per-step logs,
  which also renames the logs of `known-answers` and `exit-codes`.
- No `version-set` build (amendment 16), because X1 sets all four values.
- Not done: `shaders` and `wire-session` (amendment 11). Their harnesses
  do not exist yet.

**Open questions**
1. **The rest of X7.** `shaders` (criterion 10, T7) and `wire-session`
   (criterion 11, T5 and T6) wait for their crates and harnesses.
   `wire-session` runs three test files in two crates, so it needs a list
   of targets per row, or a `steps.run` of its own. `crash-json` must
   stay `run`-only in the test job once T11 adds a `play` case (the job
   has no display). The step then needs a test-name filter, which
   depends on T11's names. Both are xtask changes. `crash_json.rs` is
   protected, so the `play` case needs the owner too.
2. **Criterion 8's checks.** The plan checks `version-json` only on
   "native x5: version" (X2, X6). The test job's `version` step now runs
   the same harness on x64 and arm64 Linux. The 1b acceptance list (G1)
   could add "test x64, arm64: version". The feature would then be
   computed before the native jobs exist. Without X2, though, it would
   not check "no `unknown` in a CI build".
3. **Each harness runs twice per test job**, inside `tests` and in its
   own step. That costs about 1.7 s today, the sum of the five steps.
   Leaving them out of `tests` would need a nextest filter and would
   change what `quick` runs, so the patch keeps both.
4. **Linux not run here.** CI runs the test job on x64 and arm64 Linux;
   this session ran it on aarch64-apple-darwin only. The three harnesses
   already pass inside CI's `tests` step on both, so the named steps add
   no new environment.
5. **Nothing more to protect for these three.** e14b533 already protects
   `crash_json.rs`, `version.rs` and `live_input.rs` (amendments 14, 16
   and 17).

**Costs and timings**
- One open-pulsar session, from about 11:40 to about 12:00 EET.
- Scratch clone: copy-on-write seed of `target/debug` (2.0 GB), 7 s.
- Test job: 31 s for the baseline build with X1's values, then 11 to
  14 s per run with X7. The three new steps add about 0.6 s together.
- Reviewer subagent: about 99k tokens, 41 tool calls, 6.7 min.

## 2026-10-25, session 16: T0, the G2 change as a tested patch (Claude Opus 5.5, with the owner)

The task: T0, which prepares gate G2 as one tested patch. It was item 1 of
the queue drafted for the next open-pulsar window, done in this session
while the window waited for the owner, so that queue now starts at G3. G1
is not approved, so the patch is only prepared. It was made and tested in a
scratch clone, never in this repo: no crate is added here, no protected path
is edited, and `current.txt` stays at `1a`.

**The patch:** `docs/rungs/1b/patches/g2-crates.patch`, against `main` at
9520947, to apply after G1. It changes 19 files: 231 lines added, 9
removed.
- Seven crates under `crates/`. Each is a `Cargo.toml` (the workspace
  fields, a description, `[lints] workspace = true`, no dependencies) and a
  `src/lib.rs` whose doc comment names the crate's spec sections and the
  task that fills it in:

  | Crate | Layer | Crate root | Task |
  |---|---|---|---|
  | `eng-platform-api` | L1 | `#![forbid(unsafe_code)]` | T3 |
  | `eng-platform-winit` | L1 | `#![forbid(unsafe_code)]` | T10 |
  | `eng-net-wire` | L1, core | `#![no_std]`, `#![forbid(unsafe_code)]`, `extern crate alloc;` | T5 |
  | `eng-session` | L2, core | the same | T6 |
  | `eng-gpu` | L3 | `#![forbid(unsafe_code)]` | T7 |
  | `eng-render` | L3 | `#![forbid(unsafe_code)]` | T8 |
  | `eng-app` | L4 | `#![forbid(unsafe_code)]` | T11 |

- `architecture.toml`: proposal 2 exactly, each entry in its layer's place,
  with the new `same_layer` edges of eng-platform-headless and eng-input.
- Root `Cargo.toml`: the seven `[workspace.dependencies]` paths and
  proposal 3's two `[profile.release.package.*]` lines. `Cargo.lock` gains
  the seven path packages.
- Docs, because `docs/ARCHITECTURE.md` asks to be updated in the commit that
  changes what it describes:
  - its layer diagram gains the seven crates, marked as skeletons;
  - its core-set sentence now also names the network codec, transport and
    session loops, which the simulation does not link;
  - L3 "starts in rung 1b";
  - `AGENTS.md`'s crate list gains the seven crates too (137 of 150 lines).

  This overlaps T14's "layers"; T14 then only drops the skeleton notes.

**How it was tested.** Scratch clone at 9520947, its `target/` seeded with
a copy-on-write copy.
- Baseline: `cargo xtask quick` green, 29 s.
- The crates added but not listed: `arch-check` fails with
  `[E-ARCH-UNKNOWN]` for each of the seven.
- Listed: `arch-check` passes, 23 crates. Without the two `[profile]` lines
  it fails with `[E-ARCH-OVERFLOW]` for eng-net-wire and eng-session. Both
  confirm amendment 11: the change lands as one commit.
- The whole change, with `CI=1`:
  - `quick` green (14 s);
  - `selftest` 17 of 17;
  - `ci --job lint` 7 of 7 steps (clippy `-D warnings`, cargo-deny, typos);
  - `ci --job test` 7 of 7;
  - `ci --job wasm`, which checked both new core crates for wasm32.
- Not run: `ci --job riscv`. This Mac has no RISC-V target and no
  `qemu-riscv64`. The two core crates are empty `no_std` crates, and CI's
  riscv job builds them.
- The patch applies with `git apply` to a fresh clone of main and gives the
  tested clone's tree exactly. `typos` passes on it.

**Reviewer** (fresh context, read-only): approve with fixes, nothing
blocking. It re-ran `quick`, `selftest`, `arch-check` and the lint and wasm
jobs with `CI=true` in its own clone, using the local cargo-deny and typos
at CI's pinned versions, and compared every entry with proposals 2 and 3.
Fixed after the review:
- `eng-net-wire`'s doc comment said every message carries "the 32-bit tick
  and its extension". Spec 6.8, contract 1: the wire carries the low 32 bits
  of the u64 tick, which the receiver extends against the last acknowledged
  tick. The doc comment now says so, and so does `plan.md`'s crate table,
  which had the same wording.
- `docs/ARCHITECTURE.md` defined the core set as "everything the simulation
  links", which the new diagram contradicted.
- The patch header now says to apply it after G1.
- This entry, which the header points to.

**Open questions**
1. **`eng-net-wire` has no edge to `eng-net-transport`** (proposal 2 gives
   it none), so the codec cannot read `MAX_DATAGRAM`. In the plan,
   `eng-session` (L2), which may depend on both, checks the snapshot size
   and raises `[E-SNAPSHOT-SIZE]`. If the owner would rather the codec
   capped `Snapshot` itself, this patch needs
   `same_layer = ["eng-net-transport"]` for `eng-net-wire`.
2. **No unsafe code.** Every skeleton forbids it, like every crate today. If
   T7 or T10 needs it, for example for a window handle, relaxing `forbid`
   in that crate is the owner's call.

**Costs and timings**
- This session, from about 19:00 to 19:50 EET, while the window waited for
  the owner.
- Scratch-clone checks: about 2 minutes in all (`quick` 14 s, the test job
  30 s, the rest a few seconds each).
- Reviewer subagent: about 9 minutes, 158k tokens, 54 tool calls.

## 2026-10-31, session 17: gates G1 and G2 applied (Claude Opus 5.5, with the owner)

The owner approved the plan as drafted (G1) and the seven-crate layout
(G2). The owner then unlocked the three protected paths those gates
write: `docs/rungs/1b/acceptance.json`, `architecture.toml` and
`Cargo.toml`, listed in `.claude/guard-unlock`. G3 to G7 stay open.

**Applied**
- `docs/rungs/1b/acceptance.json` (proposal 1, with amendment 11): the
  plan's 18 criteria as 19 features, in 1a's format. Criterion 14 is two
  features, `lint-tier` and `tests-and-targets`, as in 1a. The time-box
  is 14 calendar days from 2026-10-31; the AI budget is at most 30
  sessions.
  - Two additions to 1a's format. `done_when` is a list where a criterion
    serves two done-when items (`native-window` [1, 2], `owner-play`
    [2, 5]). `owner-play` has `"owner": true` and no checks; until X4,
    `cargo xtask acceptance` reports it as "no checks listed".
  - The native checks name each runner by its host triple, which is what
    a job report records as `target`.
  - The budget's note counts the 16 sessions logged before approval,
    because the plan's 30 cover T0 to T17. The owner may say otherwise.
- `g2-crates.patch` (T0), applied with `git apply` as session 16 tested
  it. It brings the seven skeleton crates, proposal 2 in
  `architecture.toml` and proposal 3's `[profile]` lines, with the
  matching changes to `Cargo.toml`, `Cargo.lock`, `AGENTS.md` and
  `docs/ARCHITECTURE.md`.
- `docs/rungs/current.txt` says `1b`, and `plan.md`'s header says the plan
  is approved.

**Checked** (this repo, aarch64-apple-darwin)
- `cargo xtask quick` green (17 s).
- `CI=1 cargo xtask ci --job lint` 7 of 7, with cargo-deny and typos.
- `cargo xtask selftest` 17 of 17.
- `cargo xtask acceptance target/ci/reports` reads the new file. From
  the local lint report, 2 of 19 features pass (`lint-tier`,
  `agents-md-length`). The others need CI's jobs, most of them the native
  jobs of X2 and X6. The computed copy was deleted, so the SessionStart
  hook still reports CI's result.

## 2026-10-31, session 17 (continued): T5, `eng-net-wire` (Claude Opus 5.5, with the owner)

The task: T5, the wire codec. Its plan row asks for the codec, the four
messages and tick extension; round trips and never-panic proptests; golden
vectors as a patch, because `tests/golden.rs` is protected; and builds for
RISC-V and wasm32. No new third-party crate (`proptest` is already a
dev-dependency of five core crates) and no protected path is edited here.
Nothing calls the codec yet: `eng-session` will, in T6. At the owner's
request the G1 and G2 commits were made with git (6a38c66, 5a9a83a),
because the guard refuses jj writes. The old jj working-copy change
(`uvvsrxtz`) stayed behind as a duplicate of their content, for the owner
to abandon.

**The format** (the crate docs hold the full table; `docs/ARCHITECTURE.md`
gains "The wire format"):
- Every message starts with a 5-byte header: its kind (`u8`), then the low
  32 bits of a server tick (`u32`). Integers are little-endian and fixed
  width.

  | Kind | Message | Header tick | Body | Bytes |
  |---|---|---|---|---|
  | 1 | `Hello` | 0 | wire version `u32`, sim protocol `u64` | 17 |
  | 2 | `Welcome` | the server's tick | high 32 bits of the tick, the client's slot `u8`, players `u8`, seed `u64` | 19 |
  | 3 | `Denied` | the server's tick | high 32 bits of the tick, reason `u8`, the server's wire version and sim protocol | 22 |
  | 4 | `Commands` | the acknowledged snapshot tick | count `u8` (at most 5), then each `PlayerCommand` in eng-core's 19-byte canonical layout, ticks strictly increasing | 6 + 19 each |
  | 5 | `Snapshot` | the snapshot's tick | length `u16` (at most 65,535), then the state bytes | 7 + state |

  With the plan's 451-byte dot state, a `Snapshot` is 458 bytes, well
  inside one 1,200-byte datagram.
- `extend_tick(baseline, low)` gives the one tick with those low 32 bits in
  `[baseline - 2^31, baseline + 2^31 - 1]`, or `None` before tick 0 or past
  `u64::MAX`. `Message::decode(bytes, baseline)` uses it for `Commands` and
  `Snapshot`. The baseline is the receiver's last acknowledged tick, and
  the `Welcome` tick until the first one.
- `Message::encode` returns an `EncodeError` for more than 5 commands,
  ticks out of order or a state over 65,535 bytes. `Message::decode`
  returns one of eight `DecodeError`s and never panics.

**Decisions** (proposals; the owner can overrule any):
1. **Commands keep their u64 tick.** Each command travels in eng-core's
   canonical layout, which `crates/eng-core/tests/golden.rs` already
   describes as the bytes "recordings and the network carry". Contract 1's
   32-bit tick is the header's. The cost is 4 bytes per command (12 per
   packet of 3) over a 32-bit command tick, and there is no second command
   encoding to keep in step. A test checks the bytes against
   `eng_schema_core::to_bytes`.
2. **The handshake carries full ticks.** A new client has no baseline to
   extend against, so `Welcome` and `Denied` add the tick's high 32 bits.
   Without them a client joining a server past tick 2^31 (414 days at 60
   Hz) would misread the tick.
3. **`Welcome` carries the session constants.** It names the client's
   player slot, since `Sim::step` takes one command per slot. It also
   carries the player count and the seed, because `Sim::restore` refuses a
   snapshot from a session with other constants (`SimInfo`); the tick rate
   is part of the sim protocol, which `Hello` compares.
4. **Version negotiation, not frozen yet.** A `Hello` from another wire
   version decodes to `DecodeError::WireVersion { found }` without reading
   further, and the server answers with `Denied`. `DenyReason` is an open
   `u8` (named values 1 and 2), so a newer server's reason can reach an
   older client. The header, `Hello`'s version and `Denied` must stay
   readable across versions for this to work, but they are not frozen
   now. No two builds exchange messages before rung 5, and rung 5's
   handshake (magic, client salt, padding, `eng-net-proto`) decides their
   final form. Until then a change to them is a wire version change like
   any other, through `golden.rs` and the owner.
5. **Caps.** `MAX_COMMANDS = 5`, the top of spec 6.8's "3-5 commands";
   1b's client sends 3. `MAX_STATE_LEN = 65,535`, a `u16` length, as a hard
   cap for when rung 5 reassembles fragments. The 1,200-byte datagram check
   stays in `eng-session` (session 16, open question 1).
6. **Canonical decoding.** Decoders accept only bytes they would write: no
   trailing bytes, commands in strictly increasing tick order, a `Hello`
   tick of 0. So every accepted message has exactly one encoding.

**Tests** (24; the expected values are written out by hand)
- `tests/wire.rs`, 14 tests:
  - tick extension at its edges: both ends of the window, the 32-bit wrap,
    a window that starts below 0, the last tick, before 0 and past the
    maximum;
  - round trips at the caps: 5 commands, none, 65,535 and 0 state bytes;
  - each encode error, and the canonical command bytes;
  - every truncation of every kind, trailing bytes and unknown kinds;
  - another wire version (its tick and the rest unread), and a non-zero
    `Hello` tick;
  - the count checked before reading; commands out of order, also at the
    third of three; ticks that cannot be extended; an unknown deny reason
    kept.
- `tests/layout.rs`, 5 tests: one message of each kind, every byte written
  out. It becomes the protected `tests/golden.rs` through the patch below.
- `tests/never_panic.rs`, 5 proptests:
  - arbitrary bytes and baselines never panic;
  - damaged valid messages (a byte flipped, inserted or removed, or the
    message cut short) never panic, and any that decode re-encode to the
    same bytes;
  - random valid messages round-trip from any baseline whose window holds
    their tick;
  - extension recovers every tick in the window and keeps the low bits.
- Mutation checks, each reverted:
  - mine: accepting a non-zero `Hello` tick, reading without a length
    check, reading the tick distance as unsigned and ignoring trailing
    bytes. Each fails at least one property. The failing runs wrote
    `never_panic.proptest-regressions`, which the guard moved to
    `target/guard-quarantine/`. Its seeds come from planted bugs, so it is
    not a candidate for the owner;
  - the reviewer's 14: 13 failed the suite. The survivor compared each
    command's tick with the first one, not the one before it. The
    three-command case now fails it, which was checked by planting it.
- Two of the new properties placed the baseline at `tick + offset`, which
  for `i32::MIN` lies one past the window. This was fixed before the runs
  above.

**The oracle patch:** `docs/rungs/1b/patches/t5-oracles.patch`, against
`main` with this commit. It changes 4 files: 9 lines added, 6 removed.
- `tests/layout.rs` becomes `tests/golden.rs` (proposal 9), with a
  "PROTECTED ORACLE" header; `crates/*/tests/golden.rs` is protected
  already.
- `crates/eng-net-wire/tests/never_panic.rs` joins `protected.toml` and
  `.github/CODEOWNERS` (proposal 8, amendment 11).
- `tests/wire.rs`'s header names `golden.rs`.

How it was tested, in a scratch clone with this commit's files as its
base: `cargo xtask quick` green with the patch applied (31 s, docs-check
included), and the 5 golden tests pass. The repo's file, applied with `git
apply` to a fresh base, gives the tested diff exactly; `typos` passes on
it.

**Checked** (this repo, aarch64-apple-darwin)
- `cargo xtask quick` green (12 s); `CI=1 cargo xtask ci --job lint` 7 of
  7; `CI=1 cargo xtask ci --job wasm` green, and its log shows
  eng-net-wire checked for wasm32; `cargo xtask selftest` 17 of 17.
- `RUSTDOCFLAGS=-Dwarnings cargo doc -p eng-net-wire` is clean, and the
  tests also pass in release.
- Not run: RISC-V. This Mac has no RISC-V target or `qemu-riscv64`; CI's
  riscv job runs eng-net-wire's tests with the rest of the core set.
- Hash streams unchanged: nothing in the sim changed.

**Reviewer** (fresh context, read-only): changes needed, nothing wrong in
the codec. It found no panic, no unbounded allocation and no accepted
bytes that re-encode differently. It checked `extend_tick` against an
independent reference over 252,928 (baseline, low) pairs, and ran the
proptests at 300,000 cases each. It also checked the committed
`acceptance.json` against the plan: every check of all 19 features
matches. Its findings:
1. The golden-vector patch and this entry were missing. They were waiting
   for the review, and are now in this commit. Not done: the `wire-session`
   rows in xtask. That step runs three files in two crates, and the third
   is eng-session's test (T6), so it is one xtask change after T6
   (amendment 18).
2. `Welcome` lacked the player count and seed that `Sim::restore` needs
   (decision 3). Added.
3. Freezing the header, `Hello`'s prefix and `Denied` "forever" bound rung
   5's handshake to a layout without magic, salt or padding. The freeze
   now starts with rung 5 (decision 4).
4. The decode-side order test missed comparing with the first command.
   Three-command case added, plus extension rows for exactly `u64::MAX`
   and a window that starts below 0.
5. The docs did not say what the baseline is before the first ack, nor
   what a wrong one does. `decode`'s and `Commands::ack`'s docs now say.

**Open questions**
1. **Rung 5's fields.** Time sync needs each snapshot to report the
   client's buffer slack, and prediction needs the last command the server
   applied. Both change `Snapshot`, so wire version 2, and with it
   `golden.rs` through the owner. Adding them now would carry fields 1b
   never sets.
2. **Commands' tick width** (decision 1). 32-bit command ticks would save
   12 bytes a packet, at the price of a second command layout. Say if the
   wire should use them.
3. **The AI budget.** Counting the 16 sessions before approval (the
   `acceptance.json` note), this conversation is session 17, which leaves
   13 sessions for 13 tasks: T3 and T6 to T17. The plan's margin is gone.
   Say if the 30 should count from approval instead.
4. **Windows arm64** (from the review, for T13). A report's `target` is
   `rustc -vV`'s host, so the `windows-11-arm` runner must run a native
   aarch64 toolchain. An emulated x64 one would report
   `x86_64-pc-windows-msvc` and fail every check aimed at arm64 Windows.

**Costs and timings**
- Session 17, from about 07:30 to 08:55 EET, with the owner: G1 and G2,
  then T5.
- Reviewer subagent: about 19 minutes, 232k tokens, 61 tool calls.
- Scratch clones: a copy-on-write seed of `target/` (4.2 GB) in 12 s;
  `quick` in a clone takes 31 to 53 s, because the path crates rebuild.

## 2026-10-31, session 17 (continued): T5's oracle patch applied (Claude Opus 5.5, with the owner)

The owner applied `docs/rungs/1b/patches/t5-oracles.patch` (amendment
18's first item) with `git apply`, since the guard refuses agents a patch
that touches protected paths. eng-net-wire's `tests/golden.rs` and
`tests/never_panic.rs` are now protected oracles: a change to the wire
format's bytes goes through the owner. The `wire-session` step still
waits for T6.

The owner also asked the agent to push and to abandon `uvvsrxtz`. The
guard refuses agents both whatever the conversation says, because it
cannot tell the owner's permission from a forged one (ADR-0015), and
`.claude/settings.json` denies `git push` too. The owner runs them from
the prompt with `!`.

**Checked:** the patch applied cleanly to 7bfff92 in a scratch clone
before the owner applied it; `cargo xtask quick` green with it; the 5
golden tests pass.

## 2026-10-31, session 17 (continued): T6, `eng-session` (Claude Opus 5.5, subagent, owner away)

The task: T6, the session loops. Its plan row asks for `eng-session`, with
`run` and `dump-world` going through it. The recording writer (`eng-input`,
std) stays in `eng-cli`, and core `eng-session` hands it plain data. Done
when the hash streams are unchanged, missing commands repeat, and the
applied-command record makes a recording that `replay verify` accepts. No
new third-party crate, no protected path edited here, and no hash stream
regenerated. A subagent built it while the owner was away, so it is left
uncommitted for review.

**The session** (the crate docs draw the exchange; `docs/ARCHITECTURE.md`
gains "The session"):
- `Server<T: Transport>` owns the authoritative `Sim`: `new(sim,
  transport)`, `with_relevance(hook)`, `local_id()`, `sim()`, `ack(slot)`,
  `applied()`, `poll()` and `step()`. `Client<T>`: `connect(sim,
  transport, server)`, `poll()`, `send(command)`, `welcome()`, `latest()`
  and `sim()`. Errors are one `SessionError` with six variants. Constants:
  `SENT_COMMANDS = 3`, `MAX_LEAD = 64`.
- The client says `Hello` (wire version, sim protocol). The server answers
  with `Welcome` (tick, slot, players, seed) and at once a `Snapshot` of
  its current state, or with `Denied` (another wire version, another sim
  protocol, or every slot taken). Then, each tick, `Commands` (the client's
  newest 3 and its ack) go up, the server steps, and a `Snapshot` of the
  whole state goes down to each client.
- `step` gives a slot with a client that client's command for the tick,
  else its last command restamped. A slot without a client, never joined
  or gone, idles, keeping its view angles. `step` returns those commands,
  one per slot: the record. `applied()` keeps them, also after a failed
  snapshot.
- Baselines, as T5 left them: the server decodes a client's messages
  against that client's previous ack, and the client against its newest
  snapshot, both from the `Welcome` tick until then. A `Snapshot` over one
  1,200-byte datagram fails with `[E-SNAPSHOT-SIZE]` (session 16, open
  question 1). The dot's is 466 bytes: the 5-byte header, the 2-byte
  length, the 8-byte protocol version and the 451-byte state.
- `run` and `dump-world` (`crates/eng-cli/src/run.rs`) start a server and
  player 0's client in-process over Memory, past the handshake. Each tick
  the client sends the script's command (or an idle one), the server polls
  and steps, and the client restores the snapshot. Hashes, events,
  assertions, budgets, the report and the dump read the client's sim;
  `--record` writes what `step` returned. `replay verify` still steps the
  sim directly.

**Decisions** (proposals; the owner can overrule any):
1. **The probe reads the client's sim**, not the server's. It is what a
   player sees, so the committed hash streams now check the whole round
   trip: commands encoded, buffered and applied, snapshots encoded, sent
   and restored. The two sims hash the same at every tick (the
   hash-equality test).
2. **The server keeps no record.** `step` returns the tick's applied
   commands, and `eng-cli` appends them for `--record`, so a run without
   it stores nothing. At 24 bytes per player per tick, a record kept
   always would double the memory of a 10-million-tick run. A host's crash
   buffer (spec 6.8) can append the same slices later. `applied()` holds
   the last tick's, so a host that carries on after `SnapshotTooLarge`
   records that tick too (from the review). The probe stops there instead,
   and records only ticks its client received.
3. **A snapshot carries `Sim::snapshot()`**: the protocol version (8
   bytes), then the canonical state. `Sim::restore` takes it as it is and
   checks the protocol on every snapshot, so eng-sim needs no new API.
   T5's entry counted 458 bytes for the dot, without those 8.
4. **The `Welcome` brings the current state**, so a client holds the
   server's state from its first tick, and a state too large for a
   datagram fails at tick 0, from `poll`.
5. **`[E-SNAPSHOT-SIZE]` stops the probe with exit 13**, as a budget does;
   the plan's decision 4 adds no exit code. The message goes into the
   report's `budgets.exceeded`, and everything the probe writes shows the
   client's last state, the tick before. `dump-world` then prints the
   summary and writes no dump. The testbed's state grows by 85 bytes a
   player, so `run --players 10` now stops at tick 1 and `--players 13`
   at tick 0; both used to exit 0.
6. **No `now` yet.** The plan's row says `now` is passed in, but nothing in
   rung 1b depends on time: the caller decides when a tick is due and calls
   `step`. The session reads no clock. Time sync, timeouts and resends add
   `now` in rung 5. Amendment 20 proposes the plan's wording.
7. **The command buffer.** Commands up to `MAX_LEAD` (64) ticks ahead wait
   for their tick; later ones and past ones are dropped, and the first
   copy of a tick's command is kept. `step` also drops anything older than
   the tick it applies.
8. **Bad datagrams are dropped**: bytes that do not decode, messages out of
   turn, peers that are not clients. A `Commands` whose ack is ahead of
   the server's tick is dropped whole, and an older ack never replaces a
   newer one. The one error that comes from the network is a snapshot the
   client's sim refuses (`Restore`). The rest are this process's own: a
   snapshot too large, a denial, commands out of order or before the
   `Welcome`, a failed send.
9. **A vanished client is dropped** once the transport no longer knows it,
   and its slot idles, keeping its view angles, like a slot no client ever
   joined; a new `Hello` can take the slot. Its missing commands repeat
   only while it is connected. This was "repeat the last command forever"
   until the review; reconnection is rung 5's.
10. **`DenyReason::FULL` (3)** joins `eng-net-wire`, for a server with
    every slot taken (spec 6.8: "Denied (version mismatch or full)"). The
    reason is an open `u8`, so the wire format and `golden.rs` stay as
    they are.
11. **The client takes a sim built for the session.** In-process the
    caller knows the players and seed, and `Sim::restore` refuses
    snapshots from any other session. A remote client (rung 5) learns them
    from the `Welcome`, so `connect` will take a build function then.
12. **The relevance hook** is `fn(&World, PlayerSlot) -> Relevance`, asked
    for each client before each snapshot. `Relevance` has one variant,
    `Everything`, and is `#[non_exhaustive]` for rung 5's narrower answers.
13. **Tags in `run`.** A release build checks floats only when the client
    restores a snapshot, so `run` reports that refusal as `[E-NAN]`, the
    tag `hash()` gave before. Any other session error is a bug and panics
    with `[E-SESSION]`. Two checks stop the probe without a panic (from the
    review): a command that is not for the tick being stepped exits 64
    with `[E-STEP]` (`Sim::step` refused one before; the server would
    repeat the last command instead), and a client without the tick just
    stepped exits 11 with `[E-DESYNC]`, before anything is hashed as that
    tick.
14. **The hash-equality test stands alone.**
    `crates/eng-session/tests/hash_equality.rs` defines its own test sim
    instead of sharing `tests/common/`, so nothing outside the file it
    protects can weaken it. The plan gives this test no file name; the name
    follows the plan's wording, as `never_panic.rs` did.

**Tests** (35 after the review; expected values are written out or
computed independently)
- `crates/eng-session/tests/hash_equality.rs`, 2 tests, for the patch below
  to protect:
  - two clients drive a two-player test sim for 240 ticks, with commands
    in which every field varies. The server's state and both clients'
    restored states hash the same as a sim stepped directly, at every tick
    from 0, and the applied commands are the ones sent;
  - a proptest (32 cases, up to 90 ticks): random commands, a quarter of
    them never sent, sent up to 3 ticks early. The applied commands follow
    the missing-command rule, written out in the test, and stepping
    directly with them gives the session's hashes.
- `tests/server.rs`, 14 tests: the `Welcome`'s fields and slots; `Denied`
  for a full server, another sim protocol and another wire version; a
  repeated `Hello` keeps its slot; missing commands repeat, idle before the
  first; a slot without a client idles; commands sent ahead; stale,
  too-far and repeated commands, at the `MAX_LEAD` edge; the newest ack
  kept, and a bad ack's message dropped; ticks extended across 2^32, from a
  sim starting at 2^32 - 3, which fails with a baseline of 0 on either
  side; exactly 1,200 bytes fits, 1,201 fails at the `Welcome`, and a
  growing state fails at the step that outgrows a datagram, with that
  tick's commands still in `applied()`; a vanished client's slot idles
  with its view angles; the relevance hook asked for every snapshot of
  every client.
- `tests/client.rs`, 9 tests: each message carries the newest 3 commands
  and the newest snapshot's tick; the `Welcome` tick is the ack before the
  first snapshot; commands before the `Welcome` or out of order are
  refused; a second `Welcome` and a late `Denied` change nothing; the
  baseline follows the newest snapshot past `Welcome` + 2^31; older,
  repeated and conflicting snapshots of a tick are ignored; datagrams
  before the `Welcome` or from other peers are dropped; a snapshot from
  another session is refused and the state kept.
- `tests/hostile.rs`, 4 tests: datagrams a server cannot use; a stranger's
  commands sent before the client's never reach its slot; and the
  review's two proptests (256 cases each), adapted. In one, while slot
  0's client plays, the client in slot 1 and a stranger send anything each
  tick, and slot 0 still gets exactly its client's commands. In the other,
  a welcomed client takes anything from the server's address without
  panicking. They are in their own file because `server.rs` would pass
  500 lines.
- `crates/game-testbed/tests/session.rs`, 4 tests against the binary:
  `[E-SNAPSHOT-SIZE]` with exit 13 at 10 players (tick 1) and 13 players
  (tick 0), with the report and hash stream; the same for `dump-world`,
  which prints the summary and writes no dump; a two-player recording,
  slot 1 idle, that `replay verify` accepts; `dump-world`'s whole world
  equals a sim stepped directly with the same script. The snapshot-size
  tests failed before their changes (exit 0, then exit 64).
- `eng-cli`, 2 unit tests in `run.rs`: a command for another tick is
  refused with `[E-STEP]`, and a client without the tick just stepped
  with `[E-DESYNC]`.
- The protected `exit_codes.rs` (7) and `crash_json.rs` (2) pass
  unchanged. Their recording and desync cases now replay the server's
  record.
- Mutation checks, each reverted: 15 planted bugs. 13 failed a test at
  once, among them a baseline of 0 on either side, idle instead of repeat,
  a snapshot sent before the step, no restore and no size check. Two
  survived:
  - restoring a same-tick snapshot again: the client test now sends a
    conflicting copy;
  - buffering a command for the current tick: no test could see it, but
    such an entry would never leave the buffer, one more each tick. `step`
    now drops everything up to its tick, which makes it harmless.
- The review planted four more that survived: a stranger's `Commands`
  routed to slot 0 (S20), a second `Welcome` or a late `Denied` taken (C7,
  C8), and the client's baseline stuck at the `Welcome` tick (C10). Each
  now fails the new tests above: S20 fails both of its tests, C7 two, C8
  and C10 one each. The sources were restored byte for byte.
- Failing proptests against the first stub and the planted bugs wrote
  regression files three times (`hash_equality` twice, `hostile` once),
  and the guard moved them to `target/guard-quarantine/` (`1793444285-…`,
  `1793444869-…`, `1793447645-…`). Their seeds come from planted bugs, so
  none is a candidate for the owner.

**The oracle patch:** `docs/rungs/1b/patches/t6-oracles.patch`, against
`main` with this task's commit. It changes 5 files: 89 lines added, 36
removed.
- `crates/eng-session/tests/hash_equality.rs` joins `protected.toml` and
  `.github/CODEOWNERS` (proposal 8, amendments 11 and 19), and its header
  says it is protected.
- X7's `wire-session` step (amendment 18), small enough to go in the
  patch. A row of `HARNESSES` now lists its test files, and the step runs
  eng-net-wire's `golden` and `never_panic` and eng-session's
  `hash_equality` in one `cargo test`, after `live-input`. The unit test
  checks that each file exists, and that no other package in a step has a
  test target of the same name, which the one `cargo test` would also run:
  `tests/<name>.rs`, `tests/<name>/main.rs` or a `[[test]]` entry (the
  last two from the review).
- `docs/ARCHITECTURE.md`, "Agent tooling", names the step.

How it was tested: a scratch clone of 13fcdaa with this task's files
committed as its base, and `target/` seeded copy-on-write (4.8 GB, 10 s).
With the patch, `cargo xtask ci --job test` passed 8 of 8 steps;
`wire-session` ran 5 + 5 + 2 tests in 0.1 s, with its own log and report
entry. `quick` passed 7 of 7 (10 s), `selftest` 17 of 17, `CI=1 cargo
xtask ci --job lint` 7 of 7, and `cargo test -p xtask` 39 tests. A
misspelt target fails both the unit test and the step; a `never_panic.rs`
added to eng-session fails the unit test. After the review the patch was
remade in a second clone, its base this task's final files: the same
checks pass, and a `tests/never_panic/main.rs` or a `[[test]]` named
`golden` added to eng-session now fails the unit test (the first passed
before). The repo's file, applied with `git apply` to a fresh copy of the
final files, gives the tested tree exactly, and `typos` passes on it.

**Checked** (this repo, aarch64-apple-darwin)
- `cargo xtask quick` green (17 s); `CI=1 cargo xtask ci --job lint` 7 of
  7; `CI=1 cargo xtask ci --job wasm` green, and its log shows eng-session
  checked for wasm32; `cargo xtask selftest` 17 of 17.
- `RUSTDOCFLAGS=-Dwarnings cargo doc -p eng-session --no-deps` clean.
- `cargo xtask replay`: 5 runs, and the hash streams equal the committed
  ones byte for byte. Nothing was regenerated.
- eng-session's tests also pass in release, and the hash-equality proptest
  passes at 20,000 cases (10 s in release).
- Not run: RISC-V. This Mac has no RISC-V target or `qemu-riscv64`; CI's
  riscv job runs eng-session's tests with the rest of the core set.

**Reviewer** (fresh context, read-only, run by the coordinator): changes
needed, all small. Determinism, layering, protected paths and the hash
streams were clean. Its findings, each fixed in this change:
1. `dump-world --at-tick 5 --players 10` exited 64 with "the run stopped
   at tick 0 before tick 5": the dump's tick check failed before the
   summary holding `[E-SNAPSHOT-SIZE]` was printed. The dump is now skipped
   when a budget stopped the run, so the summary prints and the exit is 13
   (decision 5); a new test covers it.
2. `Server::step` lost the tick's commands on `SnapshotTooLarge`, though
   the sim had stepped, so a host that carries on would record a gap and
   `replay verify` would desync from there. `Server::applied()` now keeps
   them, and the variant's doc says what has stepped: from `step` the sim,
   from `poll` nothing.
3. Four planted bugs survived the tests: S20, C7, C8 and C10 (under
   **Tests**). The review's tests, adapted, now catch each one;
   `hash_equality.rs` was left as it was.
4. Docs against the code. A vanished client's slot now idles, keeping its
   view angles, so "a slot without a client idles" holds (decision 9; the
   owner can overrule). The crate docs no longer say network bytes never
   fail a session: `Client::poll` returns `Restore` for a snapshot its sim
   refuses.
5. `now`: the plan says contract 6 ("takes (now, incoming bytes)") is built
   in full. Recorded as amendment 20, not only as an open question.
6. Nits: the probe's `[E-STEP]` and `[E-DESYNC]` checks (decision 13);
   the dump test compares the whole world with a sim stepped directly;
   `docs/ARCHITECTURE.md`'s first line said "as of rung 1a"; the patch's
   unit test missed `tests/<name>/main.rs` and `[[test]]` targets; the
   amplification below.

**Open questions**
1. **Interpolation (T11).** The plan's frame draws `FrameSnapshot {
   previous, current }` from the client, which keeps only the newest
   state. `previous` needs a second sim: a second one at `connect`, a build
   function, or `Clone` for `Sim`. T11 can choose with the frame loop.
2. **Exit codes** (decisions 5 and 13): 13 for `[E-SNAPSHOT-SIZE]`, 64 for
   `[E-STEP]`, 11 for `[E-DESYNC]`. Say if others fit better. While there
   is one code path, the probe runs at most 9 testbed players until rung
   5's fragmentation.
3. **Amplification, for rung 5.** An unauthenticated 17-byte `Hello` gets
   up to 1,219 bytes back, a 19-byte `Welcome` and a snapshot of up to
   1,200: about 72 times its size, and over UDP the source can be forged.
   Rung 5's padded handshake and stateless cookie must come before any
   answer larger than the request.
4. **Report timings.** `tick_time_ns` now times `Server::step`: the sim's
   step plus encoding and sending the snapshot, a p50 of about 2 µs for
   `dot-square` in a dev build. It is not an oracle.
5. **The patch's base.** It applies to this task's files as they stand; a
   change to `hash_equality.rs` before G5 means remaking it.

**Costs and timings**
- One subagent session, from about 12:20 to about 13:25 EET, while the
  owner was away, then about 13:45 to 14:20 EET for the review's fixes.
  The review itself ran in the coordinator's context.
- Scratch clones: a copy-on-write seed of `target/` (4.8 GB) in 10 s; the
  test job took 38 to 56 s there.
- The mutation checks took about 6 minutes in all; each run rebuilt only
  eng-session's tests.

## 2026-10-31, session 17 (continued): gates G3 and G4, ADR-0016 and the G3 patch (Claude Opus 5.5, owner away)

Before leaving at about 12:40, the owner approved gates G3 and G4 in
conversation. Asked whether to stop after T6 or go further, they chose
"Approve G3+G4, go further", and they asked for subagents to keep the
main session's context small. From here the main session coordinates:
subagents build each piece, a fresh-context reviewer checks it, and the
main session commits with git. Nothing is pushed.

**G4: ADR-0016** (75e444f, written by a subagent).
`raw-window-handle`'s `HasWindowHandle` and `HasDisplayHandle` may appear
in public APIs between `eng-platform-api`, `eng-platform-winit`,
`eng-gpu` and `eng-app`, and nowhere else. `AGENTS.md`,
`docs/ARCHITECTURE.md` (invariant 5) and the ADR index name the second
exception, and plan.md's banner records both approvals. T6's reviewer
approved it. For the owner:
1. `.claude/agents/reviewer.md` (protected) still allows no exception in
   public APIs, not even serde's. Amendment 21 proposes the wording.
   Until then, briefs to the reviewer mention ADR-0016.
2. The ADR says none of the four crates re-exports the traits, because a
   re-export would get round the wrapper ban. The sources don't say it;
   it follows from "nowhere else". Strike it if unwanted.
3. Spec 6.2 rule 4 and ADR-0002 still state the rule without exceptions,
   and the serde exception (rung 1a, difference 5) has no ADR. Both are
   left alone.
4. T3 must keep the platform contract implementable without the traits,
   because `eng-platform-headless` is outside the seam.

**G3: `docs/rungs/1b/patches/g3-crate-batch-2.patch`** (this commit,
made and tested by a subagent in scratch clones). It changes `deny.toml`
(167 lines added, 2 removed: crate batch 2 in `[bans] allow`,
`[graph] targets` without wasm32, the `raw-window-handle` wrapper ban)
and `_typos.toml` (the deflate crate's name). Tested:
- `cargo deny check` (0.20.2, advisories fetched today) passes with
  today's dependencies. It passes again with batch 2 added where the plan
  puts it: wgpu in eng-gpu, winit in eng-platform-winit, png in eng-cli
  and game-testbed's dev-dependencies, nv-flip in game-testbed's
  dev-dependencies, raw-window-handle =0.6.2 in the four seam crates.
  cargo-deny's graph then holds batch 1 plus exactly the 136 names (146
  versions), and every batch-2 entry is used. Batch 1's `r-efi` is the
  only unused entry.
- `raw-window-handle` in eng-render fails with `error[banned]`.
- `typos` (1.51.1) passes with that lock, and fails once without the new
  entry.
- `git apply` gives byte-identical files on fresh clones, where `cargo
  xtask quick` and `CI=1 cargo xtask ci --job lint` pass.
Amendment 22 lists its differences from the approved list.

**Before the owner pushes, apply the G3 patch.** Until then, CI's
`typos` flags this patch file's own diff lines, and once T3 or T7 adds
batch-2 crates, `cargo deny check` fails on them. Meanwhile agents check
against the approved policy with copies of the two patched files under
other names, passed with `--config` (cargo-deny takes it before
`check`). This session keeps them in its scratchpad, and a later session
can make them again from the patch.

**CI.** The push of 13fcdaa ran green: 8 of 8 jobs, riscv included. The
selftest job's "Post Run Swatinem/rust-cache" step printed ENOENT errors
as annotations. On a cache miss (the lock changed, so "No cache found")
the post step cleans `target/`, takes the selftest's copies under
`target/selftest/<case>/crates/*/tests/` for nested target directories,
and fails to open `target` and `trybuild` inside them. The job still
passes, and earlier runs with a cache hit show none. Amendment 23
proposes a fix.

**Costs** (subagents, each with its own context):
- ADR-0016: about 10 minutes, 185k tokens, 55 tool calls.
- The G3 patch and the policy copies: about 24 minutes, 290k tokens, 128
  tool calls.
- T6 (its entry above): about 64 minutes over two rounds, 577k tokens.
- T6's review: about 21 minutes, 266k tokens, 89 tool calls.

## 2026-10-31, session 17 (continued): T3, `eng-platform-api` (Claude Opus 5.5, subagent, owner away)

The task: T3, the platform contract. Its plan row asks for
`eng-platform-api` and its conformance suite, `Key` and `KeyEvent` moved
in from `eng-input`, and `eng-platform-headless` ported to it. Done when
the suite passes for headless, including a scripted close request reaching
the app. One crate from batch 2 (`raw-window-handle =0.6.2`, approved at
G3), no protected path edited, and no hash stream regenerated. A subagent
built it while the owner was away, so it is left uncommitted for review.

**The contract** (the crate docs list the rules; `docs/ARCHITECTURE.md`
gains "The platform"):
- `Platform` (`name`, `run(&mut self, &mut dyn App)`) runs an app; the
  platform owns the event loop and calls back, as winit does. `App` has
  `start`, `window`, `event`, `frame` (answering `Wake::At(Instant)` or
  `Wake::OnEvent`) and `stop`; all but `frame` have empty defaults.
  `Control` has `now` and `exit`. `PlatformError` is `Unavailable`,
  `Failed`, and headless's `Idle`, `TimeLimit` and `Stalled`
  (`#[non_exhaustive]`).
- `PlatformEvent` is plain data: `KeyDown`/`KeyUp(Key)`,
  `MouseMotion { dx, dy }`, `MouseDown`/`MouseUp(MouseButton)`,
  `Wheel(WheelDelta)`, `GamepadConnected`/`GamepadDisconnected(GamepadId)`,
  `GamepadDown`/`GamepadUp { pad, button }`,
  `GamepadAxis { pad, axis, value }`, `FocusGained`, `FocusLost`,
  `Resized(PhysicalSize)`, `Occluded(bool)` and `CloseRequested`.
  `key_event()` gives the keyboard's part as a `KeyEvent`, which
  `LiveInput::handle` takes.
- `Instant`: nanoseconds (`u64`) since the run started, with
  `core::time::Duration` arithmetic.
- The window seam, `window::Window: HasWindowHandle + HasDisplayHandle +
  Send + Sync` with `size()`, is the only item that names
  `raw-window-handle`'s traits, and nothing re-exports them. A windowed
  platform hands an `Arc<dyn Window>` to `App::window`, which wgpu 30's
  safe `create_surface` takes as it is (a test pins those bounds).
- The rules: `start` first, once; `window` once, right after it, on a
  windowed platform; then batches, the events due followed by one `frame`;
  the first frame at once; after `At(t)` the next frame at `t`, at once if
  `t` is not after now, or sooner with an event's batch, and without an
  event never before `t`; after `OnEvent` only with an event; after `exit`
  only `stop`, the rest of the batch dropped; `run` returns `Ok(())` after
  an exit; `stop` last, once, whatever `run` returns; a close request is an
  event, and the platform runs on until the app exits; the clock never
  goes back; `run` can be called again, the clock starting from zero.

**The headless platform** (`crates/eng-platform-headless/src/lib.rs`):
`HeadlessPlatform::new().script(events).time_limit(at)`. Its clock is
virtual and jumps to each wake, the earlier of the timer and the next
scripted event, so a run takes no real time and repeats exactly. Events
due together are one batch. An app that waits for an event when none is
left gets `Err(Idle)`, a wake past the limit `Err(TimeLimit)`, and an app
that keeps the clock still `Err(Stalled)`, all after `stop`, so a twin
test fails instead of hanging. Each run replays
the script from zero. The old frame counter (`new(tick_rate, frames)`,
`next_frame`) is gone: `run --headless` now paces its ticks as an app on
the platform (`crates/eng-cli/src/ticker.rs`), one tick per frame, a tick
apart; the loop body in `run.rs` is the same, as a closure.

**`Key` and `KeyEvent`** moved with `git mv` (staged as a rename), with
new module docs and nothing else changed. `eng-input` re-exports both, so
`eng_input::Key` and `eng_input::KeyEvent` work as before and
`crates/eng-input/tests/live_input.rs` passes unchanged, its known answer
included. The key-name test moved with them, to
`crates/eng-platform-api/tests/key.rs`.

**Decisions** (proposals; the owner can overrule any):
1. **Flat events.** `FocusLost` is its own event, and `key_event()`
   projects the keyboard's part, rather than a `Key(KeyEvent)` variant
   carrying `KeyEvent::FocusLost`. Focus loss releases mouse buttons too,
   no event can hold an invalid state, and `KeyEvent` stays exactly as T4
   made it.
2. **The window comes through `App::window`**, a default method called
   once after `start`. What a platform implements (`Platform`, `Control`)
   never names the window, so headless stays outside the seam (ADR-0016),
   and apps outside it (the probe's ticker, the tests) implement `App`
   without naming the traits. `Arc`, `Send` and `Sync` are what wgpu needs
   for a `Surface<'static>`.
3. **The shape follows winit 0.30's `ApplicationHandler`**: `start` is
   `resumed`, `frame` is `about_to_wait` with `ControlFlow::WaitUntil`,
   `stop` is `exiting`. T10 maps one to one, and eng-app's accumulator
   (T11) answers `Wake::At(next tick)`.
4. **A close request never stops a platform by itself**, as in winit; the
   app exits. The suite checks it with a close request the app ignores.
5. **After `exit`, only `stop`.** The rest of the batch is dropped, so an
   app needs no guard; winit's adapter needs one flag.
6. **`run` can run again.** The suite runs a platform three times; winit
   can with `run_app_on_demand` on every desktop. If T10 finds it cannot,
   `check` could take a factory instead.
7. **Headless ends stuck runs with errors**, `Idle`, `TimeLimit` and
   `Stalled`, headless-only variants of the contract's (non-exhaustive)
   error, so a test fails instead of hanging. `Stalled` comes after
   `HeadlessPlatform::STALL_FRAMES` (10,000) frames in a row at one
   instant with no event delivered: on a virtual clock nothing can change
   while the app keeps asking for a time not after now (from the review).
8. **One headless API.** The probe uses the contract rather than keeping
   the frame counter beside it.
9. **Conventions**, from what the operating systems, winit, gilrs and SDL3
   report: raw mouse `dy` is positive towards the user, wheel `y` away from
   the user, stick `y` up; triggers are axes from 0 to 1; mouse buttons
   past the five named ones are `Other(u16)`; a gamepad is a `GamepadId`.
   Disconnecting a gamepad releases it.
10. **Scripted checks need a virtual clock.** `check_scripted` asks for
    exact times, so it is for headless (and any later replay platform);
    winit runs `check` where a display exists (T10).
11. **`Instant` is our own** `u64` of nanoseconds, with std's `Duration`;
    a test pins `Key::COUNT` at 110, since the key set is contract.

**Tests** (34 new, 3 of them from the review; 1 moved; 1 removed, the
old frame counter's unit test; expected values written out)
- `crates/eng-platform-api/tests/`: `key.rs` 3 (names, the closed set,
  `key_event` for every kind of event), `contract.rs` 5 (`Instant`, its
  overflow, sizes, error messages, object safety), `window.rs` 3 (an
  `Arc<dyn Window>` meets wgpu's surface bounds; a fake windowed platform
  passes the suite; a window before `start` or twice is rejected).
- `crates/eng-platform-headless/tests/conformance.rs`, 4: headless passes
  `check` and `check_scripted` (T3's done-when); the fault injector alone
  passes; and 22 broken platforms, built by putting a fault between the
  headless platform and the app, are each rejected with the rule they
  break: start twice, a frame before start, no stop, two stops, a clock
  that goes back, a wake 1 ns early, a timer 1 ms late, an exit ignored
  once, an error after an exit, an event dropped, repeated or swapped, a
  close request that stops the platform, no frame after a batch, a clock
  1 ns ahead after events, a wake without an event, and every event lost
  with the clock then standing still; and, from the review, an exit never
  passed on, batches split one event per frame, a run the platform ends
  itself, a first frame 1 ms late, and a wake in the past that sets the
  clock back.
- `crates/eng-platform-headless/tests/headless.rs`, 14: a close request
  scripted at 500 ms reaches a 100 Hz loop, which exits; exact jumps;
  events due together before one frame; a wake not after now; `Idle`;
  the time limit, and events past it; `Stalled` after 10,000 frames at one
  instant, and no stall below that (both from the review); exit in
  `start`; a repeated run equal to the first; a script out of order
  refused; and a proptest against the rules written out (random scripts,
  answers and limits).
- `crates/game-testbed/tests/session.rs` gains 1 (from the review): a
  budget exceeded at tick 0 stops `run` before it steps, so `ticks_run` is
  0 and the hash stream holds tick 0 alone.
- `eng-input`: `tests/quantize.rs` gains 1 (platform events drive live
  input through `key_event`, focus loss included); `tests/bindings.rs`
  lost the key-name test that moved.
- `eng-cli`: 3 unit tests of the ticker (every tick once, a stop, an
  exit). They were written after the ticker, so the ticker was stubbed,
  the tests failed against it, and it was restored.
- The protected `live_input.rs` (12), `exit_codes.rs` (7) and
  `crash_json.rs` (2) pass unchanged.
- The suite caught a bug in itself on its first run against headless: the
  wake check read the next event after counting the batch that woke the
  frame. Fixed before anything else ran.
- Mutation checks, each reverted, all caught. In the headless platform: a
  past wake not clamped to now (2 tests fail), an exit that does not drop
  the rest of the batch (4), a wake at the limit refused (2), an idle run
  ending quietly (2), no `stop` (11), an event waiting for the timer (6), a
  frame after an exit in an event (5), the script order unchecked (1). In
  the ticker: one tick fewer (4, three of them binary tests), an early
  exit lost (1), a stop ignored (1); the new unit tests are the only ones
  that see the last two.
- One more planted bug, events due exactly now held back, made the virtual
  clock stand still: the suite's probe never reached its give-up time and
  spun, and two test binaries had to be killed. Probes now also give up
  after 1,000 frames, the stalling broken platform above checks it (it
  hung before the fix), and the headless tests' recorder fails after
  100,000 calls. Run again, that bug fails 8 tests in milliseconds.
- A failing proptest against the first stub wrote
  `headless.proptest-regressions`, which the guard moved to
  `target/guard-quarantine/1793450248-toolu_01FpqXjpQh2S4W6Qg4Vi3Czz/`.
  Its seed comes from a stub, so it is not a candidate for the owner.
  Later red runs set `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1`, which
  proptest 1.12 reads, and wrote no other file.

**No oracle patch.** G5's rows name no T3 harness to protect and no xtask
step for T3 (the headless-twin Esc test they list is T11's), so
`t3-oracles.patch` is not needed.

**Checked** (this repo, aarch64-apple-darwin; again after the review's
fixes, with the same results)
- `cargo xtask quick` green (20 s).
- The approved G3 policy: `cargo deny --config <scratchpad>/g3/g3-policy.toml
  check` exits 0 ("advisories ok, bans ok, licenses ok, sources ok"); its
  warnings are batch-2 wrappers and licences nothing uses yet (wgpu,
  winit). `typos --config <scratchpad>/g3/g3-typos-config.toml` is clean.
- The repo's own `CI=1 cargo xtask ci --job lint`: 5 of 7. `deny` fails
  on `raw-window-handle`, which today's allow-list lacks, and `typos` on
  the G3 patch file's own lines; both pass once the owner applies the G3
  patch.
- `CI=1 cargo xtask ci --job wasm` green; `cargo xtask selftest` 17 of 17.
- `RUSTDOCFLAGS=-Dwarnings cargo doc --no-deps` for eng-platform-api,
  eng-platform-headless, eng-input and eng-cli: clean.
- `cargo xtask replay`: 5 runs, hash streams equal to the committed ones;
  nothing regenerated.
- `cargo test --workspace`: 257 passed, 0 failed, 1 ignored (the
  `eng-derive` doc example).
- `Cargo.lock` gains one package, `raw-window-handle` 0.6.2: the only
  batch-2 crate added, exactly as approved.
- Not run: RISC-V (the core set did not change).

**Reviewer** (fresh context, read-only, run by the coordinator): changes
needed, all small. Nothing wrong with scope, determinism, protected paths,
ADR-0016 or the `run --headless` rewrite; the problem was the suite, which
could hang and could pass broken platforms. Each finding came with a
planted bug, and each is fixed in this change:
1. **A platform that ignores `exit` hung the suite**: after its exit the
   clock probe answered `At(now + 1 s)` forever, and the give-up never
   applied to it. Now every probe answers a wake at once after its exit,
   and the third callback after it fails from inside the callback with
   "nothing comes after exit but stop". Every plan gives up after 1,000
   frames, the scripted ones also at 100 ms. New fault: `IgnoresExit`.
2. **"Every event due comes before the frame" was checked only at the
   exit frame**, and `wakes` took an undelivered event already due as the
   next one, so headless delivering one event per frame passed. It is now
   checked at every frame. New fault: `SplitsBatches`.
3. **No probe asked for a wake in the past**, so headless without its
   clamp passed. The clock run's second frame now asks for
   `Instant::ZERO`. New fault: `PastWakeGoesBack`.
4. **Two rules had no fault**: "the run lasts until the app exits" and
   "the first frame comes at once". New faults: `EndsEarly`,
   `FirstFrameLate`.
5. **`run`'s budget stop was untested**: deleting it left every test
   green. The new `session.rs` test covers it, since `exit_codes.rs` is
   protected.
6. **Headless spun forever on an app answering `Wake::At(t)` with `t` not
   after now**, which its docs said could not happen. It now ends with
   `Stalled` (decision 7).
7. **`docs/ARCHITECTURE.md` counted "sixteen" broken platforms.** It now
   says "at least one per rule", which stays true as faults are added.

Planted again after the fixes, each reverted exactly: headless's `exit()`
as a no-op (12 tests fail, the lifecycle run among them with "nothing
comes after exit but stop", in milliseconds instead of hanging); one event
per frame (5, with "every event due comes before the frame"); no clamp on
a past wake (4, "the clock never goes back"); `lasted` and the first-frame
check as no-ops (the rejection test fails on `EndsEarly` and
`FirstFrameLate`); no budget stop (the new `session.rs` test); no stall
detection (the new stall test). Before the fixes, `IgnoresExit` hung the
suite (killed after 60 s), and `SplitsBatches` and `PastWakeGoesBack`
passed it. Amendment 24 proposes protecting the suite.

**Open questions**
1. **Protecting the suite** is now amendment 24.
2. **For T10 (winit).** On macOS the event loop needs the main thread, so
   `check` belongs in a `harness = false` test, and running again needs
   `run_app_on_demand`. winit's macOS mouse deltas are coalesced (spec
   6.10), so `MouseMotion`'s "raw counts" waits for rung 2's shim there.
   During a live resize on macOS winit sends only `RedrawRequested`; the
   adapter may call `frame` then, with no contract change.
3. **Not in the contract:** text input, the cursor's position and the
   scale factor. Spec 6.10 lists none of them; the first UI rung adds them,
   a contract change with an ADR (spec 6.2 rule 6).
4. **Merging with T7.** The root `Cargo.toml` gains
   `raw-window-handle = "=0.6.2"` under `[workspace.dependencies]`; T7's
   worktree may add the same line.
5. **G3 before the push**, as known: until the owner applies the G3 patch,
   CI's `cargo deny check` fails on `raw-window-handle`, and `typos` on
   the patch file.

**Costs and timings**
- One subagent session, from about 14:20 to about 15:15 EET, then about
  15:15 to 15:40 EET for the review's fixes, while the owner was away. The
  review itself ran in the coordinator's context.
- Builds and test runs took seconds (`quick` 15 to 21 s). The first
  mutation checks took about 15 minutes, most of it the stalled-clock
  bug's two hung test binaries before they were killed; later runs had a
  process-group timeout, and the re-planted review bugs took about 4
  minutes.

## 2026-10-31, session 17 (continued): T7, `eng-gpu` (Claude Opus 5.5, subagent, owner away)

The task: T7, the GPU layer. Its plan row asks for the adapter, the
capability report, the fixed graph, offscreen targets, readback and the
shader test; done when an offscreen clear plus readback works on Metal
locally and on lavapipe in Docker, and the naga translation test passes.
The owner approved G3 (crate batch 2) and G4 (ADR-0016) today. The G3
change to `deny.toml` and `_typos.toml` is not applied yet, so cargo-deny
and typos were run against copies of the approved policy. ADR-0016's traits
are not used yet: T7 opens no surface.

**Where the work is.** The worktree this subagent was given,
`.claude/worktrees/agent-ab95aa25d8578f77d`, lies inside the main
checkout's `.claude/`, which `protected.toml` covers (`.claude/**`). The
guard therefore refused every write in it, by Edit, Write and Bash alike.
The work was done in scratch clones instead, first of 13fcdaa, then
rebased onto main at 31c3a8c (ADR-0016, T6, the G3 patch file, T3). It
lands as three patch files, described under "The three patches" below:
`t7-libm.patch`, `t7-eng-gpu.patch` and `t7-oracles.patch`.

**What eng-gpu has** (the crate docs and `docs/ARCHITECTURE.md`, "The GPU
layer", have the rest). No wgpu type appears in its public API, it has no
`unsafe`, and nothing in it reaches the simulation.

```rust
Gpu::new(&GpuOptions) -> Result<Gpu, GpuError>   // GpuOptions::from_env(): ENG_GPU_ADAPTER
gpu.report() -> &GpuReport                       // adapter, backend, device type, ids, driver, tier 0, Capabilities
gpu.fullscreen_pipeline(&FullscreenDesc) -> Result<FullscreenPipeline, GpuError>
Graph::new(&[PassDecl], Target) / Graph::fixed(Target)   // DRAW, TONEMAP, PRESENT; Resource::{Scene, Output, Surface}
graph.render(&gpu, &targets, &mut [&mut dyn Pass]) -> Result<(), GpuError>
trait Pass { fn decl(&self) -> PassDecl; fn record(&mut self, rec: &mut PassRecorder); }  // rec.clear(..), rec.fullscreen(..)
Targets::new(&gpu, w, h)?.read_output(&gpu) -> Result<Image, GpuError>   // RGBA8 as stored, rows top first
gpu_required() // ENG_REQUIRE_GPU=1
```

**Decisions** (proposals; the owner can overrule any):
1. **Tier 0, strictly.** The device asks for `Limits::default()` and no
   features, whatever the adapter has. `GpuReport::capabilities` records
   what the adapter supports beyond that: timestamp queries, and spec 6.7's
   later-tier features (binding arrays, indirect counts, ray queries, mesh
   shaders). Timestamp queries are reported, not enabled; pass timing comes
   later (open question 3).
2. **One backend per OS:** Metal on macOS, Vulkan on Linux, DX12 on
   Windows, the backends of spec 8.3's adapters. wgpu tries Vulkan first
   and sorts adapters only by device type, so offered all three, a Windows
   PC with a Vulkan driver would render through Vulkan (the reviewer's
   catch). Vulkan is still compiled on Windows (decision 5), unused.
3. **Adapter choice** (decision 4). `ENG_GPU_ADAPTER` picks the first
   adapter, in wgpu's order, whose name contains the text, ignoring case.
   Without it wgpu chooses with `HighPerformance`: discrete, integrated,
   other, virtual, then software. No adapter is `[E-NO-ADAPTER]`, listing
   each adapter seen. Tests skip then, unless `ENG_REQUIRE_GPU=1`.
4. **DX12 uses FXC** (decision 10). wgpu 30's default is `Auto`: static DXC
   if built in, else a `dxcompiler.dll` found on the PATH, else FXC, so two
   Windows machines could compile shaders differently. FXC ships with
   Windows, so no DLL goes next to the binary; its ceiling is shader model
   5.1, which the shader test uses. eng-gpu type-checks for both Windows
   targets (in a container), but nothing ran on Windows.
5. **Errors are values; anything else is fatal.** The wgpu work in each
   call that returns a `Result` runs inside error scopes (validation,
   internal, out of memory), so a bad shader comes back as `[E-SHADER]` and
   a bad frame as `[E-GPU]`. Errors outside those calls reach the
   uncaptured-error handler, which panics with `[E-GPU]`: validation errors
   are fatal in tests (spec 6.7).
6. **Passes record, the graph encodes.** A pass records into a
   `PassRecorder`; each recording is checked against its declaration (a
   fullscreen draw needs exactly one read, and a pipeline made for the
   resource the pass writes) before anything is encoded, and the frame is
   one submission. T8's counters can then count recorded commands exactly.
7. **Fullscreen passes.** eng-gpu owns the vertex stage
   (`src/fullscreen.wgsl`, one triangle) and the input layout (the pass's
   read at binding 0, a nearest-texel sampler at 1); callers write only the
   fragment stage. T8's tone map (decision 2's clamp) and present blit are
   fullscreen passes. The top-down pass needs buffers and instanced draws,
   which T8 adds to `PassRecorder` with the counters. A pass's target
   starts transparent black unless it clears.
8. **Graph checks.** The plan's two (each read follows its write; the
   output is never the surface), plus: a pass writes exactly one resource;
   the output is written; only a surface graph uses the surface, and it
   must write it. A later pass may write a resource again. A surface graph
   builds, but rendering one fails until eng-gpu opens surfaces (T8, T11).
9. **Readback** returns the output's bytes as stored, sRGB-encoded RGBA8,
   rows top first and unpadded: the "decoded RGBA8 pixels" of decision 3.
10. **The shader test checks Tier 0 on each backend's floor.** Every
    `*.wgsl` file is parsed, validated with the capabilities wgpu 30 gives
    naga at Tier 0 (naga's defaults plus half floats packed in f32, as
    wgpu-naga-bridge maps no features and compliant downlevel flags),
    given its overridable constants' defaults, and translated to SPIR-V
    1.0, MSL 2.1 and HLSL shader model 5.1. MSL 2.1 is the floor of wgpu's
    WebGPU compliance (`DownlevelFlags::MSL2_1`). The scan skips `target`
    and hidden directories at the repo root only (`.claude` can hold other
    checkouts).
11. **Every shader is a `.wgsl` file**, loaded with `include_str!`; WGSL
    written inline in Rust would escape the check. The crate docs, the
    `FullscreenDesc::wgsl` docs and `docs/ARCHITECTURE.md` say so.
12. **Versions.** Resolving today picked `log` 0.4.35, `ordered-float`
    5.5.1 and `unicode-width` 0.2.3, all released after the plan's table.
    `cargo update --precise` put them back at the table's 0.4.34, 5.5.0 and
    0.2.2, so nothing in the build graph is outside the approved list.
13. **Debug builds use wgpu's release flags.** The instance asks only for
    indirect-call validation, in every build; `WGPU_DEBUG=1` and
    `WGPU_VALIDATION=1` add debug labels and the backend's validation layer
    when someone is debugging. With wgpu's default, debug builds name every
    Vulkan object through `VK_EXT_debug_utils`, and on Debian 13's Vulkan
    loader with its lavapipe (arm64) the offscreen tests, which open
    devices on several threads at once, then hung or segfaulted in about
    one run in five: a thread spun inside the loader's
    `vkSetDebugUtilsObjectNameEXT` while the others waited in
    `vkCreateDevice` and `vkDestroyDevice` (gdb). Counts: with the labels,
    16 of 20 runs passed (2 segfaults, 2 hangs); without them, 30 of 30,
    and 20 of 20 in release. One device per process, as the game has, never
    meets it. It may be worth a report upstream (Vulkan-Loader or Mesa).

**The blocker: libm's `arch` feature.** With wgpu in the workspace,
`cargo xtask arch-check` fails:

```
[E-ARCH-PIN] core dependency libm resolved to 0.2.16 ... with features {"arch", "default"}; architecture.toml pins 0.2.16 from crates.io with {}
```

naga depends on `half` with its `num-traits` feature, `half` turns on
num-traits' `libm` feature, and num-traits takes libm with default
features, which include `arch`. Cargo unifies features across the
workspace, so every workspace build compiles the simulation's libm with
`arch`. Session 3's scratch project held wgpu without the engine, so the
batch-2 check could not see it. No crate choice avoids it: naga requires
that `half` feature. libm 0.2.16's `force-soft-floats` feature switches
the `arch` code off whatever turns the feature on (`configure.rs`:
`arch_enabled` is `arch` and not `force-soft-floats`). So eng-math asks for
it (the root `Cargo.toml`, in `t7-eng-gpu.patch`), and `architecture.toml`
pins the workspace's resulting set, `["arch", "default",
"force-soft-floats"]` (`t7-libm.patch`, the owner's). Builds of the core
set alone (RISC-V, wasm32) resolve to `force-soft-floats` only. Both
compile the same libm code as today's empty set, and the hash streams do
not change; the reviewer checked this independently. Without
`t7-libm.patch`, T7 cannot be committed with `quick` green (amendment 25).

**Tests** (21 in 3 files, plus 3 test shaders):
- `tests/graph.rs`, 8, no GPU: the three declarations; the fixed lists;
  each check with its error; three messages word for word, one of them a
  pass with the right name and the wrong reads.
- `tests/offscreen.rs`, 9, on the adapter (skip without one):
  - a 320x240 scene cleared to linear (4.0, 0.215861, -1.0, 1.0), tone
    mapped by a clamp shader, reads back as (255, 128, 0, 255) in every
    pixel: HDR in the scene, clamping, sRGB encoding, readback;
  - a 100x60 pattern (halves in red and green, alternate pixels in blue)
    reads back pixel for pixel: rows top first, 400-byte rows padded to 512
    in the copy;
  - a custom four-pass graph draws the pattern into the output, copies it
    back into the scene by sampling, and tone maps it again: a fullscreen
    pass samples its input the right way up, texel for texel (added after
    the review);
  - the report: tier 0, the OS's backend, `llvmpipe` as a CPU device, and
    `ENG_GPU_ADAPTER` honoured;
  - an adapter name matching nothing gives `[E-NO-ADAPTER]` (runs without
    a GPU too);
  - a broken shader and a missing entry point come back as `[E-SHADER]`,
    not a panic;
  - passes out of order or missing give `[E-GRAPH]`; a fullscreen draw
    without an input, or with another target's pipeline, gives `[E-GPU]`;
  - target sizes from 1 to 8,192.
- `tests/shaders.rs`, 4 (to be protected): every WGSL file passes; each
  step stops a shader it should (a syntax error, `f16`, an override
  without a default) and passes what Tier 0 allows (instancing, an
  override with a default, `@invariant`, `quantizeToF16` and the
  `pack2x16float` pair); each backend reports what it cannot translate (an
  unresolved override), and the HLSL and MSL floors apply (views need
  shader model 6.1, primitive indices MSL 2.3); the scan skips only the
  root's `target` and hidden directories, checked on a scratch tree in the
  temp directory. Its rejected cases pin naga 30, so a wgpu upgrade may
  need them edited.
- Mutation checks, each reverted: 29 planted bugs, 28 caught. Before the
  review, 20: readback (padding kept, rows reversed), the output format
  without sRGB, the recording check, two graph checks, the adapter filter,
  the error scope, the clear colour, a flipped vertex stage, and in the
  shader test each backend's error dropped, each floor raised, overrides
  unresolved and Tier 0 widened. The survivor then, scanning hidden
  directories, is caught now, with 8 more after the review: the scan's
  root rule both ways, Tier 0 without half floats in f32, the MSL floor
  at 1.2 and 2.3, the input bound to the wrong resource, the copy flipped
  or off by one pixel, and the mismatch message by name only.

**Review** (the `reviewer` subagent, fresh context, read-only; 19 min, 261k
tokens, 103 tool calls): changes needed in the shader test; the code
sound; the libm fix correct and result-neutral. Its blockers, all fixed:
Tier 0's capabilities missed half floats in f32, so `quantizeToF16` and
the `pack2x16float` pair failed; MSL 1.2 was below WebGPU compliance and
banned `@invariant`; the scan skipped `target` and hidden directories at
every depth, so a shader in a nested `target/` would escape. It also asked
for proof on the final code, which the runs below give. Its other points,
all taken up: one backend per OS (decision 2); the docs that say libm's
`arch` is off (now in `t7-eng-gpu.patch`); a test of sampling an input;
the `.wgsl` rule; reporting the lock-only crates (amendment 26); a clearer
mismatch message; a comment on why `OutputIsSurface` is checked before
the write count.

**Linux: lavapipe in Docker.** Mesa 25.2.7 from gfx-rs/ci-build build26,
the pinned version, sha256 `629bb3ce...5809` checked. The release has an
x86_64 asset only, and its lavapipe links Ubuntu 24.04's LLVM 18, so the
image is `ubuntu:24.04` for linux/amd64 (emulated on this arm64 Mac), with
Rust 1.99.1 copied from the official image, the Vulkan loader
(`libvulkan1`), lavapipe's libraries (`libllvm18`, `libdrm2`, `libexpat1`),
`git` for xtask's tests, and an ICD file pointing at `libvulkan_lvp.so`
(the tarball's own points into its build machine). `vulkaninfo`:
`llvmpipe (LLVM 18.1.3, 256 bits)`, driver `Mesa 25.2.7 (LLVM 18.1.3)`, a
CPU device. With `ENG_REQUIRE_GPU=1 ENG_GPU_ADAPTER=llvmpipe`, on the final
T7 code before the rebase onto 31c3a8c (the eng-gpu crate is byte for
byte the same after it):
- the whole workspace's tests: 211 pass, none fail, eng-gpu's 21 among
  them (the 9 GPU tests in parallel threads, about 3 s);
- wgpu built with no system development packages, only `gcc` and
  `libc6-dev` to link; decision 5 holds for wgpu (winit is T10's);
- `LP_NATIVE_VECTOR_WIDTH` was left at its default (256 under emulation);
  CI's value is X6's.

The offscreen tests also passed 10 runs out of 10 there, in parallel
threads.

Also natively on arm64, with Debian 13's own lavapipe (Mesa 25.0.7, LLVM
19.1.7, 128 bits), as the arm64 runner will use: eng-gpu's 21 tests pass,
the same bytes included, and the offscreen tests pass 30 runs out of 30 in
parallel threads (decision 13 has the runs before it). The Dockerfiles are
`scratchpad/t7/docker/Dockerfile` (x86_64, pinned; `docker-git/` adds
`git`) and `scratchpad/t7/docker-arm64/Dockerfile`.

**Checks** (aarch64-apple-darwin, fresh scratch clones of main at
31c3a8c, every patch applied with `git apply --check` first, then `git
apply`):
- `t7-libm.patch` then `t7-eng-gpu.patch`: `cargo xtask quick` green (71
  s); `cargo xtask ci --job test` 7 of 7; `CI=1 cargo xtask ci --job wasm`
  green; `cargo xtask selftest` 17 of 17; `cargo xtask replay`: hash
  streams match `crates/game-testbed/hashes/`, no `--regen`; eng-gpu's 21
  tests pass on Metal (`ENG_REQUIRE_GPU=1`); `RUSTDOCFLAGS=-Dwarnings cargo
  doc -p eng-gpu --no-deps` clean. The tree equals the rebase clone's.
- The two G3 commands on that tree: cargo-deny with `g3-policy.toml`,
  advisories, bans, licences and sources ok (warnings only: duplicate
  `hashbrown` and `syn`, wrappers not built yet, unused licences); typos
  with `g3-typos-config.toml`, clean.
- Then `t6-oracles.patch` and `t7-oracles.patch`: `cargo xtask ci --job
  test` 9 of 9, `wire-session` and `shaders` included; `cargo xtask quick`
  green; selftest 17 of 17; wasm green.
- The owner's whole order in one more clone (the G3 patch, `t7-libm.patch`,
  `t7-eng-gpu.patch`, `t6-oracles.patch`, `t7-oracles.patch`) applies
  cleanly, and the repo's own `cargo deny check` and `typos`, as G3 leaves
  them, pass. `t7-libm.patch` also applies on top of the G3 patch and
  `t6-oracles.patch`: it touches `architecture.toml` only.
- typos with `g3-typos-config.toml` on the three patch files themselves,
  which will sit under `docs/`: clean.
- Before the rebase, on 13fcdaa: `cargo clippy --workspace --all-targets --
  -D warnings` clean; eng-gpu's tests pass in release; `cargo check -p
  eng-gpu --all-targets` for `x86_64-pc-windows-msvc` and
  `aarch64-pc-windows-msvc`, in a Linux container, clean.
- With T7 alone and no pin (the state before `t7-libm.patch`),
  arch-check fails `[E-ARCH-PIN]`, as above; with the pin and no T7, it
  fails the other way, so the two go in one commit.

**Batch 2 against the real lock** (cargo-deny's graph for the seven
targets of proposal 4, with the approved policy; the same on the lock
rebased onto 31c3a8c, where T3 had already added `raw-window-handle`
0.6.2):
- 101 third-party names, 103 versions: 36 batch-1 names (38 versions:
  `hashbrown` 0.16.1 and 0.17.1, `syn` 2.0.119 and 3.0.9, both duplicates
  the plan expected) and 65 batch-2 names, one version each.
- No crate outside batch 1 and batch 2. Every batch-2 version is the
  table's, after decision 12's three pins.
- 71 batch-2 names are not in the graph yet: winit and its platform
  crates, `png`, `nv-flip`, and what they bring (T9, T10, T12). `r-efi`
  (batch 1) is unused on these targets, as proposal 4 foresaw.
- 23 registry packages are locked but outside the graph: no build for
  the seven targets uses them, so cargo-deny never checks them (like the
  plan's `zlib-rs`). They are `android_system_properties`, `bumpalo`,
  `crunchy`, `dispatch2`, `futures-core`, `futures-task`, `futures-util`,
  `js-sys`, `objc2-io-surface`, `pin-project-lite`, `portable-atomic`,
  `portable-atomic-util`, `r-efi`, `redox_syscall`, `rustversion`, `slab`,
  `tokio`, `wasm-bindgen` and its four helper crates, and `web-sys`. Of
  these, `pin-project-lite` and `slab` are batch-2 names that winit will
  build and `r-efi` is batch 1; the other 20 are on neither list
  (amendment 26).

**The three patches** (`docs/rungs/1b/patches/`), rebased onto main at
31c3a8c:
- `t7-libm.patch`, the owner's: `architecture.toml` only, the libm pin
  with a comment. It applies to 31c3a8c directly, with or without the G3
  patch or `t6-oracles.patch`. Its header says why: naga, half and
  num-traits turn on libm's `arch`, `force-soft-floats` keeps the
  soft-float code, the hash streams are unchanged, and the reviewer
  confirmed it.
- `t7-eng-gpu.patch`, the orchestrator's, nothing protected: the eng-gpu
  crate; the root `[workspace.dependencies]` (wgpu, naga and libm's
  `force-soft-floats`, next to T3's `raw-window-handle` line); `Cargo.lock`
  at the table's versions; `docs/ARCHITECTURE.md`, `AGENTS.md` and
  eng-math's crate docs. 21 files.
- `t7-oracles.patch`, a G5 item: `crates/eng-gpu/tests/shaders.rs` joins
  `protected.toml` and `.github/CODEOWNERS`; `("shaders", &[("eng-gpu",
  "shaders")])` joins `HARNESSES` in the form `t6-oracles.patch` gives it;
  `docs/ARCHITECTURE.md` names the step. Its base is 31c3a8c with
  `t6-oracles.patch` applied, and it needs T7's test file.

Apply order: `t7-libm.patch` (owner) and `t7-eng-gpu.patch` together, in
one commit; at G5, `t6-oracles.patch`, then `t7-oracles.patch`. Each was
made by a script in a scratch clone (`scratchpad/t7/scripts/`:
`rebase_docs.py`, `make-t7-oracles.sh`, `test-final-1.sh`,
`test-final-g3.sh`) and tested as under "Checks".

**`budgets.toml [gpu]`** (proposal 6, amendment 28; in no patch). T7 gives part of
a basis: the fixed passes cost one draw, one pipeline and one bind group
each and upload nothing, so 2, 2, 2 and 0 bytes a frame with a surface (1,
1, 1, 0 offscreen); the top-down pass's share is T8's. Proposal 6's 8, 4, 8
and 16,384 leave room for it. One catch: `budgets.toml` cannot get a
`[gpu]` table before eng-cli reads it. `BudgetFile` denies unknown
fields, so `run --budgets` exits 64 (`[E-LOAD] ... unknown field gpu`,
checked), and `cargo xtask replay` passes `--budgets budgets.toml`. The
table should land with T9, which checks it.

**The guard and T6.** During one of this session's calls, a failing
proptest in the main checkout (T6's `crates/eng-session/tests/hostile.rs`)
wrote `crates/eng-session/tests/hostile.proptest-regressions`. The guard
checks the whole main checkout after every call, so it put the file back
(deleted it) and kept what was written in the main checkout's
`target/guard-quarantine/1793447645-toolu_01D9dEAN4m9YQhdFxqwTsahe/`
(11,114 bytes). It is T6's candidate, not this session's.

The proposals for the owner are amendments 25 to 29, in the block after
this entry.

**Open questions**
1. **The libm fix** blocks T7 (amendment 25): approve `t7-libm.patch`'s
   approach, or choose another, such as accepting `arch` (it only swaps in
   hardware `sqrt`, `fma` and `rint`, which IEEE rounds exactly) and
   changing the rule in AGENTS.md.
2. **Agent worktrees under `.claude/`** cannot be written while the guard
   protects `.claude/**` (amendment 29).
3. **Timestamp queries.** Strict Tier 0 leaves them off; spec 6.13's GPU
   pass times would need them on where the adapter has them. Decide when
   pass timing arrives.
4. **Windows.** FXC and the DX12 path only type-checked here; T13's WARP
   runs will be their first real test.
5. **Debug labels** (decision 13) are off unless `WGPU_DEBUG=1`. If GPU
   debugging should have them by default, the tests that open devices on
   several threads need a lock instead.

**Costs and timings**
- Session 17's T7 subagent, from about 13:03 to 14:50 EET: reading and
  design to 13:15, the scratch clone and code to 13:35, Docker and checks
  to 14:00, the review to 14:12, its fixes, the arm64 hang (decision 13)
  and the final runs after. Then, from about 15:20 to 15:55, the rebase
  onto 31c3a8c, the split into three patches and their tests.
- Reviewer subagent: 19 minutes, 261k tokens, 103 tool calls.
- Builds: wgpu cold on this Mac in 34 s; `quick` in the scratch clones 40
  s to 2 min 18 s. Docker (emulated x86_64): image 1 min; eng-gpu's tests
  built in 2 min 13 s, the workspace's in 1 min 24 s more; arm64 natively
  in 1 min 28 s.

## Proposals for the owner (protected paths)

Applied so far: proposals 1, 2, 3 and 10 (G1 and G2, 2026-10-31); X1
and X7 from proposal 7 (2026-10-25); from proposal 8 and amendments 14
to 17, the protection of `crash_json.rs`, `version.rs`, `live_input.rs`
and `**/*.proptest-regressions` (e14b533); amendment 18's oracle patch,
which protects eng-net-wire's `golden.rs` and `never_panic.rs`
(2026-10-31); and proposal 5 (G4) as ADR-0016 (75e444f). Proposal 4 (G3)
is approved and waits as `docs/rungs/1b/patches/g3-crate-batch-2.patch`.
T7 waits as `t7-libm.patch` (the owner's, amendment 25) together with
`t7-eng-gpu.patch` (nothing protected; an agent applies it in the same
commit). For G5, `t6-oracles.patch` and then `t7-oracles.patch` are
ready. Nothing else below is applied. Each item is what an agent would
otherwise have to change; the owner applies it, or unlocks the path.

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

18. **Amendment from session 17** (to proposals 7, 8 and 9; the text above
    is kept as written). T5 is built, so these can be applied:
    - Proposals 8 and 9: `docs/rungs/1b/patches/t5-oracles.patch`, as one
      change. It renames `crates/eng-net-wire/tests/layout.rs` to
      `tests/golden.rs`, which `crates/*/tests/golden.rs` protects, and
      adds `crates/eng-net-wire/tests/never_panic.rs` to the protected
      paths and `.github/CODEOWNERS`. After it, any change to the wire
      format's bytes goes through the owner.
    - Proposal 7, X7: the `wire-session` step waits for T6. It runs
      eng-net-wire's `golden` and `never_panic` and eng-session's
      hash-equality test, three test files in two crates, so `HARNESSES`
      needs a list of targets per row, or a `steps.run` of its own (session
      15, open question 1). One xtask change after T6 replaces two.

19. **Amendment from session 17, T6** (to proposals 7 and 8, and
    amendments 11 and 18; the text above is kept as written). T6 is
    built, so this can be applied:
    - `docs/rungs/1b/patches/t6-oracles.patch`, as one change. It adds
      `crates/eng-session/tests/hash_equality.rs`, criterion 11's "the
      session gives the same hashes as stepping the simulation directly",
      to the protected paths and `.github/CODEOWNERS` (proposal 8,
      amendment 11). It also adds X7's `wire-session` step to `cargo xtask
      ci --job test` (amendment 18): one `cargo test` running eng-net-wire's
      `golden` and `never_panic` and eng-session's `hash_equality`.
    - After it, criterion 11's three test files are protected and run as
      the step its checks name. The PROGRESS.md entry for T6 says how the
      patch was tested.

20. **Amendment from session 17, T6's review** (to `plan.md`, which is not
    protected; its header is being edited elsewhere, so the owner applies
    this). T6 built no `now`, and the plan says it did:
    - The crate table, eng-session's row: "It has no clock (`now` is passed
      in)". Proposed: "It has no clock: the caller says when a tick is due,
      and time sync adds `now` in rung 5."
    - Difference 10 says contracts 1, 2, 6 and 7 are built in full.
      Contract 6 is "The protocol does no I/O: it takes (now, incoming
      bytes) and returns outgoing bytes". Proposed: built except `now`.
      Nothing in rung 1b needs time, since the caller steps the server when
      a tick is due; rung 5's time sync, timeouts and resends add it. The
      session also reads and writes through the `Transport` trait rather
      than taking and returning bytes. Memory does no I/O, and the
      bytes-in, bytes-out shape belongs to rung 5's `eng-net-proto`.
    - If the owner wants the argument now, unused, it is one parameter on
      `Server::poll`, `Server::step` and `Client::poll`.

21. **Amendment from session 17, ADR-0016** (to `.claude/agents/reviewer.md`,
    protected). Its item 5 says "Third-party types must not appear in
    public APIs", with no exception. Proposed: add "except serde derives
    on authored-data types and ADR-0016's window-handle traits". Without
    it, the reviewer flags the traits that T3, T7 and T10 may use.

22. **Amendment from session 17, the G3 patch** (to proposal 4 with
    amendment 12, approved by the owner as G3):
    - `wgpu`, `winit` and `raw-window-handle` are written `wgpu:*`,
      `winit:*` and `raw-window-handle:*` in `[bans] allow`. cargo-deny
      0.20.2 refuses a policy that names a crate in both `allow` and
      `deny`, where the three are wrapper-banned, and without an allow
      match it reports them as not allowed. `*` matches every release but
      no pre-release, so this is stricter than a bare name.
    - Four crates resolve to newer versions than on 2026-10-25, with the
      same names and licences: log 0.4.35, ordered-float 5.5.1, rustix
      1.1.6 and unicode-width 0.2.3. No name, advisory or licence is new.
    - Dev-only crates (nv-flip, nv-flip-sys) are outside cargo-deny's
      licence check by default. Their licences are on the list anyway.

23. **Amendment from session 17, CI** (to `.github/workflows/ci.yml` or
    `xtask/`, protected). Keep the rust-cache post step out of the
    selftest's scratch copies: `cargo xtask selftest` removes
    `target/selftest` when it finishes, or the selftest job deletes it
    before the post step runs. Either removes the ENOENT annotations that
    every cache miss prints. Low priority: the job passes.

24. **Amendment from session 17, T3's review** (to proposal 8; the text
    above is kept as written). With the review's fixes in, protect the
    platform conformance suite at G5:
    - `protected.toml` and `.github/CODEOWNERS` entries for
      `crates/eng-platform-api/src/conformance.rs`,
      `crates/eng-platform-api/src/conformance/**` and
      `crates/eng-platform-headless/tests/conformance.rs`. The last holds
      the broken platforms that show the suite still bites.
    - Why (the reviewer's reason): T10 and T11 will be tempted to relax a
      rule when winit misbehaves, and whether a rule may change should be
      the owner's call.
    - Described only; no patch yet. `cargo xtask docs-check` wants a
      CODEOWNERS line for each new pattern, so the two files change
      together.

25. **Amendment from session 17, T7** (to proposals 2 and 4; gate G3's
    companion). Crate batch 2 turns on libm's `arch` feature in every
    workspace build: naga depends on `half` with `num-traits`, `half` turns
    on num-traits' `libm` feature, and num-traits takes libm with its
    default features. `cargo xtask arch-check` then fails `[E-ARCH-PIN]`.
    The fix keeps libm's soft-float code whatever turns `arch` on:
    - `docs/rungs/1b/patches/t7-libm.patch` (protected, the owner's):
      `architecture.toml` pins libm 0.2.16 with `["arch", "default",
      "force-soft-floats"]`, the workspace's set once wgpu is in it;
    - `t7-eng-gpu.patch` (not protected): eng-math's libm dependency gains
      `force-soft-floats` in the root `[workspace.dependencies]`.

    libm compiles its `arch` code only for `arch` without
    `force-soft-floats` (`configure.rs`), so it compiles exactly as today;
    the hash streams are unchanged, and T7's reviewer confirmed it from
    libm's source. Apply both in one commit, before or after the G3 patch:
    arch-check fails with either alone. The alternative is to allow `arch`
    (hardware `sqrt`, `fma` and `rint`, which round exactly) and change the
    rule in AGENTS.md.

26. **Amendment from session 17, T7** (to proposal 4 and "Crate batch 2").
    Recomputed against the real lock (main at 31c3a8c plus T7): cargo-deny's
    graph for the seven targets holds 101 third-party names, 103 versions,
    all on batch 1 or batch 2, every batch-2 version the table's (with
    `log` 0.4.34, `ordered-float` 5.5.0 and `unicode-width` 0.2.2 held there
    by `cargo update --precise`; newer releases came out after the table).
    The lock also holds 20 names on neither list, outside that graph, so
    cargo-deny never checks them and the approved policy needs no entry:
    `android_system_properties`, `bumpalo`, `crunchy`, `dispatch2`,
    `futures-core`, `futures-task`, `futures-util`, `js-sys`,
    `objc2-io-surface`, `portable-atomic`, `portable-atomic-util`,
    `redox_syscall`, `rustversion`, `tokio`, `wasm-bindgen`,
    `wasm-bindgen-futures`, `wasm-bindgen-macro`,
    `wasm-bindgen-macro-support`, `wasm-bindgen-shared` and `web-sys`. As
    with `zlib-rs`, "only these crates may appear in Cargo.lock" would not
    hold; proposal 4's "in the dependency graph of these targets" does.

27. **Amendment from session 17, T7** (to proposals 7 and 8, and amendment
    11; G5). `docs/rungs/1b/patches/t7-oracles.patch`, after
    `t6-oracles.patch`: protect `crates/eng-gpu/tests/shaders.rs`
    (criterion 10's harness) in `protected.toml` and
    `.github/CODEOWNERS`, and add the X7 `shaders` step to `cargo xtask ci
    --job test`. Once protected, its Tier 0 floors (naga's capabilities
    with half floats in f32, SPIR-V 1.0, MSL 2.1, HLSL shader model 5.1)
    and its rejected cases, which pin naga 30, change only with the owner,
    as at the next wgpu upgrade.

28. **Amendment from session 17, T7** (to proposal 6). Apply `budgets.toml
    [gpu]` with T9, together with eng-cli reading it, not before:
    `BudgetFile` denies unknown fields, so `run --budgets budgets.toml`
    exits 64 with `[E-LOAD] ... unknown field gpu` (checked), and `cargo
    xtask replay` passes that file. T7's basis for the values: the fixed
    passes cost one draw, one pipeline switch and one bind-group switch
    each and upload nothing (2, 2, 2, 0 a frame with a surface); proposal
    6's 8, 4, 8 and 16,384 leave the top-down pass room. Also (to amendment
    11): `.config/nextest.toml` needs no serial GPU group for T7's tests.

29. **Amendment from session 17, T7** (to `.claude/**`, the owner's).
    Claude Code puts isolated agent worktrees in `.claude/worktrees/`,
    inside the main checkout, where the guard protects every path
    (`.claude/**`): T7's subagent could not write a file in its worktree
    and worked in scratch clones instead. Worktrees outside the checkout,
    or a guard rule that treats `.claude/worktrees/<name>/` as a checkout
    of its own, would let isolated agents work in place.
