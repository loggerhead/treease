import { defineConfig, devices } from '@playwright/test';

const CI = !!process.env.CI;
const CPU_CONSTRAINED = process.env.TREEASE_E2E_CPU_CONSTRAINED === '1';
const e2ePort = process.env.TREEASE_E2E_PORT ?? '8080';
const e2eBaseUrl = `http://localhost:${e2ePort}`;

export default defineConfig({
  testDir: './test/e2e',
  testIgnore: [
    'fixture-corpus.spec.ts',
    'editor-core-workflow.spec.ts',
  ],
  // CPU-constrained runs preserve the assertions and only widen their
  // diagnostic budget; readiness still decides completion.
  timeout: CPU_CONSTRAINED ? 30_000 : 10_000,
  retries: 1,
  expect: {
    timeout: CPU_CONSTRAINED ? 30_000 : 10_000,
  },
  fullyParallel: true,
  // Exercise the same semantic waits with scheduling pressure removed.  This
  // is intentionally a test-runner switch, not a production timing change.
  workers: CI || CPU_CONSTRAINED ? 1 : '50%',
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
    baseURL: e2eBaseUrl,
    trace: 'off',
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
    reuseExistingServer: !CI,
    timeout: 120_000,
  },
});
