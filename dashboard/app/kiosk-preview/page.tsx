'use client';

// TEMPORARY design-review page (brief-kiosk-preview-2026-10-04). Shows the
// three kiosk themes and the four "recently played" placements so the
// Director can pick. Dev-only; delete this route once the picks are made.

import { useEffect, useState, type CSSProperties, type ReactNode } from 'react';
import { notFound } from 'next/navigation';

const THEMES = [
  { name: 'Obsidian Amber', accent: '#f59e0b' },
  { name: 'Electric Violet', accent: '#a855f7' },
  { name: 'Monochrome Studio', accent: '#ffffff' },
];

const PLAYING = { title: 'Midnight City', artist: 'M83', requestedBy: 'Ada' };
const JUST_PLAYED = [
  { title: 'Midnight City', artist: 'M83' },
  { title: 'Blinding Lights', artist: 'The Weeknd' },
  { title: 'Losing It', artist: 'Fisher' },
];
const QUEUE = [
  { title: 'Strobe', artist: 'deadmau5', votes: 14 },
  { title: 'One More Time', artist: 'Daft Punk', votes: 9 },
  { title: 'Titanium', artist: 'David Guetta ft. Sia', votes: 7 },
  { title: 'Innerbloom', artist: 'RÜFÜS DU SOL', votes: 4 },
  { title: 'Show Me Love', artist: 'Robin S', votes: 2 },
];

type Placement = '2a' | '2b' | '2c' | '2d' | null;

const PLACEMENTS: { id: Exclude<Placement, null>; label: string }[] = [
  { id: '2a', label: 'One line under the hero' },
  { id: '2b', label: 'Compact strip inside the queue area' },
  { id: '2c', label: 'Rotating ticker in the hero, only when nothing is playing' },
  { id: '2d', label: 'Third column' },
];

// Sizes are in cqw (percent of the kiosk block's width), so a block drawn at
// 1920px and one drawn at 1280px have the same proportions.
const muted = 'rgba(255,255,255,0.55)';

function Kiosk({ accent, placement }: { accent: string; placement: Placement }) {
  const idle = placement === '2c';
  const sideColumn = placement === '2d';

  return (
    <div
      style={{
        containerType: 'inline-size',
        width: '100%',
        aspectRatio: '16 / 9',
        background: '#000',
        color: '#fff',
        borderRadius: 8,
        overflow: 'hidden',
        fontFamily: 'var(--font-sans, system-ui, sans-serif)',
      }}
    >
      <div style={{ padding: '2.5cqw', height: '100%', boxSizing: 'border-box', display: 'flex', flexDirection: 'column', gap: '1.2cqw' }}>
        {/* Hero: 65% artwork + track, 35% QR (Q3 default). About 40% of the height. */}
        <div style={{ display: 'flex', gap: '2.5cqw', height: '40%' }}>
          <div style={{ flex: '65 1 0', display: 'flex', gap: '2cqw', alignItems: 'center', minWidth: 0 }}>
            <div style={{ height: '100%', aspectRatio: '1', borderRadius: '1cqw', background: '#1c1c1c', flexShrink: 0 }} />
            <div style={{ minWidth: 0 }}>
              {idle ? (
                <Ticker />
              ) : (
                <>
                  <div style={{ fontSize: '1.1cqw', letterSpacing: '0.2em', color: accent, fontWeight: 600 }}>NOW PLAYING</div>
                  <div style={{ fontSize: '4.6cqw', fontWeight: 700, lineHeight: 1.05, margin: '0.6cqw 0' }}>{PLAYING.title}</div>
                  <div style={{ fontSize: '2.2cqw', color: muted }}>{PLAYING.artist}</div>
                  <div style={{ fontSize: '1.4cqw', color: accent, marginTop: '1cqw' }}>Requested by {PLAYING.requestedBy}</div>
                </>
              )}
            </div>
          </div>
          <div style={{ flex: '35 1 0', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: '0.8cqw' }}>
            <div style={{ height: '78%', aspectRatio: '1', background: '#fff', borderRadius: '0.6cqw' }} />
            <div style={{ fontSize: '1.4cqw', fontWeight: 600 }}>Scan to request</div>
            <div style={{ fontSize: '1cqw', color: muted }}>Scan · Request · Vote</div>
          </div>
        </div>

        {placement === '2a' && (
          <div style={{ fontSize: '1.2cqw', color: muted }}>
            Just played: {JUST_PLAYED[0].title} — {JUST_PLAYED[0].artist}
          </div>
        )}

        {/* Reserved meter spot (Q4): fixed height, intentionally empty until 1B. */}
        <div style={{ height: '8%', flexShrink: 0 }} aria-hidden="true" />

        <div style={{ flex: 1, display: 'flex', gap: '2.5cqw', minHeight: 0 }}>
          <div style={{ flex: sideColumn ? '65 1 0' : '1 1 0', minWidth: 0 }}>
            {placement === '2b' && (
              <Section title="Just played" accent={accent}>
                {JUST_PLAYED.map((t) => (
                  <div key={t.title} style={{ fontSize: '1.1cqw', color: muted, padding: '0.2cqw 0' }}>
                    {t.title} — {t.artist}
                  </div>
                ))}
              </Section>
            )}
            <Section title="Up next" accent={accent}>
              {QUEUE.slice(0, placement === '2b' ? 3 : 5).map((t, i) => (
                <div key={t.title} style={{ display: 'flex', alignItems: 'baseline', gap: '1cqw', padding: '0.35cqw 0', fontSize: '1.5cqw' }}>
                  <span style={{ color: muted, width: '2cqw' }}>{i + 1}</span>
                  <span style={{ fontWeight: 600 }}>{t.title}</span>
                  <span style={{ color: muted }}>{t.artist}</span>
                  <span style={{ marginLeft: 'auto', color: accent }}>{t.votes}</span>
                </div>
              ))}
            </Section>
          </div>
          {sideColumn && (
            <div style={{ flex: '35 1 0', minWidth: 0 }}>
              <Section title="Recently played" accent={accent}>
                {JUST_PLAYED.map((t) => (
                  <div key={t.title} style={{ fontSize: '1.3cqw', padding: '0.35cqw 0' }}>
                    {t.title} <span style={{ color: muted }}>— {t.artist}</span>
                  </div>
                ))}
              </Section>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function Section({ title, accent, children }: { title: string; accent: string; children: ReactNode }) {
  return (
    <div style={{ marginBottom: '1cqw' }}>
      <div style={{ fontSize: '1cqw', letterSpacing: '0.2em', color: accent, fontWeight: 600, marginBottom: '0.4cqw' }}>
        {title.toUpperCase()}
      </div>
      {children}
    </div>
  );
}

function Ticker() {
  const [i, setI] = useState(0);
  useEffect(() => {
    const t = window.setInterval(() => setI((n) => (n + 1) % JUST_PLAYED.length), 4000);
    return () => window.clearInterval(t);
  }, []);
  const t = JUST_PLAYED[i];
  return (
    <>
      <div style={{ fontSize: '1.1cqw', letterSpacing: '0.2em', color: muted, fontWeight: 600 }}>BETWEEN TRACKS</div>
      <div style={{ fontSize: '3cqw', fontWeight: 700, margin: '0.6cqw 0' }}>Just played: {t.title}</div>
      <div style={{ fontSize: '2cqw', color: muted }}>{t.artist}</div>
    </>
  );
}

const label: CSSProperties = { fontSize: 20, margin: '40px 0 12px', fontWeight: 600 };

export default function KioskPreviewPage() {
  if (process.env.NODE_ENV === 'production') notFound();

  return (
    <main style={{ background: '#111', color: '#eee', minHeight: '100vh', padding: '32px 16px' }}>
      <div style={{ maxWidth: 1920, margin: '0 auto' }}>
        <h1 style={{ fontSize: 28, margin: 0 }}>Kiosk preview (temporary)</h1>
        <p style={{ color: '#999', maxWidth: 720 }}>
          Pick one theme and one recently-played placement. Placeholder art and QR; the empty band
          under the hero is the reserved meter spot. This page is deleted after the pick.
        </p>

        <h2 style={{ fontSize: 24, marginTop: 48 }}>1. Theme (only the accent colour changes)</h2>
        {THEMES.map((t) => (
          <section key={t.name}>
            <h3 style={label}>{t.name}</h3>
            <Kiosk accent={t.accent} placement={null} />
          </section>
        ))}

        <h2 style={{ fontSize: 24, marginTop: 64 }}>2. Recently played (shown in Monochrome Studio)</h2>
        {PLACEMENTS.map((p) => (
          <section key={p.id}>
            <h3 style={label}>
              {p.id} — {p.label}
            </h3>
            <Kiosk accent="#ffffff" placement={p.id} />
          </section>
        ))}
      </div>
    </main>
  );
}
