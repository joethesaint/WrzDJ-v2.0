//! Port of `bridge/src/plugins/traktor-broadcast-plugin.ts`: Traktor's broadcast as a track source.
//!
//! Traktor (Settings → Broadcasting) streams to `127.0.0.1:<port>`. With an `icy-metaint` header
//! the stream carries a metadata block every `metaint` audio bytes (one length byte, then
//! length × 16 bytes such as `StreamTitle='Artist - Title';`). Without it, any chunk that
//! contains `StreamTitle=` is read as metadata. A title different from the last one is a track.
//!
//! [`IcyParser`], [`header_chunk`] and [`parse_stream_title`] are pure and are compared with the
//! TypeScript in `tests/traktor_differential.rs`. [`listen`] is the TCP side. Unlike the Node
//! server, it also reads a body sent with no length (an Icecast `SOURCE` client) to its end.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;

use crate::log;

pub const DEFAULT_PORT: u16 = 8123;
pub const MAX_ICY_META_LENGTH: usize = 16_384;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamTrack {
    pub artist: String,
    pub title: String,
}

/// What the listener reports.
#[derive(Debug)]
pub enum TraktorEvent {
    Connected,
    Disconnected,
    Metadata(String),
}

/// JavaScript's `String.prototype.trim` whitespace (it differs from Rust's: U+FEFF yes, U+0085 no).
fn is_js_space(c: char) -> bool {
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
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

fn is_js_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `parseStreamTitle`: `StreamTitle='Artist - Title'` (or bare text) to artist and title.
pub fn parse_stream_title(raw: &str) -> Option<StreamTrack> {
    if js_trim(raw).is_empty() {
        return None;
    }
    // /StreamTitle='(.*)'/: the first position that matches, up to the last quote on that line.
    let mut content = raw;
    let mut from = 0;
    while let Some(i) = raw[from..].find("StreamTitle='") {
        let start = from + i + "StreamTitle='".len();
        let line = &raw[start..];
        let line = &line[..line.find(is_js_line_end).unwrap_or(line.len())];
        if let Some(q) = line.rfind('\'') {
            content = &line[..q];
            break;
        }
        from = from + i + 1;
    }
    if js_trim(content).is_empty() {
        return None;
    }
    Some(match content.find(" - ") {
        None => StreamTrack {
            artist: String::new(),
            title: js_trim(content).to_string(),
        },
        Some(sep) => StreamTrack {
            artist: js_trim(&content[..sep]).to_string(),
            title: js_trim(&content[sep + 3..]).to_string(),
        },
    })
}

/// The ICY reader: audio bytes are skipped, metadata blocks are returned as text.
pub struct IcyParser {
    meta_int: usize,
    byte_count: usize,
    meta: Vec<u8>,
    reading_meta: bool,
    meta_length: usize,
}

impl IcyParser {
    pub fn new(meta_int: usize) -> Self {
        IcyParser {
            meta_int,
            byte_count: 0,
            meta: Vec::new(),
            reading_meta: false,
            meta_length: 0,
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut found = Vec::new();
        let mut pos = 0;
        while pos < chunk.len() {
            if self.reading_meta {
                let take = (self.meta_length - self.meta.len()).min(chunk.len() - pos);
                self.meta.extend_from_slice(&chunk[pos..pos + take]);
                pos += take;
                if self.meta.len() >= self.meta_length {
                    let text = String::from_utf8_lossy(&self.meta);
                    let text = text.trim_end_matches('\0');
                    if !text.is_empty() {
                        found.push(text.to_string());
                    }
                    self.reading_meta = false;
                    self.meta.clear();
                    self.byte_count = 0;
                }
            } else {
                let audio = (self.meta_int - self.byte_count).min(chunk.len() - pos);
                pos += audio;
                self.byte_count += audio;
                if self.byte_count >= self.meta_int && pos < chunk.len() {
                    self.meta_length = chunk[pos] as usize * 16;
                    pos += 1;
                    if self.meta_length == 0 || self.meta_length > MAX_ICY_META_LENGTH {
                        self.meta_length = 0;
                        self.byte_count = 0;
                    } else {
                        self.reading_meta = true;
                        self.meta.clear();
                    }
                }
            }
        }
        found
    }
}

/// Header mode: a chunk is metadata when its text contains `StreamTitle=`.
pub fn header_chunk(chunk: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(chunk);
    text.contains("StreamTitle=").then(|| text.into_owned())
}

/// `processMetadata`: a track when the title differs from the last one.
#[derive(Default)]
pub struct TrackFilter {
    last: Option<String>,
}

impl TrackFilter {
    pub fn accept(&mut self, raw: &str) -> Option<StreamTrack> {
        let t = parse_stream_title(raw)?;
        let key = format!("{}::{}", t.artist, t.title);
        if self.last.as_deref() == Some(&key) {
            return None;
        }
        self.last = Some(key);
        Some(t)
    }
}

/// Accepts Traktor on `127.0.0.1:port`, one thread per connection; events go to `tx`.
pub fn listen(port: u16, tx: Sender<TraktorEvent>) -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let bound = listener.local_addr()?.port();
    std::thread::Builder::new()
        .name("traktor-listen".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let stream = match stream {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn("traktor:accept", e);
                        continue;
                    }
                };
                let tx = tx.clone();
                let spawned = std::thread::Builder::new()
                    .name("traktor-conn".into())
                    .spawn(move || log::safely("traktor:conn", (), || serve(stream, &tx)));
                if let Err(e) = spawned {
                    log::error(
                        "traktor:thread",
                        format!("could not start a connection thread: {e}"),
                    );
                }
            }
        })?;
    Ok(bound)
}

fn serve(stream: TcpStream, tx: &Sender<TraktorEvent>) {
    let _ = tx.send(TraktorEvent::Connected);
    if let Err(e) = read_stream(stream, tx) {
        log::warn("traktor:read", e);
    }
    let _ = tx.send(TraktorEvent::Disconnected);
}

fn read_stream(mut stream: TcpStream, tx: &Sender<TraktorEvent>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let (mut meta_int, mut chunked, mut length) = (0usize, false, None::<usize>);
    let mut line = String::new();
    let mut head = 0;
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        head += n;
        if n == 0 || head > 16_384 {
            return Ok(());
        }
        let l = line.trim_end();
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            let v = v.trim();
            match k.trim().to_ascii_lowercase().as_str() {
                "icy-metaint" => meta_int = v.parse().unwrap_or(0),
                "transfer-encoding" => chunked = v.eq_ignore_ascii_case("chunked"),
                "content-length" => length = v.parse().ok(),
                _ => {}
            }
        }
    }
    stream.write_all(b"HTTP/1.1 200 OK\r\nicy-metaint: 0\r\n\r\n")?;
    let mut icy = (meta_int > 0).then(|| IcyParser::new(meta_int));
    let mut deliver = |chunk: &[u8]| {
        let texts = match icy.as_mut() {
            Some(p) => p.feed(chunk),
            None => header_chunk(chunk).into_iter().collect(),
        };
        for t in texts {
            let _ = tx.send(TraktorEvent::Metadata(t));
        }
    };
    let mut buf = vec![0u8; 16 * 1024];
    let cap = buf.len();
    if chunked {
        loop {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                return Ok(());
            }
            let size =
                usize::from_str_radix(line.trim().split(';').next().unwrap_or(""), 16).unwrap_or(0);
            if size == 0 {
                return Ok(());
            }
            let mut left = size;
            while left > 0 {
                let n = reader.read(&mut buf[..left.min(cap)])?;
                if n == 0 {
                    return Ok(());
                }
                deliver(&buf[..n]);
                left -= n;
            }
            line.clear();
            reader.read_line(&mut line)?; // the CRLF after each chunk
        }
    }
    let mut left = length.unwrap_or(usize::MAX);
    while left > 0 {
        let n = reader.read(&mut buf[..left.min(cap)])?;
        if n == 0 {
            return Ok(());
        }
        deliver(&buf[..n]);
        left -= n;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_artist_and_title() {
        let t = parse_stream_title("StreamTitle='Daft Punk - One More Time';").unwrap();
        assert_eq!(
            (t.artist.as_str(), t.title.as_str()),
            ("Daft Punk", "One More Time")
        );
    }

    #[test]
    fn reads_a_metadata_block_split_across_chunks() {
        let mut p = IcyParser::new(4);
        let meta = b"StreamTitle='A - B';";
        let mut block = vec![(meta.len().div_ceil(16)) as u8];
        block.extend_from_slice(meta);
        block.resize(1 + 16 * block[0] as usize, 0);
        let mut stream = b"abcd".to_vec();
        stream.extend_from_slice(&block);
        let (x, y) = stream.split_at(5);
        assert!(p.feed(x).is_empty());
        assert_eq!(p.feed(y), vec!["StreamTitle='A - B';".to_string()]);
    }
}
