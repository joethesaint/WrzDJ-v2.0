//! Talks to a WrzDJ server the way the Bridge does, for checking the Rust client end to end.
//!
//!   WRZDJ_API_URL=http://localhost:8000 WRZDJ_BRIDGE_API_KEY=... WRZDJ_EVENT_CODE=ABC123 \
//!     bridge-post nowplaying "Title" "Artist" | bridge-post status | bridge-post clear
//!
//! Same variables as the Node bridge (`bridge/src/config.ts`). Exit status 1 when the server
//! didn't take it after every retry.
use wrzdj_bridge_core::client::{Client, ClientConfig};
use wrzdj_bridge_core::http::HttpIo;

fn main() {
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let (url, key, code) = (
        env("WRZDJ_API_URL", "http://localhost:8000"),
        env("WRZDJ_BRIDGE_API_KEY", ""),
        env("WRZDJ_EVENT_CODE", ""),
    );
    if key.is_empty() || code.is_empty() {
        eprintln!("WRZDJ_BRIDGE_API_KEY and WRZDJ_EVENT_CODE are required");
        std::process::exit(2);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut client = Client::new(
        HttpIo::new(&url, &key),
        ClientConfig {
            event_code: code,
            min_play_s: 5,
        },
    );
    let ok = match args.first().map(String::as_str) {
        Some("nowplaying") if args.len() >= 3 => {
            client.post_now_playing(&args[1], &args[2], "1", "bridge-core")
        }
        Some("status") => client.post_status(true, Some("bridge-core"), "bridge-core"),
        Some("clear") => {
            client.clear_now_playing();
            true
        }
        _ => {
            eprintln!("usage: bridge-post nowplaying TITLE ARTIST | status | clear");
            std::process::exit(2);
        }
    };
    println!("{}", if ok { "ok" } else { "failed after retries" });
    std::process::exit(if ok { 0 } else { 1 });
}
