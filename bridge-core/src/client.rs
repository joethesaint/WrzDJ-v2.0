//! Port of `bridge/src/bridge.ts` and `circuit-breaker.ts`: how the Bridge talks to the server.
//!
//! - `POST /api/bridge/nowplaying`, `POST /api/bridge/status`, `DELETE /api/bridge/nowplaying/{code}`,
//!   authenticated with `X-Bridge-API-Key`.
//! - A POST is tried 4 times (backoff 2, 4, 8 s); a DELETE 3 times (1, 2 s). A request that hasn't
//!   answered in 10 s counts as failed (the transport's job).
//! - Circuit breaker: 3 failed POSTs in a row open it for 60 s; then one probe decides.
//! - A now-playing post that fails is kept (the last 20) and replayed, marked `delayed`, when the
//!   breaker closes again. As in the TypeScript, a replay that fails stops and drops what it
//!   had taken from the buffer.
//! - Debounce: the same track, or any track within `min_play_s` of the last post, is skipped.
//!
//! The network and the clock are behind [`Io`], so the same code runs on real HTTP or on a
//! script (`client_trace`).

use std::collections::VecDeque;

use serde_json::{Value, json};

pub const MAX_RETRIES: u32 = 3;
pub const INITIAL_BACKOFF_MS: u64 = 2000;
pub const DELETE_MAX_RETRIES: u32 = 2;
pub const DELETE_BACKOFF_MS: u64 = 1000;
pub const BUFFER_CAPACITY: usize = 20;

/// The network and the clock.
pub trait Io {
    /// Milliseconds on a clock that only goes forward.
    fn now(&self) -> u64;
    fn sleep(&mut self, ms: u64);
    /// One request; `Ok` for a 2xx answer, `Err` (a short reason) for anything else.
    fn send(&mut self, method: &str, path: &str, body: Option<&Value>) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl BreakerState {
    pub fn name(self) -> &'static str {
        match self {
            BreakerState::Closed => "CLOSED",
            BreakerState::Open => "OPEN",
            BreakerState::HalfOpen => "HALF_OPEN",
        }
    }
}

pub struct CircuitBreaker {
    pub state: BreakerState,
    pub failures: u32,
    last_failure: u64,
    probe_in_flight: bool,
    threshold: u32,
    cooldown_ms: u64,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, cooldown_ms: u64) -> Self {
        CircuitBreaker {
            state: BreakerState::Closed,
            failures: 0,
            last_failure: 0,
            probe_in_flight: false,
            threshold,
            cooldown_ms,
        }
    }

    pub fn allow(&mut self, now: u64) -> bool {
        match self.state {
            BreakerState::Closed => true,
            BreakerState::Open => {
                if now - self.last_failure >= self.cooldown_ms {
                    self.state = BreakerState::HalfOpen;
                    self.probe_in_flight = true;
                    true
                } else {
                    false
                }
            }
            BreakerState::HalfOpen => {
                if self.probe_in_flight {
                    return false;
                }
                self.probe_in_flight = true;
                true
            }
        }
    }

    /// Returns true when this success closed a breaker that wasn't closed.
    pub fn success(&mut self) -> bool {
        self.failures = 0;
        self.probe_in_flight = false;
        let reopened = self.state != BreakerState::Closed;
        self.state = BreakerState::Closed;
        reopened
    }

    pub fn failure(&mut self, now: u64) {
        self.failures += 1;
        self.last_failure = now;
        self.probe_in_flight = false;
        if self.state == BreakerState::HalfOpen
            || (self.state == BreakerState::Closed && self.failures >= self.threshold)
        {
            self.state = BreakerState::Open;
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub event_code: String,
    pub min_play_s: u64,
}

pub struct Client<I: Io> {
    pub io: I,
    cfg: ClientConfig,
    pub breaker: CircuitBreaker,
    buffer: VecDeque<Value>,
    last_track_key: Option<String>,
    last_post: u64,
}

fn track_key(artist: &str, title: &str) -> String {
    format!(
        "{}::{}",
        artist.trim().to_lowercase(),
        title.trim().to_lowercase()
    )
}

impl<I: Io> Client<I> {
    pub fn new(io: I, cfg: ClientConfig) -> Self {
        Client {
            io,
            cfg,
            breaker: CircuitBreaker::new(3, 60_000),
            buffer: VecDeque::new(),
            last_track_key: None,
            last_post: 0,
        }
    }

    pub fn buffered(&self) -> usize {
        self.buffer.len()
    }

    /// `shouldSkipTrack`: an empty title, the same track again, or too soon after the last post.
    pub fn should_skip(&self, artist: &str, title: &str) -> bool {
        if title.is_empty() || self.last_track_key.as_deref() == Some(&track_key(artist, title)) {
            return true;
        }
        self.io.now() - self.last_post < self.cfg.min_play_s * 1000
    }

    pub fn update_last_track(&mut self, artist: &str, title: &str) {
        self.last_track_key = Some(track_key(artist, title));
        self.last_post = self.io.now();
    }

    fn post_with_retry(&mut self, path: &str, payload: &Value) -> bool {
        if !self.breaker.allow(self.io.now()) {
            return false;
        }
        for attempt in 0..=MAX_RETRIES {
            match self.io.send("POST", path, Some(payload)) {
                Ok(()) => {
                    if self.breaker.success() {
                        self.replay();
                    }
                    return true;
                }
                Err(_) if attempt < MAX_RETRIES => {
                    self.io.sleep(INITIAL_BACKOFF_MS << attempt);
                }
                Err(_) => {}
            }
        }
        self.breaker.failure(self.io.now());
        false
    }

    fn replay(&mut self) {
        let tracks: Vec<Value> = self.buffer.drain(..).collect();
        for mut payload in tracks {
            payload["delayed"] = json!(true);
            if !self.post_with_retry("/api/bridge/nowplaying", &payload) {
                return; // backend down again: the rest of this batch is dropped, as in bridge.ts
            }
        }
    }

    pub fn post_now_playing(
        &mut self,
        title: &str,
        artist: &str,
        deck: &str,
        source: &str,
    ) -> bool {
        let payload = json!({
            "event_code": self.cfg.event_code, "title": title, "artist": artist,
            "album": null, "deck": deck, "source": source,
        });
        let ok = self.post_with_retry("/api/bridge/nowplaying", &payload);
        if !ok {
            if self.buffer.len() >= BUFFER_CAPACITY {
                self.buffer.pop_front();
            }
            self.buffer.push_back(payload);
        }
        ok
    }

    pub fn post_status(&mut self, connected: bool, device: Option<&str>, plugin_id: &str) -> bool {
        let payload = json!({
            "event_code": self.cfg.event_code, "connected": connected, "device_name": device,
            "circuit_breaker_state": null, "buffer_size": null, "plugin_id": plugin_id,
            "deck_count": null, "uptime_seconds": null,
        });
        self.post_with_retry("/api/bridge/status", &payload)
    }

    /// `clearNowPlaying`: no breaker, 3 tries.
    pub fn clear_now_playing(&mut self) {
        let path = format!("/api/bridge/nowplaying/{}", self.cfg.event_code);
        for attempt in 0..=DELETE_MAX_RETRIES {
            match self.io.send("DELETE", &path, None) {
                Ok(()) => return,
                Err(_) if attempt < DELETE_MAX_RETRIES => {
                    self.io.sleep(DELETE_BACKOFF_MS << attempt)
                }
                Err(_) => {}
            }
        }
    }
}
