import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { NewcomerGuideModal } from '../NewcomerGuideModal';

describe('NewcomerGuideModal', () => {
  it('renders 3 newcomer onboarding steps', () => {
    const handleClose = vi.fn();
    render(<NewcomerGuideModal onClose={handleClose} accentColor="#f59e0b" />);

    expect(screen.getByText(/HOW TO PARTICIPATE/i)).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /Request a Song/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /Vote & Climb the Queue/i })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /Hear It Live on Deck/i })).toBeInTheDocument();
  });

  it('calls onClose when close button or got it button is clicked', () => {
    const handleClose = vi.fn();
    render(<NewcomerGuideModal onClose={handleClose} />);

    const closeBtn = screen.getByRole('button', { name: /close guide/i });
    fireEvent.click(closeBtn);
    expect(handleClose).toHaveBeenCalledTimes(1);

    const gotItBtn = screen.getByRole('button', { name: /let's party/i });
    fireEvent.click(gotItBtn);
    expect(handleClose).toHaveBeenCalledTimes(2);
  });
});
