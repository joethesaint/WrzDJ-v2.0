import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import GuestBotAvatar, { GUEST_BOT_TYPES, guestBotFor } from '../GuestBotAvatar';

vi.mock('bot-avatars', () => ({
  BotAvatar: ({ type, seed }: { type: string; seed: number }) => (
    <span data-testid="bot-avatar" data-type={type} data-seed={seed} />
  ),
}));

describe('guestBotFor', () => {
  it('gives the same guest the same bot every time', () => {
    expect(guestBotFor(42)).toEqual(guestBotFor(42));
  });

  it('spreads consecutive guests across all shapes', () => {
    const types = new Set(GUEST_BOT_TYPES.map((_, i) => guestBotFor(i + 1).type));
    expect(types.size).toBe(GUEST_BOT_TYPES.length);
  });

  it('keeps the blink seed within 0..1', () => {
    for (const id of [0, 1, 2, 999, 123456]) {
      const { seed } = guestBotFor(id);
      expect(seed).toBeGreaterThanOrEqual(0);
      expect(seed).toBeLessThan(1);
    }
  });
});

describe('GuestBotAvatar', () => {
  it('renders the guest shape, hidden from screen readers', () => {
    render(<GuestBotAvatar guestId={1} />);
    const wrapper = screen.getByTestId('guest-bot');
    expect(wrapper).toHaveAttribute('aria-hidden', 'true');
    expect(screen.getByTestId('bot-avatar')).toHaveAttribute('data-type', guestBotFor(1).type);
  });
});
