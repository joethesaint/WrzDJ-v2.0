//! The real network for [`crate::client::Client`]: HTTP with a 10 s limit per request.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::client::{Io, SendError};

pub const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

pub struct HttpIo {
    agent: ureq::Agent,
    base_url: String,
    api_key: String,
    start: Instant,
    start_epoch_ms: u64,
}

impl HttpIo {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(FETCH_TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();
        let start_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        HttpIo {
            agent,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            start: Instant::now(),
            start_epoch_ms,
        }
    }
}

impl Io for HttpIo {
    /// Wall-clock ms like `Date.now()`, but it never goes backwards.
    fn now(&self) -> u64 {
        self.start_epoch_ms + self.start.elapsed().as_millis() as u64
    }

    fn sleep(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    fn send(&mut self, method: &str, path: &str, body: Option<&Value>) -> Result<(), SendError> {
        let url = format!("{}{}", self.base_url, path);
        let response = match (method, body) {
            ("POST", Some(body)) => self
                .agent
                .post(&url)
                .header("Content-Type", "application/json")
                .header("X-Bridge-API-Key", &self.api_key)
                .send(body.to_string()),
            ("DELETE", _) => self
                .agent
                .delete(&url)
                .header("X-Bridge-API-Key", &self.api_key)
                .call(),
            _ => return Err(SendError::Network(format!("unsupported request {method}"))),
        };
        match response {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(SendError::Status(r.status().as_u16())),
            Err(e) => Err(SendError::Network(e.to_string())),
        }
    }
}
