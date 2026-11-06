//! The headless Rust Bridge: `bridge/src/index.ts` in Rust, for Traktor broadcast and Serato.
//!
//! Same environment as the Node bridge (`bridge/src/config.ts`): WRZDJ_API_URL,
//! WRZDJ_BRIDGE_API_KEY, WRZDJ_EVENT_CODE, WRZDJ_PLUGIN (`traktor-broadcast` or `serato`),
//! MIN_PLAY_SECONDS, LIVE_THRESHOLD_SECONDS, PAUSE_GRACE_SECONDS, NOW_PLAYING_PAUSE_SECONDS,
//! USE_FADER_DETECTION, MASTER_DECK_PRIORITY. Rust-only: WRZDJ_TRAKTOR_PORT (default 8123),
//! WRZDJ_SERATO_PATH (default ~/Music/_Serato_/History/Sessions), WRZDJ_SERATO_POLL_MS (1000).
//!
//! Also: WRZDJ_LOG_DIR (the run log: errors.log for people, errors.jsonl for agents), and
//! WRZDJ_REMEMBER_KEY=true to keep the key in the OS keychain, so later starts need no
//! WRZDJ_BRIDGE_API_KEY. The event is checked every 30 s; deleted or expired stops the Bridge.
//!
//! Dashboard commands are polled every 5 s (`command-poller.ts`): reset_decks, reconnect,
//! restart, setbuilder_transport (logged). After a disconnect the source restarts with backoff
//! (2 s, doubling, at most 30 s; `plugin-bridge.ts`). For Serato a restart starts a new session
//! watcher; for Traktor the listener stays bound and the title filter starts afresh.
//!
//! Threads: the source (Traktor listener or Serato poller) sends events; this loop runs the deck
//! state machine (sleeping until its next timer or the 120 s heartbeat); one worker thread owns
//! the server client, so a slow or failing server never holds up deck timing.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wrzdj_bridge_core::client::{Client, ClientConfig};
use wrzdj_bridge_core::deck::{Config, DeckStateManager, Output, Track};
use wrzdj_bridge_core::http::HttpIo;
use wrzdj_bridge_core::log as runlog_helpers;
use wrzdj_bridge_core::serato::{self, SeratoEvent, SessionWatcher};
use wrzdj_bridge_core::shell::keychain::KeychainVault;
use wrzdj_bridge_core::shell::runlog;
use wrzdj_bridge_core::shell::settings::TokenVault;
use wrzdj_bridge_core::shell::watch::{HEALTH_CHECK_INTERVAL, HealthWatcher, network_warnings};
use wrzdj_bridge_core::traktor::{self, TrackFilter, TraktorEvent};

const HEARTBEAT: Duration = Duration::from_secs(120);
/// `command-poller.ts` POLL_INTERVAL_MS.
const COMMAND_POLL: Duration = Duration::from_secs(5);
/// `plugin-bridge.ts` RECONNECT_INITIAL_MS, RECONNECT_MAX_MS (doubling each attempt).
const RECONNECT_INITIAL_MS: u64 = 2_000;
const RECONNECT_MAX_MS: u64 = 30_000;

/// The wait before reconnect attempt `attempt` (1-based).
fn reconnect_delay(attempt: u32) -> Duration {
    let ms = RECONNECT_INITIAL_MS.saturating_mul(1u64 << (attempt - 1).min(20));
    Duration::from_millis(ms.min(RECONNECT_MAX_MS))
}

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
    /// A dashboard command (`command_type`, `payload`).
    Command(String, serde_json::Value),
}

/// A source, as `plugin-types.ts` describes it.
struct Source {
    id: &'static str,
    name: &'static str,
    multi_deck: bool,
    /// Stop the source and start it again (the plugin's stop() then start()).
    restart: Box<dyn FnMut()>,
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
    runlog_helpers::error("bridge:start", message);
    summarize();
    std::process::exit(1);
}

/// Where the run log goes: WRZDJ_LOG_DIR, else the system's place for an app's logs.
fn log_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("WRZDJ_LOG_DIR") {
        return PathBuf::from(dir);
    }
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        if let Some(d) = var("LOCALAPPDATA") {
            return d.join("WrzDJ Bridge").join("logs");
        }
    } else if cfg!(target_os = "macos") {
        if let Some(h) = var("HOME") {
            return h.join("Library").join("Logs").join("WrzDJ Bridge");
        }
    } else if let Some(d) =
        var("XDG_STATE_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("state")))
    {
        return d.join("wrzdj-bridge");
    }
    std::env::temp_dir().join("wrzdj-bridge-logs")
}

/// "N issues this run — see <errors.log>", when anything went wrong.
fn summarize() {
    if let Some(summary) = runlog::global().and_then(|l| l.summarize_run()) {
        eprintln!("[Bridge] {summary}");
    }
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
    let filter = Arc::new(Mutex::new(TrackFilter::default()));
    let shared = Arc::clone(&filter);
    std::thread::spawn(move || {
        for ev in trx {
            let out = match ev {
                TraktorEvent::Connected => SourceEvent::Connected("Traktor"),
                TraktorEvent::Disconnected => SourceEvent::Disconnected,
                TraktorEvent::Metadata(raw) => match shared
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .accept(&raw)
                {
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
        name: "Traktor Broadcast",
        multi_deck: false,
        // The listener stays bound (Traktor reconnects to it by itself); a restart forgets the
        // last title, so the same song is a track again.
        restart: Box::new(move || {
            *filter.lock().unwrap_or_else(|p| p.into_inner()) = TrackFilter::default();
        }),
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
    let watch = move |stop: Arc<AtomicBool>| {
        let (dir, tx) = (dir.clone(), tx.clone());
        std::thread::spawn(move || {
            let (mut watcher, first) = SessionWatcher::start(dir);
            let mut events = first;
            loop {
                for ev in std::mem::take(&mut events) {
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
                    if stop.load(Ordering::SeqCst) || tx.send(out).is_err() {
                        return;
                    }
                }
                std::thread::sleep(Duration::from_millis(poll));
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                events = runlog_helpers::safely("serato:poll", Vec::new(), || watcher.poll());
            }
        });
    };
    let mut stop = Arc::new(AtomicBool::new(false));
    watch(Arc::clone(&stop));
    Source {
        id: "serato",
        name: "Serato DJ",
        multi_deck: true,
        restart: Box::new(move || {
            stop.store(true, Ordering::SeqCst);
            stop = Arc::new(AtomicBool::new(false));
            watch(Arc::clone(&stop));
        }),
    }
}

fn same_track(a: &Track, b: &Track) -> bool {
    a.title.trim().to_lowercase() == b.title.trim().to_lowercase()
        && a.artist.trim().to_lowercase() == b.artist.trim().to_lowercase()
}

fn main() {
    let dir = log_dir();
    let run = runlog::init(&dir);
    runlog::install_panic_hook();
    log(&format!(
        "Run log: {} (run {})",
        run.log_path().display(),
        run.run_id()
    ));
    for warning in network_warnings(runlog::global()) {
        log(&format!("Network: {warning}"));
    }
    let (url, code) = (
        env("WRZDJ_API_URL", "http://localhost:8000"),
        env("WRZDJ_EVENT_CODE", ""),
    );
    // The key: from the environment, else the OS keychain (kept there only when asked).
    let mut vault = KeychainVault::os(runlog::global());
    let key = match env("WRZDJ_BRIDGE_API_KEY", "") {
        k if !k.is_empty() => {
            if env("WRZDJ_REMEMBER_KEY", "") == "true" {
                vault.set(&k);
                log("Key saved in the OS keychain for later starts");
            }
            k
        }
        _ => vault.get().unwrap_or_default(),
    };
    if key.is_empty() {
        fail(
            "WRZDJ_BRIDGE_API_KEY is required (or saved earlier with WRZDJ_REMEMBER_KEY=true). Generate one with: openssl rand -hex 32",
        );
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
    let health_tx = tx.clone();
    let signal_tx = tx.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        let _ = signal_tx.send(SourceEvent::Shutdown);
    }) {
        log(&format!("Could not listen for Ctrl+C: {e}"));
    }
    let mut source = match env("WRZDJ_PLUGIN", "traktor-broadcast").as_str() {
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
    // bridge-runner.ts: every 30 s, a deleted or expired event stops the Bridge.
    let _health = match HealthWatcher::spawn(
        &url,
        &code,
        HEALTH_CHECK_INTERVAL,
        runlog::global(),
        move |reason| {
            let _ = health_tx.send(SourceEvent::Stop(reason.message()));
        },
    ) {
        Ok(watcher) => Some(watcher),
        Err(e) => {
            runlog_helpers::error(
                "health:start",
                format!("could not start the event check: {e}"),
            );
            None
        }
    };
    log("Bridge online — sending initial status to backend");
    let _ = jobs.send(Job::Status {
        connected: true,
        device: None,
        enriched: true,
        decks: 0,
    });

    let mut decks = DeckStateManager::new(cfg);
    let mut next_heartbeat: Option<Instant> = None;
    // plugin-bridge.ts auto-reconnect: only after a source that had connected drops.
    let (mut was_connected, mut reconnect_attempt, mut reconnect_at) =
        (false, 0u32, None::<Instant>);
    let exit_code;
    loop {
        let now_ms = started.elapsed().as_millis() as u64;
        let mut wait = Duration::from_secs(3600);
        if let Some(due) = decks.next_due() {
            wait = wait.min(Duration::from_millis(due.saturating_sub(now_ms)));
        }
        for t in [next_heartbeat, reconnect_at].into_iter().flatten() {
            wait = wait.min(t.saturating_duration_since(Instant::now()));
        }
        // Every wait has a floor, so a timer due now can't spin the loop.
        let event = rx.recv_timeout(wait.max(Duration::from_millis(5)));
        decks.advance_to(started.elapsed().as_millis() as u64);
        match event {
            Ok(SourceEvent::Connected(device)) => {
                log(&format!("Device connected: {device}"));
                was_connected = true;
                reconnect_attempt = 0;
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
                if was_connected && reconnect_at.is_none() {
                    reconnect_attempt += 1;
                    let delay = reconnect_delay(reconnect_attempt);
                    log(&format!(
                        "Device disconnected — reconnecting in {}s (attempt {reconnect_attempt})",
                        delay.as_secs()
                    ));
                    reconnect_at = Some(Instant::now() + delay);
                }
            }
            Ok(SourceEvent::Command(kind, payload)) => {
                log(&format!("Executing command: {kind}"));
                match kind.as_str() {
                    // resetDecks(): reset, then clear.
                    "reset_decks" => {
                        decks.reset();
                        let _ = jobs.send(Job::Clear);
                    }
                    // reconnect(): cancel any wait and reconnect now.
                    "reconnect" => {
                        reconnect_at = None;
                        reconnect_attempt = 0;
                        log(&format!("Reconnecting to {}...", source.name));
                        (source.restart)();
                        decks.reset();
                        log(&format!("Reconnected to {} successfully", source.name));
                    }
                    // restart(): stop (clears now playing), start again.
                    "restart" => {
                        reconnect_at = None;
                        reconnect_attempt = 0;
                        was_connected = false;
                        next_heartbeat = None;
                        decks.reset();
                        let _ = jobs.send(Job::Clear);
                        (source.restart)();
                    }
                    "setbuilder_transport" => {
                        let action = payload["action"].as_str().unwrap_or("unknown");
                        let title = payload["title"].as_str().unwrap_or("");
                        let position = payload["position_sec"].as_f64().unwrap_or(0.0);
                        let title = if title.is_empty() {
                            String::new()
                        } else {
                            format!(" \"{title}\"")
                        };
                        log(&format!(
                            "Setbuilder transport command received: {action}{title} @ {position:.1}s"
                        ));
                    }
                    other => runlog_helpers::warn(
                        "commands:unknown",
                        format!("Unknown command type: {other}"),
                    ),
                }
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
                    if decks.update_track_info(&deck, Some(track)).is_err()
                        || decks.update_play_state(&deck, true).is_err()
                    {
                        runlog_helpers::warn(
                            "deck:limit",
                            format!("16 decks in use; deck {deck} ignored"),
                        );
                    }
                }
            }
            Ok(SourceEvent::Shutdown) => {
                // index.ts shutdown(): one clear, waited for, then disconnected (not two clears).
                log("Received a stop signal, shutting down...");
                exit_code = 0;
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
                // bridge-runner.ts stop(reason): clear, report disconnected, stop. After a 401 the
                // worker has already done both and ended, so these go nowhere.
                eprintln!("[Bridge] Stopping: {reason}");
                runlog_helpers::error("bridge:stop", reason);
                let _ = jobs.send(Job::Clear);
                let _ = jobs.send(Job::Status {
                    connected: false,
                    device: None,
                    enriched: false,
                    decks: 0,
                });
                exit_code = 3;
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => fail("the source stopped"),
        }
        if reconnect_at.is_some_and(|t| Instant::now() >= t) {
            reconnect_at = None;
            log(&format!("Reconnecting to {}...", source.name));
            (source.restart)();
            decks.reset();
            log(&format!("Reconnected to {} successfully", source.name));
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
    if client_thread.join().is_err() {
        runlog_helpers::error("client:thread", "the server client thread panicked");
    }
    log("Bridge stopped.");
    summarize();
    std::process::exit(exit_code);
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
            log("Command poller started");
            let mut next_poll = Instant::now() + COMMAND_POLL;
            loop {
                let wait = next_poll.saturating_duration_since(Instant::now());
                let job = match rx.recv_timeout(wait.max(Duration::from_millis(5))) {
                    Ok(job) => Some(job),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                };
                let Some(job) = job else {
                    next_poll = Instant::now() + COMMAND_POLL;
                    let commands = runlog_helpers::safely("commands:poll", Vec::new(), || {
                        client.poll_commands()
                    });
                    for (kind, payload) in commands {
                        log(&format!("Received command: {kind}"));
                        let _ = stop.send(SourceEvent::Command(kind, payload));
                    }
                    if client.session_expired() {
                        expire(&mut client, plugin_id, &stop);
                        return;
                    }
                    continue;
                };
                runlog_helpers::safely("client:job", (), || match job {
                    Job::Live { deck, track: t } => {
                        if client.should_skip(&t.artist, &t.title) {
                            return; // skipped, as shouldSkipTrack does
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
                });
                // bridge-runner.ts: a 401 stops the Bridge; its stop() clears and reports
                // disconnected first.
                if client.session_expired() {
                    expire(&mut client, plugin_id, &stop);
                    return;
                }
            }
        })
        .unwrap_or_else(|e| fail(&format!("could not start the server client: {e}")));
    (tx, handle)
}

/// bridge-runner.ts: a 401 stops the Bridge; its stop() clears and reports disconnected first.
fn expire(client: &mut Client<HttpIo>, plugin_id: &'static str, stop: &Sender<SourceEvent>) {
    eprintln!("[Bridge] POST returned 401 — session expired");
    client.clear_now_playing();
    client.post_status(false, None, plugin_id);
    let _ = stop.send(SourceEvent::Stop("Session expired — please log in again"));
}

#[cfg(test)]
mod tests {
    use super::reconnect_delay;

    #[test]
    fn reconnect_waits_double_from_two_seconds_up_to_thirty() {
        let secs: Vec<u64> = (1..=7).map(|a| reconnect_delay(a).as_secs()).collect();
        assert_eq!(secs, [2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(reconnect_delay(500).as_secs(), 30);
    }
}
