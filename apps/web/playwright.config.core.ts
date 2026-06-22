import { defineConfig, devices } from '@playwright/test';

const CI = !!process.env.CI;

export default defineConfig({
  testDir: './test/e2e',
  testMatch: [
    'editor-core-real-chain.spec.ts',
    'import-format-recognition.spec.ts',
    'format-minify-sort.spec.ts',
    'bidirectional-edit-sync.spec.ts',
    'graph-edit-blur-commit.spec.ts',
    'reveal-sync.spec.ts',
    'drop-import-regression.spec.ts',
    'invalid-json-graph-diagnostics.spec.ts',
  ],
  timeout: CI ? 10_000 : 10_000,
  expect: {
    timeout: CI ? 10_000 : 10_000,
  },
  fullyParallel: true,
  workers: '50%',
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
    timeout: CI ? 120_000 : 10_000,
  },
});
