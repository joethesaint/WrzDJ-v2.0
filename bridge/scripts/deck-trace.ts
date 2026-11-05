/**
 * Runs deck-event traces through DeckStateManager on a simulated clock and prints what it emits.
 *
 * The Rust port in bridge-core/ reads the same traces (cargo run --bin deck-trace) and must print
 * the same lines; bridge-core/tests/differential.rs compares the two.
 *
 *   npx tsx scripts/deck-trace.ts < traces.json > ts.jsonl
 *
 * Input: [{ "config": {...}, "events": [{ "t": ms, "op": "track"|"play"|"fader"|"master"|"advance", ... }] }]
 * Output: one JSON line per trace: { "out": [...emitted], "end": {...final state} }.
 *
 * The clock mimics Node: timers due at the same moment fire in creation order, and a delay
 * below 1 ms runs as 1 ms.
 */
import { readFileSync } from "node:fs";

type Timer = { id: number; due: number; fn: () => void };
let now = 0;
let nextId = 1;
let timers: Timer[] = [];

globalThis.setTimeout = ((fn: () => void, ms?: number) => {
  const timer = { id: nextId++, due: now + Math.max(1, ms ?? 0), fn };
  timers.push(timer);
  return timer.id;
}) as unknown as typeof setTimeout;
globalThis.clearTimeout = ((id?: number) => {
  timers = timers.filter((t) => t.id !== id);
}) as unknown as typeof clearTimeout;
Date.now = () => now;

function advanceTo(t: number): void {
  for (;;) {
    const due = timers.filter((x) => x.due <= t).sort((a, b) => a.due - b.due || a.id - b.id)[0];
    if (!due) break;
    timers = timers.filter((x) => x.id !== due.id);
    now = due.due;
    due.fn();
  }
  now = t;
}

const { DeckStateManager } = await import("../src/deck-state-manager.js");

type Event = {
  t: number;
  op: string;
  deck?: string;
  title?: string | null;
  artist?: string;
  playing?: boolean;
  level?: number;
};
const traces = JSON.parse(readFileSync(0, "utf8")) as { config: never; events: Event[] }[];

for (const trace of traces) {
  now = 0;
  nextId = 1;
  timers = [];
  const out: unknown[] = [];
  const m = new DeckStateManager(trace.config);
  m.on("deckLive", (e: { deckId: string; track: { title: string; artist: string } }) =>
    out.push({ t: now, live: e.deckId, title: e.track.title, artist: e.track.artist }),
  );
  m.on("nowPlayingCleared", () => out.push({ t: now, cleared: true }));
  for (const e of trace.events) {
    advanceTo(e.t);
    const deck = e.deck ?? "1";
    if (e.op === "track") {
      m.updateTrackInfo(deck, e.title == null ? null : { title: e.title, artist: e.artist ?? "" });
    } else if (e.op === "play") m.updatePlayState(deck, e.playing ?? false);
    else if (e.op === "fader") m.updateFaderLevel(deck, e.level ?? 0);
    else if (e.op === "master") m.setMasterDeck(deck);
  }
  const decks = m.getDeckIds().map((id: string) => {
    const s = m.getDeckState(id);
    return [id, s.state, s.isPlaying, s.accumulatedPlayTime, s.hasBeenReported, s.isMaster];
  });
  console.log(JSON.stringify({ out, end: { now: m.getCurrentNowPlayingDeckId(), decks } }));
}
