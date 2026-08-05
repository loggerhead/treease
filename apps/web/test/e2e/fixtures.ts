import { expect, test as base, type ConsoleMessage, type Page, type TestInfo } from '@playwright/test';

const usageSummary = {
  tier: 'free',
  periodKey: '2026-07',
  limits: {
    graphViewDocumentsMonthly: { kind: 'limited', limit: 10 },
    largeFileProcessingRunsMonthly: { kind: 'limited', limit: 10 },
    aiProcessingMonthly: { kind: 'limited', limit: 10 },
    shareMaxAgeDays: 7,
  },
  usage: {},
};

function formatMessageLocation(message: ConsoleMessage): string {
  const location = message.location();
  if (!location.url) return '';
  const line = typeof location.lineNumber === 'number' ? location.lineNumber + 1 : 0;
  const column = typeof location.columnNumber === 'number' ? location.columnNumber + 1 : 0;
  const position = line > 0 ? `:${line}${column > 0 ? `:${column}` : ''}` : '';
  return ` @ ${location.url}${position}`;
}

function collectPageError(error: Error | unknown): string {
  if (error instanceof Error) {
    return error.stack ?? error.message;
  }
  return String(error);
}

function collectConsoleError(message: ConsoleMessage): string {
  return `[console.${message.type()}] ${message.text()}${formatMessageLocation(message)}`;
}

async function attachBrowserErrors(testInfo: TestInfo, errors: string[]): Promise<void> {
  if (errors.length === 0) return;
  await testInfo.attach('browser-errors', {
    body: errors.join('\n\n'),
    contentType: 'text/plain',
  });
}

function filterAllowedBrowserErrors(testInfo: TestInfo, errors: string[]): string[] {
  const allowed = testInfo.annotations
    .filter((annotation) => annotation.type === 'allow-browser-error')
    .map((annotation) => annotation.description)
    .filter((description): description is string => !!description);
  if (allowed.length === 0) return errors;
  return errors.filter((error) => !allowed.some((needle) => error.includes(needle)));
}

export const test = base.extend<{ _browserErrorCheck: void }>({
  _browserErrorCheck: [async ({ page }, use, testInfo) => {
    const fulfillUsageSummary = async (route: import('@playwright/test').Route) => {
      await route.fulfill({ contentType: 'application/json', body: JSON.stringify(usageSummary) });
    };
    await page.route('**/v1/usage?**', fulfillUsageSummary);
    await page.route('**/v1/usage/events', fulfillUsageSummary);
    await page.route('**/v1/billing/pricing-prewarm**', async (route) => {
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ plans: [], checkouts: [] }) });
    });
    const browserErrors: string[] = [];
    const onPageError = (error: Error | unknown) => {
      browserErrors.push(`[pageerror] ${collectPageError(error)}`);
    };
    const onConsole = (message: ConsoleMessage) => {
      if (message.type() !== 'error') return;
      browserErrors.push(collectConsoleError(message));
    };

    page.on('pageerror', onPageError);
    page.on('console', onConsole);

    try {
      await use();
    } finally {
      page.off('pageerror', onPageError);
      page.off('console', onConsole);
    }

    const unexpectedBrowserErrors = filterAllowedBrowserErrors(testInfo, browserErrors);
    await attachBrowserErrors(testInfo, unexpectedBrowserErrors);
    expect(unexpectedBrowserErrors).toEqual([]);
  }, { auto: true }],
});

export { expect, type Page, type ConsoleMessage };
