import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './test',
  testMatch: '**/*.integration.test.ts',
  timeout: 30_000,
  workers: 1,
  use: { headless: false },
});
