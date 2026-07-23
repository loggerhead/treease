import { expect, test, type Page } from './fixtures';
import {
  getMonacoValue,
  setEditorContent,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

async function waitForGraphSnapshot(page: Page): Promise<void> {
  try {
    await waitForGraphRendered(page, 500);
    return;
  } catch (error) {
    const graphErrors = await page.getByTestId('graph-error-message').allTextContents();
    if (graphErrors.some((message) => message.includes('Document analysis did not produce a snapshot'))) {
      throw error;
    }
  }

  await waitForGraphRendered(page, 500);
}

test('full-edit source writeback does not cancel the newer graph render job across 10 runs', async ({ page, context }, testInfo) => {
  test.setTimeout(180_000);
  testInfo.annotations.push({ type: 'allow-browser-error', description: 'http://localhost:3000/v1/usage' });
  testInfo.annotations.push({ type: 'allow-browser-error', description: 'googleads.g.doubleclick.net' });

  const pasteText = JSON.stringify({
    Accept: '*/*',
    'Content-Type': 'application/json',
    Authorization: 'redacted-authorization',
    'X-Foo': 'bar',
    'X-Bar': 'redacted-value',
    'X-Trace': JSON.stringify({ id: 'redacted-trace' }),
    'X-Example-Token': 'redacted-token',
  }, null, 2);

  for (let run = 1; run <= 10; run += 1) {
    await test.step(`run ${run}/10`, async () => {
      await page.goto('/editor');
      await waitForEditorReady(page);
      await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: new URL(page.url()).origin });
      await setEditorContent(page, { sourceText: '', language: 'json' });

      const editor = page.getByTestId('monaco-source-editor').last();
      await editor.click({ position: { x: 16, y: 16 } });
      await page.evaluate(async (text) => navigator.clipboard.writeText(text), pasteText);
      await page.keyboard.press('ControlOrMeta+V');
      await expect
        .poll(() => getMonacoValue(page, 'source-editor'), { timeout: 10_000 })
        .toContain('X-Example-Token');

      await waitForGraphSnapshot(page);
      await expect(page.getByTestId('graph-error-message')).toHaveCount(0);
    });
  }
});
