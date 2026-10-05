import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import VibeMascot, { vibeMascotState } from '../VibeMascot';

vi.mock('bot-avatars', () => ({
  BotAvatar: ({ state }: { state: string }) => <span data-testid="bot-avatar" data-state={state} />,
}));

describe('vibeMascotState', () => {
  it('sleeps with no score or a cold room', () => {
    expect(vibeMascotState(null)).toBe('sleeping');
    expect(vibeMascotState(0)).toBe('sleeping');
    expect(vibeMascotState(39)).toBe('sleeping');
  });

  it('idles in the middle', () => {
    expect(vibeMascotState(40)).toBe('default');
    expect(vibeMascotState(69)).toBe('default');
  });

  it('hops when the room is hot', () => {
    expect(vibeMascotState(70)).toBe('working');
    expect(vibeMascotState(100)).toBe('working');
  });
});

describe('VibeMascot', () => {
  it('passes the mood to the avatar and labels it', () => {
    render(<VibeMascot score={85} />);
    expect(screen.getByTestId('bot-avatar')).toHaveAttribute('data-state', 'working');
    expect(screen.getByTestId('vibe-mascot')).toHaveAttribute('title', 'The room is on fire');
  });
});
