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

## Piece 2: the server client

`src/client.rs` ports `bridge/src/bridge.ts` and `circuit-breaker.ts`: now playing, status and
clear requests, retries with backoff (POST 2/4/8 s, DELETE 1/2 s), the circuit breaker (3 failures
open it for 60 s, then one probe), the 20-track replay buffer, and the track debounce. The
network and the clock sit behind the `Io` trait: `src/http.rs` is the real one (10 s per
request); `src/client_trace.rs` is a scripted one.

**Proof it matches.** `tests/client_differential.rs` scripts the network (200, 500, 404 or a
dropped connection, in runs so the breaker opens, probes and closes) and the Bridge's calls,
then runs each trace through `bridge.ts` (`bridge/scripts/client-trace.ts`, scripted `fetch`,
simulated clock) and through the port. The requests (method, path, key, body, time in ms),
results and final state must be identical.

```bash
DIFF_TRACES=5000 cargo test --release --test client_differential
```

5,000 traces, 135,292 requests (2,570 replays; 241 traces end with the breaker open): identical.
Two deliberate one-line bugs (the cooldown boundary, the replay stop) are each caught.

**Against a real server** (`bridge-post`, same variables as the Node bridge):

```bash
WRZDJ_API_URL=http://localhost:8000 WRZDJ_BRIDGE_API_KEY=... WRZDJ_EVENT_CODE=ABC123 \
  cargo run --release --bin bridge-post -- nowplaying "Title" "Artist"   # or: status, clear
```

Checked on a local server: status marks the Bridge connected; now playing replaces the song and
moves the old one to history; clear empties it; a wrong key gives up after 4 tries in 14 s.

**Kept as it is, on purpose:** when a replay fails partway, `bridge.ts` drops the tracks it had
already taken from the buffer. The port does the same so the two can be compared; fixing it is a
separate change to both.

## Piece 3: Traktor broadcast and a running Bridge

`src/traktor.rs` ports `bridge/src/plugins/traktor-broadcast-plugin.ts`: the ICY reader
(`icy-metaint` blocks), header-mode metadata, `StreamTitle` parsing with JavaScript's matching and
trimming rules, and the repeat filter, plus a TCP listener on `127.0.0.1:8123`. Unlike the Node
server, the listener also reads a body sent with no length (an Icecast `SOURCE` client).

`src/bin/wrzdj-bridge.rs` is `bridge/src/index.ts` for Traktor: same environment variables, plus
`WRZDJ_TRAKTOR_PORT`. One loop runs the deck state machine, sleeping until its next timer or the
120 s heartbeat; one worker thread owns the server client, so a slow server never delays deck
timing. Not ported yet: the command poller, and the clear on Ctrl+C.

**Proof it matches.** `tests/traktor_differential.rs` generates streams (random audio bytes;
titles with accents, emoji, invalid UTF-8, a byte-order mark, NUL padding, extra quotes, line
breaks; bare text; empty blocks), cuts them into chunks at random points, and feeds the same
chunks to the TypeScript plugin's readers (`bridge/scripts/traktor-trace.ts`) and to the port.
20,000 streams, 72,627 tracks: identical. Removing U+FEFF from the trim set is caught.

**Side by side.** Both Bridges ran against a local server, each fed by a simulated Traktor (chunked
`PUT`, `icy-metaint` 8192, 128 kbps, a new track every 25 s) for 4 minutes, sampled every 5 s.
The Node Bridge ran as compiled JavaScript (`tsc`, then `node dist/index.js`), not through `tsx`.

| | Node (`node dist/index.js`) | Rust (`wrzdj-bridge`, release) |
|---|---|---|
| Tracks taken live | 10 of 10 | 10 of 10 |
| Server afterwards | 9 in history, song 10 playing | the same |
| Memory (RSS) | 89.6-93.0 MB | 3.0-3.8 MB |
| CPU over 4 min | 0.54 s (0.22% of one core) | 0.04 s (0.02% of one core) |
| Program | Node.js runtime + `dist/` + `node_modules` | 2.3 MB binary |

Measured on Linux in a cloud container. The Electron app adds Chromium on top of the Node
figure; on a DJ's Windows laptop next to Serato, measure again.

## Next pieces

1. The command poller and a clean shutdown, so the Rust Bridge does everything the Node one does
   for Traktor.
2. The other plugins (StageLinQ, Pro DJ Link, Serato), then the app shell.
