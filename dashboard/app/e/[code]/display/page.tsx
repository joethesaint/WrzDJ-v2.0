'use client';

import { useEffect, useState, useCallback, useRef } from 'react';
import { useParams, useRouter } from 'next/navigation';
import { QRCodeSVG } from 'qrcode.react';
import { api, ApiError, KioskDisplay, NowPlayingInfo, PlayHistoryItem, PUBLIC_PAGE_MAX } from '@/lib/api';
import { useEventStream } from '@/lib/use-event-stream';
import { usePollingLoop } from '@/lib/usePollingLoop';
import { RequestModal } from './components/RequestModal';
import { ExpiredStage, ExpiredStamp } from './components/ExpiredStage';

const AUTO_SCROLL_INTERVAL = 5000; // 5 seconds between auto-scrolls
const PUBLIC_PAGE_MAX_INITIAL = 100;
const SESSION_CHECK_INTERVAL = 10_000; // 10 seconds between kiosk session checks
const SESSION_TOKEN_KEY = 'kiosk_session_token';
const PAIR_CODE_KEY = 'kiosk_pair_code';
const HEX_COLOR_RE = /^#[0-9a-f]{6}$/i;

function safeColor(c: string | undefined, fallback: string): string {
  return c && HEX_COLOR_RE.test(c) ? c : fallback;
}

function withAlpha(hex: string, a: number): string {
  if (!hex || hex[0] !== '#') return hex;
  const h = hex.slice(1);
  const r = parseInt(h.substring(0, 2), 16);
  const g = parseInt(h.substring(2, 4), 16);
  const b = parseInt(h.substring(4, 6), 16);
  return `rgba(${r},${g},${b},${a})`;
}

// ── Electric Violet with Liquid Metal (Director clarification) ─────────────
const KIOSK_THEME = {
  name: 'Electric Violet with Liquid Metal',
  accent: '#a855f7',
  accentBright: '#e9d5ff',
  bgGradient: 'radial-gradient(ellipse at 50% 12%, rgba(88, 28, 135, 0.28) 0%, transparent 48%), radial-gradient(ellipse at 50% 15%, #18131f 0%, #0d0e11 70%, #050608 100%)',
  metal: 'linear-gradient(110deg, #6d28d9 0%, #a855f7 28%, #ede4ff 45%, #a855f7 62%, #7c3aed 100%)',
  panelBg: 'rgba(20, 22, 26, 0.78)',
  glow: 'rgba(168, 85, 247, 0.36)',
};

// Clean 2D Vector SVGs (Strictly zero emojis)
function Vinyl2DPlaceholder({ size = 180 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      style={{ flexShrink: 0 }}
      aria-label="2D Vinyl Disc"
    >
      <circle cx="50" cy="50" r="48" fill="#121316" stroke="rgba(255,255,255,0.18)" strokeWidth="1.5" />
      <circle cx="50" cy="50" r="42" stroke="rgba(255,255,255,0.06)" strokeWidth="0.8" />
      <circle cx="50" cy="50" r="36" stroke="rgba(255,255,255,0.06)" strokeWidth="0.8" />
      <circle cx="50" cy="50" r="30" stroke="rgba(255,255,255,0.06)" strokeWidth="0.8" />
      <circle cx="50" cy="50" r="24" stroke="rgba(255,255,255,0.06)" strokeWidth="0.8" />
      <circle cx="50" cy="50" r="18" fill="#1f2024" stroke="#ffffff" strokeWidth="1.5" />
      <circle cx="50" cy="50" r="15" fill="none" stroke="rgba(255,255,255,0.2)" strokeWidth="0.6" />
      <circle cx="50" cy="50" r="4.5" fill="#090a0d" stroke="rgba(255,255,255,0.3)" strokeWidth="1" />
      {/* 2D Tonearm */}
      <line x1="88" y1="12" x2="68" y2="44" stroke="rgba(255,255,255,0.4)" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="88" cy="12" r="3" fill="rgba(255,255,255,0.6)" />
      <rect x="65" y="42" width="6" height="4" rx="1" fill="#ffffff" transform="rotate(-30 68 44)" />
    </svg>
  );
}

function Signal2DIcon({ size = 16, color = '#fff' }: { size?: number; color?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4.93 19.07a10 10 0 0 1 0-14.14" />
      <path d="M7.76 16.24a6 6 0 0 1 0-8.48" />
      <circle cx="12" cy="12" r="2" fill={color} />
      <path d="M16.24 7.76a6 6 0 0 1 0 8.48" />
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
    </svg>
  );
}

function Upvote2DIcon({ size = 14, color = '#fff' }: { size?: number; color?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="18 15 12 9 6 15" />
    </svg>
  );
}

export default function KioskDisplayPage() {
  const params = useParams();
  const router = useRouter();
  const code = params.code as string;

  const [display, setDisplay] = useState<KioskDisplay | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<{ message: string; status: number } | null>(null);

  // StageLinQ data
  const [stagelinqNowPlaying, setStagelinqNowPlaying] = useState<NowPlayingInfo | null>(null);
  const [playHistory, setPlayHistory] = useState<PlayHistoryItem[]>([]);

  // Vibe Meter
  const [vibeScore, setVibeScore] = useState<number | null>(null);

  // Sticky now-playing: keep showing last track for 10s after it goes null
  const [lastKnownNowPlaying, setLastKnownNowPlaying] = useState<NowPlayingInfo | null>(null);
  const [nowPlayingFading, setNowPlayingFading] = useState(false);
  const staleTimerRef = useRef<NodeJS.Timeout | null>(null);

  // Track previous accepted queue IDs for animation
  const prevAcceptedIdsRef = useRef<Set<number>>(new Set());
  const [newItemIds, setNewItemIds] = useState<Set<number>>(new Set());

  // Auto-scroll ref for display-only mode
  const queueListRef = useRef<HTMLDivElement>(null);

  // Request modal state
  const [showRequestModal, setShowRequestModal] = useState(false);

  // Track whether initial load succeeded (ref avoids stale closure in useCallback)
  const hasLoadedRef = useRef(false);

  // Automatic growing window for the accepted queue
  const [displayLimit, setDisplayLimit] = useState(PUBLIC_PAGE_MAX_INITIAL);
  const displayLimitRef = useRef(displayLimit);
  displayLimitRef.current = displayLimit;

  const loadSeqRef = useRef(0);

  // Load kiosk display data and StageLinQ data
  const loadDisplay = useCallback(async (): Promise<boolean> => {
    const seq = ++loadSeqRef.current;
    try {
      const limit = displayLimitRef.current;
      const [kioskData, nowPlayingData, historyData] = await Promise.all([
        api.getKioskDisplay(code, { limit, offset: 0 }),
        api.getNowPlaying(code).catch((): undefined => undefined),
        api.getPlayHistory(code).catch((): undefined => undefined),
      ]);
      if (seq !== loadSeqRef.current) return true;
      setDisplay(kioskData);
      if (
        kioskData.accepted_queue_total > kioskData.accepted_queue.length &&
        displayLimitRef.current < PUBLIC_PAGE_MAX
      ) {
        setDisplayLimit(Math.min(limit + PUBLIC_PAGE_MAX_INITIAL, PUBLIC_PAGE_MAX));
      }
      if (nowPlayingData !== undefined) {
        setStagelinqNowPlaying(nowPlayingData);
      }
      if (historyData !== undefined) {
        setPlayHistory(historyData.items);
      }
      setError(null);
      hasLoadedRef.current = true;
      return true;
    } catch (err) {
      if (err instanceof ApiError) {
        if (err.status === 404 || err.status === 410) {
          setError({ message: err.message, status: err.status });
          return false;
        }
      }
      if (!hasLoadedRef.current) {
        setError({ message: 'Event not found or expired', status: 0 });
      }
      return true;
    } finally {
      setLoading(false);
    }
  }, [code]);

  usePollingLoop(true, loadDisplay, 10_000);

  const loadDisplayRef = useRef(loadDisplay);
  loadDisplayRef.current = loadDisplay;
  useEventStream(code, {
    onRequestCreated: () => { loadDisplayRef.current(); },
    onRequestStatusChanged: () => { loadDisplayRef.current(); },
    onNowPlayingChanged: () => { 
      loadDisplayRef.current(); 
      setVibeScore(null);
    },
    onRequestsBulkUpdate: () => { loadDisplayRef.current(); },
    onBridgeStatusChanged: () => { loadDisplayRef.current(); },
    onVibeUpdated: (data) => { setVibeScore(data.vibe_score); },
  });

  useEffect(() => {
    const token = typeof window !== 'undefined'
      ? localStorage.getItem(SESSION_TOKEN_KEY)
      : null;
    api.setKioskSession(token);
    return () => api.setKioskSession(null);
  }, []);

  useEffect(() => {
    const token = typeof window !== 'undefined'
      ? localStorage.getItem(SESSION_TOKEN_KEY)
      : null;
    if (!token) return;

    const checkSession = async () => {
      try {
        const assignment = await api.getKioskAssignment(token);
        if (assignment.status === 'unassigned' || assignment.status === 'expired') {
          localStorage.removeItem(SESSION_TOKEN_KEY);
          localStorage.removeItem(PAIR_CODE_KEY);
          router.push('/kiosk-pair');
        }
      } catch (err) {
        if (err instanceof ApiError && err.status === 404) {
          localStorage.removeItem(SESSION_TOKEN_KEY);
          localStorage.removeItem(PAIR_CODE_KEY);
          router.push('/kiosk-pair');
        }
      }
    };

    const intervalId = setInterval(checkSession, SESSION_CHECK_INTERVAL);
    return () => clearInterval(intervalId);
  }, [router]);

  useEffect(() => {
    if (stagelinqNowPlaying) {
      if (staleTimerRef.current) {
        clearTimeout(staleTimerRef.current);
        staleTimerRef.current = null;
      }
      setLastKnownNowPlaying(stagelinqNowPlaying);
      setNowPlayingFading(false);
    } else if (lastKnownNowPlaying) {
      if (!staleTimerRef.current) {
        setNowPlayingFading(true);
        staleTimerRef.current = setTimeout(() => {
          staleTimerRef.current = null;
          setLastKnownNowPlaying(null);
          setNowPlayingFading(false);
        }, 10_000);
      }
    }
  }, [stagelinqNowPlaying, lastKnownNowPlaying]);

  useEffect(() => {
    return () => {
      if (staleTimerRef.current) {
        clearTimeout(staleTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    const preventDefaults = (e: Event) => {
      e.preventDefault();
      return false;
    };
    document.addEventListener('contextmenu', preventDefaults);
    document.addEventListener('selectstart', preventDefaults);
    document.addEventListener('dragstart', preventDefaults);
    return () => {
      document.removeEventListener('contextmenu', preventDefaults);
      document.removeEventListener('selectstart', preventDefaults);
      document.removeEventListener('dragstart', preventDefaults);
    };
  }, []);

  useEffect(() => {
    if (!display) return;
    const currentIds = new Set(display.accepted_queue.map((item) => item.id));
    const prev = prevAcceptedIdsRef.current;

    const fresh = new Set<number>();
    for (const id of currentIds) {
      if (!prev.has(id)) fresh.add(id);
    }

    if (fresh.size > 0) {
      setNewItemIds(fresh);
      const timer = setTimeout(() => setNewItemIds(new Set()), 800);
      return () => clearTimeout(timer);
    }
    prevAcceptedIdsRef.current = currentIds;
  }, [display]);

  useEffect(() => {
    if (!display?.kiosk_display_only) return;
    const el = queueListRef.current;
    if (!el) return;

    let direction = 1;
    const scrollStep = () => {
      if (!el) return;
      const maxScroll = el.scrollHeight - el.clientHeight;
      if (maxScroll <= 0) return;

      if (el.scrollTop >= maxScroll) direction = -1;
      else if (el.scrollTop <= 0) direction = 1;

      el.scrollBy({ top: direction * 80, behavior: 'smooth' });
    };

    const interval = setInterval(scrollStep, AUTO_SCROLL_INTERVAL);
    return () => clearInterval(interval);
  }, [display?.kiosk_display_only]);

  if (loading) {
    return (
      <div className="kiosk-loading">
        <div className="loading-spinner" />
        <p>Loading kiosk...</p>
        <style jsx>{`
          .kiosk-loading {
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            background: #000;
            color: #fff;
            font-family: sans-serif;
            gap: 16px;
          }
          .loading-spinner {
            width: 48px;
            height: 48px;
            border: 4px solid rgba(255,255,255,0.15);
            border-top-color: #fff;
            border-radius: 50%;
            animation: spin 0.8s linear infinite;
          }
          @keyframes spin {
            to { transform: rotate(360deg); }
          }
        `}</style>
      </div>
    );
  }

  if (error || !display) {
    const isUnpaired = error?.status === 404;
    const isExpired = error?.status === 410;
    return (
      <div className="kiosk-error">
        {isExpired && <ExpiredStage />}
        <h1>{isExpired ? 'Event Expired' : isUnpaired ? 'Kiosk Not Paired' : 'Event Unavailable'}</h1>
        <p>
          {isExpired
            ? 'This event has ended and is no longer accepting requests.'
            : isUnpaired
            ? 'This kiosk screen is not paired with an active event.'
            : (error?.message || 'Unable to load event.')}
        </p>
        {isExpired && <ExpiredStamp />}
        <button onClick={() => router.push('/kiosk-pair')} className="kiosk-pair-btn">
          Go to Pairing Screen
        </button>
        <style jsx>{`
          .kiosk-error {
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            background: #000;
            color: #fff;
            font-family: sans-serif;
            gap: 16px;
            text-align: center;
            padding: 24px;
          }
          h1 { font-size: 32px; font-weight: 800; margin: 0; }
          p { color: rgba(255,255,255,0.6); max-width: 420px; margin: 0; }
          .kiosk-pair-btn {
            margin-top: 12px;
            padding: 12px 24px;
            background: #fff;
            color: #000;
            border: none;
            border-radius: 8px;
            font-weight: 700;
            cursor: pointer;
          }
        `}</style>
      </div>
    );
  }

  const nowPlaying = display.now_playing_hidden ? null : (stagelinqNowPlaying || lastKnownNowPlaying || display.now_playing);
  const isLive = !!stagelinqNowPlaying && stagelinqNowPlaying.source !== 'manual' && stagelinqNowPlaying.bridge_connected !== false;
  const queue = display.accepted_queue;
  const acceptedTotal = display.accepted_queue_total ?? queue.length;
  const hasMoreThanShown = acceptedTotal > queue.length;
  const maxVotes = queue.length > 0 ? Math.max(...queue.map((item) => item.vote_count)) : 0;
  const bannerAccent = safeColor(display.banner_colors?.[0], KIOSK_THEME.accent);

  return (
    <>
      <div className="kiosk-container">
        {/* Subtle Ambient Radial Glow */}
        <div className="kiosk-ambient-glow" />

        {/* Top Guidance Header (Clean SVG, No Emojis) */}
        <div className="kiosk-guidance-strip">
          <div className="guidance-tag">
            <Signal2DIcon size={13} color="#fff" />
            <span>HOW IT WORKS</span>
          </div>
          <div className="guidance-step">
            <span className="step-num">1</span>
            <span>Scan QR code with phone</span>
          </div>
          <div className="guidance-divider" aria-hidden="true">&rsaquo;</div>
          <div className="guidance-step">
            <span className="step-num">2</span>
            <span>Request songs & upvote queue</span>
          </div>
          <div className="guidance-divider" aria-hidden="true">&rsaquo;</div>
          <div className="guidance-step">
            <span className="step-num">3</span>
            <span>Live crowd score on screen</span>
          </div>
        </div>

        {/* Header Bar */}
        <div className="kiosk-header">
          <div className="kiosk-header-left">
            <div className="kiosk-live-label">
              <span className="kiosk-live-dot" />
              <span>LIVE KIOSK DISPLAY</span>
            </div>
            <h1 className="kiosk-event-name">{display.event.name}</h1>
            <div className="kiosk-stats">
              <div className="kiosk-stat">
                <span className="kiosk-stat-value">{acceptedTotal}</span>
                <span className="kiosk-stat-label">REQUESTS</span>
              </div>
              <div className="kiosk-stat">
                <span className="kiosk-stat-value">{display.requests_open ? 'OPEN' : 'CLOSED'}</span>
                <span className="kiosk-stat-label">REQUEST LINE</span>
              </div>
            </div>
          </div>

          {!display.requests_open && (
            <div className="kiosk-closed-banner">REQUESTS CLOSED</div>
          )}
        </div>

        {/* ── Main Layout: 2-Column Grid (Now Playing Left, Queue Right) ── */}
        <div className={`kiosk-main${!nowPlaying ? ' kiosk-main-single' : ''}`}>
          {/* NOW PLAYING HERO (65% Artwork & Track / 35% QR) */}
          {nowPlaying && (
            <div className={`kiosk-panel now-playing-section${nowPlayingFading ? ' fading' : ''}`}>
              <div className="kiosk-panel-label">
                <span>NOW PLAYING</span>
                {isLive && <span className="live-badge">LIVE</span>}
              </div>

              {/* 65% / 35% Split Layout (Council Lock Q3 - 3b) */}
              <div className="now-playing-hero-grid">
                {/* 65% Track Information & 2D Vector Artwork */}
                <div className="now-playing-left-65">
                  <div className="now-playing-artwork-wrap">
                    {nowPlaying.artwork_url ? (
                      <img
                        src={nowPlaying.artwork_url}
                        alt={nowPlaying.title}
                        className="now-playing-art"
                      />
                    ) : (
                      <div className="now-playing-placeholder">
                        <Vinyl2DPlaceholder size={180} />
                      </div>
                    )}
                  </div>

                  <div className="now-playing-meta">
                    <h2 className="now-playing-title">{nowPlaying.title}</h2>
                    <p className="now-playing-artist">{nowPlaying.artist}</p>
                    <div className="now-playing-chips">
                      <span className="now-playing-chip">STAGE MIX</span>
                      {nowPlaying.matched_request_id && (
                        <span className="now-playing-chip requested">CROWD PICK</span>
                      )}
                    </div>
                  </div>
                </div>

                {/* 35% QR Scan Block */}
                <div className="now-playing-right-35">
                  <div className="hero-qr-container">
                    <div className="hero-qr-frame">
                      <QRCodeSVG value={display.qr_join_url} size={130} />
                    </div>
                    <span className="hero-qr-callout">SCAN TO VOTE</span>
                    <span className="hero-qr-sub">Join on your phone</span>
                  </div>
                </div>
              </div>

              {/* EQ audio visualizer bars */}
              <div className="kiosk-eq-bars">
                {Array.from({ length: 16 }, (_, i) => (
                  <div
                    key={i}
                    className="kiosk-eq-bar"
                    style={{
                      animation: `kiosk-eq ${0.6 + (i % 4) * 0.15}s ${i * 0.07}s infinite alternate ease-in-out`,
                    }}
                  />
                ))}
              </div>

              {/* VIBE METER (1B) */}
              {vibeScore !== null ? (
                <div className="kiosk-vibe-meter">
                  <div 
                    className="kiosk-vibe-meter-fill" 
                    style={{ width: `${Math.max(0, Math.min(100, vibeScore))}%` }} 
                  />
                  <div className="kiosk-vibe-meter-overlay" />
                </div>
              ) : (
                <div
                  className="kiosk-reserved-meter-spot"
                  data-testid="kiosk-reserved-meter-spot"
                  data-slot="vibe-meter"
                  aria-hidden="true"
                />
              )}
            </div>
          )}

          {/* QUEUE PANEL (Includes Option 2b Recently Played at bottom) */}
          <div className="kiosk-panel queue-panel">
            <div className="queue-header">
              <div className="kiosk-panel-label" style={{ marginBottom: 0 }}>
                QUEUE
              </div>
              <div style={{ flex: 1 }} />
              <span className="queue-track-count">
                {hasMoreThanShown
                  ? `${queue.length} OF ${acceptedTotal} TRACKS`
                  : `${acceptedTotal} TRACKS`}
              </span>
            </div>

            {queue.length > 0 ? (
              <div className="queue-list" ref={queueListRef}>
                {queue.map((item, i) => {
                  const pct = maxVotes > 0 ? (item.vote_count / maxVotes) * 100 : 0;
                  const isTop3 = i < 3;
                  const rankColor = i === 0 ? '#ffffff' : i === 1 ? '#e5e7eb' : i === 2 ? '#9ca3af' : 'rgba(255,255,255,0.35)';

                  return (
                    <div
                      key={item.id}
                      className={`queue-item${i === 0 ? ' queue-item-top1' : ''}${newItemIds.has(item.id) ? ' queue-item-new' : ''}`}
                    >
                      {/* Vote bar fill */}
                      <div
                        className="queue-item-vote-bar"
                        style={{
                          width: `${pct}%`,
                          background: 'linear-gradient(90deg, rgba(255,255,255,0.12), transparent 90%)',
                        }}
                      />

                      {/* Rank badge */}
                      <div
                        className="queue-item-rank"
                        style={{
                          background: isTop3 ? rankColor : 'transparent',
                          color: isTop3 ? '#000' : 'rgba(255,255,255,0.35)',
                          border: isTop3 ? 'none' : '1px solid rgba(255,255,255,0.1)',
                        }}
                      >
                        {i + 1}
                      </div>

                      {/* Track info */}
                      <div className="queue-item-info">
                        <div className="queue-item-title">{item.title}</div>
                        <div className="queue-item-artist">{item.artist}</div>
                        {item.nickname && (
                          <div className="queue-item-nickname">@{item.nickname}</div>
                        )}
                      </div>

                      {/* Vote count */}
                      <div className="queue-item-votes">
                        <div className="queue-item-votes-row">
                          <Upvote2DIcon size={14} color="#fff" />
                          <span className="queue-item-vote-num">{item.vote_count}</span>
                        </div>
                        <span className="queue-item-vote-label">VOTES</span>
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="queue-empty">
                <p>No songs in queue yet.</p>
                <p>Be the first to request!</p>
              </div>
            )}

            {/* ── OPTION 2B: RECENTLY PLAYED COMPACT STRIP (Council Lock Q2) ── */}
            <div className="kiosk-just-played-panel" data-testid="kiosk-just-played-panel">
              <div className="kiosk-panel-label kiosk-just-played-title">
                RECENTLY PLAYED
              </div>
              {playHistory.length > 0 ? (
                <div className="kiosk-just-played-items">
                  {playHistory.slice(0, 3).map((item) => (
                    <div key={item.id} className="kiosk-just-played-row">
                      <span className="kiosk-just-played-dot" aria-hidden="true">&bull;</span>
                      <span className="kiosk-just-played-track">{item.title}</span>
                      <span className="kiosk-just-played-sep" aria-hidden="true">&mdash;</span>
                      <span className="kiosk-just-played-artist">{item.artist}</span>
                      {item.matched_request_id && (
                        <span className="requested-badge">Requested</span>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="history-empty">
                  <p>No songs played yet.</p>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* CTA Button */}
        {!display.kiosk_display_only && display.requests_open && (
          <button
            className="request-button"
            onClick={() => setShowRequestModal(true)}
          >
            <svg width="18" height="18" viewBox="0 0 14 14" fill="none">
              <path d="M7 1.5v11M1.5 7h11" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"/>
            </svg>
            <span>REQUEST A SONG</span>
            <span className="touch-prompt-pill">TOUCH SCREEN</span>
          </button>
        )}
      </div>

      {showRequestModal && (
        <RequestModal
          code={code}
          onClose={() => setShowRequestModal(false)}
          onRequestsClosed={() => loadDisplay()}
        />
      )}

      {/* ── Styles (Scoped JSX) ── */}
      <style jsx global>{`
        *, *::before, *::after {
          box-sizing: border-box;
          user-select: none;
          -webkit-user-select: none;
        }
        body, html {
          margin: 0;
          padding: 0;
          background: #000000;
          color: #ffffff;
          overflow: hidden;
          width: 100vw;
          height: 100vh;
        }
      `}</style>

      <style jsx>{`
        .kiosk-container {
          width: 100vw;
          height: 100vh;
          background: #050608;
          background-image: ${KIOSK_THEME.bgGradient};
          color: #ffffff;
          display: flex;
          flex-direction: column;
          padding: 24px 36px 30px;
          overflow: hidden;
          font-family: var(--font-sans, system-ui, -apple-system, sans-serif);
          position: relative;
        }

        .kiosk-ambient-glow {
          position: absolute;
          top: 0;
          left: 50%;
          transform: translateX(-50%);
          width: 900px;
          height: 250px;
          background: radial-gradient(ellipse at center, ${KIOSK_THEME.glow} 0%, transparent 70%);
          pointer-events: none;
          z-index: 0;
        }

        /* ── Top Guidance Strip ── */
        .kiosk-guidance-strip {
          display: flex;
          align-items: center;
          gap: 16px;
          padding: 8px 16px;
          background: linear-gradient(110deg, rgba(168,85,247,0.10), rgba(255,255,255,0.035) 42%, rgba(168,85,247,0.045));
          border: 1px solid rgba(192,132,252,0.18);
          border-radius: 10px;
          margin-bottom: 20px;
          font-size: 13px;
          color: rgba(255,255,255,0.65);
          position: relative;
          z-index: 1;
        }
        .guidance-tag {
          display: flex;
          align-items: center;
          gap: 6px;
          font-family: var(--font-mono, monospace);
          font-weight: 700;
          letter-spacing: 1.5px;
          color: ${KIOSK_THEME.accentBright};
          font-size: 11px;
        }
        .guidance-step {
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .step-num {
          display: inline-flex;
          align-items: center;
          justify-content: center;
          width: 18px;
          height: 18px;
          border-radius: 50%;
          background: rgba(255,255,255,0.12);
          color: #fff;
          font-size: 11px;
          font-weight: 700;
        }
        .guidance-divider {
          color: rgba(255,255,255,0.25);
          font-size: 16px;
        }

        /* ── Header Bar ── */
        .kiosk-header {
          display: flex;
          justify-content: space-between;
          align-items: flex-start;
          margin-bottom: 24px;
          position: relative;
          z-index: 1;
        }
        .kiosk-live-label {
          font-family: var(--font-mono, monospace);
          font-size: 13px;
          letter-spacing: 3px;
          color: ${KIOSK_THEME.accentBright};
          font-weight: 700;
          text-transform: uppercase;
          margin-bottom: 8px;
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .kiosk-live-dot {
          display: inline-block;
          width: 8px;
          height: 8px;
          border-radius: 50%;
          background: ${KIOSK_THEME.metal};
          background-size: 250% 100%;
          box-shadow: 0 0 12px ${KIOSK_THEME.glow};
          animation: kiosk-pulse 1.8s ease-in-out infinite, liquid-metal-drift 9s linear infinite;
        }
        .kiosk-header-left {
          flex: 1;
          min-width: 0;
        }
        .kiosk-event-name {
          font-size: 52px;
          font-weight: 800;
          letter-spacing: -1.5px;
          line-height: 1.05;
          margin: 0;
          text-shadow: 0 4px 24px rgba(0,0,0,0.6);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .kiosk-stats {
          display: flex;
          gap: 12px;
          margin-top: 14px;
        }
        .kiosk-stat {
          padding: 8px 14px;
          border-radius: 8px;
          border: 1px solid rgba(255,255,255,0.1);
          display: flex;
          flex-direction: column;
          align-items: flex-start;
          min-width: 80px;
          background: rgba(255,255,255,0.03);
        }
        .kiosk-stat-value {
          font-family: var(--font-mono, monospace);
          font-size: 20px;
          font-weight: 800;
          line-height: 1;
        }
        .kiosk-stat-label {
          font-family: var(--font-mono, monospace);
          font-size: 10px;
          color: rgba(255,255,255,0.5);
          letter-spacing: 1.5px;
          margin-top: 4px;
          font-weight: 600;
        }
        .kiosk-closed-banner {
          background: rgba(255, 255, 255, 0.08);
          border: 1px solid rgba(255, 255, 255, 0.2);
          color: #fff;
          padding: 0.8rem 1.6rem;
          border-radius: 0.8rem;
          font-size: 1.1rem;
          font-weight: 700;
          text-transform: uppercase;
          letter-spacing: 0.1em;
        }

        /* ── Main 2-Column Grid ── */
        .kiosk-main {
          flex: 1;
          display: grid;
          grid-template-columns: 1.2fr 1fr;
          gap: 24px;
          min-height: 0;
          position: relative;
          z-index: 1;
        }
        .kiosk-main-single {
          grid-template-columns: 1fr;
        }

        /* ── Glass Panels ── */
        .kiosk-panel {
          background: linear-gradient(${KIOSK_THEME.panelBg}, ${KIOSK_THEME.panelBg}) padding-box,
            linear-gradient(135deg, rgba(192,132,252,0.38), rgba(255,255,255,0.16) 28%, rgba(168,85,247,0.10) 64%, rgba(192,132,252,0.3)) border-box;
          border: 1px solid transparent;
          border-radius: 20px;
          padding: 22px;
          display: flex;
          flex-direction: column;
          backdrop-filter: blur(12px);
          -webkit-backdrop-filter: blur(12px);
          min-height: 0;
          position: relative;
          overflow: hidden;
        }
        .kiosk-panel-label {
          font-family: var(--font-mono, monospace);
          font-size: 12px;
          font-weight: 700;
          letter-spacing: 2px;
          text-transform: uppercase;
          color: ${KIOSK_THEME.accentBright};
          margin-bottom: 16px;
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .kiosk-live-label > span:not(.kiosk-live-dot),
        .kiosk-panel-label {
          background-image: ${KIOSK_THEME.metal};
          background-size: 250% 100%;
          -webkit-background-clip: text;
          background-clip: text;
          -webkit-text-fill-color: transparent;
          animation: liquid-metal-drift 9s linear infinite;
        }

        /* ── Now Playing Hero 65% / 35% Grid ── */
        .now-playing-section {
          transition: opacity 1s ease-out;
        }
        .now-playing-section.fading {
          opacity: 0.5;
        }
        .now-playing-hero-grid {
          display: grid;
          grid-template-columns: 65% 35%;
          gap: 20px;
          align-items: center;
          margin-bottom: 16px;
        }
        .now-playing-left-65 {
          display: flex;
          align-items: center;
          gap: 20px;
          min-width: 0;
        }
        .now-playing-artwork-wrap {
          flex-shrink: 0;
        }
        .now-playing-art {
          width: 170px;
          height: 170px;
          border-radius: 14px;
          object-fit: cover;
          box-shadow: 0 12px 32px rgba(0,0,0,0.6);
        }
        .now-playing-placeholder {
          width: 170px;
          height: 170px;
          border-radius: 14px;
          background: rgba(255,255,255,0.03);
          border: 1px solid rgba(255,255,255,0.08);
          display: flex;
          align-items: center;
          justify-content: center;
        }
        .now-playing-meta {
          flex: 1;
          min-width: 0;
        }
        .now-playing-title {
          font-size: 34px;
          font-weight: 800;
          letter-spacing: -0.6px;
          line-height: 1.1;
          margin: 0;
          overflow: hidden;
          text-overflow: ellipsis;
          display: -webkit-box;
          -webkit-line-clamp: 2;
          -webkit-box-orient: vertical;
        }
        .now-playing-artist {
          font-size: 20px;
          color: rgba(255,255,255,0.6);
          margin: 6px 0 0;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .now-playing-chips {
          display: flex;
          gap: 8px;
          margin-top: 12px;
        }
        .now-playing-chip {
          padding: 4px 10px;
          border-radius: 99px;
          border: 1px solid rgba(255,255,255,0.14);
          font-family: var(--font-mono, monospace);
          font-size: 11px;
          font-weight: 700;
          letter-spacing: 1px;
          color: rgba(255,255,255,0.7);
        }
        .now-playing-chip.requested {
          background: rgba(255,255,255,0.1);
          color: #fff;
        }

        /* 35% QR Block */
        .now-playing-right-35 {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          border-left: 1px solid rgba(255,255,255,0.08);
          padding-left: 16px;
        }
        .hero-qr-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 6px;
        }
        .hero-qr-frame {
          /* Keep a white quiet zone so the QR remains easy to scan. */
          background: #ffffff;
          padding: 8px;
          border-radius: 10px;
          box-shadow: 0 8px 24px rgba(0,0,0,0.5);
        }
        .hero-qr-callout {
          font-size: 11px;
          font-weight: 800;
          letter-spacing: 1.5px;
          color: ${KIOSK_THEME.accentBright};
          margin-top: 4px;
        }
        .hero-qr-sub {
          font-size: 11px;
          color: rgba(255,255,255,0.45);
        }

        .live-badge {
          background: linear-gradient(110deg, ${KIOSK_THEME.accent}, ${KIOSK_THEME.accentBright}, ${KIOSK_THEME.accent});
          color: #160b20;
          font-size: 0.65rem;
          padding: 0.15rem 0.45rem;
          border-radius: 0.25rem;
          font-weight: 800;
          margin-left: 6px;
        }

        /* ── EQ Bars ── */
        .kiosk-eq-bars {
          display: flex;
          align-items: flex-end;
          gap: 5px;
          height: 40px;
          margin-top: 10px;
        }
        .kiosk-eq-bar {
          width: 5px;
          height: 35%;
          border-radius: 3px;
          background: linear-gradient(to top, ${KIOSK_THEME.accent}, ${KIOSK_THEME.accentBright});
          transform-origin: bottom;
        }

        /* ── Reserved Vibe Meter Spot (Q4 locked: 64px fixed) ── */
        .kiosk-reserved-meter-spot {
          height: 64px;
          min-height: 64px;
          width: 100%;
          border-radius: 10px;
          margin-top: 16px;
          /* Intentionally empty until 1B ships */
        }

        /* ── Queue ── */
        .queue-panel {
          display: flex;
          flex-direction: column;
        }
        .queue-header {
          display: flex;
          align-items: center;
          gap: 12px;
          margin-bottom: 14px;
        }
        .queue-track-count {
          font-family: var(--font-mono, monospace);
          font-size: 11px;
          color: rgba(255,255,255,0.4);
          letter-spacing: 1.5px;
        }
        .queue-list {
          display: flex;
          flex-direction: column;
          gap: 8px;
          overflow-y: auto;
          flex: 1;
          min-height: 0;
          max-height: 380px;
        }
        .queue-list::-webkit-scrollbar {
          width: 4px;
        }
        .queue-list::-webkit-scrollbar-thumb {
          background: rgba(255,255,255,0.15);
          border-radius: 2px;
        }
        .queue-item {
          position: relative;
          background: rgba(255,255,255,0.04);
          border: 1px solid rgba(255,255,255,0.08);
          border-radius: 12px;
          padding: 10px 14px;
          display: flex;
          align-items: center;
          gap: 12px;
          overflow: hidden;
          flex-shrink: 0;
        }
        .queue-item-top1 {
          border-color: rgba(255,255,255,0.3);
          background: rgba(255,255,255,0.08);
        }
        .queue-item-vote-bar {
          position: absolute;
          left: 0;
          top: 0;
          bottom: 0;
          pointer-events: none;
        }
        .queue-item-rank {
          position: relative;
          z-index: 1;
          width: 34px;
          height: 34px;
          flex-shrink: 0;
          border-radius: 8px;
          font-family: var(--font-mono, monospace);
          font-weight: 800;
          font-size: 15px;
          display: flex;
          align-items: center;
          justify-content: center;
        }
        .queue-item-info {
          flex: 1;
          min-width: 0;
          position: relative;
          z-index: 1;
        }
        .queue-item-title {
          font-size: 18px;
          font-weight: 700;
          letter-spacing: -0.3px;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .queue-item-artist {
          font-size: 14px;
          color: rgba(255,255,255,0.55);
          margin-top: 1px;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .queue-item-nickname {
          font-size: 11px;
          color: rgba(255,255,255,0.35);
          margin-top: 2px;
          font-family: var(--font-mono, monospace);
          letter-spacing: 0.5px;
        }
        .queue-item-votes {
          position: relative;
          z-index: 1;
          display: flex;
          flex-direction: column;
          align-items: flex-end;
          flex-shrink: 0;
        }
        .queue-item-votes-row {
          display: flex;
          align-items: center;
          gap: 4px;
        }
        .queue-item-vote-num {
          font-family: var(--font-mono, monospace);
          font-weight: 800;
          font-size: 20px;
          line-height: 1;
        }
        .queue-item-vote-label {
          font-family: var(--font-mono, monospace);
          font-size: 9px;
          color: rgba(255,255,255,0.4);
          letter-spacing: 1.5px;
          margin-top: 2px;
        }
        .queue-item-new {
          animation: slide-in-glow 0.8s ease-out;
        }
        .queue-empty {
          color: rgba(255,255,255,0.35);
          text-align: center;
          padding: 2rem;
          font-size: 16px;
        }

        /* ── Option 2b: Recently Played Compact Strip (Inside Queue Area) ── */
        .kiosk-just-played-panel {
          margin-top: 16px;
          padding-top: 14px;
          border-top: 1px solid rgba(255,255,255,0.1);
        }
        .kiosk-just-played-title {
          font-size: 11px;
          letter-spacing: 2px;
          color: rgba(255,255,255,0.55);
          margin-bottom: 8px;
        }
        .kiosk-just-played-items {
          display: flex;
          flex-direction: column;
          gap: 6px;
        }
        .kiosk-just-played-row {
          display: flex;
          align-items: center;
          gap: 8px;
          font-size: 13px;
          color: rgba(255,255,255,0.85);
          padding: 2px 0;
        }
        .kiosk-just-played-dot {
          color: rgba(255,255,255,0.35);
          font-size: 12px;
        }
        .kiosk-just-played-track {
          font-weight: 600;
          color: #ffffff;
        }
        .kiosk-just-played-sep {
          color: rgba(255,255,255,0.3);
        }
        
        /* ── 1B: Vibe Meter ── */
        .kiosk-vibe-meter {
          height: 64px;
          margin-top: 24px;
          border-radius: 12px;
          background: rgba(255,255,255,0.05);
          position: relative;
          overflow: hidden;
          box-shadow: inset 0 2px 10px rgba(0,0,0,0.5);
        }
        .kiosk-vibe-meter-fill {
          height: 100%;
          background: linear-gradient(90deg, #3b82f6 0%, #a855f7 50%, #ef4444 100%);
          transition: width 0.8s cubic-bezier(0.2, 0.8, 0.2, 1);
          border-radius: 12px;
          box-shadow: 0 0 20px rgba(168, 85, 247, 0.4);
        }
        .kiosk-vibe-meter-overlay {
          position: absolute;
          inset: 0;
          background: linear-gradient(180deg, rgba(255,255,255,0.1) 0%, transparent 50%, rgba(0,0,0,0.2) 100%);
          border-radius: 12px;
          border: 1px solid rgba(255,255,255,0.15);
          pointer-events: none;
        }
        .kiosk-just-played-artist {
          color: rgba(255,255,255,0.6);
        }
        .history-empty {
          color: rgba(255,255,255,0.35);
          font-size: 13px;
          padding: 8px 0;
        }
        .requested-badge {
          background: rgba(255,255,255,0.15);
          color: #fff;
          font-size: 0.65rem;
          padding: 0.15rem 0.4rem;
          border-radius: 0.25rem;
          white-space: nowrap;
          margin-left: auto;
        }

        /* ── CTA Button ── */
        .request-button {
          margin-top: 20px;
          align-self: center;
          flex-shrink: 0;
          position: relative;
          z-index: 1;
          border: 1px solid rgba(255,255,255,0.25);
          background: #ffffff;
          color: #000000;
          font-family: var(--font-sans, sans-serif);
          font-size: 18px;
          font-weight: 800;
          letter-spacing: 0.5px;
          padding: 16px 44px;
          border-radius: 999px;
          cursor: pointer;
          display: flex;
          align-items: center;
          gap: 12px;
          transition: transform 0.2s, box-shadow 0.2s;
          box-shadow: 0 12px 36px rgba(255,255,255,0.2);
        }
        .request-button:hover {
          transform: scale(1.03);
          box-shadow: 0 16px 48px rgba(255,255,255,0.3);
        }
        .touch-prompt-pill {
          background: #000000;
          color: #ffffff;
          font-size: 10px;
          font-weight: 700;
          letter-spacing: 1px;
          padding: 3px 8px;
          border-radius: 99px;
          margin-left: 4px;
        }

        /* ── Animations ── */
        @keyframes kiosk-eq {
          from { height: 18%; }
          to { height: 100%; }
        }
        @keyframes kiosk-pulse {
          0%, 100% { transform: scale(1); opacity: 1; }
          50% { transform: scale(1.3); opacity: 0.6; }
        }

        @keyframes liquid-metal-drift {
          from { background-position: 100% 0; }
          to { background-position: -150% 0; }
        }

        @media (prefers-reduced-motion: reduce) {
          .kiosk-live-label > span:not(.kiosk-live-dot),
          .kiosk-panel-label,
          .kiosk-live-dot {
            animation: none;
            background-position: 50% 0;
          }
        }
        @keyframes slide-in-glow {
          0% {
            transform: translateX(-20px);
            opacity: 0;
          }
          100% {
            transform: translateX(0);
            opacity: 1;
          }
        }
      `}</style>
    </>
  );
}
