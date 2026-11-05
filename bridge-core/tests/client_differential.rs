//! The Rust server client against `bridge/src/bridge.ts` on generated traces.
//!
//! Each trace scripts the network (200, 500, 404 or "down", in runs so the breaker opens, probes
//! and closes) and the Bridge's calls (a track going live, a status heartbeat, a clear), at times
//! that cross the 5 s debounce and the 60 s breaker cooldown. Both sides must send the same
//! requests at the same simulated ms, return the same results and end in the same state.
//! Needs `npm ci` in `bridge/`; without it the test says so and passes as skipped.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use wrzdj_bridge_core::client_trace::{ClientTrace, run};

struct Rng(u64);
impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

fn generate(seed: u64, count: usize) -> Value {
    let mut r = Rng(seed);
    let traces: Vec<Value> = (0..count)
        .map(|_| {
            let mut net = Vec::new();
            for _ in 0..r.below(12) {
                let run = 1 + r.below(8);
                let outcome = [json!(500), json!(404), json!("down")][r.below(3) as usize].clone();
                net.extend(std::iter::repeat_n(outcome, run as usize));
                net.extend(std::iter::repeat_n(json!(200), r.below(4) as usize));
            }
            let mut t = r.below(20_000);
            let mut ops = Vec::new();
            for _ in 0..(5 + r.below(30)) {
                t += [0, 1, 3000, 6000, 20_000, 61_000, 120_000][r.below(7) as usize];
                ops.push(match r.below(10) {
                    0..=5 => {
                        let title = ["A", "B", "C", "a ", ""][r.below(5) as usize];
                        json!({"t": t, "op": "live", "title": title, "artist": "X"})
                    }
                    6..=8 => json!({"t": t, "op": "status", "connected": r.below(4) > 0, "device": "Traktor"}),
                    _ => json!({"t": t, "op": "clear"}),
                });
            }
            json!({"net": net, "ops": ops})
        })
        .collect();
    Value::Array(traces)
}

#[test]
fn rust_client_matches_typescript_on_generated_traces() {
    let bridge = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bridge");
    if !bridge.join("node_modules").exists() {
        eprintln!("skipped: run `npm ci` in bridge/ to compare with the TypeScript");
        return;
    }
    let count = std::env::var("DIFF_TRACES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);
    let traces = generate(1105, count);
    let mut child = Command::new("npx")
        .args(["tsx", "scripts/client-trace.ts"])
        .current_dir(&bridge)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run the TypeScript harness");
    let input = serde_json::to_vec(&traces).unwrap();
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), &input).unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "TypeScript harness failed");
    let ts: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let parsed: Vec<ClientTrace> = serde_json::from_value(traces.clone()).unwrap();
    assert_eq!(ts.len(), parsed.len());
    let (mut requests, mut replays, mut opened) = (0, 0, 0);
    for (i, (trace, expected)) in parsed.iter().zip(&ts).enumerate() {
        let got = run(trace);
        assert_eq!(&got, expected, "trace {i} differs:\n{}", traces[i]);
        let reqs = expected["requests"].as_array().unwrap();
        requests += reqs.len();
        replays += reqs
            .iter()
            .filter(|q| q["body"]["delayed"] == json!(true))
            .count();
        opened += usize::from(expected["end"]["breaker"] != json!("CLOSED"));
    }
    eprintln!(
        "{count} traces, {requests} requests ({replays} replays, {opened} ending with the breaker open): Rust == TypeScript"
    );
}
