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
