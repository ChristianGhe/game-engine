# ADR-0023: Maps and sim data

- **Status:** Accepted
- **Date:** 2026-11-07
- **Decision:** brings spec 6.11's sim data forward from rung 3, as
  constant resources outside the per-tick state, with `Sim::sim_data_hash`
  filling the recording header's existing field (6.4, ADR-0008); departs
  from 6.4's and 6.5's "whole world" hash and snapshot; adds a map
  selector beside `GameDef` (rung 2 plan, decision 1 and difference 2)
- **Who:** Owner decided (2026-11-07, at rung 2 gate G4, with O2: a second
  map), on Claude's proposal (rung 2 plan, decision 1; owner decision O2).

## Context

Rung 2 adds a second testbed map, `room`, whose walls, slabs and grid the
simulation reads every tick but never changes. Spec 6.11 calls such data
*sim data*: target-independent, cooked once, compared across x64 and
arm64, and covered by the handshake's content hash (6.8, rung 5). Rung 3's
level compiler was to bring it.

What the code does at HEAD:
- **Everything is state.** `World` keeps components, resources and events
  by stable name (`eng-ecs/src/world.rs`). `visit_state` walks all of
  them; the state hash, `Sim::snapshot`, `state_len` and `restore` use it.
  `Sim::protocol_version` hashes their names, the registered schemas, the
  system order and the tick rate (`eng-sim/src/sim.rs:334`).
- **The dot map's geometry is state.** `game_testbed_sim::build` inserts
  `BoxRoom` and `MoveConfig` as ordinary resources (`lib.rs:61-62`), so
  they are in the dot's state hash, its five streams and its snapshot.
- **Snapshots are small.** A `Snapshot` message must fit `MAX_DATAGRAM`,
  1,200 bytes, or the session fails with `[E-SNAPSHOT-SIZE]`. The dot's is
  466 bytes (rung 1b PROGRESS: header, length, the 8-byte protocol and a
  451-byte state). A wall segment in f64 (two endpoints and a z range) is
  48 bytes, so 25 of them alone fill a datagram.
- **Recordings already have the field.** `RecordingHeader::sim_data_hash`
  (`eng-input/src/recording.rs:19`) is "0 until levels exist", and
  eng-cli writes 0 (`run.rs:315`). `replay verify` checks the protocol,
  both SHAs, rustc and the tick rate, not this field (`replay.rs`).
- **Protected tests pin the 1b shapes:** `SimConfig`'s fields and
  `SimBuilder::new` (`hash_equality.rs:48`, `twin.rs:213`); `GameDef`'s
  fields (`twin.rs:33`); `formats` with `recording: 1` and `hash_stream: 1`,
  the stream's first line, and `sim.protocol` equal to dot-square's stream
  (`version.rs:321-379`); `Welcome`'s fields (`never_panic.rs:44`,
  `golden.rs`); `[E-RECORDING-BUILD]` (`exit_codes.rs:220`).

## Decision

- **Constant resources** (eng-ecs). A fourth kind beside components,
  resources and events: inserted with its schema and stable name, under
  the same registration rules, while the sim is built, and read by systems
  through a shared reference. There is no mutable access, and
  `SimBuilder::build` seals them: a later insert panics.
- **Outside the state.** Neither `visit_state` nor `visit_dump` walks
  them, so the state hash, snapshots, `state_len` (the `[sim]` budget),
  `restore` and the `[E-NAN]` walk after each system leave them out. They
  are checked finite once, when the sim is built. If `dump-world` shows
  them, it is under a key of their own, only for a map that has them, so
  dot dumps and their pointers do not change (T6's call).
- **A departure from the spec, for the owner.** 6.4 hashes "the whole
  world, every tick" (spec line 300), and 6.5 says "the whole world can be
  snapshotted, hashed" (line 323). Here the world is its per-tick state
  plus its constants, and only the state is hashed and snapshotted each
  tick. The constants are still covered: their schemas are in the
  protocol, and their content hash is in every recording header and in
  the map's stream header, which `compare-hashes` holds equal across
  targets. Since they cannot change, hashing them once covers every tick.
- **The protocol** (`protocol_version`) gains a `constants` section, with
  their names and schemas, only when the world has at least one. A world
  without them hashes exactly as today, so the dot's protocol and its five
  streams keep their bytes. The schema of sim data is protocol; its
  content is the sim-data hash.
- **`Sim::sim_data_hash`**: FNV-1a-64 over the constants' canonical
  encoding, in stable-name order, computed when the sim is built; 0 when
  there are none.
- **Maps** (eng-cli). A `GameMaps` static beside `GameDef` names each map
  and its build function. The first map is `GameDef::build`, `dots`, and
  registers exactly what it does today. New entry points take both; the
  old ones mean the first map. `--map <name>` defaults to the first map;
  an unknown name exits 64 with `[E-MAP]`.
- **Only the first map may lack sim data,** and no two maps share a hash,
  so a hash names a map. The room's geometry and grid are its constants.
- **Recordings** carry `sim_data_hash` in the existing header field, so
  the format stays v1. `replay verify` picks the map whose hash matches,
  then makes its existing checks; no match exits 64 with
  `[E-RECORDING-BUILD]`. A 1b recording, hash 0, picks `dots`.
- **Hash streams.** For a map with sim data the second header line ends
  with ` map=<name> sim_data=0x<16 hex>`. The dot streams keep their bytes
  and `hash_stream` stays v1. `compare-hashes` compares whole files,
  headers included, so the sim data's hash is held equal across targets:
  6.11's byte-equality check, in rung 2's form.
- **Reports** name `map`, `sim_protocol` and `sim_data_hash` (new keys in
  report v2). `--version --json` describes the first map only, since
  `version.rs` pins its keys and values.
- **The wire does not change.** Over Memory the probe builds server and
  client from one map. A client whose map has another protocol is denied
  (`DenyReason::SIM_PROTOCOL`, `eng-session/src/server.rs:182`).

## Consequences

- The room's snapshot carries only its per-tick state: the tick, the
  players and the things. Its walls never cross the wire, so it can fit
  1,200 bytes (criterion 5 checks it).
- 1b's oracles stay byte-identical: the dot's protocol, streams,
  snapshots, dumps and assertions. `hash_equality.rs`, `never_panic.rs`,
  `twin.rs`, `version.rs` and `exit_codes.rs` pass unchanged (T6's
  done-when).
- Between machines nothing checks sim data until rung 5's handshake
  carries its hash (6.8). Two maps with the same schemas and systems but
  different data would share a protocol; rung 2 has no such pair, and
  `dots` and `room` differ in both.
- The dot room's `BoxRoom` stays an ordinary resource, inside the state,
  although it never changes. Moving it would change the dot's streams.
- Rung 3's level compiler cooks into the same constants; the hash and the
  header field do not change then.
- The stamp cannot show the room's protocol or sim data; reports and
  stream headers do.

## Alternatives considered

- **Replace the dot room** (O2's other answer). One map and no selector,
  but the five dot streams are regenerated, the top-down goldens and the
  protected dot assertions change, and the room still needs constants.
- **Sim data in the state,** chunked across snapshots. It needs
  fragmentation, which is rung 5's (6.8), and every tick's hash would
  cover bytes that never change.
- **Sim data outside the world**, as a plain value beside `Sim`. Systems
  are `fn(&mut World)`, so they could not reach it.
- **Fold the sim-data hash into the protocol.** The handshake would then
  catch a map mismatch now. But the protocol is code (schemas, order, tick
  rate) and sim data is content: 6.4 and the header keep them apart, and
  rung 3's data reload would change the protocol mid-run.
- **A new recording field naming the map.** It bumps the recording format,
  which `version.rs` pins at 1, and the hash already names it.

## Owner decisions, gates and protected files

- **O2.** Recommended: a second map with sim data. If the owner picks
  replacing the dot room, `GameMaps`, `--map` and the replay's map choice
  drop; constants, `sim_data_hash` and the recording field stay; the dot
  streams are regenerated with `--regen`, and the owner changes the dot
  assertions and top-down goldens.
- **G4** accepts this before T6.
- **G5:** `xtask/**` (replay reads `map` from `scenarios.toml`, T6's
  patch); `scenarios.toml` and the room's `*.assert.toml` (T7's patch);
  `protected.toml` with `.github/CODEOWNERS` protecting T6's sim-data
  tests. eng-ecs, eng-sim, eng-session and eng-cli are not protected.

## Revisit when

Rung 3's level compiler cooks sim data; rung 5's handshake carries its
hash; or a map's sim data changes during a run (hot reload).
