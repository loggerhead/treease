import { defineConfig, devices } from '@playwright/test';

const CI = !!process.env.CI;

export default defineConfig({
  testDir: './test/e2e',
  testIgnore: 'fixture-corpus.spec.ts',
  timeout: CI ? 10_000 : 5_000,
  retries: 1,
  expect: {
    timeout: CI ? 10_000 : 5_000,
  },
  fullyParallel: true,
  workers: CI ? 1 : '50%',
  reporter: 'dot',
  use: {
    baseURL: 'http://127.0.0.1:4173',
    trace: 'off',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'pnpm dev:vite -- --host 127.0.0.1 --port 4173',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: true,
    timeout: CI ? 120_000 : 5_000,
  },
});
