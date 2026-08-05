import { defineConfig, devices } from '@playwright/test';

const e2ePort = process.env.TREEASE_E2E_PORT ?? '8080';
const e2eBaseUrl = `http://localhost:${e2ePort}`;

export default defineConfig({
  testDir: './test/e2e',
  testMatch: 'fixture-corpus.spec.ts',
  timeout: 10_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: true,
  workers: '50%',
  reporter: 'list',
  use: {
    baseURL: e2eBaseUrl,
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `pnpm exec vp preview --outDir "$PWD/build" --host localhost --port ${e2ePort} --strictPort`,
    url: e2eBaseUrl,
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
