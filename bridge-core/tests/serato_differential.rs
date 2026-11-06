//! The Rust Serato reader against `serato-plugin.ts` + `serato-session-parser.ts`.
//!
//! Generated session files (emoji, lone surrogates, odd-length and 70 KB fields, lengths that
//! overrun, unknown fields and chunks, high-bit tags, several adat blocks, BPM text such as
//! " 1e3x", "Infinity" and "-0", decks 0 and 2^32-1, empty entries, repeats) are appended to a real
//! file in random cuts. After each append the real TypeScript plugin polls (via
//! `bridge/scripts/serato-trace.ts`) and so does the Rust watcher: the tracks emitted and the full
//! parse of the finished file must match. Lone surrogates: TypeScript output passes through
//! `toWellFormed()`, since Rust replaces them with U+FFFD by design.
//! Needs `npm ci` in `bridge/`; without it the test says so and passes as skipped.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use wrzdj_bridge_core::serato::{SeratoEvent, SessionWatcher, parse_session_bytes, write};

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

fn utf16_raw(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|u| u.to_be_bytes()).collect()
}

fn text(r: &mut Rng) -> Vec<u8> {
    let pool = [
        "Water",
        "Tyla",
        "Calm Down",
        "Rema",
        "Café del Mar",
        "Ojuelegba 🔥",
        "",
        "  spaced  ",
    ];
    match r.below(12) {
        0 => utf16_raw(&[0x0041, 0xD800, 0x0042]), // a lone surrogate
        1 => utf16_raw(&[0x0041, 0x0000, 0x0043]), // stops at the zero unit
        2 => {
            let mut v = utf16_raw(&[0x0058, 0x0059]);
            v.push(0x41); // odd length
            v
        }
        3 => utf16_raw(&vec![0x0061; 35_000]), // about 70 KB
        _ => {
            let t = pool[r.below(pool.len() as u64) as usize];
            let mut v: Vec<u8> = t.encode_utf16().flat_map(u16::to_be_bytes).collect();
            if r.below(2) == 0 {
                v.extend_from_slice(&[0, 0]);
            }
            v
        }
    }
}

fn entry(r: &mut Rng) -> Vec<u8> {
    let bpms = [
        "128.00",
        " 1e3x",
        "Infinity",
        "-0",
        ".5",
        "abc",
        "",
        "-12.5e-1BPM",
    ];
    let mut fields = Vec::new();
    for _ in 0..(1 + r.below(8)) {
        fields.push(match r.below(11) {
            0 | 1 => write::field(2, &text(r)),
            2 | 3 => write::field(6, &text(r)),
            4 => write::field(8, &text(r)),
            5 => write::field(10, &text(r)),
            6 => write::text_field(13, bpms[r.below(bpms.len() as u64) as usize]),
            7 => write::field(29, &text(r)),
            8 => write::field(
                52,
                &[
                    [0, 0, 0, 0],
                    [0, 0, 0, 2],
                    [255, 255, 255, 255],
                    [0, 0, 0, 3],
                ][r.below(4) as usize],
            ),
            9 => write::field(31, &(r.below(1 << 31) as u32).to_be_bytes()),
            _ => write::field(999, &[1, 2, 3]),
        });
    }
    if r.below(15) == 0 {
        let mut f = write::field(2, b"\x00A");
        f[4..8].copy_from_slice(&1000u32.to_be_bytes()); // a length that overruns
        fields.push(f);
    }
    let mut e = write::entry(&fields);
    if r.below(20) == 0 {
        e[0] |= 0x80; // "oent" with the high bit set still reads as oent in Node
    }
    e
}

fn generate(seed: u64, count: usize) -> (Value, Vec<Vec<Vec<u8>>>) {
    let mut r = Rng(seed);
    let (mut js, mut raw) = (Vec::new(), Vec::new());
    for _ in 0..count {
        let mut file = if r.below(4) == 0 {
            Vec::new()
        } else {
            write::header()
        };
        for _ in 0..(1 + r.below(10)) {
            match r.below(10) {
                0 => file.extend(write::track("Water", "Tyla", "TYLA", 1)), // repeats on purpose
                1 => file.extend([b'x', b'x', b'x', b'x', 0, 0, 0, 2, 7, 7]), // an unknown chunk
                _ => file.extend(entry(&mut r)),
            }
        }
        let mut appends = Vec::new();
        let mut at = 0;
        while at < file.len() {
            let max = [1, 7, 64, 4096, 100_000][r.below(5) as usize];
            let n = (1 + r.below(max)) as usize;
            let n = n.min(file.len() - at);
            appends.push(file[at..at + n].to_vec());
            at += n;
        }
        js.push(json!({"appends": appends.iter().map(|a| b64(a)).collect::<Vec<_>>()}));
        raw.push(appends);
    }
    (Value::Array(js), raw)
}

fn rust_run(appends: &[Vec<u8>], scratch: &Path) -> Value {
    std::fs::create_dir_all(scratch).unwrap();
    let file = scratch.join("1.session");
    std::fs::write(&file, b"").unwrap();
    let mut w = SessionWatcher::at(scratch.to_path_buf(), file.clone(), 0);
    let mut tracks = Vec::new();
    for a in appends {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap()
            .write_all(a)
            .unwrap();
        for ev in w.poll() {
            if let SeratoEvent::Track {
                deck,
                title,
                artist,
                album,
            } = ev
            {
                tracks.push(json!([deck, title, artist, album]));
            }
        }
    }
    let entries: Vec<Value> = parse_session_bytes(&std::fs::read(&file).unwrap())
        .0
        .into_iter()
        .map(|e| {
            json!([
                e.title,
                e.artist,
                e.album,
                e.genre,
                e.key,
                e.bpm,
                e.deck,
                e.start_time
            ])
        })
        .collect();
    std::fs::remove_dir_all(scratch).unwrap();
    json!({"tracks": tracks, "entries": entries})
}

/// Numbers compare as f64 (JavaScript prints 128 for 128.0, and 0 for -0).
fn norm(v: Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().unwrap()),
        Value::Array(a) => Value::Array(a.into_iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, norm(v))).collect()),
        other => other,
    }
}

#[test]
fn rust_serato_reader_matches_typescript_on_generated_sessions() {
    let bridge = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bridge");
    if !bridge.join("node_modules").exists() {
        eprintln!("skipped: run `npm ci` in bridge/ to compare with the TypeScript");
        return;
    }
    let count = std::env::var("DIFF_TRACES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let (traces, raw) = generate(5150, count);
    let mut child = Command::new("npx")
        .args(["tsx", "scripts/serato-trace.ts"])
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
    let scratch = std::env::temp_dir().join(format!("serato-diff-{}", std::process::id()));
    let (mut tracks, mut entries) = (0, 0);
    for (i, (appends, expected)) in raw.iter().zip(ts).enumerate() {
        let got = norm(rust_run(appends, &scratch));
        let expected = norm(expected);
        assert_eq!(got, expected, "session {i} differs");
        tracks += expected["tracks"].as_array().unwrap().len();
        entries += expected["entries"].as_array().unwrap().len();
    }
    eprintln!("{count} sessions, {entries} entries, {tracks} tracks emitted: Rust == TypeScript");
}
