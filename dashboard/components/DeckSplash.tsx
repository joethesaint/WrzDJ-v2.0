'use client';

import { useEffect, useState } from 'react';
import { HairlineDeck } from './HairlineDeck';

const SHOW_MS = 1500;
const FADE_MS = 300;

/**
 * A short intro over the guest join page: the Deck spinning on the page
 * background. Shown once per event per browser session; a tap skips it.
 */
export function DeckSplash({ sessionKey }: { sessionKey: string }) {
  const [phase, setPhase] = useState<'hidden' | 'shown' | 'leaving'>('hidden');

  useEffect(() => {
    const key = `wrzdj:splash:${sessionKey}`;
    try {
      if (sessionStorage.getItem(key)) return;
      sessionStorage.setItem(key, '1');
    } catch {
      // Storage blocked: still show it once for this page load.
    }
    setPhase('shown');
    const t = window.setTimeout(() => setPhase('leaving'), SHOW_MS);
    return () => window.clearTimeout(t);
  }, [sessionKey]);

  useEffect(() => {
    if (phase !== 'leaving') return;
    const t = window.setTimeout(() => setPhase('hidden'), FADE_MS);
    return () => window.clearTimeout(t);
  }, [phase]);

  if (phase === 'hidden') return null;

  return (
    <div
      data-testid="deck-splash"
      onClick={() => setPhase('leaving')}
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 1000,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'var(--bg, #0a0a0a)',
        colorScheme: 'dark',
        ['--hairline-plate' as string]: 'var(--bg, #0a0a0a)',
        opacity: phase === 'leaving' ? 0 : 1,
        transition: `opacity ${FADE_MS}ms ease-out`,
        cursor: 'pointer',
      }}
    >
      <div style={{ width: 'min(80vw, 420px)' }}>
        <HairlineDeck />
      </div>
    </div>
  );
}
