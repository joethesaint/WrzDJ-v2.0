import { describe, it, expect, beforeEach, vi } from 'vitest';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import KioskDisplayPage from '../page';

/**
 * D22 (approved preview governs the live kiosk) + D21 / 1B spec §6 (meter).
 * Hero: song 65 / QR 35 · meter bar below the hero · Up Next ≤ 5 rows ·
 * Just played = last 3 finished tracks on one line.
 */

vi.mock('next/navigation', () => ({
  useParams: () => ({ code: 'TEST01' }),
  useRouter: () => ({ push: vi.fn() }),
}));

const store: Record<string, string> = {};
Object.defineProperty(globalThis, 'localStorage', {
  value: {
    getItem: (k: string) => store[k] ?? null,
    setItem: (k: string, v: string) => { store[k] = v; },
    removeItem: (k: string) => { delete store[k]; },
    clear: () => {},
  },
  configurable: true,
});

type StreamHandlers = {
  onVibeUpdated?: (d: { vibe_score: number | null; vote_count: number }) => void;
  onNowPlayingChanged?: () => void;
};
let handlers: StreamHandlers = {};
vi.mock('@/lib/use-event-stream', () => ({
  useEventStream: (_code: string, h: StreamHandlers) => {
    handlers = h;
    return { connected: true };
  },
}));

vi.mock('qrcode.react', () => ({
  QRCodeSVG: ({ value }: { value: string }) => <div data-testid="qr-code" data-value={value} />,
}));
vi.mock('../components/RequestModal', () => ({ RequestModal: () => null }));

const { req } = vi.hoisted(() => ({ req: (id: number, title: string, votes: number) => ({
  id, title, artist: `Artist ${id}`, artwork_url: null, bpm: null, genre: null,
  musical_key: null, nickname: null, vote_count: votes, requester_verified: false,
  }),
}));

vi.mock('@/lib/api', () => ({
  api: {
    getKioskDisplay: vi.fn().mockResolvedValue({
      event: { code: 'TEST01', name: 'Eko Rooftop Sessions' },
      qr_join_url: 'http://localhost:3000/join/VIBE24',
      accepted_queue: [req(1, 'One', 9), req(2, 'Two', 8), req(3, 'Three', 7), req(4, 'Four', 6), req(5, 'Five', 5), req(6, 'Six', 4)],
      accepted_queue_total: 6,
      now_playing: null,
      now_playing_hidden: false,
      requests_open: true,
      kiosk_display_only: true,
      updated_at: new Date().toISOString(),
      banner_url: null, banner_kiosk_url: null, banner_colors: null,
    }),
    getNowPlaying: vi.fn().mockResolvedValue({
      title: 'Calm Down', artist: 'Rema', album: null, album_art_url: null,
      matched_request_id: null, source: 'stagelinq', spotify_uri: null,
      started_at: new Date().toISOString(), bridge_connected: true,
    }),
    getPlayHistory: vi.fn().mockResolvedValue({
      total: 5,
      items: [
        { id: 15, title: 'Calm Down', artist: 'Rema', ended_at: null, play_order: 5, matched_request_id: null, album: null, album_art_url: null, source: 'stagelinq', spotify_uri: null, started_at: '' },
        { id: 14, title: 'Sungba', artist: 'Asake', ended_at: 'x', play_order: 4, matched_request_id: null, album: null, album_art_url: null, source: 'stagelinq', spotify_uri: null, started_at: '' },
        { id: 13, title: 'Commas', artist: 'Ayra Starr', ended_at: 'x', play_order: 3, matched_request_id: null, album: null, album_art_url: null, source: 'stagelinq', spotify_uri: null, started_at: '' },
        { id: 12, title: 'Ye', artist: 'Burna Boy', ended_at: 'x', play_order: 2, matched_request_id: null, album: null, album_art_url: null, source: 'stagelinq', spotify_uri: null, started_at: '' },
        { id: 11, title: 'Peru', artist: 'Fireboy DML', ended_at: 'x', play_order: 1, matched_request_id: null, album: null, album_art_url: null, source: 'stagelinq', spotify_uri: null, started_at: '' },
      ],
    }),
    getKioskAssignment: vi.fn(),
    setKioskSession: vi.fn(),
  },
  ApiError: class extends Error { status = 0; },
  PUBLIC_PAGE_MAX: 500,
}));

async function renderKiosk() {
  render(<KioskDisplayPage />);
  await screen.findByRole('heading', { name: 'Calm Down' });
}

describe('Kiosk layout (D22)', () => {
  beforeEach(() => { handlers = {}; });

  it('puts the song in the hero and the QR beside it', async () => {
    await renderKiosk();
    const hero = screen.getByTestId('kiosk-hero');
    expect(within(hero).getByRole('heading', { name: 'Calm Down' })).toBeTruthy();
    expect(within(hero).getByText('Rema')).toBeTruthy();
    expect(within(hero).getByTestId('qr-code')).toBeTruthy();
    expect(within(hero).getByText('Scan to request')).toBeTruthy();
  });

  it('shows at most 5 rows in Up Next', async () => {
    await renderKiosk();
    const upNext = screen.getByTestId('kiosk-up-next');
    expect(within(upNext).getAllByRole('listitem')).toHaveLength(5);
    expect(within(upNext).queryByText('Six')).toBeNull();
  });

  it('lists the last 3 finished tracks in Just played, not the current one', async () => {
    await renderKiosk();
    const jp = screen.getByTestId('kiosk-just-played-panel');
    expect(within(jp).getAllByRole('listitem').map((li) => li.textContent)).toEqual([
      'Sungba — Asake', 'Commas — Ayra Starr', 'Ye — Burna Boy',
    ]);
  });
});

describe('Kiosk vibe meter (D21, 1B spec §6)', () => {
  beforeEach(() => { handlers = {}; });

  it('starts empty and keeps its slot', async () => {
    await renderKiosk();
    const meter = screen.getByTestId('kiosk-vibe-meter');
    expect(meter.getAttribute('data-state')).toBe('empty');
    expect(within(meter).getByText('Vote on your phone')).toBeTruthy();
  });

  it('shows the score as NN% ENERGY and the vote count', async () => {
    await renderKiosk();
    act(() => handlers.onVibeUpdated?.({ vibe_score: 72, vote_count: 48 }));
    const meter = screen.getByTestId('kiosk-vibe-meter');
    await waitFor(() => expect(meter.getAttribute('data-state')).toBe('live'));
    expect(within(meter).getByText('72%')).toBeTruthy();
    expect(within(meter).getByText('ENERGY')).toBeTruthy();
    expect(within(meter).getByText('48 votes')).toBeTruthy();
    expect(within(meter).getByRole('meter').getAttribute('aria-valuenow')).toBe('72');
  });

  it('empties when the server publishes null (track change)', async () => {
    await renderKiosk();
    act(() => handlers.onVibeUpdated?.({ vibe_score: 72, vote_count: 48 }));
    act(() => handlers.onVibeUpdated?.({ vibe_score: null, vote_count: 0 }));
    await waitFor(() =>
      expect(screen.getByTestId('kiosk-vibe-meter').getAttribute('data-state')).toBe('empty'),
    );
  });

  it('uses singular "vote" for one vote', async () => {
    await renderKiosk();
    act(() => handlers.onVibeUpdated?.({ vibe_score: 100, vote_count: 1 }));
    expect(await screen.findByText('1 vote')).toBeTruthy();
  });
});
