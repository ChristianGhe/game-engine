# ADR-0013: R1 player count

- **Status:** Accepted
- **Date:** 2026-10-24
- **Decision:** D21 (spec sections 6.8, 10.1)
- **Who:** Claude's call

## Context

The owner asked for a LAN escape-room puzzle: "first the puzzle, then multiplayer". The player count for the first game was never stated. The spec proposes R1 as a small co-op escape-puzzle game (the ladder and games are still pending under P6).

## Decision

R1 has exactly 2 players, co-op only.

- 4-6 handcrafted rooms, full sound, no combat.
- Played over LAN or direct IP.
- Co-op puzzles separate the players physically: one can see the clue, the other can act on it.
- The puzzle solver checks a 2-player plan. A co-op room must have a 2-player plan and no 1-player plan.
- Presence predicates cover "a player is in region R" and "N players are in regions R1..Rn at the same tick".
- The playtest needs one other person (P5).

## Consequences

- Tests, soak runs and the Mac + PC playtest use 2-player sessions.
- The network model supports more players for later games; R1 does not use that.
- The solver only needs to handle two players in R1.

## Revisit when

If a later game needs more than 2 players. Otherwise no trigger planned; change only by a new ADR.
