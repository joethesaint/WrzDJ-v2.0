/**
 * DEV-ONLY mock world. One consistent dataset that every mocked endpoint
 * reads from, so ids, event codes and counts agree across the dashboard,
 * event page, kiosk display and guest join page.
 *
 * Invariants (keep these true when editing):
 *  - every request.event_id points at an event in EVENTS
 *  - event.request_count === number of requests for that event
 *  - play history matched_request_id points at a request with status 'played'
 *    (or 'playing' for the current track) in the same event
 *  - activity log event_code values are real event codes
 *  - all timestamps are relative to NOW so the data never goes stale
 */
import type { ActivityLogEntry, ArchivedEvent, Event, SongRequest } from '../api-types';

const NOW = Date.now();
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const iso = (offsetMs: number) => new Date(NOW + offsetMs).toISOString();

export const MOCK_USER = {
  id: 1,
  username: 'dj_tobi',
  role: 'admin',
  help_pages_seen: [] as string[],
  pending_email: null,
  email: 'dj_tobi@wrzdj.test',
  frictionless_join_default: false,
};

// ---------------------------------------------------------------------------
// Track catalogue (shared by requests and play history)
// ---------------------------------------------------------------------------

export interface MockTrack {
  title: string;
  artist: string;
  album: string;
  bpm: number;
  key: string; // Camelot
  genre: string;
}

export const TRACKS: MockTrack[] = [
  { title: 'Calm Down', artist: 'Rema', album: 'Rave & Roses', bpm: 107, key: '11B', genre: 'Afrobeats' },
  { title: 'Essence', artist: 'Wizkid ft. Tems', album: 'Made in Lagos', bpm: 104, key: '1A', genre: 'Afrobeats' },
  { title: 'Last Last', artist: 'Burna Boy', album: 'Love, Damini', bpm: 85, key: '8A', genre: 'Afrobeats' },
  { title: 'Rush', artist: 'Ayra Starr', album: '19 & Dangerous', bpm: 108, key: '9B', genre: 'Afropop' },
  { title: 'Unavailable', artist: 'Davido ft. Musa Keys', album: 'Timeless', bpm: 113, key: '4A', genre: 'Amapiano' },
  { title: 'Water', artist: 'Tyla', album: 'Tyla', bpm: 117, key: '2A', genre: 'Amapiano' },
  { title: 'Sungba (Remix)', artist: 'Asake ft. Burna Boy', album: 'Mr. Money with the Vibe', bpm: 112, key: '6A', genre: 'Afro-house' },
  { title: 'Free Mind', artist: 'Tems', album: 'For Broken Ears', bpm: 96, key: '10A', genre: 'Afro R&B' },
  { title: 'Peru', artist: 'Fireboy DML', album: 'Playboy', bpm: 103, key: '5A', genre: 'Afrobeats' },
  { title: 'Lonely At The Top', artist: 'Asake', album: 'Work of Art', bpm: 115, key: '7A', genre: 'Amapiano' },
  { title: 'Ku Lo Sa', artist: 'Oxlade', album: 'Ku Lo Sa', bpm: 98, key: '3A', genre: 'Afropop' },
  { title: 'Soweto', artist: 'Victony & Tempoe', album: 'Outlaw', bpm: 100, key: '12A', genre: 'Afropop' },
  { title: 'Ye', artist: 'Burna Boy', album: 'Outside', bpm: 98, key: '9A', genre: 'Afrobeats' },
  { title: 'Fall', artist: 'Davido', album: 'A Good Time', bpm: 104, key: '6B', genre: 'Afropop' },
  { title: 'Ojuelegba', artist: 'Wizkid', album: 'Ayo', bpm: 101, key: '8B', genre: 'Afrobeats' },
  { title: 'Kwaku the Traveller', artist: 'Black Sherif', album: 'The Villain I Never Was', bpm: 92, key: '11A', genre: 'Afro-drill' },
  { title: 'Commas', artist: 'Ayra Starr', album: 'The Year I Turned 21', bpm: 106, key: '4B', genre: 'Afropop' },
  { title: 'Tshwala Bam', artist: 'TitoM & Yuppe', album: 'Tshwala Bam', bpm: 113, key: '1B', genre: 'Amapiano' },
  { title: 'Joha', artist: 'Asake', album: 'Mr. Money with the Vibe', bpm: 110, key: '5B', genre: 'Amapiano' },
  { title: 'Love Nwantiti', artist: 'CKay', album: 'CKay the First', bpm: 93, key: '10B', genre: 'Afropop' },
  { title: 'Bandana', artist: 'Fireboy DML & Asake', album: 'Playboy', bpm: 109, key: '3B', genre: 'Afrobeats' },
  { title: 'Overloading', artist: 'Mavins', album: 'Overloading', bpm: 111, key: '7B', genre: 'Afropop' },
  { title: 'Gimme Dat', artist: 'Ayra Starr ft. Wizkid', album: 'The Year I Turned 21', bpm: 104, key: '2B', genre: 'Afropop' },
  { title: 'Big Baller', artist: 'Flavour', album: 'Afroculture', bpm: 118, key: '12B', genre: 'Highlife' },
];

const art = (i: number) => `https://picsum.photos/seed/wrzdj-${i}/300/300`;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

type EventSeed = Omit<Event, 'request_count'>;

function makeEvent(e: Partial<EventSeed> & Pick<EventSeed, 'id' | 'code' | 'join_code' | 'name' | 'created_at' | 'expires_at'>): EventSeed {
  return {
    archived_at: null,
    banner_colors: null,
    banner_kiosk_url: null,
    banner_url: null,
    beatport_playlist_id: null,
    beatport_sync_enabled: false,
    collect_url: null,
    collection_opens_at: null,
    collection_phase_override: null,
    frictionless_join: false,
    is_active: true,
    join_url: null,
    live_starts_at: null,
    requests_open: true,
    status: 'active',
    submission_cap_per_guest: 3,
    tidal_playlist_id: null,
    tidal_sync_enabled: false,
    ...e,
  };
}

/** Live right now: full queue, now playing, history. */
const LIVE = makeEvent({
  id: 101,
  code: 'LAGOS1',
  join_code: 'VIBE24',
  name: 'Eko Rooftop Sessions',
  created_at: iso(-3 * HOUR),
  expires_at: iso(3 * HOUR),
  live_starts_at: iso(-2 * HOUR),
  tidal_sync_enabled: true,
  tidal_playlist_id: 'tidal-pl-lagos1',
  join_url: 'http://localhost:3000/join/VIBE24',
});

/** Tomorrow: pre-event collection is open, a few early submissions. */
const UPCOMING = makeEvent({
  id: 102,
  code: 'ABJ2NT',
  join_code: 'WEDS26',
  name: 'Adaeze & Kunle Wedding Reception',
  created_at: iso(-2 * DAY),
  expires_at: iso(1 * DAY + 8 * HOUR),
  collection_opens_at: iso(-1 * DAY),
  live_starts_at: iso(1 * DAY + 2 * HOUR),
  collect_url: 'http://localhost:3000/collect/WEDS26',
  join_url: 'http://localhost:3000/join/WEDS26',
  submission_cap_per_guest: 5,
  frictionless_join: true,
});

/** Created but empty — exercises empty states. */
const EMPTY = makeEvent({
  id: 103,
  code: 'IBDN03',
  join_code: 'CAMP03',
  name: 'UI Campus Throwback Night',
  created_at: iso(-20 * MIN),
  expires_at: iso(6 * HOUR),
  requests_open: false,
});

export const EVENTS: EventSeed[] = [LIVE, UPCOMING, EMPTY];

/** Past events for the archive panel. */
export const ARCHIVED_SEED: (Omit<ArchivedEvent, 'request_count'> & { request_count: number })[] = [
  {
    ...makeEvent({
      id: 90,
      code: 'DETTY1',
      join_code: 'DEC251',
      name: 'Detty December Block Party',
      created_at: iso(-12 * DAY),
      expires_at: iso(-11 * DAY),
      is_active: false,
      requests_open: false,
    }),
    status: 'expired',
    request_count: 64,
    archived_at: null,
  },
  {
    ...makeEvent({
      id: 85,
      code: 'OWAMBE',
      join_code: 'PARTY9',
      name: "Mama Bisi's 60th Owambe",
      created_at: iso(-30 * DAY),
      expires_at: iso(-29 * DAY),
      is_active: false,
      requests_open: false,
    }),
    status: 'archived',
    request_count: 41,
    archived_at: iso(-25 * DAY),
  },
];

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

const NICKS = ['Chioma', 'Tunde', 'Amaka', 'Seyi', 'Ife', 'Kelechi', 'Bukola', 'Emeka', 'Zainab', 'Femi', null];
const NOTES = ['For the birthday girl!', 'Please play this next 🙏', 'Throwback!!', null, null, null];

let nextRequestId = 1000;

function makeRequest(
  eventId: number,
  trackIdx: number,
  status: string,
  minutesAgo: number,
  votes: number,
  extra: Partial<SongRequest> = {},
): SongRequest {
  const t = TRACKS[trackIdx % TRACKS.length];
  const id = nextRequestId++;
  const created = iso(-minutesAgo * MIN);
  const accepted = status === 'new' || status === 'rejected' ? null : iso(-(minutesAgo - 2) * MIN);
  return {
    id,
    event_id: eventId,
    song_title: t.title,
    artist: t.artist,
    artwork_url: art(trackIdx),
    bpm: t.bpm,
    musical_key: t.key,
    genre: t.genre,
    status,
    vote_count: votes,
    nickname: NICKS[id % NICKS.length],
    note: NOTES[id % NOTES.length],
    source: id % 3 === 0 ? 'tidal' : 'spotify',
    source_url: `https://open.spotify.com/track/mock${trackIdx}`,
    raw_search_query: `${t.artist} ${t.title}`,
    is_duplicate: false,
    priority_score: Math.round((votes * 1.5 + Math.max(0, 60 - minutesAgo) / 10) * 10) / 10,
    sync_results_json: null,
    created_at: created,
    updated_at: accepted ?? created,
    accepted_at: accepted,
    ...extra,
  };
}

/**
 * LIVE event queue, ordered oldest → newest. The 'played' rows here line up
 * one-to-one with the matched entries in PLAY_HISTORY below.
 */
const LIVE_REQUESTS: SongRequest[] = [
  makeRequest(LIVE.id, 2, 'played', 115, 9),
  makeRequest(LIVE.id, 14, 'played', 104, 6),
  makeRequest(LIVE.id, 8, 'played', 92, 12),
  makeRequest(LIVE.id, 12, 'played', 80, 4),
  makeRequest(LIVE.id, 6, 'played', 66, 15),
  makeRequest(LIVE.id, 0, 'playing', 50, 21),
  makeRequest(LIVE.id, 4, 'accepted', 41, 17),
  makeRequest(LIVE.id, 9, 'accepted', 37, 11),
  makeRequest(LIVE.id, 3, 'accepted', 30, 8),
  makeRequest(LIVE.id, 17, 'accepted', 26, 5),
  makeRequest(LIVE.id, 5, 'new', 18, 7),
  makeRequest(LIVE.id, 20, 'new', 14, 3),
  makeRequest(LIVE.id, 1, 'new', 11, 9),
  makeRequest(LIVE.id, 15, 'new', 7, 1),
  makeRequest(LIVE.id, 21, 'new', 4, 0),
  makeRequest(LIVE.id, 23, 'new', 2, 0),
  makeRequest(LIVE.id, 19, 'rejected', 60, 0, { note: 'Too slow for peak time' }),
  makeRequest(LIVE.id, 0, 'new', 3, 0, { is_duplicate: true, nickname: 'Femi' }),
];

const UPCOMING_REQUESTS: SongRequest[] = [
  makeRequest(UPCOMING.id, 13, 'new', 20 * 60, 14),
  makeRequest(UPCOMING.id, 22, 'new', 16 * 60, 9),
  makeRequest(UPCOMING.id, 16, 'accepted', 12 * 60, 11),
  makeRequest(UPCOMING.id, 10, 'new', 6 * 60, 4),
  makeRequest(UPCOMING.id, 18, 'new', 90, 2),
];

export const REQUESTS: SongRequest[] = [...LIVE_REQUESTS, ...UPCOMING_REQUESTS];

// ---------------------------------------------------------------------------
// Now playing + play history (LIVE event only)
// ---------------------------------------------------------------------------

const playingReq = LIVE_REQUESTS.find((r) => r.status === 'playing')!;
const playedReqs = LIVE_REQUESTS.filter((r) => r.status === 'played');

export const NOW_PLAYING_STARTED = iso(-2 * MIN);

export interface MockHistoryRow {
  id: number;
  event_id: number;
  title: string;
  artist: string;
  album: string | null;
  album_art_url: string | null;
  matched_request_id: number | null;
  started_at: string;
  ended_at: string | null;
  source: string;
  spotify_uri: string | null;
}

/** DJ played their own picks in between requests (matched_request_id null). */
const historyTimeline: { req?: SongRequest; trackIdx?: number }[] = [
  { trackIdx: 11 },
  { req: playedReqs[0] },
  { req: playedReqs[1] },
  { trackIdx: 7 },
  { req: playedReqs[2] },
  { req: playedReqs[3] },
  { trackIdx: 16 },
  { req: playedReqs[4] },
  { req: playingReq },
];

export const PLAY_HISTORY: MockHistoryRow[] = historyTimeline.map((h, i, all) => {
  const t = h.req ? TRACKS.find((x) => x.title === h.req!.song_title)! : TRACKS[h.trackIdx!];
  const isLast = i === all.length - 1;
  const startMinAgo = isLast ? 2 : 2 + (all.length - 1 - i) * 4;
  return {
    id: 5000 + i,
    event_id: LIVE.id,
    title: t.title,
    artist: t.artist,
    album: t.album,
    album_art_url: art(TRACKS.indexOf(t)),
    matched_request_id: h.req?.id ?? null,
    started_at: iso(-startMinAgo * MIN),
    ended_at: isLast ? null : iso(-(startMinAgo - 4) * MIN),
    source: 'stagelinq',
    spotify_uri: null,
  };
});

// ---------------------------------------------------------------------------
// Activity log
// ---------------------------------------------------------------------------

export const ACTIVITY: ActivityLogEntry[] = [
  { id: 1, created_at: iso(-1 * MIN), level: 'info', source: 'bridge', event_code: LIVE.code, message: `Now playing: ${playingReq.artist} – ${playingReq.song_title} (matched request #${playingReq.id})` },
  { id: 2, created_at: iso(-4 * MIN), level: 'info', source: 'requests', event_code: LIVE.code, message: 'New request from guest: Flavour – Big Baller' },
  { id: 3, created_at: iso(-9 * MIN), level: 'warning', source: 'tidal', event_code: LIVE.code, message: 'Tidal sync: no match found for "Kwaku the Traveller"' },
  { id: 4, created_at: iso(-22 * MIN), level: 'info', source: 'tidal', event_code: LIVE.code, message: 'Synced 4 accepted requests to Tidal playlist' },
  { id: 5, created_at: iso(-35 * MIN), level: 'error', source: 'bridge', event_code: LIVE.code, message: 'Bridge disconnected (timeout) — reconnected after 12s' },
  { id: 6, created_at: iso(-90 * MIN), level: 'info', source: 'collect', event_code: UPCOMING.code, message: 'Pre-event submission received (5 total)' },
  { id: 7, created_at: iso(-2 * HOUR), level: 'info', source: 'events', event_code: LIVE.code, message: 'Event went live' },
  { id: 8, created_at: iso(-20 * MIN), level: 'info', source: 'events', event_code: EMPTY.code, message: 'Event created' },
];

// ---------------------------------------------------------------------------
// Per-event display settings
// ---------------------------------------------------------------------------

export const DISPLAY_SETTINGS: Record<string, { now_playing_hidden: boolean; now_playing_auto_hide_minutes: number; kiosk_display_only: boolean }> = {
  [LIVE.code]: { now_playing_hidden: false, now_playing_auto_hide_minutes: 10, kiosk_display_only: false },
  [UPCOMING.code]: { now_playing_hidden: true, now_playing_auto_hide_minutes: 10, kiosk_display_only: false },
  [EMPTY.code]: { now_playing_hidden: false, now_playing_auto_hide_minutes: 10, kiosk_display_only: true },
};

/** Vibe meter state for the LIVE event (Phase 1B). */
export const VIBE = { [LIVE.code]: { vibe_score: 72, vote_count: 48 } } as Record<string, { vibe_score: number; vote_count: number }>;

export const nextIds = { request: () => nextRequestId++, event: (() => { let n = 200; return () => n++; })() };
