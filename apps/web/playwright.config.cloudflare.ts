import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './test/e2e',
  testMatch: 'cloudflare-assets.spec.ts',
  timeout: 30_000,
  expect: { timeout: 15_000 },
  reporter: 'dot',
  use: {
    baseURL: 'http://127.0.0.1:4175',
    trace: 'off',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'pnpm exec wrangler dev --local --config wrangler.jsonc --ip 127.0.0.1 --port 4175',
    url: 'http://127.0.0.1:4175',
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
