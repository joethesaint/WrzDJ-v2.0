/**
 * Runs scripted traces through the Bridge's server client (src/bridge.ts) with a scripted
 * network and a simulated clock, and prints every request it makes.
 *
 * The Rust port in bridge-core/ reads the same traces (cargo run --bin client-trace) and must
 * print the same lines; bridge-core/tests/client_differential.rs compares the two.
 *
 *   npx tsx scripts/client-trace.ts < traces.json > ts.jsonl
 *
 * Input: [{ "net": [200 | 500 | "down", ...], "ops": [{ "t": ms, "op": "live"|"status"|"clear", ... }] }]
 * Each request takes the next "net" outcome (200 when the list runs out). "live" is what
 * index.ts does on a deckLive event: skip check, then post now playing.
 * Output: one JSON line per trace: { "requests": [...], "results": [...], "end": {...} }.
 */
import { readFileSync } from "node:fs";

process.env.WRZDJ_API_URL = "http://bridge.test";
process.env.WRZDJ_BRIDGE_API_KEY = "test-key";
process.env.WRZDJ_EVENT_CODE = "EVT1";
process.env.MIN_PLAY_SECONDS = "5";

// The Bridge logs with console.*; keep stdout for the result lines only.
console.log = console.warn = console.error = console.info = console.debug = () => {};

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

let net: (number | string)[] = [];
let requests: unknown[] = [];
globalThis.fetch = (async (url: string, init: RequestInit) => {
  const body = typeof init.body === "string" ? JSON.parse(init.body) : null;
  const key = (init.headers as Record<string, string>)["X-Bridge-API-Key"];
  requests.push({ t: now, method: init.method, path: url.replace("http://bridge.test", ""), key, body });
  const outcome = net.length ? net.shift()! : 200;
  if (outcome === "down") throw new TypeError("fetch failed");
  // A plain object, not a Response: it settles in one microtask, so the driver sees idle exactly.
  const status = outcome as number;
  return { ok: status >= 200 && status < 300, status, text: async () => "x" };
}) as unknown as typeof fetch;

/** Runs `work` to the end, firing the earliest simulated timer whenever nothing else can run. */
async function drive(work: Promise<unknown>): Promise<unknown> {
  let done = false;
  let value: unknown;
  work.then((v) => {
    done = true;
    value = v;
  });
  const idle = () => new Promise((r) => setImmediate(r));
  for (;;) {
    await idle();
    if (done && timers.length === 0) return value;
    if (timers.length === 0) continue;
    const next = [...timers].sort((a, b) => a.due - b.due || a.id - b.id)[0];
    timers = timers.filter((x) => x.id !== next.id);
    now = next.due;
    next.fn();
  }
}

type Op = { t: number; op: string; title?: string; artist?: string; connected?: boolean; device?: string };
const traces = JSON.parse(readFileSync(0, "utf8")) as { net: (number | string)[]; ops: Op[] }[];

for (const [i, trace] of traces.entries()) {
  now = 0;
  nextId = 1;
  timers = [];
  net = [...trace.net];
  requests = [];
  // A fresh module per trace: bridge.ts keeps its breaker, buffer and last track at module level.
  const b = await import(`../src/bridge.js?trace=${i}`);
  const results: unknown[] = [];
  for (const op of trace.ops) {
    now = Math.max(now, op.t);
    if (op.op === "live") {
      if (b.shouldSkipTrack(op.artist, op.title)) {
        results.push("skipped");
        continue;
      }
      b.updateLastTrack(op.artist, op.title);
      results.push(await drive(b.postNowPlaying(op.title, op.artist, undefined, "1", "traktor-broadcast")));
    } else if (op.op === "status") {
      const r = await drive(b.postBridgeStatus(op.connected ?? true, op.device, { plugin_id: "traktor-broadcast" }));
      results.push(r);
    } else if (op.op === "clear") {
      results.push(await drive(b.clearNowPlaying()) ?? null);
    }
  }
  const cb = b.getCircuitBreaker();
  const status = b.getDetailedStatus();
  process.stdout.write(
    JSON.stringify({
      requests,
      results,
      end: { breaker: cb.getState(), failures: cb.getConsecutiveFailures(), buffered: status.buffer_size, now },
    }) + "\n",
  );
}
