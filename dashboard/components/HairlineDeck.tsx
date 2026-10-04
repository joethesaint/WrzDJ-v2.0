'use client';

import { useEffect, useRef } from 'react';
import HL from '@/lib/hairline/kernel';
import { deck } from '@/lib/hairline/deck';

interface HairlineDeckProps {
  className?: string;
  /** Called with the figure's read-out: the pitch while scratched, "rest" otherwise. */
  onRead?: (text: string) => void;
}

/**
 * The Deck: a line-drawn turntable that spins at play speed and can be
 * scratched with the pointer. Fills its parent's width at a 5:4 ratio.
 */
export function HairlineDeck({ className, onRead }: HairlineDeckProps) {
  const ref = useRef<HTMLDivElement>(null);
  const onReadRef = useRef(onRead);
  onReadRef.current = onRead;

  useEffect(() => {
    const stage = ref.current;
    if (!stage) return;
    HL.inject(document);
    const svg = HL.mk('svg', { viewBox: '0 0 400 320', 'aria-hidden': 'true' }, stage);
    const read = {
      set textContent(value: string) {
        onReadRef.current?.(value);
      },
      get textContent() {
        return '';
      },
    };
    const handle = deck.mount({ stage, svg, read }, deck.range[1]);
    return () => {
      handle.destroy();
      svg.remove();
    };
  }, []);

  return (
    <div
      ref={ref}
      data-hairline="deck"
      role="img"
      aria-label={deck.means}
      className={className}
      style={{ width: '100%', aspectRatio: '5 / 4', touchAction: 'none' }}
    />
  );
}
