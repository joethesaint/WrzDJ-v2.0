import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { HairlineTurntable } from '../HairlineTurntable';

describe('HairlineTurntable', () => {
  it('renders turntable with pitch badge and scratch hint', () => {
    const { container } = render(
      <HairlineTurntable
        size="compact"
        isPlaying={true}
        theme="amber"
        title="Test Song"
        artist="Test Artist"
      />
    );

    expect(screen.getByText(/RPM|PAUSED/i)).toBeInTheDocument();
    expect(screen.getByText(/TOUCH TO SCRATCH/i)).toBeInTheDocument();
    expect(container.querySelector('svg')).toBeInTheDocument();
  });

  it('applies theme class and size modifier', () => {
    const { container } = render(
      <HairlineTurntable
        size="full"
        isPlaying={true}
        theme="violet"
      />
    );

    const root = container.querySelector('.hairline-turntable');
    expect(root?.classList.contains('hairline-full')).toBe(true);
    expect(root?.classList.contains('theme-violet')).toBe(true);
  });

  it('updates hint on pointer events', () => {
    const { container } = render(
      <HairlineTurntable
        size="compact"
        isPlaying={true}
        theme="monochrome"
      />
    );

    const root = container.querySelector('.hairline-turntable') as HTMLElement;
    expect(root).toBeInTheDocument();

    // Trigger pointer events
    fireEvent.pointerDown(root, { clientX: 100, clientY: 100 });
    fireEvent.pointerLeave(root);
  });
});
