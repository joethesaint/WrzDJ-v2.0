/**
 * Feeds byte streams to the Traktor broadcast plugin's stream readers and prints the tracks it
 * emits. bridge-core/tests/traktor_differential.rs compares the Rust port with this output.
 *
 *   npx tsx scripts/traktor-trace.ts < traces.json > ts.jsonl
 *
 * Input: [{ "metaInt": n (0 = header mode), "chunks": [base64, ...] }]
 * Output: one JSON line per trace: [[artist, title], ...].
 */
import { EventEmitter } from "node:events";
import { readFileSync } from "node:fs";

import { TraktorBroadcastPlugin } from "../src/plugins/traktor-broadcast-plugin.js";

type Reader = {
  handleIcyStream(req: EventEmitter, metaInt: number): void;
  handleHeaderMetadata(req: EventEmitter & { headers: Record<string, string> }): void;
};

const traces = JSON.parse(readFileSync(0, "utf8")) as { metaInt: number; chunks: string[] }[];
for (const trace of traces) {
  const plugin = new TraktorBroadcastPlugin();
  const tracks: [string, string][] = [];
  plugin.on("track", (e: { track: { artist: string; title: string } }) =>
    tracks.push([e.track.artist, e.track.title]),
  );
  const req = Object.assign(new EventEmitter(), { headers: {} as Record<string, string> });
  const reader = plugin as unknown as Reader;
  if (trace.metaInt > 0) reader.handleIcyStream(req, trace.metaInt);
  else reader.handleHeaderMetadata(req);
  for (const c of trace.chunks) req.emit("data", Buffer.from(c, "base64"));
  process.stdout.write(JSON.stringify(tracks) + "\n");
}
