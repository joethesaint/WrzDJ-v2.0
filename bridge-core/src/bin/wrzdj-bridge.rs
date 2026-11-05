//! The headless Rust Bridge, Traktor broadcast only for now: `bridge/src/index.ts` in Rust.
//!
//! Same environment as the Node bridge (`bridge/src/config.ts`): WRZDJ_API_URL,
//! WRZDJ_BRIDGE_API_KEY, WRZDJ_EVENT_CODE, WRZDJ_PLUGIN (only `traktor-broadcast`),
//! MIN_PLAY_SECONDS, LIVE_THRESHOLD_SECONDS, PAUSE_GRACE_SECONDS, NOW_PLAYING_PAUSE_SECONDS,
//! USE_FADER_DETECTION, MASTER_DECK_PRIORITY; plus WRZDJ_TRAKTOR_PORT (default 8123).
//!
//! Not yet ported: the command poller (dashboard "reset decks" and the like) and the shutdown
//! clear on Ctrl+C.
//!
//! Threads: the Traktor listener sends stream events; this loop runs the deck state machine
//! (sleeping until its next timer or the 120 s heartbeat); one worker thread owns the server
//! client, so a slow or failing server never holds up deck timing.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use wrzdj_bridge_core::client::{Client, ClientConfig};
use wrzdj_bridge_core::deck::{Config, DeckStateManager, Output, Track};
use wrzdj_bridge_core::http::HttpIo;
use wrzdj_bridge_core::traktor::{self, TrackFilter, TraktorEvent};

const PLUGIN_ID: &str = "traktor-broadcast";
const VIRTUAL_DECK: &str = "1";
const HEARTBEAT: Duration = Duration::from_secs(120);

enum Job {
    Live(Track),
    Status {
        connected: bool,
        device: Option<&'static str>,
        enriched: bool,
        decks: usize,
    },
    Clear,
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn seconds(key: &str, default: u64) -> u64 {
    match env(key, &default.to_string()).parse() {
        Ok(v) => v,
        Err(_) => fail(&format!("{key} must be a non-negative number")),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("[Bridge] {message}");
    std::process::exit(1);
}

fn log(message: &str) {
    println!("[Bridge] {message}");
}

fn main() {
    let (url, key, code) = (
        env("WRZDJ_API_URL", "http://localhost:8000"),
        env("WRZDJ_BRIDGE_API_KEY", ""),
        env("WRZDJ_EVENT_CODE", ""),
    );
    if key.is_empty() {
        fail("WRZDJ_BRIDGE_API_KEY is required. Generate one with: openssl rand -hex 32");
    }
    if code.is_empty() {
        fail("WRZDJ_EVENT_CODE is required. This is the event code from the DJ dashboard.");
    }
    let plugin = env("WRZDJ_PLUGIN", PLUGIN_ID);
    if plugin != PLUGIN_ID {
        fail(&format!(
            "Plugin \"{plugin}\" isn't in the Rust Bridge yet. Available: {PLUGIN_ID}"
        ));
    }
    let cfg = Config {
        live_threshold_s: seconds("LIVE_THRESHOLD_SECONDS", 15),
        pause_grace_s: seconds("PAUSE_GRACE_SECONDS", 3),
        now_playing_pause_s: seconds("NOW_PLAYING_PAUSE_SECONDS", 10),
        use_fader_detection: env("USE_FADER_DETECTION", "") == "true",
        master_deck_priority: env("MASTER_DECK_PRIORITY", "") == "true",
    };
    let min_play_s = seconds("MIN_PLAY_SECONDS", 5);
    let port: u16 = env("WRZDJ_TRAKTOR_PORT", &traktor::DEFAULT_PORT.to_string())
        .parse()
        .unwrap_or_else(|_| fail("WRZDJ_TRAKTOR_PORT must be a port number"));

    let started = Instant::now();
    let jobs = spawn_client(url.clone(), key, code.clone(), min_play_s, started);
    let (tx, rx) = mpsc::channel();
    let port = traktor::listen(port, tx).unwrap_or_else(|e| fail(&format!("listen: {e}")));
    log(&format!(
        "WrzDJ Bridge (Rust) for event {code}, server {url}"
    ));
    log(&format!(
        "Configure Traktor: Settings → Broadcasting → Address: 127.0.0.1, Port: {port}"
    ));
    log("Bridge online — sending initial status to backend");
    let _ = jobs.send(Job::Status {
        connected: true,
        device: None,
        enriched: true,
        decks: 0,
    });

    let mut decks = DeckStateManager::new(cfg);
    let mut filter = TrackFilter::default();
    let mut next_heartbeat: Option<Instant> = None;
    loop {
        let now_ms = started.elapsed().as_millis() as u64;
        let mut wait = Duration::from_secs(3600);
        if let Some(due) = decks.next_due() {
            wait = wait.min(Duration::from_millis(due.saturating_sub(now_ms)));
        }
        if let Some(h) = next_heartbeat {
            wait = wait.min(h.saturating_duration_since(Instant::now()));
        }
        // Every wait has a floor, so a timer due now can't spin the loop.
        let event = rx.recv_timeout(wait.max(Duration::from_millis(5)));
        decks.advance_to(started.elapsed().as_millis() as u64);
        match event {
            Ok(TraktorEvent::Connected) => {
                log("Device connected: Traktor");
                next_heartbeat = Some(Instant::now() + HEARTBEAT);
                let n = decks.decks().len();
                let _ = jobs.send(Job::Status {
                    connected: true,
                    device: Some("Traktor"),
                    enriched: true,
                    decks: n,
                });
            }
            Ok(TraktorEvent::Disconnected) => {
                log("Device disconnected");
                next_heartbeat = None;
                let _ = jobs.send(Job::Clear);
                let _ = jobs.send(Job::Status {
                    connected: false,
                    device: None,
                    enriched: false,
                    decks: 0,
                });
            }
            Ok(TraktorEvent::Metadata(raw)) => {
                if let Some(t) = filter.accept(&raw) {
                    let track = Track {
                        title: t.title,
                        artist: t.artist,
                    };
                    let same = decks.track_on(VIRTUAL_DECK).is_some_and(|c| {
                        c.title.trim().to_lowercase() == track.title.trim().to_lowercase()
                            && c.artist.trim().to_lowercase() == track.artist.trim().to_lowercase()
                    });
                    if !same {
                        log(&format!("Track: \"{}\" by {}", track.title, track.artist));
                        let _ = decks.update_track_info(VIRTUAL_DECK, Some(track));
                        let _ = decks.update_play_state(VIRTUAL_DECK, true);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => fail("the Traktor listener stopped"),
        }
        if next_heartbeat.is_some_and(|h| Instant::now() >= h) {
            next_heartbeat = Some(Instant::now() + HEARTBEAT);
            let n = decks.decks().len();
            let _ = jobs.send(Job::Status {
                connected: true,
                device: None,
                enriched: true,
                decks: n,
            });
        }
        for out in decks.take_outputs() {
            match out {
                Output::Live { deck, track, .. } => {
                    log(&format!(
                        "Deck {deck} LIVE: \"{}\" by {}",
                        track.title, track.artist
                    ));
                    let _ = jobs.send(Job::Live(track));
                }
                Output::Cleared { .. } => {
                    let _ = jobs.send(Job::Clear);
                }
            }
        }
    }
}

fn spawn_client(
    url: String,
    key: String,
    code: String,
    min_play_s: u64,
    started: Instant,
) -> Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    std::thread::Builder::new()
        .name("server-client".into())
        .spawn(move || {
            let cfg = ClientConfig {
                event_code: code,
                min_play_s,
            };
            let mut client = Client::new(HttpIo::new(&url, &key), cfg);
            for job in rx {
                match job {
                    Job::Live(t) => {
                        if client.should_skip(&t.artist, &t.title) {
                            continue;
                        }
                        client.update_last_track(&t.artist, &t.title);
                        let ok =
                            client.post_now_playing(&t.title, &t.artist, VIRTUAL_DECK, PLUGIN_ID);
                        log(&format!(
                            "Now Playing \"{}\": {}",
                            t.title,
                            if ok { "sent" } else { "buffered" }
                        ));
                    }
                    Job::Status {
                        connected,
                        device,
                        enriched: true,
                        decks,
                    } => {
                        let up = started.elapsed().as_secs();
                        client.post_status_enriched(connected, device, PLUGIN_ID, decks, up);
                    }
                    Job::Status {
                        connected, device, ..
                    } => {
                        client.post_status(connected, device, PLUGIN_ID);
                    }
                    Job::Clear => client.clear_now_playing(),
                }
            }
        })
        .expect("start the server client");
    tx
}
