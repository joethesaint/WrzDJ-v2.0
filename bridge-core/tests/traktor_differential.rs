//! The Rust Traktor stream readers against `traktor-broadcast-plugin.ts` on generated streams.
//!
//! Streams mix random audio bytes with metadata blocks (accents, emoji, invalid UTF-8, NUL
//! padding, extra quotes, line breaks, bare text, empty blocks), cut into chunks at random
//! points, in ICY mode (`icy-metaint`) and header mode. Both sides must emit the same tracks.
//! Needs `npm ci` in `bridge/`; without it the test says so and passes as skipped.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use wrzdj_bridge_core::traktor::{IcyParser, TrackFilter, header_chunk};

struct Rng(u64);
impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.below(256) as u8).collect()
    }
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            s.push(if i <= c.len() {
                T[(n >> (18 - 6 * i)) as usize & 63] as char
            } else {
                '='
            });
        }
    }
    s
}

fn meta_text(r: &mut Rng) -> Vec<u8> {
    let titles: [&[u8]; 12] = [
        b"StreamTitle='Daft Punk - One More Time';",
        b"StreamTitle='Caf\xc3\xa9 del Mar - Energy 52';",
        b"StreamTitle='Burna Boy - Last Last \xf0\x9f\x94\xa5';",
        b"StreamTitle='Bad \xff\xfe bytes - Title';StreamUrl='';",
        b"StreamTitle='It's - A - Quote';",
        b"StreamTitle='';",
        b"StreamTitle='  only title  ';",
        b"StreamTitle='Line\nBreak - X';",
        b"StreamTitle=no quotes - here",
        b"Artist Only - Bare Text",
        b"   ",
        b"StreamTitle='\xef\xbb\xbfBOM - Song\xef\xbb\xbf';",
    ];
    titles[r.below(titles.len() as u64) as usize].to_vec()
}

/// A stream: its `icy-metaint` (0 = header mode) and its chunks.
type Stream = (usize, Vec<Vec<u8>>);

fn generate(seed: u64, count: usize) -> (Value, Vec<Stream>) {
    let mut r = Rng(seed);
    let mut json_traces = Vec::new();
    let mut raw = Vec::new();
    for _ in 0..count {
        let header_mode = r.below(4) == 0;
        let meta_int = if header_mode {
            0
        } else {
            1 + r.below(64) as usize
        };
        let mut stream = Vec::new();
        for _ in 0..(1 + r.below(12)) {
            if header_mode {
                let n = r.below(40) as usize;
                stream.extend(r.bytes(n));
                stream.extend(meta_text(&mut r));
            } else {
                stream.extend(r.bytes(meta_int));
                let mut m = if r.below(5) == 0 {
                    Vec::new()
                } else {
                    meta_text(&mut r)
                };
                let blocks = m.len().div_ceil(16);
                m.resize(16 * blocks, 0);
                stream.push(blocks as u8);
                stream.extend(m);
            }
        }
        let mut chunks = Vec::new();
        let mut at = 0;
        while at < stream.len() {
            let max = if r.below(2) == 0 { 8 } else { 200 };
            let n = (1 + r.below(max) as usize).min(stream.len() - at);
            chunks.push(stream[at..at + n].to_vec());
            at += n;
        }
        json_traces.push(json!({"metaInt": meta_int, "chunks": chunks.iter().map(|c| b64(c)).collect::<Vec<_>>()}));
        raw.push((meta_int, chunks));
    }
    (Value::Array(json_traces), raw)
}

fn rust_tracks(meta_int: usize, chunks: &[Vec<u8>]) -> Value {
    let mut filter = TrackFilter::default();
    let mut icy = (meta_int > 0).then(|| IcyParser::new(meta_int));
    let mut out = Vec::new();
    for c in chunks {
        let texts = match icy.as_mut() {
            Some(p) => p.feed(c),
            None => header_chunk(c).into_iter().collect(),
        };
        for t in texts {
            if let Some(track) = filter.accept(&t) {
                out.push(json!([track.artist, track.title]));
            }
        }
    }
    Value::Array(out)
}

#[test]
fn rust_traktor_reader_matches_typescript_on_generated_streams() {
    let bridge = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bridge");
    if !bridge.join("node_modules").exists() {
        eprintln!("skipped: run `npm ci` in bridge/ to compare with the TypeScript");
        return;
    }
    let count = std::env::var("DIFF_TRACES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let (traces, raw) = generate(77, count);
    let mut child = Command::new("npx")
        .args(["tsx", "scripts/traktor-trace.ts"])
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
    assert_eq!(ts.len(), raw.len());
    let mut tracks = 0;
    for (i, ((meta_int, chunks), expected)) in raw.iter().zip(&ts).enumerate() {
        let got = rust_tracks(*meta_int, chunks);
        assert_eq!(
            &got, expected,
            "stream {i} differs (metaInt {meta_int}):\n{}",
            traces[i]
        );
        tracks += expected.as_array().unwrap().len();
    }
    eprintln!("{count} streams, {tracks} tracks: Rust == TypeScript");
}
