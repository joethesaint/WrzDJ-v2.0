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

**Replay that fails partway (#961):** the failed track and the unsent ones go back to the front
of the buffer, in order, as the fixed TypeScript does; the differential test compares against
that fix.

## Piece 3: Traktor broadcast and a running Bridge

`src/traktor.rs` ports `bridge/src/plugins/traktor-broadcast-plugin.ts`: the ICY reader
(`icy-metaint` blocks), header-mode metadata, `StreamTitle` parsing with JavaScript's matching and
trimming rules, and the repeat filter, plus a TCP listener on `127.0.0.1:8123`. Unlike the Node
server, the listener also reads a body sent with no length (an Icecast `SOURCE` client).

`src/bin/wrzdj-bridge.rs` is `bridge/src/index.ts` for Traktor: same environment variables, plus
`WRZDJ_TRAKTOR_PORT`. One loop runs the deck state machine, sleeping until its next timer or the
120 s heartbeat; one worker thread owns the server client, so a slow server never delays deck
timing. The worker also polls `/api/bridge/commands/{code}` every 5 s (`command-poller.ts`; skipped
while the breaker is open): `reset_decks`, `reconnect`, `restart` and `setbuilder_transport` (logged)
act as in `plugin-bridge.ts`, and anything else is warned about. After a source that had connected
drops, it restarts with backoff (2, 4, 8, 16, then 30 s). Serato's restart starts a new session
watcher; Traktor's listener stays bound and the title filter starts afresh.

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

## Piece 4: Serato

`src/serato.rs` ports `bridge/src/plugins/serato-plugin.ts` and `serato-session-parser.ts`: the
`oent`/`adat` chunk reader (UTF-16BE text, BPM read with JavaScript's `parseFloat`, tags compared
as Node's `ascii` decoding does), and the session watcher: newest `.session` watched from its end,
new bytes only, half-written chunks read again, deleted file means disconnected, 5 read errors
rescan, per-deck repeat filter, album sent when known. `wrzdj-bridge` runs it with
`WRZDJ_PLUGIN=serato` (`WRZDJ_SERATO_PATH`, `WRZDJ_SERATO_POLL_MS` optional).

Two differences, on purpose: a newer session file is found by checking on each poll rather than
with `fs.watch` (same outcome as the watcher: read from its start), and a lone UTF-16 surrogate
becomes U+FFFD (JavaScript keeps it, and the server can't store it).

**Proof it matches.** `tests/serato_differential.rs` appends generated session bytes (emoji, lone
surrogates, odd-length and 70 KB fields, overrunning lengths, unknown fields and chunks, high-bit
tags, BPM text such as " 1e3x" and "Infinity", decks 0 and 2^32-1, empty entries, repeats) to a
real file in random cuts, polling after each: the real TypeScript plugin
(`bridge/scripts/serato-trace.ts`) and the port must emit the same tracks and parse the same
entries. 3,000 sessions, 15,274 entries, 11,494 tracks: identical. Removing the tag's high-bit
mask, or `parseFloat`'s leading-space skip, is caught.

**Simulator.** `serato-sim <dir> [step ms]` writes a live session as a DJ would: two decks, a
repeat, a track written in two halves, a new session file, then its deletion.

**Side by side**, 2 minutes, each Bridge on its own simulated Serato, local server:

| | Node (`node dist/index.js`) | Rust (`wrzdj-bridge`) |
|---|---|---|
| Live, in order | Water (deck 1), Calm Down (deck 2) | the same |
| Tracks seen, repeats filtered, session switches | the same sequence | the same sequence |
| Memory (RSS) | 86.9-92.2 MB | 3.7-3.9 MB |
| CPU over 2 min | 0.23 s | 0.01 s |

Only Water and Calm Down go live in both: Serato reports no play or pause, so a deck counts as
playing until its track changes, and deck 2 holds now playing. The one difference in the server's
records came from stopping the Node Bridge, which clears now playing on SIGTERM; the Rust Bridge
doesn't yet (next piece).

## Errors, keys and stops

- **Run log** (`shell::runlog`): `wrzdj-bridge` writes `errors.log` (for people) and `errors.jsonl` (for
  agents) to `WRZDJ_LOG_DIR`, or the system's log folder. A panic on any thread is recorded. Every
  failure in the Bridge's own code is logged under `<area>:<detail>`: `client:post`, `client:auth`,
  `client:replay`, `client:clear`, `client:breaker`, `serato:read`, `serato:session`, `serato:rescan`,
  `traktor:accept`, `traktor:read`, `deck:limit`, `bridge:stop`. Each Serato poll, Traktor connection
  and server job runs inside `run_safely`, so a panic is recorded and the loop carries on. At exit
  it prints "N issues this run — see …".
- **Key:** `WRZDJ_BRIDGE_API_KEY`, else the OS keychain; `WRZDJ_REMEMBER_KEY=true` saves it there.
- **Stops**, as `bridge-runner.ts`: a 401 ("Session expired — please log in again"), or the event
  deleted or expired (checked every 30 s). Each clears, reports disconnected and exits 3. Ctrl+C
  and SIGTERM clear once and exit 0.

## Next pieces

1. A scenario harness that drives both Bridges and compares what the server got.
2. StageLinQ and Pro DJ Link, each with a simulator; then the app shell.
