# engine

A game engine in Rust, built by AI agents and meant to be used by them: no
visual editor, everything is code and plain text, and every program can run
headless, replay its inputs exactly and report in JSON.

The first games look and play like the first Doom; underneath, the engine
is built to grow towards modern rendering without rewrites. It is tested
with personal games that are never sold.

**Status:** rung 1a, "headless dot": the deterministic core, a dot moved by
an input script, and the checks around it. Nothing is playable yet.

```sh
cargo run -p game-testbed -- run --headless --ticks 120 --seed 1 \
    --input crates/game-testbed/scenarios/dot-square.input.toml --report report.json
cargo xtask quick
```

- How to work in this repo (people and agents): [AGENTS.md](AGENTS.md)
- How it fits together: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Decisions: [docs/adr/](docs/adr/)
- The plan: [docs/superpowers/specs/](docs/superpowers/specs/)

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
