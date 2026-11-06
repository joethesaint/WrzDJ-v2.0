//! Runs client traces, the same JSON `bridge/scripts/client-trace.ts` reads, on a scripted
//! network and a simulated clock, and returns the same output lines.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::client::{Client, ClientConfig, Io, SendError};

#[derive(Deserialize)]
pub struct Op {
    pub t: u64,
    pub op: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub connected: Option<bool>,
    pub device: Option<String>,
}

#[derive(Deserialize)]
pub struct ClientTrace {
    pub net: Vec<Value>,
    pub ops: Vec<Op>,
}

/// A network that answers from a script (200 once it runs out) and a clock sleep moves.
pub struct ScriptedIo {
    pub now: u64,
    net: std::collections::VecDeque<Value>,
    pub requests: Vec<Value>,
}

impl Io for ScriptedIo {
    fn now(&self) -> u64 {
        self.now
    }
    fn sleep(&mut self, ms: u64) {
        self.now += ms.max(1);
    }
    fn send(&mut self, method: &str, path: &str, body: Option<&Value>) -> Result<(), SendError> {
        self.requests.push(json!({
            "t": self.now, "method": method, "path": path, "key": "test-key", "body": body,
        }));
        match self.net.pop_front().unwrap_or(json!(200)) {
            Value::Number(n) if (200..300).contains(&n.as_u64().unwrap_or(0)) => Ok(()),
            Value::Number(n) => Err(SendError::Status(n.as_u64().unwrap_or(0) as u16)),
            other => Err(SendError::Network(other.to_string())),
        }
    }
}

pub fn run(trace: &ClientTrace) -> Value {
    let io = ScriptedIo {
        now: 0,
        net: trace.net.iter().cloned().collect(),
        requests: Vec::new(),
    };
    let cfg = ClientConfig {
        event_code: "EVT1".into(),
        min_play_s: 5,
    };
    let mut c = Client::new(io, cfg);
    let mut results = Vec::new();
    for op in &trace.ops {
        c.io.now = c.io.now.max(op.t);
        let title = op.title.as_deref().unwrap_or("");
        let artist = op.artist.as_deref().unwrap_or("");
        results.push(match op.op.as_str() {
            "live" if c.should_skip(artist, title) => json!("skipped"),
            "live" => {
                c.update_last_track(artist, title);
                json!(c.post_now_playing(title, artist, None, "1", "traktor-broadcast"))
            }
            "status" => json!(c.post_status(
                op.connected.unwrap_or(true),
                op.device.as_deref(),
                "traktor-broadcast"
            )),
            "clear" => {
                c.clear_now_playing();
                Value::Null
            }
            _ => Value::Null,
        });
    }
    json!({
        "requests": c.io.requests,
        "results": results,
        "end": {
            "breaker": c.breaker.state.name(), "failures": c.breaker.failures,
            "buffered": c.buffered(), "now": c.io.now,
        },
    })
}
