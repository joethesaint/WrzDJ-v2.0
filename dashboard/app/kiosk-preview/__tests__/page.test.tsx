import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import KioskPreviewPage from '../page';

describe('KioskPreviewPage', () => {
  it('renders the three theme blocks', () => {
    render(<KioskPreviewPage />);
    for (const name of ['Obsidian Amber', 'Electric Violet', 'Monochrome Studio']) {
      expect(screen.getByRole('heading', { name })).toBeInTheDocument();
    }
  });

  it('renders the four recently-played placements', () => {
    render(<KioskPreviewPage />);
    for (const id of ['2a', '2b', '2c', '2d']) {
      expect(screen.getByRole('heading', { name: new RegExp(`^${id}`) })).toBeInTheDocument();
    }
  });
});
