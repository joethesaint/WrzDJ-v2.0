//! The headless Rust Bridge: `bridge/src/index.ts` in Rust, for Traktor broadcast and Serato.
//!
//! Same environment as the Node bridge (`bridge/src/config.ts`): WRZDJ_API_URL,
//! WRZDJ_BRIDGE_API_KEY, WRZDJ_EVENT_CODE, WRZDJ_PLUGIN (`traktor-broadcast` or `serato`),
//! MIN_PLAY_SECONDS, LIVE_THRESHOLD_SECONDS, PAUSE_GRACE_SECONDS, NOW_PLAYING_PAUSE_SECONDS,
//! USE_FADER_DETECTION, MASTER_DECK_PRIORITY. Rust-only: WRZDJ_TRAKTOR_PORT (default 8123),
//! WRZDJ_SERATO_PATH (default ~/Music/_Serato_/History/Sessions), WRZDJ_SERATO_POLL_MS (1000).
//!
//! Not yet ported: the command poller (dashboard "reset decks" and the like), the shutdown clear
//! on Ctrl+C, and the plugin restart after a disconnect.
//!
//! Threads: the source (Traktor listener or Serato poller) sends events; this loop runs the deck
//! state machine (sleeping until its next timer or the 120 s heartbeat); one worker thread owns
//! the server client, so a slow or failing server never holds up deck timing.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use wrzdj_bridge_core::client::{Client, ClientConfig};
use wrzdj_bridge_core::deck::{Config, DeckStateManager, Output, Track};
use wrzdj_bridge_core::http::HttpIo;
use wrzdj_bridge_core::serato::{self, SeratoEvent, SessionWatcher};
use wrzdj_bridge_core::traktor::{self, TrackFilter, TraktorEvent};

const HEARTBEAT: Duration = Duration::from_secs(120);

/// What any source tells the loop (the plugin events `plugin-bridge.ts` listens to).
enum SourceEvent {
    Connected(&'static str),
    Disconnected,
    Track {
        deck: String,
        track: Track,
    },
    /// Stop the Bridge (from the server client: the key was refused).
    Stop(&'static str),
    /// Ctrl+C or SIGTERM: clear once, report disconnected, exit.
    Shutdown,
}

/// A source, as `plugin-types.ts` describes it.
struct Source {
    id: &'static str,
    multi_deck: bool,
}

enum Job {
    Live {
        deck: String,
        track: Track,
    },
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

fn start_traktor(tx: Sender<SourceEvent>) -> Source {
    let port: u16 = env("WRZDJ_TRAKTOR_PORT", &traktor::DEFAULT_PORT.to_string())
        .parse()
        .unwrap_or_else(|_| fail("WRZDJ_TRAKTOR_PORT must be a port number"));
    let (ttx, trx) = mpsc::channel();
    let port = traktor::listen(port, ttx).unwrap_or_else(|e| fail(&format!("listen: {e}")));
    log(&format!(
        "Configure Traktor: Settings → Broadcasting → Address: 127.0.0.1, Port: {port}"
    ));
    std::thread::spawn(move || {
        let mut filter = TrackFilter::default();
        for ev in trx {
            let out = match ev {
                TraktorEvent::Connected => SourceEvent::Connected("Traktor"),
                TraktorEvent::Disconnected => SourceEvent::Disconnected,
                TraktorEvent::Metadata(raw) => match filter.accept(&raw) {
                    Some(t) => SourceEvent::Track {
                        deck: "1".into(),
                        track: Track {
                            title: t.title,
                            artist: t.artist,
                            album: None,
                        },
                    },
                    None => continue,
                },
            };
            if tx.send(out).is_err() {
                return;
            }
        }
    });
    Source {
        id: "traktor-broadcast",
        multi_deck: false,
    }
}

fn start_serato(tx: Sender<SourceEvent>) -> Source {
    let dir = std::env::var_os("WRZDJ_SERATO_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(serato::default_sessions_dir);
    let poll = seconds("WRZDJ_SERATO_POLL_MS", serato::DEFAULT_POLL_MS);
    if !(serato::MIN_POLL_MS..=serato::MAX_POLL_MS).contains(&poll) {
        fail("WRZDJ_SERATO_POLL_MS must be between 200 and 10000");
    }
    log(&format!(
        "Looking for Serato session files in: {}",
        dir.display()
    ));
    std::thread::spawn(move || {
        let (mut watcher, first) = SessionWatcher::start(dir);
        let mut events = first;
        loop {
            for ev in events.drain(..) {
                let out = match ev {
                    SeratoEvent::Connected => SourceEvent::Connected("Serato DJ"),
                    SeratoEvent::Disconnected => SourceEvent::Disconnected,
                    SeratoEvent::Track {
                        deck,
                        title,
                        artist,
                        album,
                    } => SourceEvent::Track {
                        deck,
                        track: Track {
                            title,
                            artist,
                            album,
                        },
                    },
                };
                if tx.send(out).is_err() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(poll));
            events = watcher.poll();
        }
    });
    Source {
        id: "serato",
        multi_deck: true,
    }
}

fn same_track(a: &Track, b: &Track) -> bool {
    a.title.trim().to_lowercase() == b.title.trim().to_lowercase()
        && a.artist.trim().to_lowercase() == b.artist.trim().to_lowercase()
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
    let cfg = Config {
        live_threshold_s: seconds("LIVE_THRESHOLD_SECONDS", 15),
        pause_grace_s: seconds("PAUSE_GRACE_SECONDS", 3),
        now_playing_pause_s: seconds("NOW_PLAYING_PAUSE_SECONDS", 10),
        use_fader_detection: env("USE_FADER_DETECTION", "") == "true",
        master_deck_priority: env("MASTER_DECK_PRIORITY", "") == "true",
    };
    let min_play_s = seconds("MIN_PLAY_SECONDS", 5);

    let started = Instant::now();
    let (tx, rx) = mpsc::channel();
    let stop_tx = tx.clone();
    let signal_tx = tx.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        let _ = signal_tx.send(SourceEvent::Shutdown);
    }) {
        log(&format!("Could not listen for Ctrl+C: {e}"));
    }
    let source = match env("WRZDJ_PLUGIN", "traktor-broadcast").as_str() {
        "traktor-broadcast" => start_traktor(tx),
        "serato" => start_serato(tx),
        other => fail(&format!(
            "Plugin \"{other}\" isn't in the Rust Bridge yet. Available: traktor-broadcast, serato"
        )),
    };
    let (jobs, client_thread) = spawn_client(
        url.clone(),
        key,
        code.clone(),
        min_play_s,
        started,
        source.id,
        stop_tx,
    );
    log(&format!(
        "WrzDJ Bridge (Rust, {}) for event {code}, server {url}",
        source.id
    ));
    log("Bridge online — sending initial status to backend");
    let _ = jobs.send(Job::Status {
        connected: true,
        device: None,
        enriched: true,
        decks: 0,
    });

    let mut decks = DeckStateManager::new(cfg);
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
            Ok(SourceEvent::Connected(device)) => {
                log(&format!("Device connected: {device}"));
                next_heartbeat = Some(Instant::now() + HEARTBEAT);
                let n = decks.decks().len();
                let _ = jobs.send(Job::Status {
                    connected: true,
                    device: Some(device),
                    enriched: true,
                    decks: n,
                });
            }
            Ok(SourceEvent::Disconnected) => {
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
            Ok(SourceEvent::Track { deck, track }) => {
                // plugin-bridge.ts: one virtual deck for single-deck sources, skip a repeat,
                // and fake "playing" for sources that don't report it.
                let deck = if source.multi_deck {
                    deck
                } else {
                    "1".to_string()
                };
                if !decks.track_on(&deck).is_some_and(|c| same_track(c, &track)) {
                    log(&format!(
                        "Deck {deck}: \"{}\" by {}",
                        track.title, track.artist
                    ));
                    let _ = decks.update_track_info(&deck, Some(track));
                    let _ = decks.update_play_state(&deck, true);
                }
            }
            Ok(SourceEvent::Shutdown) => {
                // index.ts shutdown(): one clear, waited for, then disconnected (not two clears).
                log("Received a stop signal, shutting down...");
                let _ = jobs.send(Job::Clear);
                let _ = jobs.send(Job::Status {
                    connected: false,
                    device: None,
                    enriched: false,
                    decks: 0,
                });
                break;
            }
            Ok(SourceEvent::Stop(reason)) => {
                eprintln!("[Bridge] Stopped: {reason}");
                std::process::exit(3);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => fail("the source stopped"),
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
                    let _ = jobs.send(Job::Live { deck, track });
                }
                Output::Cleared { .. } => {
                    let _ = jobs.send(Job::Clear);
                }
            }
        }
    }
    // Shutdown: let the worker finish the clear and the status, then exit.
    drop(jobs);
    let _ = client_thread.join();
    log("Bridge stopped.");
}

fn spawn_client(
    url: String,
    key: String,
    code: String,
    min_play_s: u64,
    started: Instant,
    plugin_id: &'static str,
    stop: Sender<SourceEvent>,
) -> (Sender<Job>, std::thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<Job>();
    let handle = std::thread::Builder::new()
        .name("server-client".into())
        .spawn(move || {
            let cfg = ClientConfig {
                event_code: code,
                min_play_s,
            };
            let mut client = Client::new(HttpIo::new(&url, &key), cfg);
            for job in rx {
                match job {
                    Job::Live { deck, track: t } => {
                        if client.should_skip(&t.artist, &t.title) {
                            continue;
                        }
                        client.update_last_track(&t.artist, &t.title);
                        let ok = client.post_now_playing(
                            &t.title,
                            &t.artist,
                            t.album.as_deref(),
                            &deck,
                            plugin_id,
                        );
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
                        client.post_status_enriched(connected, device, plugin_id, decks, up);
                    }
                    Job::Status {
                        connected, device, ..
                    } => {
                        client.post_status(connected, device, plugin_id);
                    }
                    Job::Clear => client.clear_now_playing(),
                }
                // bridge-runner.ts: a 401 stops the Bridge; its stop() clears and reports
                // disconnected first.
                if client.session_expired() {
                    eprintln!("[Bridge] POST returned 401 — session expired");
                    client.clear_now_playing();
                    client.post_status(false, None, plugin_id);
                    let _ = stop.send(SourceEvent::Stop("Session expired — please log in again"));
                    return;
                }
            }
        })
        .expect("start the server client");
    (tx, handle)
}
