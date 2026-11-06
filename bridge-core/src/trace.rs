//! Runs deck-event traces, the same JSON `bridge/scripts/deck-trace.ts` reads, and returns the
//! same output lines, so the Rust and TypeScript state machines can be compared.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::deck::{Config, DeckStateManager, Output, State, Track};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceConfig {
    pub live_threshold_seconds: u64,
    pub pause_grace_seconds: u64,
    pub now_playing_pause_seconds: u64,
    pub use_fader_detection: bool,
    pub master_deck_priority: bool,
}

#[derive(Deserialize)]
pub struct Event {
    pub t: u64,
    pub op: String,
    pub deck: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub playing: Option<bool>,
    pub level: Option<f64>,
}

#[derive(Deserialize)]
pub struct Trace {
    pub config: TraceConfig,
    pub events: Vec<Event>,
}

fn state_name(s: State) -> &'static str {
    match s {
        State::Empty => "EMPTY",
        State::Loaded => "LOADED",
        State::Cueing => "CUEING",
        State::Playing => "PLAYING",
        State::Ended => "ENDED",
    }
}

/// One output line: `{ "out": [...], "end": { "now": deck|null, "decks": [...] } }`.
pub fn run(trace: &Trace) -> Value {
    let c = &trace.config;
    let mut m = DeckStateManager::new(Config {
        live_threshold_s: c.live_threshold_seconds,
        pause_grace_s: c.pause_grace_seconds,
        now_playing_pause_s: c.now_playing_pause_seconds,
        use_fader_detection: c.use_fader_detection,
        master_deck_priority: c.master_deck_priority,
    });
    let mut out = Vec::new();
    for e in &trace.events {
        m.advance_to(e.t);
        out.extend(m.take_outputs());
        let deck = e.deck.as_deref().unwrap_or("1");
        let _ = match e.op.as_str() {
            "track" => m.update_track_info(
                deck,
                e.title.clone().map(|title| Track {
                    title,
                    artist: e.artist.clone().unwrap_or_default(),
                    album: None,
                }),
            ),
            "play" => m.update_play_state(deck, e.playing.unwrap_or(false)),
            "fader" => m.update_fader_level(deck, e.level.unwrap_or(0.0)),
            "master" => m.set_master_deck(deck),
            _ => Ok(()),
        };
        out.extend(m.take_outputs());
    }
    let out: Vec<Value> = out
        .into_iter()
        .map(|o| match o {
            Output::Live { at, deck, track } => {
                json!({"t": at, "live": deck, "title": track.title, "artist": track.artist})
            }
            Output::Cleared { at } => json!({"t": at, "cleared": true}),
        })
        .collect();
    let decks: Vec<Value> = m
        .decks()
        .iter()
        .map(|d| {
            json!([
                d.id,
                state_name(d.state),
                d.is_playing,
                d.accumulated_ms,
                d.reported,
                d.is_master
            ])
        })
        .collect();
    json!({"out": out, "end": {"now": m.current_now_playing(), "decks": decks}})
}
