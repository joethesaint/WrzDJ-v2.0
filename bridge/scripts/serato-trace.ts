/**
 * Appends byte chunks to a real Serato session file and polls the real SeratoPlugin after each
 * one, printing the tracks it emits and the full parse of the finished file.
 * bridge-core/tests/serato_differential.rs compares the Rust port with this output.
 *
 *   npx tsx scripts/serato-trace.ts < traces.json > ts.jsonl
 *
 * Input: [{ "appends": [base64, ...] }]
 * Output: one JSON line per trace: { "tracks": [[deck, title, artist, album|null]], "entries": [...] }.
 * Strings pass through toWellFormed(): Rust replaces a lone UTF-16 surrogate with U+FFFD by design.
 */
import { appendFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { SeratoPlugin } from "../src/plugins/serato-plugin.js";
import { parseSessionBytes } from "../src/plugins/serato-session-parser.js";

const wf = (s: string) => (s as unknown as { toWellFormed(): string }).toWellFormed();

type Internals = {
  running: boolean;
  sessionsDir: string;
  sessionPath: string | null;
  fileOffset: number;
  pollSessionFile(): void;
};

const traces = JSON.parse(readFileSync(0, "utf8")) as { appends: string[] }[];
for (const trace of traces) {
  const dir = mkdtempSync(join(tmpdir(), "serato-trace-"));
  const file = join(dir, "1.session");
  writeFileSync(file, Buffer.alloc(0));
  const plugin = new SeratoPlugin();
  const tracks: unknown[] = [];
  plugin.on("track", (e: { deckId: string; track: { title: string; artist: string; album?: string } }) =>
    tracks.push([e.deckId, wf(e.track.title), wf(e.track.artist), e.track.album === undefined ? null : wf(e.track.album)]),
  );
  const p = plugin as unknown as Internals;
  p.running = true;
  p.sessionsDir = dir;
  p.sessionPath = file;
  p.fileOffset = 0;
  for (const chunk of trace.appends) {
    appendFileSync(file, Buffer.from(chunk, "base64"));
    p.pollSessionFile();
  }
  const entries = parseSessionBytes(readFileSync(file)).entries.map((e) => [
    wf(e.title), wf(e.artist), wf(e.album), wf(e.genre), wf(e.key), e.bpm, e.deck, e.startTime,
  ]);
  process.stdout.write(JSON.stringify({ tracks, entries }) + "\n");
  rmSync(dir, { recursive: true, force: true });
}
