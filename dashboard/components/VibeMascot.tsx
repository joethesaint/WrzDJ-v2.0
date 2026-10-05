'use client';

import { BotAvatar } from 'bot-avatars';

type MascotState = 'default' | 'working' | 'sleeping';

/** Room energy to mascot mood: asleep when quiet or cold, hopping when hot. */
export function vibeMascotState(score: number | null): MascotState {
  if (score === null || score < 40) return 'sleeping';
  if (score < 70) return 'default';
  return 'working';
}

const LABELS: Record<MascotState, string> = {
  sleeping: 'The room is quiet',
  default: 'The room is warming up',
  working: 'The room is on fire',
};

/** A small bot beside Now Playing that acts out the live Vibe Meter score. */
export default function VibeMascot({ score, size = 36 }: { score: number | null; size?: number }) {
  const state = vibeMascotState(score);
  return (
    <span data-testid="vibe-mascot" data-state={state} title={LABELS[state]} style={{ display: 'inline-flex' }}>
      <BotAvatar type="star" face="mouth" size={size} state={state} theme="dark" interactive={false} />
    </span>
  );
}
