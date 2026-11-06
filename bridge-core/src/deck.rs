//! Port of `bridge/src/deck-state-manager.ts`: which deck is live, and when.
//!
//! A deck goes EMPTY → LOADED (track) → CUEING (play) → PLAYING after `live_threshold` seconds of
//! play, and is then reported once as now playing. A pause in PLAYING keeps it for
//! `pause_grace` seconds before ENDED; a pause of the now-playing deck starts a
//! `now_playing_pause` timer, after which another PLAYING deck takes over.
//!
//! The TypeScript uses `setTimeout`. Here time is passed in: every call happens at `now()`, and
//! [`DeckStateManager::advance_to`] fires due timers the way Node does (earliest first, ties in
//! creation order, a delay under 1 ms runs as 1 ms). A timer the TypeScript leaves scheduled
//! after replacing its handle still fires here too; its callback guards itself, as there.

use std::cmp::Ordering;

pub const MAX_DECKS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum State {
    Empty,
    Loaded,
    Cueing,
    Playing,
    Ended,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    /// Sent when the source knows it (Serato); never part of "same track".
    pub album: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Deck {
    pub id: String,
    pub state: State,
    pub track: Option<Track>,
    pub is_playing: bool,
    pub play_start: Option<u64>,
    pub accumulated_ms: u64,
    pub last_pause: Option<u64>,
    pub fader: f64,
    pub is_master: bool,
    pub reported: bool,
}

impl Deck {
    fn empty(id: &str) -> Self {
        Deck {
            id: id.to_string(),
            state: State::Empty,
            track: None,
            is_playing: false,
            play_start: None,
            accumulated_ms: 0,
            last_pause: None,
            fader: 1.0,
            is_master: false,
            reported: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub live_threshold_s: u64,
    pub pause_grace_s: u64,
    pub now_playing_pause_s: u64,
    pub use_fader_detection: bool,
    pub master_deck_priority: bool,
}

/// What the manager tells the rest of the Bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    Live { at: u64, deck: String, track: Track },
    Cleared { at: u64 },
}

#[derive(Debug, PartialEq, Eq)]
pub struct DeckLimit;

#[derive(Clone, Debug)]
enum Kind {
    Threshold(String),
    Grace(String),
    Switch,
}

#[derive(Clone, Debug)]
struct Timer {
    id: u64,
    due: u64,
    kind: Kind,
}

pub struct DeckStateManager {
    cfg: Config,
    now: u64,
    decks: Vec<Deck>,               // insertion order, like the TypeScript Map
    deck_timer: Vec<(String, u64)>, // the handle each deck holds (timers Map)
    switch_timer: Option<u64>,
    scheduled: Vec<Timer>,
    next_id: u64,
    current: Option<String>,
    out: Vec<Output>,
}

/// Node's ordering for `localeCompare(b, undefined, { numeric: true })` on the deck ids players
/// send (digits, sometimes letters): digit runs compare by value, the rest case-insensitively.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(p), Some(q)) if p.is_ascii_digit() && q.is_ascii_digit() => {
                let run = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (m, n) = (run(&mut x), run(&mut y));
                let (m, n) = (m.trim_start_matches('0'), n.trim_start_matches('0'));
                let o = m.len().cmp(&n.len()).then_with(|| m.cmp(n));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(p), Some(q)) => {
                let o = p.to_ascii_lowercase().cmp(&q.to_ascii_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

impl DeckStateManager {
    pub fn new(cfg: Config) -> Self {
        DeckStateManager {
            cfg,
            now: 0,
            decks: Vec::new(),
            deck_timer: Vec::new(),
            switch_timer: None,
            scheduled: Vec::new(),
            next_id: 1,
            current: None,
            out: Vec::new(),
        }
    }

    pub fn now(&self) -> u64 {
        self.now
    }
    pub fn current_now_playing(&self) -> Option<&str> {
        self.current.as_deref()
    }
    pub fn decks(&self) -> &[Deck] {
        &self.decks
    }
    /// When the next timer fires, if any (for a caller that sleeps until then).
    pub fn next_due(&self) -> Option<u64> {
        self.scheduled.iter().map(|t| t.due).min()
    }
    /// The track on a deck, if it has one.
    pub fn track_on(&self, deck: &str) -> Option<&Track> {
        self.idx(deck).and_then(|i| self.decks[i].track.as_ref())
    }
    pub fn take_outputs(&mut self) -> Vec<Output> {
        std::mem::take(&mut self.out)
    }

    /// Fires every timer due at or before `t`, then sets the clock to `t`.
    pub fn advance_to(&mut self, t: u64) {
        loop {
            let next = self
                .scheduled
                .iter()
                .filter(|x| x.due <= t)
                .min_by_key(|x| (x.due, x.id))
                .cloned();
            let Some(timer) = next else { break };
            self.scheduled.retain(|x| x.id != timer.id);
            self.now = timer.due;
            match timer.kind {
                Kind::Threshold(d) => self.on_threshold(&d),
                Kind::Grace(d) => self.on_grace_expired(&d),
                Kind::Switch => {
                    self.switch_timer = None;
                    self.scan_for_candidate();
                }
            }
        }
        self.now = self.now.max(t);
    }

    fn schedule(&mut self, delay_ms: u64, kind: Kind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let due = self.now + delay_ms.max(1);
        self.scheduled.push(Timer { id, due, kind });
        id
    }

    fn cancel(&mut self, id: u64) {
        self.scheduled.retain(|x| x.id != id);
    }

    fn set_deck_timer(&mut self, deck: &str, id: u64) {
        self.deck_timer.retain(|(d, _)| d != deck);
        self.deck_timer.push((deck.to_string(), id));
    }

    fn clear_deck_timer(&mut self, deck: &str) {
        if let Some(i) = self.deck_timer.iter().position(|(d, _)| d == deck) {
            let (_, id) = self.deck_timer.remove(i);
            self.cancel(id);
        }
    }

    fn clear_switch_timer(&mut self) {
        if let Some(id) = self.switch_timer.take() {
            self.cancel(id);
        }
    }

    fn start_switch_timer(&mut self) {
        self.clear_switch_timer();
        let id = self.schedule(self.cfg.now_playing_pause_s * 1000, Kind::Switch);
        self.switch_timer = Some(id);
    }

    fn idx(&self, deck: &str) -> Option<usize> {
        self.decks.iter().position(|d| d.id == deck)
    }

    /// `getDeckState`: the deck, created EMPTY on first sight (evicting an idle one at 16).
    fn get(&mut self, deck: &str) -> Result<usize, DeckLimit> {
        if let Some(i) = self.idx(deck) {
            return Ok(i);
        }
        if self.decks.len() >= MAX_DECKS && !self.evict_stale() {
            return Err(DeckLimit);
        }
        self.decks.push(Deck::empty(deck));
        Ok(self.decks.len() - 1)
    }

    fn evict_stale(&mut self) -> bool {
        for want in [State::Empty, State::Ended, State::Loaded, State::Cueing] {
            if let Some(i) = self.decks.iter().position(|d| d.state == want) {
                let id = self.decks[i].id.clone();
                self.clear_deck_timer(&id);
                if self.current.as_deref() == Some(&id) {
                    self.current = None;
                }
                self.decks.remove(i);
                return true;
            }
        }
        false
    }

    pub fn update_track_info(&mut self, deck: &str, track: Option<Track>) -> Result<(), DeckLimit> {
        let i = self.get(deck)?;
        self.clear_deck_timer(deck);
        let was_now_playing = self.current.as_deref() == Some(deck);
        let (fader, is_master) = (self.decks[i].fader, self.decks[i].is_master);
        let mut fresh = Deck::empty(deck);
        fresh.fader = fader;
        fresh.is_master = is_master;
        if track.is_some() {
            fresh.state = State::Loaded;
            fresh.track = track;
        }
        self.decks[i] = fresh;
        if was_now_playing {
            self.clear_switch_timer();
            self.scan_for_candidate();
        }
        Ok(())
    }

    pub fn update_play_state(&mut self, deck: &str, playing: bool) -> Result<(), DeckLimit> {
        let i = self.get(deck)?;
        let d = &self.decks[i];
        if d.state == State::Empty || d.is_playing == playing {
            return Ok(());
        }
        if playing {
            self.play_start(i);
        } else {
            self.play_stop(i);
        }
        Ok(())
    }

    fn play_start(&mut self, i: usize) {
        let now = self.now;
        let grace = self.cfg.pause_grace_s * 1000;
        let d = &mut self.decks[i];
        let mut accumulated = 0;
        if let Some(paused) = d.last_pause
            && now - paused <= grace
        {
            accumulated = d.accumulated_ms;
        }
        let next = if matches!(d.state, State::Playing | State::Ended) {
            State::Playing
        } else {
            State::Cueing
        };
        d.state = next;
        d.is_playing = true;
        d.play_start = Some(now);
        d.accumulated_ms = accumulated;
        d.last_pause = None;
        let id = d.id.clone();
        if self.current.as_deref() == Some(&id) {
            self.clear_switch_timer();
        }
        if next == State::Cueing {
            let remaining = (self.cfg.live_threshold_s * 1000).saturating_sub(accumulated);
            let t = self.schedule(remaining, Kind::Threshold(id.clone()));
            self.set_deck_timer(&id, t);
        }
    }

    fn play_stop(&mut self, i: usize) {
        let now = self.now;
        let id = self.decks[i].id.clone();
        let mut total = self.decks[i].accumulated_ms;
        if let Some(start) = self.decks[i].play_start {
            total += now - start;
        }
        self.clear_deck_timer(&id);
        let state = if self.decks[i].state == State::Playing {
            let t = self.schedule(self.cfg.pause_grace_s * 1000, Kind::Grace(id.clone()));
            self.set_deck_timer(&id, t);
            if self.current.as_deref() == Some(&id) {
                self.start_switch_timer();
            }
            State::Playing
        } else {
            State::Loaded
        };
        let d = &mut self.decks[i];
        d.state = state;
        d.is_playing = false;
        d.play_start = None;
        d.accumulated_ms = total;
        d.last_pause = Some(now);
    }

    fn on_threshold(&mut self, deck: &str) {
        let Ok(i) = self.get(deck) else { return };
        if self.decks[i].state != State::Cueing || !self.decks[i].is_playing {
            return;
        }
        self.decks[i].state = State::Playing;
        self.check_and_report(deck);
    }

    fn on_grace_expired(&mut self, deck: &str) {
        let Ok(i) = self.get(deck) else { return };
        if self.decks[i].state != State::Playing || self.decks[i].is_playing {
            return;
        }
        self.decks[i].state = State::Ended;
        if self.current.as_deref() == Some(deck) {
            self.clear_switch_timer();
            self.scan_for_candidate();
        }
    }

    pub fn update_fader_level(&mut self, deck: &str, level: f64) -> Result<(), DeckLimit> {
        let i = self.get(deck)?;
        let level = level.clamp(0.0, 1.0);
        let was_low = self.decks[i].fader == 0.0;
        self.decks[i].fader = level;
        if was_low && level > 0.0 && self.decks[i].state == State::Playing {
            self.check_and_report(deck);
        }
        if self.current.as_deref() == Some(deck) && level == 0.0 && !was_low {
            self.start_switch_timer();
        }
        Ok(())
    }

    pub fn set_master_deck(&mut self, deck: &str) -> Result<(), DeckLimit> {
        let previous = self
            .decks
            .iter()
            .find(|d| d.is_master)
            .map(|d| d.id.clone());
        for d in &mut self.decks {
            d.is_master = false;
        }
        let i = self.get(deck)?;
        self.decks[i].is_master = true;
        if self.decks[i].state == State::Playing && previous.as_deref() != Some(deck) {
            self.check_and_report(deck);
        }
        Ok(())
    }

    fn has_master(&self) -> bool {
        self.decks.iter().any(|d| d.is_master)
    }

    fn blocked(&self, d: &Deck) -> bool {
        (self.cfg.use_fader_detection && d.fader == 0.0)
            || (self.cfg.master_deck_priority && self.has_master() && !d.is_master)
    }

    fn check_and_report(&mut self, deck: &str) {
        let Some(i) = self.idx(deck) else { return };
        let d = &self.decks[i];
        if d.state != State::Playing || d.reported || self.blocked(d) {
            return;
        }
        if let Some(cur) = self.current.clone().filter(|c| c != deck)
            && let Some(c) = self.idx(&cur).map(|j| &self.decks[j])
            && (c.is_playing || c.state == State::Playing)
        {
            return;
        }
        if let Some(track) = self.decks[i].track.clone() {
            self.decks[i].reported = true;
            self.clear_switch_timer();
            self.current = Some(deck.to_string());
            self.out.push(Output::Live {
                at: self.now,
                deck: deck.to_string(),
                track,
            });
        }
    }

    fn scan_for_candidate(&mut self) {
        let previous = self.current.take();
        let mut ids: Vec<String> = self.decks.iter().map(|d| d.id.clone()).collect();
        ids.sort_by(|a, b| natural_cmp(a, b));
        for id in ids {
            if previous.as_deref() == Some(&id) {
                continue;
            }
            let Some(i) = self.idx(&id) else { continue };
            let d = &self.decks[i];
            if d.state == State::Playing && !d.reported {
                if self.blocked(d) {
                    continue;
                }
                self.decks[i].reported = true;
                self.current = Some(id.clone());
                self.clear_switch_timer();
                if let Some(track) = self.decks[i].track.clone() {
                    self.out.push(Output::Live {
                        at: self.now,
                        deck: id,
                        track,
                    });
                }
                return;
            }
        }
        self.out.push(Output::Cleared { at: self.now });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            live_threshold_s: 15,
            pause_grace_s: 3,
            now_playing_pause_s: 10,
            use_fader_detection: false,
            master_deck_priority: false,
        }
    }

    fn track(title: &str) -> Option<Track> {
        Some(Track {
            title: title.into(),
            artist: "X".into(),
            album: None,
        })
    }

    #[test]
    fn a_deck_goes_live_after_the_threshold_of_play() {
        let mut m = DeckStateManager::new(cfg());
        m.update_track_info("1", track("A")).unwrap();
        m.update_play_state("1", true).unwrap();
        m.advance_to(14_999);
        assert!(m.take_outputs().is_empty());
        m.advance_to(15_000);
        assert_eq!(
            m.take_outputs(),
            vec![Output::Live {
                at: 15_000,
                deck: "1".into(),
                track: track("A").unwrap()
            }]
        );
    }

    #[test]
    fn a_short_pause_while_cueing_keeps_the_play_time() {
        let mut m = DeckStateManager::new(cfg());
        m.update_track_info("1", track("A")).unwrap();
        m.update_play_state("1", true).unwrap();
        m.advance_to(10_000);
        m.update_play_state("1", false).unwrap();
        m.advance_to(12_000);
        m.update_play_state("1", true).unwrap();
        m.advance_to(16_999);
        assert!(m.take_outputs().is_empty());
        m.advance_to(17_000);
        assert_eq!(m.take_outputs().len(), 1);
    }

    #[test]
    fn the_next_playing_deck_takes_over_when_the_live_one_stops() {
        let mut m = DeckStateManager::new(cfg());
        for d in ["1", "2"] {
            m.update_track_info(d, track(d)).unwrap();
            m.update_play_state(d, true).unwrap();
        }
        m.advance_to(15_000);
        assert_eq!(m.take_outputs().len(), 1); // deck 1; deck 2 waits while 1 plays
        m.update_play_state("1", false).unwrap();
        m.advance_to(18_000); // grace over: deck 1 ENDED, deck 2 reported
        let out = m.take_outputs();
        assert!(
            matches!(&out[..], [Output::Live { deck, .. }] if deck == "2"),
            "{out:?}"
        );
        assert_eq!(m.current_now_playing(), Some("2"));
    }

    #[test]
    fn deck_ids_sort_by_number() {
        let mut ids = vec!["10", "2", "1", "B", "a"];
        ids.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(ids, ["1", "2", "10", "a", "B"]);
    }
}
