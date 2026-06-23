import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './test/e2e',
  testMatch: 'fixture-corpus.spec.ts',
  timeout: 5_000,
  expect: {
    timeout: 5_000,
  },
  // This corpus suite validates fixture semantics through a shared dev server.
  // Running cases in parallel mostly measures cold-start contention in Vite +
  // Monaco instead of parser/graph correctness.
  fullyParallel: false,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:4173',
    trace: 'retain-on-failure',
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
    timeout: 10_000,
  },
});
