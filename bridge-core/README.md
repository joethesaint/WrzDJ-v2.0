# bridge-core

The Bridge's core in Rust, ported from `bridge/` one piece at a time. Each piece must behave
exactly like the TypeScript it replaces before anything uses it. The Electron app stays the
default until the Rust version matches it.

## Piece 1: the deck state machine

`src/deck.rs` ports `bridge/src/deck-state-manager.ts`, which decides which deck is live and
when. It is a pure state machine with the clock passed in (`advance_to`), so it can be tested
without real timers or hardware.

**Proof it matches.** `tests/differential.rs` generates traces of deck events (track loads, play
and pause, fader moves, master-deck changes, simultaneous events) and runs each trace twice:
through the real TypeScript class via `bridge/scripts/deck-trace.ts`, and through the Rust port.
Every live report, every clear, its time in ms, and each deck's final state must be identical.

```bash
cd bridge && npm ci && cd ../bridge-core
cargo test                                                    # unit tests + 1,000 traces
DIFF_TRACES=20000 cargo test --release --test differential    # more traces
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```

Measured on Linux (Node 22, Rust 1.97), on 20,000 traces with 1,013,386 events:

| | TypeScript (`tsx`) | Rust (release) |
|---|---|---|
| Output | 20,000 lines | the same 20,000 lines |
| Wall time | 2.8-2.9 s | 1.0-1.2 s |
| Start and exit with no work | 0.47 s | 0.002 s |
| Program size | Node.js runtime + `tsx` | 461 KB binary |

Peak memory in that run (about 257 MB for TypeScript, 299 MB for Rust) is dominated by loading
the 70 MB trace file at once, so it says nothing about the Bridge. Memory next to running DJ
software is measured once a long-running piece exists.

## Next pieces

1. The server client: now playing, status, clear, retries, circuit breaker, replay buffer.
2. One equipment plugin (Traktor broadcast), giving a working headless Bridge.
3. The remaining plugins, then the app shell.
