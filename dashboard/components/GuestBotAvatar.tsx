'use client';

import { BotAvatar } from 'bot-avatars';

/** Shapes a guest can get, each with the library's own colour. */
export const GUEST_BOT_TYPES = [
  'clover', 'flower', 'triangle', 'square', 'blob', 'ghost', 'circle', 'drop', 'star',
  'droid', 'mech', 'alien', 'hexagon', 'cat', 'cloud', 'pill', 'pebble', 'puddle',
] as const;

export type GuestBotType = (typeof GUEST_BOT_TYPES)[number];

/** A guest keeps the same bot on every visit: shape and blink offset come from the id. */
export function guestBotFor(guestId: number): { type: GuestBotType; seed: number } {
  const index = ((guestId % GUEST_BOT_TYPES.length) + GUEST_BOT_TYPES.length) % GUEST_BOT_TYPES.length;
  // Golden-ratio spread so neighbouring ids do not blink in unison.
  const seed = (guestId * 0.6180339887) % 1;
  return { type: GUEST_BOT_TYPES[index], seed };
}

export default function GuestBotAvatar({
  guestId,
  size = 22,
  theme = 'auto',
}: {
  guestId: number;
  size?: number;
  theme?: 'auto' | 'dark' | 'light';
}) {
  const { type, seed } = guestBotFor(guestId);
  return (
    <span data-testid="guest-bot" data-type={type} aria-hidden="true" style={{ display: 'inline-flex', verticalAlign: 'middle' }}>
      <BotAvatar type={type} size={size} seed={seed} theme={theme} />
    </span>
  );
}
