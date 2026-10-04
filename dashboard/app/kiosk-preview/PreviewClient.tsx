'use client';

import React, { useState, useEffect } from 'react';

// Brief-specified themes
const THEMES = [
  {
    id: 'amber',
    name: 'Obsidian Amber',
    accent: '#f59e0b',
    bg: '#000000',
    cardBg: '#0b0c10',
    border: 'rgba(245, 158, 11, 0.25)',
    subText: 'rgba(255, 255, 255, 0.55)',
    description: 'Hardware VU meter aesthetic. Warm, tactile, familiar to DJs.',
  },
  {
    id: 'violet',
    name: 'Electric Violet',
    accent: '#a855f7',
    bg: '#000000',
    cardBg: '#0f0c18',
    border: 'rgba(168, 85, 247, 0.3)',
    subText: 'rgba(255, 255, 255, 0.55)',
    description: 'Nightclub & dancefloor vibe. High energy neon glow.',
  },
  {
    id: 'monochrome',
    name: 'Monochrome Studio',
    accent: '#ffffff',
    bg: '#000000',
    cardBg: '#0f1012',
    border: 'rgba(255, 255, 255, 0.2)',
    subText: 'rgba(255, 255, 255, 0.5)',
    description: 'Professional hardware instrument. Neutral, unopinionated, maximum legibility.',
  },
];

const MOCK_TRACK = {
  title: 'Midnight City',
  artist: 'M83',
  requestedBy: 'Ada',
};

// Exact tracks locked in Brief §5
const MOCK_RECENT = [
  { title: 'Midnight City', artist: 'M83' },
  { title: 'Blinding Lights', artist: 'The Weeknd' },
  { title: 'Losing It', artist: 'Fisher' },
];

const MOCK_QUEUE = [
  { rank: 1, title: 'Strobe', artist: 'deadmau5', votes: 14 },
  { rank: 2, title: 'One More Time', artist: 'Daft Punk', votes: 9 },
  { rank: 3, title: 'Titanium', artist: 'David Guetta ft. Sia', votes: 7 },
  { rank: 4, title: 'Innerbloom', artist: 'RÜFÜS DU SOL', votes: 4 },
  { rank: 5, title: 'Show Me Love', artist: 'Robin S', votes: 2 },
];

// Clean 2D Vector SVG Icons (No Emojis)
function Vinyl2DSvg({ accent, size = 120, spinning = false }: { accent: string; size?: number; spinning?: boolean }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      style={{
        animation: spinning ? 'spin 12s linear infinite' : 'none',
        flexShrink: 0,
      }}
      aria-label="2D Vinyl Disc"
    >
      <circle cx="50" cy="50" r="48" fill="#141416" stroke="rgba(255,255,255,0.12)" strokeWidth="1.5" />
      <circle cx="50" cy="50" r="42" stroke="rgba(255,255,255,0.06)" strokeWidth="0.8" />
      <circle cx="50" cy="50" r="36" stroke="rgba(255,255,255,0.06)" strokeWidth="0.8" />
      <circle cx="50" cy="50" r="30" stroke="rgba(255,255,255,0.06)" strokeWidth="0.8" />
      <circle cx="50" cy="50" r="24" stroke="rgba(255,255,255,0.06)" strokeWidth="0.8" />
      <circle cx="50" cy="50" r="18" fill="#1f2024" stroke={accent} strokeWidth="1.5" />
      <circle cx="50" cy="50" r="15" fill="none" stroke={`${accent}40`} strokeWidth="0.6" />
      <circle cx="50" cy="50" r="4.5" fill="#090a0d" stroke="rgba(255,255,255,0.2)" strokeWidth="1" />
      {/* Tonearm line */}
      <line x1="88" y1="12" x2="68" y2="44" stroke="rgba(255,255,255,0.4)" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="88" cy="12" r="3" fill="rgba(255,255,255,0.6)" />
      <rect x="65" y="42" width="6" height="4" rx="1" fill={accent} transform="rotate(-30 68 44)" />
    </svg>
  );
}

function Qr2DSvg({ size = 100 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect width="100" height="100" rx="6" fill="#ffffff" />
      {/* Top Left Finder */}
      <rect x="10" y="10" width="26" height="26" rx="2" fill="#000000" />
      <rect x="15" y="15" width="16" height="16" rx="1" fill="#ffffff" />
      <rect x="19" y="19" width="8" height="8" fill="#000000" />
      {/* Top Right Finder */}
      <rect x="64" y="10" width="26" height="26" rx="2" fill="#000000" />
      <rect x="69" y="15" width="16" height="16" rx="1" fill="#ffffff" />
      <rect x="73" y="19" width="8" height="8" fill="#000000" />
      {/* Bottom Left Finder */}
      <rect x="10" y="64" width="26" height="26" rx="2" fill="#000000" />
      <rect x="15" y="69" width="16" height="16" rx="1" fill="#ffffff" />
      <rect x="19" y="73" width="8" height="8" fill="#000000" />
      {/* Matrix data cells */}
      <rect x="42" y="12" width="6" height="6" fill="#000000" />
      <rect x="52" y="12" width="6" height="6" fill="#000000" />
      <rect x="42" y="24" width="6" height="12" fill="#000000" />
      <rect x="52" y="30" width="6" height="6" fill="#000000" />
      <rect x="12" y="42" width="12" height="6" fill="#000000" />
      <rect x="30" y="42" width="6" height="6" fill="#000000" />
      <rect x="42" y="42" width="16" height="6" fill="#000000" />
      <rect x="64" y="42" width="8" height="6" fill="#000000" />
      <rect x="78" y="42" width="10" height="6" fill="#000000" />
      <rect x="12" y="52" width="6" height="6" fill="#000000" />
      <rect x="24" y="52" width="12" height="6" fill="#000000" />
      <rect x="42" y="54" width="6" height="12" fill="#000000" />
      <rect x="52" y="48" width="6" height="18" fill="#000000" />
      <rect x="64" y="54" width="12" height="6" fill="#000000" />
      <rect x="82" y="54" width="6" height="12" fill="#000000" />
      <rect x="42" y="72" width="12" height="6" fill="#000000" />
      <rect x="60" y="70" width="6" height="8" fill="#000000" />
      <rect x="72" y="72" width="16" height="6" fill="#000000" />
      <rect x="42" y="82" width="6" height="8" fill="#000000" />
      <rect x="54" y="82" width="14" height="6" fill="#000000" />
      <rect x="74" y="84" width="14" height="6" fill="#000000" />
    </svg>
  );
}

function Signal2DSvg({ color = '#fff', size = 16 }: { color?: string; size?: number }) {
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

function Upvote2DSvg({ color = '#fff', size = 14 }: { color?: string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="18 15 12 9 6 15" />
    </svg>
  );
}

export default function KioskPreviewPage() {
  const [tickerIndex, setTickerIndex] = useState(0);

  useEffect(() => {
    const timer = setInterval(() => {
      setTickerIndex((prev) => (prev + 1) % MOCK_RECENT.length);
    }, 4000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div style={{ background: '#050608', color: '#f3f4f6', minHeight: '100vh', padding: '36px 48px', fontFamily: 'system-ui, -apple-system, sans-serif' }}>
      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>

      {/* Header Bar */}
      <header style={{ marginBottom: 44, borderBottom: '1px solid rgba(255,255,255,0.1)', paddingBottom: 24 }}>
        <div style={{ display: 'inline-flex', alignItems: 'center', gap: 6, background: 'rgba(255, 255, 255, 0.08)', color: '#fff', padding: '5px 12px', borderRadius: 6, fontSize: 12, fontWeight: 700, letterSpacing: 1.5, marginBottom: 14 }}>
          <Signal2DSvg color="#10b981" size={14} />
          <span>DEV PREVIEW &middot; COUNCIL BRIEF 2026-10-04</span>
        </div>
        <h1 style={{ fontSize: 34, fontWeight: 800, margin: '0 0 10px 0', letterSpacing: -0.5 }}>
          Kiosk Visual Review for Joe (Director)
        </h1>
        <p style={{ color: 'rgba(255,255,255,0.65)', margin: 0, fontSize: 15, maxWidth: 960, lineHeight: 1.5 }}>
          Compare the three color themes (Section 1) and the four candidate placements for Recently Played (Section 2).
          Built strictly with 2D vector SVGs (no unicode emojis). All views feature the Council-locked empty 64px meter slot (Q4).
        </p>
      </header>

      {/* ── SECTION 1: THE THREE THEMES ────────────────────────────────────────── */}
      <section style={{ marginBottom: 72 }}>
        <div style={{ marginBottom: 28 }}>
          <h2 style={{ fontSize: 24, fontWeight: 800, margin: '0 0 8px 0', color: '#fff', display: 'flex', alignItems: 'center', gap: 10 }}>
            <span>Section 1: The Three Themes (Question 1)</span>
          </h2>
          <p style={{ margin: 0, fontSize: 14, color: 'rgba(255,255,255,0.5)' }}>
            Rendered at 1920px proportional layout. Content, 2D vector hairline disc, typography, and hierarchy are strictly identical.
          </p>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 40 }}>
          {THEMES.map((theme) => (
            <div
              key={theme.id}
              style={{
                background: theme.bg,
                border: `1px solid ${theme.border}`,
                borderRadius: 14,
                padding: 24,
                boxShadow: '0 24px 48px rgba(0,0,0,0.7)',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 18 }}>
                <div>
                  <h3 style={{ fontSize: 19, fontWeight: 800, color: theme.accent, margin: 0, display: 'inline' }}>{theme.name}</h3>
                  <span style={{ marginLeft: 16, fontSize: 13, color: 'rgba(255,255,255,0.5)' }}>{theme.description}</span>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 10px', background: 'rgba(255,255,255,0.05)', borderRadius: 6 }}>
                  <span style={{ width: 10, height: 10, borderRadius: '50%', background: theme.accent, boxShadow: `0 0 10px ${theme.accent}` }} />
                  <span style={{ fontSize: 12, fontFamily: 'monospace', fontWeight: 700, color: theme.accent }}>{theme.accent}</span>
                </div>
              </div>

              {/* 1920-style Kiosk Frame: 65% Hero / 35% Queue */}
              <div style={{ display: 'grid', gridTemplateColumns: '65% 35%', gap: 24, background: theme.cardBg, padding: 24, borderRadius: 10, border: `1px solid ${theme.border}` }}>
                {/* Hero Area */}
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 24 }}>
                    {/* 2D Hairline Vector Turntable Disc */}
                    <div style={{ width: 130, height: 130, borderRadius: 12, background: '#0a0a0c', border: `1px solid ${theme.border}`, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                      <Vinyl2DSvg accent={theme.accent} size={110} spinning={true} />
                    </div>

                    {/* Track info */}
                    <div style={{ flex: 1 }}>
                      <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: 2, color: theme.accent, marginBottom: 6, display: 'flex', alignItems: 'center', gap: 6 }}>
                        <span style={{ width: 6, height: 6, borderRadius: '50%', background: theme.accent }} />
                        <span>NOW PLAYING</span>
                      </div>
                      <div style={{ fontSize: 26, fontWeight: 800, color: '#fff', letterSpacing: -0.5, lineHeight: 1.1 }}>{MOCK_TRACK.title}</div>
                      <div style={{ fontSize: 16, color: theme.subText, marginTop: 4 }}>{MOCK_TRACK.artist}</div>
                      <div style={{ fontSize: 12, color: theme.accent, marginTop: 8, fontWeight: 600 }}>Requested by {MOCK_TRACK.requestedBy}</div>
                    </div>

                    {/* QR Code */}
                    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6 }}>
                      <Qr2DSvg size={90} />
                      <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: 'rgba(255,255,255,0.6)' }}>SCAN TO VOTE</span>
                    </div>
                  </div>

                  {/* Reserved Meter Bar (Empty per D9 / Q4: 64px height) */}
                  <div style={{ marginTop: 20, height: 56, borderRadius: 8, border: `1px dashed ${theme.border}`, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(0,0,0,0.4)' }}>
                    <span style={{ fontSize: 11, fontFamily: 'monospace', color: 'rgba(255,255,255,0.3)', letterSpacing: 1.2 }}>
                      [ RESERVED VIBE METER SLOT &mdash; INTENTIONALLY EMPTY PER Q4 ]
                    </span>
                  </div>
                </div>

                {/* Queue Column */}
                <div style={{ borderLeft: '1px solid rgba(255,255,255,0.08)', paddingLeft: 20 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
                    <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: 1.5, color: 'rgba(255,255,255,0.7)' }}>UP NEXT</div>
                    <div style={{ fontSize: 11, fontFamily: 'monospace', color: 'rgba(255,255,255,0.4)' }}>5 TRACKS</div>
                  </div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                    {MOCK_QUEUE.slice(0, 4).map((q) => (
                      <div key={q.rank} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 12px', background: 'rgba(255,255,255,0.02)', borderRadius: 6, border: '1px solid rgba(255,255,255,0.05)' }}>
                        <span style={{ fontSize: 12, fontWeight: 700, fontFamily: 'monospace', color: q.rank === 1 ? theme.accent : 'rgba(255,255,255,0.4)', width: 16 }}>{q.rank}</span>
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{ fontSize: 13, fontWeight: 700, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{q.title}</div>
                          <div style={{ fontSize: 11, color: theme.subText }}>{q.artist}</div>
                        </div>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 3, color: theme.accent, fontSize: 12, fontWeight: 700, fontFamily: 'monospace' }}>
                          <Upvote2DSvg color={theme.accent} size={12} />
                          <span>{q.votes}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* ── SECTION 2: THE FOUR RECENTLY-PLAYED PLACEMENTS ───────────────────────── */}
      <section style={{ marginBottom: 72 }}>
        <div style={{ marginBottom: 28 }}>
          <h2 style={{ fontSize: 24, fontWeight: 800, margin: '0 0 8px 0', color: '#fff' }}>
            Section 2: The Four Recently-Played Placements (Question 2)
          </h2>
          <p style={{ margin: 0, fontSize: 14, color: 'rgba(255,255,255,0.5)' }}>
            Comparing structural layouts using the neutral Monochrome theme. Evaluates where past tracks live without crowding the queue.
          </p>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 36 }}>
          {/* Option 2a */}
          <div style={{ background: '#000', border: '1px solid rgba(255,255,255,0.15)', borderRadius: 14, padding: 24 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 14 }}>
              <div>
                <h3 style={{ fontSize: 18, fontWeight: 700, color: '#fff', margin: 0 }}>Option 2a &mdash; One Line Under the Hero</h3>
                <p style={{ margin: '4px 0 0', fontSize: 13, color: 'rgba(255,255,255,0.5)' }}>
                  A quiet, single-line text readout directly below Now Playing info. Preserves maximum vertical queue space.
                </p>
              </div>
              <span style={{ padding: '4px 10px', background: 'rgba(255,255,255,0.08)', borderRadius: 6, fontSize: 12, fontFamily: 'monospace' }}>2-COLUMN</span>
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: '60% 40%', gap: 20, background: '#0c0d10', padding: 20, borderRadius: 10 }}>
              <div>
                <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
                  <Vinyl2DSvg accent="#fff" size={80} />
                  <div>
                    <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: 1.5, color: '#fff', marginBottom: 4 }}>NOW PLAYING</div>
                    <div style={{ fontSize: 20, fontWeight: 800 }}>{MOCK_TRACK.title}</div>
                    <div style={{ fontSize: 14, color: 'rgba(255,255,255,0.6)' }}>{MOCK_TRACK.artist}</div>
                  </div>
                </div>

                {/* 2a placement: Single line */}
                <div style={{ marginTop: 14, padding: '7px 12px', background: 'rgba(255,255,255,0.04)', borderRadius: 6, fontSize: 12, color: 'rgba(255,255,255,0.75)', display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span style={{ color: 'rgba(255,255,255,0.4)', fontWeight: 700 }}>JUST PLAYED:</span>
                  <span>{MOCK_RECENT[0].title} &mdash; {MOCK_RECENT[0].artist}</span>
                </div>

                <div style={{ marginTop: 14, height: 44, borderRadius: 6, border: '1px dashed rgba(255,255,255,0.2)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 10, fontFamily: 'monospace', color: 'rgba(255,255,255,0.3)' }}>
                  [ RESERVED METER SLOT ]
                </div>
              </div>
              <div style={{ borderLeft: '1px solid rgba(255,255,255,0.08)', paddingLeft: 20 }}>
                <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: 1.5, color: 'rgba(255,255,255,0.6)', marginBottom: 10 }}>QUEUE (FULL 5 ROWS)</div>
                {MOCK_QUEUE.map((q) => (
                  <div key={q.rank} style={{ fontSize: 13, padding: '7px 0', borderBottom: '1px solid rgba(255,255,255,0.04)', display: 'flex', justifyContent: 'space-between' }}>
                    <span>#{q.rank} {q.title} &mdash; {q.artist}</span>
                    <span style={{ fontFamily: 'monospace', color: '#fff' }}>{q.votes}v</span>
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* Option 2b */}
          <div style={{ background: '#000', border: '1px solid rgba(255,255,255,0.15)', borderRadius: 14, padding: 24 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 14 }}>
              <div>
                <h3 style={{ fontSize: 18, fontWeight: 700, color: '#fff', margin: 0 }}>Option 2b &mdash; Compact Strip Inside Queue Area</h3>
                <p style={{ margin: '4px 0 0', fontSize: 13, color: 'rgba(255,255,255,0.5)' }}>
                  (Recommended default) Places last 3 tracks in a compact sub-panel below 3-4 queue items. Keeps track flow grouped.
                </p>
              </div>
              <span style={{ padding: '4px 10px', background: 'rgba(255, 255, 255, 0.1)', color: '#fff', borderRadius: 6, fontSize: 12, fontFamily: 'monospace', fontWeight: 700 }}>RECOMMENDED SAFE DEFAULT</span>
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: '60% 40%', gap: 20, background: '#0c0d10', padding: 20, borderRadius: 10 }}>
              <div>
                <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
                  <Vinyl2DSvg accent="#fff" size={80} />
                  <div>
                    <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: 1.5, color: '#fff', marginBottom: 4 }}>NOW PLAYING</div>
                    <div style={{ fontSize: 20, fontWeight: 800 }}>{MOCK_TRACK.title}</div>
                    <div style={{ fontSize: 14, color: 'rgba(255,255,255,0.6)' }}>{MOCK_TRACK.artist}</div>
                  </div>
                </div>
                <div style={{ marginTop: 14, height: 44, borderRadius: 6, border: '1px dashed rgba(255,255,255,0.2)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 10, fontFamily: 'monospace', color: 'rgba(255,255,255,0.3)' }}>
                  [ RESERVED METER SLOT ]
                </div>
              </div>
              <div style={{ borderLeft: '1px solid rgba(255,255,255,0.08)', paddingLeft: 20, display: 'flex', flexDirection: 'column', gap: 14 }}>
                <div>
                  <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: 1.5, color: 'rgba(255,255,255,0.6)', marginBottom: 8 }}>UP NEXT (TOP 3)</div>
                  {MOCK_QUEUE.slice(0, 3).map((q) => (
                    <div key={q.rank} style={{ fontSize: 13, padding: '5px 0', borderBottom: '1px solid rgba(255,255,255,0.04)', display: 'flex', justifyContent: 'space-between' }}>
                      <span>#{q.rank} {q.title} &mdash; {q.artist}</span>
                      <span style={{ fontFamily: 'monospace', color: '#fff' }}>{q.votes}v</span>
                    </div>
                  ))}
                </div>
                {/* 2b placement: bottom strip with exact Brief tracks */}
                <div style={{ background: 'rgba(255,255,255,0.03)', padding: 12, borderRadius: 8, border: '1px solid rgba(255,255,255,0.06)' }}>
                  <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1.2, color: 'rgba(255,255,255,0.4)', marginBottom: 6 }}>JUST PLAYED (LAST 3)</div>
                  {MOCK_RECENT.map((r, i) => (
                    <div key={i} style={{ fontSize: 12, color: 'rgba(255,255,255,0.65)', padding: '2px 0' }}>
                      &bull; {r.title} &mdash; {r.artist}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>

          {/* Option 2c */}
          <div style={{ background: '#000', border: '1px solid rgba(255,255,255,0.15)', borderRadius: 14, padding: 24 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 14 }}>
              <div>
                <h3 style={{ fontSize: 18, fontWeight: 700, color: '#fff', margin: 0 }}>Option 2c &mdash; Rotating Ticker in Hero (Idle Only)</h3>
                <p style={{ margin: '4px 0 0', fontSize: 13, color: 'rgba(255,255,255,0.5)' }}>
                  When no track is playing, the hero displays a cycling ticker of recently played songs. Hidden when live.
                </p>
              </div>
              <span style={{ padding: '4px 10px', background: 'rgba(255,255,255,0.08)', borderRadius: 6, fontSize: 12, fontFamily: 'monospace' }}>DYNAMIC TICKER</span>
            </div>
            <div style={{ background: '#0c0d10', padding: 22, borderRadius: 10 }}>
              <div style={{ padding: 18, background: 'rgba(255,255,255,0.03)', borderRadius: 8, textAlign: 'center', border: '1px dashed rgba(255,255,255,0.15)' }}>
                <span style={{ fontSize: 11, letterSpacing: 2, color: 'rgba(255,255,255,0.4)', fontWeight: 700, display: 'block', marginBottom: 4 }}>IDLE / BETWEEN TRACKS</span>
                <span style={{ fontSize: 16, fontWeight: 700, color: '#fff' }}>
                  Just Played: {MOCK_RECENT[tickerIndex].title} &mdash; {MOCK_RECENT[tickerIndex].artist}
                </span>
                <span style={{ display: 'block', marginTop: 4, fontSize: 11, color: 'rgba(255,255,255,0.35)', fontFamily: 'monospace' }}>
                  (Rotating automatically every 4s)
                </span>
              </div>
            </div>
          </div>

          {/* Option 2d */}
          <div style={{ background: '#000', border: '1px solid rgba(255,255,255,0.15)', borderRadius: 14, padding: 24 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 14 }}>
              <div>
                <h3 style={{ fontSize: 18, fontWeight: 700, color: '#fff', margin: 0 }}>Option 2d &mdash; Dedicated Third Column</h3>
                <p style={{ margin: '4px 0 0', fontSize: 13, color: 'rgba(255,255,255,0.5)' }}>
                  Restores the 3-column layout (Now Playing | Queue | Recently Played). Highest information density, narrower columns.
                </p>
              </div>
              <span style={{ padding: '4px 10px', background: 'rgba(255,255,255,0.08)', borderRadius: 6, fontSize: 12, fontFamily: 'monospace' }}>3-COLUMN</span>
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: '40% 35% 25%', gap: 16, background: '#0c0d10', padding: 20, borderRadius: 10 }}>
              <div style={{ background: 'rgba(255,255,255,0.02)', padding: 14, borderRadius: 8 }}>
                <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1.5, color: '#fff' }}>COL 1: NOW PLAYING</div>
                <div style={{ fontSize: 16, fontWeight: 700, marginTop: 8 }}>{MOCK_TRACK.title}</div>
                <div style={{ fontSize: 13, color: 'rgba(255,255,255,0.5)' }}>{MOCK_TRACK.artist}</div>
                <div style={{ marginTop: 12, height: 36, borderRadius: 6, border: '1px dashed rgba(255,255,255,0.15)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 9, fontFamily: 'monospace', color: 'rgba(255,255,255,0.3)' }}>
                  [ METER SLOT ]
                </div>
              </div>
              <div style={{ background: 'rgba(255,255,255,0.02)', padding: 14, borderRadius: 8 }}>
                <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1.5, color: '#fff' }}>COL 2: QUEUE</div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginTop: 8 }}>
                  {MOCK_QUEUE.slice(0, 3).map((q) => (
                    <div key={q.rank} style={{ fontSize: 12, color: 'rgba(255,255,255,0.8)' }}>
                      #{q.rank} {q.title}
                    </div>
                  ))}
                </div>
              </div>
              <div style={{ background: 'rgba(255,255,255,0.02)', padding: 14, borderRadius: 8 }}>
                <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1.5, color: '#fff' }}>COL 3: RECENT</div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginTop: 8 }}>
                  {MOCK_RECENT.map((r, i) => (
                    <div key={i} style={{ fontSize: 12, color: 'rgba(255,255,255,0.65)' }}>
                      {r.title}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
