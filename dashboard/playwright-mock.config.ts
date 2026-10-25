import { defineConfig } from '@playwright/test';

/**
 * Runs the UI against the in-browser mock backend (lib/mock). No server
 * needed; the dev server must be started with NEXT_PUBLIC_DEV_AUTH_BYPASS=1.
 */
export default defineConfig({
  testDir: './e2e-mock',
  fullyParallel: false,
  retries: 0,
  workers: 1,
  use: {
    baseURL: process.env.MOCK_BASE_URL || 'http://localhost:3000',
    viewport: { width: 1440, height: 900 },
    screenshot: 'only-on-failure',
  },
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:3000',
    reuseExistingServer: true,
    env: { NEXT_PUBLIC_DEV_AUTH_BYPASS: '1' },
    timeout: 180_000,
  },
  projects: [{ name: 'mock', use: { browserName: 'chromium' } }],
});
