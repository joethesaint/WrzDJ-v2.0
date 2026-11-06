//! The Rust state machine against the TypeScript one on generated traces.
//!
//! Generates traces (seeded), runs them through `bridge/scripts/deck-trace.ts` (the real
//! `DeckStateManager`) and through `trace::run`, and requires identical output: every live
//! report, every clear, its time in ms, and each deck's final state.
//! Needs `npm ci` in `bridge/`; without it the test says so and passes as skipped.
//! `DIFF_TRACES=5000 cargo test --test differential` runs more.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use wrzdj_bridge_core::trace::{Trace, run};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len() as u64) as usize]
    }
}

fn generate(seed: u64, count: usize) -> Value {
    let mut r = Rng(seed);
    let decks = ["1", "2", "3", "4", "10"];
    let titles = ["A", "B", "C", "D"];
    let traces: Vec<Value> = (0..count)
        .map(|_| {
            let mut t = 0;
            let mut events = Vec::new();
            for _ in 0..(20 + r.below(60)) {
                t += [0, 0, 1, 500, 2000, 3000, 8000, 16000][r.below(8) as usize];
                let deck = r.pick(&decks);
                events.push(match r.below(20) {
                    0..=4 => {
                        let title = (r.below(10) > 0).then(|| r.pick(&titles));
                        json!({"t": t, "op": "track", "deck": deck, "title": title, "artist": "X"})
                    }
                    5..=11 => {
                        json!({"t": t, "op": "play", "deck": deck, "playing": r.below(3) > 0})
                    }
                    12..=14 => {
                        let level = [0.0, 0.0, 0.5, 1.0][r.below(4) as usize];
                        json!({"t": t, "op": "fader", "deck": deck, "level": level})
                    }
                    15 => json!({"t": t, "op": "master", "deck": deck}),
                    16 => json!({"t": t, "op": if r.below(3) == 0 { "reset" } else { "master" }, "deck": deck}),
                    _ => json!({"t": t, "op": "advance"}),
                });
            }
            events.push(json!({"t": t + 60_000, "op": "advance"}));
            json!({
                "config": {
                    "liveThresholdSeconds": r.below(21),
                    "pauseGraceSeconds": r.below(6),
                    "nowPlayingPauseSeconds": r.below(13),
                    "useFaderDetection": r.below(2) == 1,
                    "masterDeckPriority": r.below(2) == 1,
                },
                "events": events,
            })
        })
        .collect();
    Value::Array(traces)
}

#[test]
fn rust_matches_typescript_on_generated_traces() {
    let bridge = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bridge");
    if !bridge.join("node_modules").exists() {
        eprintln!("skipped: run `npm ci` in bridge/ to compare with the TypeScript");
        return;
    }
    let count = std::env::var("DIFF_TRACES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let traces = generate(2026, count);
    let input = serde_json::to_vec(&traces).unwrap();

    let mut child = Command::new("npx")
        .args(["tsx", "scripts/deck-trace.ts"])
        .current_dir(&bridge)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run the TypeScript harness");
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), &input).unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "TypeScript harness failed");
    let ts: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let parsed: Vec<Trace> = serde_json::from_value(traces.clone()).unwrap();
    assert_eq!(ts.len(), parsed.len());
    let mut outputs = 0;
    for (i, (trace, expected)) in parsed.iter().zip(&ts).enumerate() {
        let got = run(trace);
        assert_eq!(&got, expected, "trace {i} differs:\n{}", traces[i]);
        outputs += expected["out"].as_array().unwrap().len();
    }
    eprintln!("{count} traces, {outputs} live/clear outputs: Rust == TypeScript");
}
