//! A scripted HTTP server for the shell tests: one connection per reply, in order.
//! Records every request so tests can check method, path, headers and body.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

pub enum Reply {
    /// Status and body.
    Status(u16, &'static str),
    /// Status, then a body cut short of its Content-Length (a body-stream failure).
    Truncated(u16),
    /// Read the request, then drop the connection without answering.
    Close,
    /// Read the request, wait, then drop the connection (for timeouts).
    Hang(u64),
}

/// Start the server. Returns its base URL and the requests it receives.
pub fn serve(replies: Vec<Reply>) -> (String, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for reply in replies {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            if let Some(seen) = handle(stream, reply) {
                let _ = tx.send(seen);
            }
        }
    });
    (url, rx)
}

fn handle(mut stream: TcpStream, reply: Reply) -> Option<Seen> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    let mut length = 0usize;
    loop {
        let mut h = String::new();
        reader.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            let (k, v) = (k.trim().to_string(), v.trim().to_string());
            if k.eq_ignore_ascii_case("content-length") {
                length = v.parse().unwrap_or(0);
            }
            headers.push((k, v));
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    let seen = Seen {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    };
    let head = |status: u16, len: usize| {
        format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
        )
    };
    match reply {
        Reply::Status(status, text) => {
            let _ = stream.write_all(format!("{}{text}", head(status, text.len())).as_bytes());
        }
        Reply::Truncated(status) => {
            let _ = stream.write_all(format!("{}{{\"user", head(status, 64)).as_bytes());
        }
        Reply::Close => {}
        Reply::Hang(ms) => thread::sleep(Duration::from_millis(ms)),
    }
    let _ = stream.flush();
    Some(seen)
}
