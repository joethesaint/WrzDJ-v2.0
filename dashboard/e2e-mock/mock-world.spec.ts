import { expect, test, type Page } from '@playwright/test';

/**
 * Smoke tests for the mock world in lib/mock/seed.ts. They pin the numbers
 * the seed promises, so a change to the seed or the router that breaks
 * consistency between pages shows up here.
 */

const unmocked: string[] = [];

test.beforeEach(async ({ page }) => {
  unmocked.length = 0;
  // The DJ session is an HttpOnly cookie (#754); the app only probes /me when
  // this non-secret marker says a login happened on this browser. Set it so the
  // AuthProvider calls /api/auth/me (mocked → MOCK_USER) instead of bouncing to
  // /login. SESSION_HINT_KEY = 'wrzdj_session_hint' in lib/auth.tsx.
  await page.addInitScript(() =>
    localStorage.setItem('wrzdj_session_hint', 'mock-session'),
  );
  page.on('console', (m) => {
    if (m.text().startsWith('[mock] no mock for')) unmocked.push(m.text());
  });
});

test.afterEach(() => {
  expect(unmocked, 'every API call on the page should be mocked').toEqual([]);
});

async function skipTour(page: Page) {
  const skip = page.getByRole('button', { name: 'Skip' });
  if (await skip.isVisible().catch(() => false)) await skip.click();
}

test('dashboard lists the three active seed events', async ({ page }) => {
  await page.goto('/dashboard');
  await skipTour(page);
  for (const name of ['Eko Rooftop Sessions', 'Adaeze & Kunle Wedding Reception', 'UI Campus Throwback Night']) {
    await expect(page.getByText(name)).toBeVisible();
  }
  await expect(page.getByText('Connected', { exact: true })).toBeVisible(); // Tidal
  await expect(page.getByText('Not connected')).toBeVisible(); // Beatport
});

test('event page shows the live queue, now playing and seeded vibe', async ({ page }) => {
  await page.goto('/events/LAGOS1');
  await skipTour(page);
  await expect(page.getByText('All (18)')).toBeVisible();
  await expect(page.getByText('Played (5)')).toBeVisible();
  await expect(page.getByText('Calm Down').first()).toBeVisible();
  // Requests carry seeded upvotes; the DJ event page renders them as "N votes"
  // (e.g. "9 votes"), not the parenthesised "(N votes)" an older UI used. Assert
  // at least one request shows a non-zero count.
  await expect(page.getByText(/\b[1-9]\d* votes?\b/).first()).toBeVisible({ timeout: 5000 });
});

test('accepting a request updates the status counts', async ({ page }) => {
  await page.goto('/events/LAGOS1');
  await skipTour(page);
  const before = Number((await page.getByText(/^Accepted \(\d+\)$/).first().innerText()).match(/\d+/)![0]);
  await page.getByRole('button', { name: 'Accept', exact: true }).first().click();
  await expect(page.getByText(`Accepted (${before + 1})`)).toBeVisible();
});

test('kiosk shows the accepted queue sorted by votes', async ({ page }) => {
  await page.goto('/e/LAGOS1/display');
  const queue = page.getByText(/^(Unavailable|Lonely At The Top|Rush|Tshwala Bam)$/);
  await expect(queue).toHaveText(['Unavailable', 'Lonely At The Top', 'Rush', 'Tshwala Bam']);
});

test('guest join page resolves the join code to the live event', async ({ page }) => {
  await page.goto('/join/VIBE24');
  await expect(page.getByText('Eko Rooftop Sessions').first()).toBeVisible();
  await expect(page.getByText('Calm Down').first()).toBeVisible();
});
