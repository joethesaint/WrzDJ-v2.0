/**
 * DEV-ONLY in-memory mock backend. Maps (method, path) to handlers that read
 * and mutate the seed world in ./seed.ts, so accepting a request on the event
 * page is reflected in the dashboard counts, kiosk queue and guest list.
 *
 * Returns null for routes it doesn't know; the caller then falls through to
 * the real backend so unmocked screens still work when the server is up.
 */
import type { Event, KioskInfo, SongRequest } from '../api-types';
import {
  ACTIVITY,
  ARCHIVED_SEED,
  DISPLAY_SETTINGS,
  EVENTS,
  MOCK_USER,
  NOW_PLAYING_STARTED,
  PLAY_HISTORY,
  REQUESTS,
  VIBE,
  nextIds,
} from './seed';

export interface MockResponse {
  status: number;
  body?: unknown;
}

type Ctx = { method: string; params: string[]; query: URLSearchParams; body: any };
type Handler = (ctx: Ctx) => MockResponse;

const ok = (body: unknown): MockResponse => ({ status: 200, body });
const noContent = (): MockResponse => ({ status: 204 });
const notFound = (detail = 'Not found'): MockResponse => ({ status: 404, body: { detail } });

// ---------------------------------------------------------------------------
// Lookups / derived shapes
// ---------------------------------------------------------------------------

const findEvent = (code: string) =>
  EVENTS.find((e) => e.code === code || e.join_code === code);

const requestsFor = (eventId: number) => REQUESTS.filter((r) => r.event_id === eventId);

function eventOut(e: (typeof EVENTS)[number]): Event {
  return { ...e, request_count: requestsFor(e.id).length };
}

function phaseOf(e: (typeof EVENTS)[number]): 'pre_announce' | 'collection' | 'live' | 'closed' {
  if (!e.is_active) return 'closed';
  if (e.collection_phase_override === 'force_live') return 'live';
  if (e.collection_phase_override === 'force_collection') return 'collection';
  const now = Date.now();
  if (e.live_starts_at && new Date(e.live_starts_at).getTime() > now) {
    if (e.collection_opens_at && new Date(e.collection_opens_at).getTime() <= now) return 'collection';
    return 'pre_announce';
  }
  return 'live';
}

function displaySettings(code: string) {
  const e = findEvent(code)!;
  const d = (DISPLAY_SETTINGS[e.code] ??= {
    now_playing_hidden: false,
    now_playing_auto_hide_minutes: 10,
    kiosk_display_only: false,
  });
  return { status: 'ok', requests_open: e.requests_open, ...d };
}

function nowPlaying(eventId: number) {
  const row = PLAY_HISTORY.filter((h) => h.event_id === eventId && h.ended_at === null).at(-1);
  if (!row) return null;
  return {
    title: row.title,
    artist: row.artist,
    album: row.album,
    album_art_url: row.album_art_url,
    matched_request_id: row.matched_request_id,
    source: row.source,
    spotify_uri: row.spotify_uri,
    started_at: row.started_at ?? NOW_PLAYING_STARTED,
    bridge_connected: true,
  };
}

const publicReq = (r: SongRequest) => ({
  id: r.id,
  title: r.song_title,
  artist: r.artist,
  artwork_url: r.artwork_url,
  bpm: r.bpm,
  genre: r.genre,
  musical_key: r.musical_key,
  nickname: r.nickname,
  vote_count: r.vote_count,
  requester_verified: r.id % 4 === 0,
});

const SORTERS: Record<string, (a: SongRequest, b: SongRequest) => number> = {
  date_requested: (a, b) => a.created_at.localeCompare(b.created_at),
  date_accepted: (a, b) => (a.accepted_at ?? '').localeCompare(b.accepted_at ?? ''),
  upvotes: (a, b) => a.vote_count - b.vote_count,
  bpm: (a, b) => (a.bpm ?? 0) - (b.bpm ?? 0),
  key: (a, b) => (a.musical_key ?? '').localeCompare(b.musical_key ?? '', undefined, { numeric: true }),
  title: (a, b) => a.song_title.localeCompare(b.song_title),
  artist: (a, b) => a.artist.localeCompare(b.artist),
  best_match: (a, b) => (a.priority_score ?? 0) - (b.priority_score ?? 0),
};

function setStatus(r: SongRequest, status: string) {
  const now = new Date().toISOString();
  if (status === 'playing') {
    // Only one 'playing' per event: demote the previous one.
    REQUESTS.filter((x) => x.event_id === r.event_id && x.status === 'playing').forEach((x) => {
      x.status = 'played';
      x.updated_at = now;
    });
  }
  r.status = status;
  r.updated_at = now;
  if (status === 'accepted' && !r.accepted_at) r.accepted_at = now;
}

/** The browsing guest. Their requests are the ones whose nickname matches. */
const GUEST = { id: 7, nickname: 'Amaka', email_verified: false };

const guestRequests = (eventId: number) =>
  requestsFor(eventId).filter((r) => r.nickname === GUEST.nickname);

function guestProfile(e: (typeof EVENTS)[number]) {
  return {
    nickname: GUEST.nickname,
    email_verified: GUEST.email_verified,
    submission_cap: e.submission_cap_per_guest,
    submission_count: guestRequests(e.id).length,
  };
}

const KIOSKS: KioskInfo[] = [
  { id: 1, name: 'Bar screen', status: 'active', event_code: 'LAGOS1', event_join_code: 'VIBE24', event_name: 'Eko Rooftop Sessions', paired_at: new Date(Date.now() - 3 * 3600_000).toISOString(), last_seen_at: new Date().toISOString() },
  { id: 2, name: 'DJ booth tablet', status: 'active', event_code: 'LAGOS1', event_join_code: 'VIBE24', event_name: 'Eko Rooftop Sessions', paired_at: new Date(Date.now() - 2 * 3600_000).toISOString(), last_seen_at: new Date(Date.now() - 5 * 60_000).toISOString() },
];

// ---------------------------------------------------------------------------
// Routes — first match wins, so specific paths go before `{code}` catch-alls.
// ---------------------------------------------------------------------------

const routes: [string, RegExp, Handler][] = [
  // --- auth ---
  ['POST', /^\/api\/auth\/login$/, () => ok({ access_token: 'mock-jwt-token' })],
  ['GET', /^\/api\/auth\/me$/, () => ok(MOCK_USER)],
  ['POST', /^\/api\/auth\/help-seen$/, ({ body }) => {
    if (body?.page && !MOCK_USER.help_pages_seen.includes(body.page)) MOCK_USER.help_pages_seen.push(body.page);
    return noContent();
  }],

  // --- events (DJ) ---
  ['GET', /^\/api\/events$/, () => ok(EVENTS.filter((e) => e.is_active).map(eventOut))],
  ['POST', /^\/api\/events$/, ({ body }) => {
    const id = nextIds.event();
    const code = `EV${id}`;
    const now = Date.now();
    const e = {
      ...EVENTS[2],
      id,
      code,
      join_code: `J${id}`,
      name: body?.name ?? 'New event',
      created_at: new Date(now).toISOString(),
      expires_at: new Date(now + (body?.expires_hours ?? 6) * 3600_000).toISOString(),
      requests_open: true,
      join_url: `http://localhost:3000/join/J${id}`,
    };
    EVENTS.unshift(e);
    return ok(eventOut(e));
  }],
  ['GET', /^\/api\/events\/archived$/, () => ok(ARCHIVED_SEED)],
  ['GET', /^\/api\/events\/activity$/, ({ query }) => {
    const code = query.get('event_code');
    const limit = Number(query.get('limit') ?? 50);
    const rows = ACTIVITY.filter((a) => !code || a.event_code === code)
      .sort((a, b) => b.created_at.localeCompare(a.created_at));
    return ok(rows.slice(0, limit));
  }],
  ['POST', /^\/api\/events\/bulk-delete$/, ({ body }) => {
    const codes: string[] = body?.codes ?? [];
    let count = 0;
    for (const c of codes) {
      const i = EVENTS.findIndex((e) => e.code === c);
      if (i >= 0) { EVENTS.splice(i, 1); count++; }
    }
    return ok({ status: 'ok', count });
  }],
  ['POST', /^\/api\/events\/([^/]+)\/requests\/accept-all$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const rows = requestsFor(e.id).filter((r) => r.status === 'new');
    rows.forEach((r) => setStatus(r, 'accepted'));
    return ok({ status: 'ok', accepted_count: rows.length });
  }],
  ['POST', /^\/api\/events\/([^/]+)\/requests\/reject-all$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const rows = requestsFor(e.id).filter((r) => r.status === 'new');
    rows.forEach((r) => setStatus(r, 'rejected'));
    return ok({ status: 'ok', count: rows.length });
  }],
  ['DELETE', /^\/api\/events\/([^/]+)\/requests\/bulk$/, ({ params, query }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const status = query.get('status');
    let count = 0;
    for (let i = REQUESTS.length - 1; i >= 0; i--) {
      const r = REQUESTS[i];
      if (r.event_id === e.id && (!status || r.status === status)) { REQUESTS.splice(i, 1); count++; }
    }
    return ok({ status: 'ok', count });
  }],
  ['GET', /^\/api\/events\/([^/]+)\/requests$/, ({ params, query }) => {
    const e = findEvent(params[0]); if (!e) return notFound('Event not found');
    const all = requestsFor(e.id);
    const status_counts: Record<string, number> = {};
    for (const r of all) status_counts[r.status] = (status_counts[r.status] ?? 0) + 1;
    const status = query.get('status');
    const sort = query.get('sort') ?? 'date_requested';
    const direction = query.get('direction') ?? 'desc';
    const limit = Number(query.get('limit') ?? 100);
    const offset = Number(query.get('offset') ?? 0);
    const sorter = SORTERS[sort] ?? SORTERS.date_requested;
    const rows = all.filter((r) => !status || r.status === status)
      .sort((a, b) => (direction === 'asc' ? sorter(a, b) : sorter(b, a)));
    return ok({ requests: rows.slice(offset, offset + limit), total: rows.length, limit, offset, sort, direction, status_counts });
  }],
  ['POST', /^\/api\/events\/([^/]+)\/requests$/, ({ params, body }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const now = new Date().toISOString();
    const r: SongRequest = {
      id: nextIds.request(), event_id: e.id, song_title: body?.title ?? body?.song_title ?? 'Untitled', artist: body?.artist ?? 'Unknown',
      artwork_url: body?.artwork_url ?? null, bpm: body?.bpm ?? null, musical_key: body?.musical_key ?? null, genre: body?.genre ?? null,
      status: 'new', vote_count: 0, nickname: body?.nickname ?? null, note: body?.note ?? null, source: body?.source ?? 'manual',
      source_url: body?.source_url ?? null, raw_search_query: null, is_duplicate: false, priority_score: 0, sync_results_json: null,
      created_at: now, updated_at: now, accepted_at: null,
    };
    REQUESTS.push(r);
    return ok(r);
  }],
  ['GET', /^\/api\/events\/([^/]+)\/display-settings$/, ({ params }) =>
    findEvent(params[0]) ? ok(displaySettings(params[0])) : notFound()],
  ['PATCH', /^\/api\/events\/([^/]+)\/display-settings$/, ({ params, body }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    displaySettings(params[0]);
    const d = DISPLAY_SETTINGS[e.code];
    if (body?.requests_open !== undefined) e.requests_open = body.requests_open;
    if (body?.now_playing_hidden !== undefined) d.now_playing_hidden = body.now_playing_hidden;
    if (body?.now_playing_auto_hide_minutes !== undefined) d.now_playing_auto_hide_minutes = body.now_playing_auto_hide_minutes;
    if (body?.kiosk_display_only !== undefined) d.kiosk_display_only = body.kiosk_display_only;
    return ok(displaySettings(params[0]));
  }],
  ['GET', /^\/api\/events\/([^/]+)\/collection$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    return ok({
      collection_opens_at: e.collection_opens_at, live_starts_at: e.live_starts_at,
      submission_cap_per_guest: e.submission_cap_per_guest,
      collection_phase_override: e.collection_phase_override, phase: phaseOf(e),
      tidal_sync_enabled: e.tidal_sync_enabled, tidal_collection_playlist_id: null, tidal_collection_bidirectional: false,
    });
  }],
  ['PATCH', /^\/api\/events\/([^/]+)\/collection$/, (ctx) => {
    const e = findEvent(ctx.params[0]); if (!e) return notFound();
    const b = ctx.body ?? {};
    for (const k of ['collection_opens_at', 'live_starts_at', 'submission_cap_per_guest', 'collection_phase_override', 'tidal_sync_enabled'] as const) {
      if (k in b) (e as any)[k] = b[k];
    }
    return routeGet('/api/events/' + e.code + '/collection', ctx.query);
  }],
  ['GET', /^\/api\/events\/([^/]+)$/, ({ params }) => {
    const e = findEvent(params[0]);
    return e ? ok(eventOut(e)) : notFound('Event not found');
  }],
  ['PATCH', /^\/api\/events\/([^/]+)$/, ({ params, body }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    Object.assign(e, body ?? {});
    return ok(eventOut(e));
  }],
  ['DELETE', /^\/api\/events\/([^/]+)$/, ({ params }) => {
    const i = EVENTS.findIndex((e) => e.code === params[0]);
    if (i < 0) return notFound();
    EVENTS.splice(i, 1);
    return noContent();
  }],

  // --- requests (DJ) ---
  ['PATCH', /^\/api\/requests\/(\d+)$/, ({ params, body }) => {
    const r = REQUESTS.find((x) => x.id === Number(params[0])); if (!r) return notFound();
    if (body?.status) setStatus(r, body.status);
    return ok(r);
  }],
  ['DELETE', /^\/api\/requests\/(\d+)$/, ({ params }) => {
    const i = REQUESTS.findIndex((x) => x.id === Number(params[0]));
    if (i >= 0) REQUESTS.splice(i, 1);
    return noContent();
  }],
  ['POST', /^\/api\/requests\/(\d+)\/refresh-metadata$/, ({ params }) => {
    const r = REQUESTS.find((x) => x.id === Number(params[0]));
    return r ? ok(r) : notFound();
  }],
  ['POST', /^\/api\/requests\/(\d+)\/vote$/, ({ params }) => {
    const r = REQUESTS.find((x) => x.id === Number(params[0])); if (!r) return notFound();
    r.vote_count++;
    return ok({ status: 'ok', vote_count: r.vote_count, has_voted: true });
  }],

  // --- integrations ---
  ['GET', /^\/api\/tidal\/status$/, () => ok({ linked: true, integration_enabled: true, user_id: 'tidal-user-42', expires_at: new Date(Date.now() + 30 * 86400_000).toISOString() })],
  ['GET', /^\/api\/beatport\/status$/, () => ok({ linked: false, configured: true, integration_enabled: true, subscription: null, expires_at: null })],
  ['GET', /^\/api\/kiosk\/mine$/, () => ok(KIOSKS)],

  // --- public / guest / kiosk ---
  ['GET', /^\/api\/public\/e\/([^/]+)\/nowplaying$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound('Event not found');
    return ok(nowPlaying(e.id));
  }],
  ['GET', /^\/api\/public\/e\/([^/]+)\/bridge-status$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const live = e.code === EVENTS[0]?.code;
    return ok({
      connected: live, device_name: live ? 'Denon SC6000 Prime' : null, last_seen: live ? new Date().toISOString() : null,
      circuit_breaker_state: live ? 'closed' : null, buffer_size: live ? 0 : null, plugin_id: live ? 'stagelinq' : null,
      deck_count: live ? 2 : null, uptime_seconds: live ? 7260 : null,
    });
  }],
  ['GET', /^\/api\/public\/e\/([^/]+)\/history$/, ({ params, query }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const limit = Number(query.get('limit') ?? 100);
    const offset = Number(query.get('offset') ?? 0);
    const rows = PLAY_HISTORY.filter((h) => h.event_id === e.id);
    const items = rows.map((h, i) => ({ ...h, play_order: i + 1 })).reverse();
    return ok({ items: items.slice(offset, offset + limit), total: rows.length });
  }],
  ['GET', /^\/api\/public\/events\/([^/]+)\/display$/, ({ params, query }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const d = displaySettings(e.code);
    const accepted = requestsFor(e.id).filter((r) => r.status === 'accepted').sort((a, b) => b.vote_count - a.vote_count);
    const playing = requestsFor(e.id).find((r) => r.status === 'playing');
    const limit = Number(query.get('limit') ?? 50);
    const offset = Number(query.get('offset') ?? 0);
    return ok({
      event: { code: e.code, name: e.name },
      accepted_queue: accepted.slice(offset, offset + limit).map(publicReq),
      accepted_queue_total: accepted.length,
      now_playing: playing ? publicReq(playing) : null,
      now_playing_hidden: d.now_playing_hidden,
      kiosk_display_only: d.kiosk_display_only,
      requests_open: e.requests_open,
      qr_join_url: e.join_url ?? `http://localhost:3000/join/${e.join_code}`,
      banner_url: e.banner_url, banner_kiosk_url: e.banner_kiosk_url, banner_colors: e.banner_colors,
      updated_at: new Date().toISOString(),
    });
  }],
  ['GET', /^\/api\/public\/events\/([^/]+)\/requests$/, ({ params, query }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const rows = requestsFor(e.id).filter((r) => r.status === 'new' || r.status === 'accepted')
      .sort((a, b) => b.vote_count - a.vote_count);
    const limit = Number(query.get('limit') ?? 50);
    const offset = Number(query.get('offset') ?? 0);
    const np = nowPlaying(e.id);
    return ok({
      event: { code: e.code, name: e.name },
      now_playing: np ? { title: np.title, artist: np.artist, album_art_url: np.album_art_url, source: np.source } : null,
      requests: rows.slice(offset, offset + limit).map((r) => ({ ...publicReq(r), status: r.status as 'new' | 'accepted' })),
      total: rows.length,
    });
  }],
  // --- guest identity (one mock guest across all events) ---
  ['POST', /^\/api\/public\/guest\/identify$/, () => ok({ guest_id: GUEST.id, action: 'cookie_hit', reconcile_hint: false })],
  ['GET', /^\/api\/public\/collect\/([^/]+)\/join-config$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    return ok({ frictionless_join: e.frictionless_join });
  }],
  ['GET', /^\/api\/public\/collect\/([^/]+)\/profile$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    return ok(guestProfile(e));
  }],
  ['POST', /^\/api\/public\/collect\/([^/]+)\/profile$/, ({ params, body }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    if (body?.nickname) GUEST.nickname = body.nickname;
    return ok(guestProfile(e));
  }],
  ['POST', /^\/api\/public\/collect\/([^/]+)\/guest\/ensure-name$/, () => ok({ nickname: GUEST.nickname, auto_generated: false })],
  ['GET', /^\/api\/public\/events\/([^/]+)\/has-requested$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    return ok({ has_requested: guestRequests(e.id).length > 0 });
  }],
  ['GET', /^\/api\/public\/events\/([^/]+)\/my-requests$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    return ok({
      requests: guestRequests(e.id).map((r) => ({
        id: r.id, title: r.song_title, artist: r.artist, artwork_url: r.artwork_url,
        status: r.status, vote_count: r.vote_count, created_at: r.created_at,
      })),
    });
  }],
  ['POST', /^\/api\/public\/events\/([^/]+)\/vibe$/, ({ params, body }) => {
    const e = findEvent(params[0]); if (!e) return notFound();
    const v = (VIBE[e.code] ??= { vibe_score: 50, vote_count: 0 });
    const score = Number(body?.score ?? 50);
    v.vibe_score = Math.round((v.vibe_score * v.vote_count + score) / (v.vote_count + 1));
    v.vote_count++;
    const np = nowPlaying(e.id);
    return ok({ status: 'ok', ...v, track_title: np?.title ?? null, artist: np?.artist ?? null, created_at: new Date().toISOString() });
  }],
  ['GET', /^\/api\/public\/events\/([^/]+)$/, ({ params }) => {
    const e = findEvent(params[0]); if (!e) return notFound('Event not found');
    return ok({
      name: e.name, banner_url: e.banner_url, banner_colors: e.banner_colors, collection_code: e.join_code,
      frictionless_join: e.frictionless_join, phase: phaseOf(e), requests_open: e.requests_open,
      submission_cap_per_guest: e.submission_cap_per_guest,
    });
  }],
];

function routeGet(path: string, query: URLSearchParams): MockResponse {
  return handleMockRequest('GET', path, query, undefined) ?? notFound();
}

export function handleMockRequest(method: string, path: string, query: URLSearchParams, body: unknown): MockResponse | null {
  for (const [m, re, handler] of routes) {
    if (m !== method) continue;
    const match = re.exec(path);
    if (match) return handler({ method, params: match.slice(1).map(decodeURIComponent), query, body });
  }
  return null;
}
