//! The WrzDJ Bridge core in Rust, ported from `bridge/` one piece at a time.
//!
//! Piece 1: [`deck::DeckStateManager`], the state machine that decides which deck is live.
//! Piece 2: [`client::Client`], how the Bridge talks to the server (retries, breaker, replay).
//! It is a line-for-line port of `bridge/src/deck-state-manager.ts` with the clock passed in, and
//! `tests/differential.rs` checks it against the TypeScript on generated traces.

pub mod client;
pub mod client_trace;
pub mod deck;
pub mod http;
pub mod serato;
pub mod trace;
pub mod traktor;
