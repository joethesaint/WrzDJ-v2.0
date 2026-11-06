//! Port of `bridge/src/plugins/serato-plugin.ts` and `serato-session-parser.ts`: Serato DJ as a
//! track source, read from its history files.
//!
//! Serato appends to `~/Music/_Serato_/History/Sessions/<n>.session` while it plays. The file is a
//! run of chunks: a 4-byte ASCII tag, a big-endian u32 length, then the body. An `oent` chunk
//! holds an `adat` chunk whose body is fields: u32 id, u32 length, data. Text fields are UTF-16BE.
//!
//! [`SessionWatcher`] keeps the TypeScript's rules: on start it watches the newest session from its
//! end (history is skipped); each poll reads only new bytes; a chunk not yet fully written is read
//! again next time; a deleted file means disconnected; 5 read errors in a row rescan; a track with
//! no title and no artist is ignored; the same track on the same deck isn't sent twice.
//!
//! Two differences, both on purpose:
//! - The TypeScript notices a new session file through `fs.watch` (not reliable everywhere); this
//!   checks for the newest file on every poll. The outcome is the same as the watcher's: the new
//!   file is read from its start.
//! - JavaScript keeps a lone UTF-16 surrogate in a string, which the server can't store; Rust
//!   strings can't hold one, so it becomes U+FFFD.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::log;

pub const DEFAULT_POLL_MS: u64 = 1000;
pub const MIN_POLL_MS: u64 = 200;
pub const MAX_POLL_MS: u64 = 10_000;
pub const READ_ERRORS_BEFORE_RESCAN: u32 = 5;

const FIELD_TITLE: u32 = 2;
const FIELD_ARTIST: u32 = 6;
const FIELD_ALBUM: u32 = 8;
const FIELD_GENRE: u32 = 10;
const FIELD_BPM: u32 = 13;
const FIELD_KEY: u32 = 29;
const FIELD_ADDED: u32 = 31;
const FIELD_DECK: u32 = 52;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionEntry {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub bpm: f64,
    pub key: String,
    pub genre: String,
    pub deck: u32,
    pub start_time: u32,
}

/// `decodeUtf16BE`: code units up to the first zero; an odd last byte is ignored.
pub fn decode_utf16be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// JavaScript's `parseFloat`: the longest number at the start, after leading whitespace.
pub fn js_parse_float(text: &str) -> Option<f64> {
    let s = text.trim_start_matches(|c: char| {
        matches!(
            c,
            '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
                ..='\u{200a}'
                    | '\u{2028}'
                    | '\u{2029}'
                    | '\u{202f}'
                    | '\u{205f}'
                    | '\u{3000}'
                    | '\u{feff}'
        )
    });
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    if s[i..].starts_with("Infinity") {
        return Some(if b.first() == Some(&b'-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - start
    };
    let mut n = digits(&mut i);
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        let frac = digits(&mut j);
        if n + frac > 0 {
            i = j;
            n += frac;
        }
    }
    if n == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if digits(&mut j) > 0 {
            i = j;
        }
    }
    s[..i].parse().ok()
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Node's `ascii` decoding drops each byte's high bit, so tags compare that way.
fn tag_is(b: &[u8], at: usize, tag: &[u8; 4]) -> bool {
    (0..4).all(|k| b[at + k] & 0x7f == tag[k])
}

/// `parseAdatContent`: the fields of one entry.
pub fn parse_adat(b: &[u8]) -> SessionEntry {
    let mut e = SessionEntry::default();
    let mut pos = 0usize;
    while pos + 8 <= b.len() {
        let id = u32_at(b, pos);
        let len = u32_at(b, pos + 4) as usize;
        pos += 8;
        if pos + len > b.len() {
            break;
        }
        let data = &b[pos..pos + len];
        match id {
            FIELD_TITLE => e.title = decode_utf16be(data),
            FIELD_ARTIST => e.artist = decode_utf16be(data),
            FIELD_ALBUM => e.album = decode_utf16be(data),
            FIELD_GENRE => e.genre = decode_utf16be(data),
            FIELD_BPM => {
                if let Some(v) = js_parse_float(&decode_utf16be(data)) {
                    e.bpm = v;
                }
            }
            FIELD_KEY => e.key = decode_utf16be(data),
            FIELD_DECK if len >= 4 => e.deck = u32_at(data, 0),
            FIELD_ADDED if len >= 4 => e.start_time = u32_at(data, 0),
            _ => {}
        }
        pos += len;
    }
    e
}

/// `parseSessionBytes`: the entries in whole chunks, and how many bytes those chunks used.
pub fn parse_session_bytes(b: &[u8]) -> (Vec<SessionEntry>, usize) {
    let mut entries = Vec::new();
    let (mut pos, mut consumed) = (0usize, 0usize);
    while pos + 8 <= b.len() {
        let is_oent = tag_is(b, pos, b"oent");
        let len = u32_at(b, pos + 4) as usize;
        pos += 8;
        if pos + len > b.len() {
            break;
        }
        if is_oent {
            let end = pos + len;
            let mut inner = pos;
            while inner + 8 <= end {
                let is_adat = tag_is(b, inner, b"adat");
                let ilen = u32_at(b, inner + 4) as usize;
                inner += 8;
                if inner + ilen > end {
                    break;
                }
                if is_adat {
                    entries.push(parse_adat(&b[inner..inner + ilen]));
                }
                inner += ilen;
            }
        }
        pos += len;
        consumed = pos;
    }
    (entries, consumed)
}

/// The sessions folder Serato writes to.
pub fn default_sessions_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Music")
        .join("_Serato_")
        .join("History")
        .join("Sessions")
}

/// `findLatestSessionFile`: the `.session` file changed most recently.
pub fn find_latest_session(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("session") {
            continue;
        }
        if let Ok(modified) = entry.metadata().and_then(|m| m.modified())
            && best.as_ref().is_none_or(|(t, _)| modified > *t)
        {
            best = Some((modified, path));
        }
    }
    best.map(|(_, p)| p)
}

/// What the watcher reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeratoEvent {
    Connected,
    Disconnected,
    Track {
        deck: String,
        title: String,
        artist: String,
        album: Option<String>,
    },
}

pub struct SessionWatcher {
    dir: PathBuf,
    session: Option<PathBuf>,
    offset: u64,
    last_per_deck: HashMap<String, String>,
    read_errors: u32,
}

impl SessionWatcher {
    /// `start`: the newest session, watched from its end.
    pub fn start(dir: PathBuf) -> (Self, Vec<SeratoEvent>) {
        let mut w = SessionWatcher {
            dir,
            session: None,
            offset: 0,
            last_per_deck: HashMap::new(),
            read_errors: 0,
        };
        let events = w.locate();
        (w, events)
    }

    /// Watch a session file from a given offset (for tests and the trace harness).
    pub fn at(dir: PathBuf, session: PathBuf, offset: u64) -> Self {
        SessionWatcher {
            dir,
            session: Some(session),
            offset,
            last_per_deck: HashMap::new(),
            read_errors: 0,
        }
    }

    pub fn session(&self) -> Option<&Path> {
        self.session.as_deref()
    }

    fn locate(&mut self) -> Vec<SeratoEvent> {
        match find_latest_session(&self.dir) {
            None => vec![SeratoEvent::Disconnected],
            Some(path) => {
                self.offset = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                self.session = Some(path);
                vec![SeratoEvent::Connected]
            }
        }
    }

    /// One poll: a newer session file is switched to and read from its start (the TypeScript's
    /// directory watcher), then any new whole chunks of the current file become tracks.
    pub fn poll(&mut self) -> Vec<SeratoEvent> {
        let mut out = Vec::new();
        if let Some(newest) = find_latest_session(&self.dir)
            && self.session.as_deref() != Some(newest.as_path())
        {
            self.switch_to(newest, &mut out);
        }
        let Some(path) = self.session.clone() else {
            return out;
        };
        let mut file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::warn("serato:session", format!("{} is gone", path.display()));
                self.session = None;
                out.push(SeratoEvent::Disconnected);
                return out;
            }
            Err(e) => {
                log::warn("serato:read", format!("open {}: {e}", path.display()));
                self.read_error(&mut out);
                return out;
            }
        };
        let bytes = match read_from(&mut file, self.offset) {
            Ok(Some(b)) => b,
            Ok(None) => return out,
            Err(e) => {
                log::warn("serato:read", format!("read {}: {e}", path.display()));
                self.read_error(&mut out);
                return out;
            }
        };
        self.read_errors = 0;
        let (entries, consumed) = parse_session_bytes(&bytes);
        self.offset += consumed as u64;
        for e in entries {
            self.process(e, &mut out);
        }
        out
    }

    fn switch_to(&mut self, path: PathBuf, out: &mut Vec<SeratoEvent>) {
        self.session = Some(path);
        self.offset = 0;
        self.last_per_deck.clear();
        out.push(SeratoEvent::Connected);
    }

    fn read_error(&mut self, out: &mut Vec<SeratoEvent>) {
        self.read_errors += 1;
        if self.read_errors >= READ_ERRORS_BEFORE_RESCAN {
            log::warn(
                "serato:rescan",
                format!(
                    "session file unreadable after {} attempts — rescanning",
                    self.read_errors
                ),
            );
            self.session = None;
            self.offset = 0;
            self.read_errors = 0;
            out.push(SeratoEvent::Disconnected);
        }
    }

    /// `processEntry`: a track per deck, without repeats.
    pub fn process(&mut self, e: SessionEntry, out: &mut Vec<SeratoEvent>) {
        if e.title.is_empty() && e.artist.is_empty() {
            return;
        }
        let deck = if e.deck == 0 {
            "1".to_string()
        } else {
            e.deck.to_string()
        };
        let key = format!("{}::{}", e.artist, e.title);
        if self.last_per_deck.get(&deck) == Some(&key) {
            return;
        }
        self.last_per_deck.insert(deck.clone(), key);
        let album = (!e.album.is_empty()).then_some(e.album);
        out.push(SeratoEvent::Track {
            deck,
            title: e.title,
            artist: e.artist,
            album,
        });
    }
}

/// The bytes after `offset`, or None when the file isn't longer than that.
fn read_from(file: &mut fs::File, offset: u64) -> std::io::Result<Option<Vec<u8>>> {
    let size = file.metadata()?.len();
    if size <= offset {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity((size - offset) as usize);
    file.take(size - offset).read_to_end(&mut buf)?;
    Ok(Some(buf))
}

/// Writing Serato's format: for the simulator and the tests.
pub mod write {
    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(body);
        out
    }

    pub fn text_field(id: u32, text: &str) -> Vec<u8> {
        let mut data: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        data.extend_from_slice(&[0, 0]);
        field(id, &data)
    }

    pub fn field(id: u32, data: &[u8]) -> Vec<u8> {
        let mut out = id.to_be_bytes().to_vec();
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(data);
        out
    }

    /// An `oent` chunk wrapping an `adat` chunk made of `fields`.
    pub fn entry(fields: &[Vec<u8>]) -> Vec<u8> {
        chunk(b"oent", &chunk(b"adat", &fields.concat()))
    }

    /// One played track.
    pub fn track(title: &str, artist: &str, album: &str, deck: u32) -> Vec<u8> {
        entry(&[
            text_field(super::FIELD_TITLE, title),
            text_field(super::FIELD_ARTIST, artist),
            text_field(super::FIELD_ALBUM, album),
            field(super::FIELD_DECK, &deck.to_be_bytes()),
        ])
    }

    /// The version chunk a session file starts with.
    pub fn header() -> Vec<u8> {
        let v: Vec<u8> = "2.0/Serato Scratch LIVE Review"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        chunk(b"vrsn", &v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_written_track() {
        let bytes = write::track("Water", "Tyla", "TYLA", 2);
        let (entries, used) = parse_session_bytes(&bytes);
        assert_eq!(used, bytes.len());
        assert_eq!(entries[0].title, "Water");
        assert_eq!((entries[0].artist.as_str(), entries[0].deck), ("Tyla", 2));
    }

    #[test]
    fn leaves_a_half_written_chunk_for_the_next_read() {
        let mut bytes = write::header();
        let whole = bytes.len();
        bytes.extend_from_slice(&write::track("A", "B", "", 1)[..10]);
        assert_eq!(parse_session_bytes(&bytes), (vec![], whole));
    }

    #[test]
    fn parse_float_reads_a_leading_number_like_javascript() {
        assert_eq!(js_parse_float(" 128.00 BPM"), Some(128.0));
        assert_eq!(js_parse_float("1e3x"), Some(1000.0));
        assert_eq!(js_parse_float(".5"), Some(0.5));
        assert_eq!(js_parse_float("-Infinity"), Some(f64::NEG_INFINITY));
        assert_eq!(js_parse_float("abc"), None);
        assert_eq!(js_parse_float("."), None);
    }
}
