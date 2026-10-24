# ADR-0007: Server-authoritative netcode

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D11 (spec section 6.8; related D18)
- **Who:** Claude's call

## Context

The owner said: "the first game should be a lan game", built after the first puzzle. The client/server split must exist from rung 1 so adding LAN is not a rewrite.

## Decision

Server-authoritative client/server in the Quake 3 / Source tradition, for every mode, including single-player through an in-process server.

- The server ticks at 60 Hz. Clients send only commands, unreliably; each packet repeats the last 3-5 commands.
- The server sends delta snapshots against each client's last acknowledged snapshot: 60 Hz on LAN, 20-30 Hz online.
- The client predicts only its own player and interpolates everything else about two snapshot intervals in the past.
- Single-player is the same server over the Memory transport. Messages are still encoded to bytes.
- A listen server hosts. No socket opens until a player picks Host or Join. The session ends when the host quits.
- Transport trait: `send(peer, bytes)` and `poll_recv()`, unreliable datagrams of at most 1,200 bytes. Implementations: Memory, Sim, UDP.
- **Contracts fixed in rung 1:** every message carries a server tick; everything that changes the simulation is a `PlayerCommand`; generational network entity IDs; replicated state apart from presentation state; a per-client relevance hook and baselines; a protocol that does no I/O; a little-endian fixed-width wire format with an integer protocol version.
- **Rejected:** lockstep (needs bit-exact peers, slowest peer sets the pace, no late join, a desync is fatal) and rollback (built for two-player games with small state).
- Level 1 plays over LAN and direct IP only (D18). Steam's relay would need a Steam app, so it is dropped.
- Security baseline even on LAN: challenge cookie, session token, CRC32, capped decoders that never panic, fuzzing. No encryption in Level 1.

## Consequences

- We build the codec, connection state machine, snapshots, prediction and interpolation. We borrow only std sockets and socket2.
- We avoid laminar, bincode and async runtimes.
- macOS Local Network and Windows Firewall prompts need explicit handling.
- Movers are deterministic functions of the server tick, so predicted lift rides are not corrected.

## Revisit when

If Level 1 includes shooting (P6), lag compensation is added at that rung. Easier online play would be a transport swap, about one rung of work. Otherwise no trigger planned; change only by a new ADR.
