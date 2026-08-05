import { defineConfig, devices } from '@playwright/test';

const CI = !!process.env.CI;

export default defineConfig({
  testDir: './test/e2e',
  testIgnore: 'fixture-corpus.spec.ts',
  timeout: CI ? 10_000 : 10_000,
  retries: 1,
  expect: {
    timeout: CI ? 10_000 : 10_000,
  },
  fullyParallel: true,
  workers: CI ? 1 : '50%',
  reporter: process.env.TREEASE_E2E_COVERAGE === '1'
    ? [
        ['dot'],
        ['monocart-reporter', {
          name: 'Treease E2E Coverage',
          outputFile: './e2e-coverage/index.html',
          coverage: {
            outputDir: './e2e-coverage',
            reports: ['v8', 'html', 'lcovonly'],
            sourceFilter: (sourcePath: string) => /[/\\]src[/\\]/.test(sourcePath),
          },
        }],
      ]
    : 'dot',
  use: {
    baseURL: 'http://localhost:8080',
    trace: 'off',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'pnpm serve:e2e',
    url: 'http://localhost:8080',
    reuseExistingServer: !CI,
    timeout: 120_000,
  },
});
