import { expect, test } from './fixtures';
import { readGraphClickProbes, waitForEditorReady, waitForGraphRendered } from './utils';

test('initial json example renders a graph without showing an analysis failure', async ({ page }, testInfo) => {
  testInfo.annotations.push({
    type: 'allow-browser-error',
    description: 'Failed to load resource: the server responded with a status of 404 (Not Found)',
  });
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForGraphRendered(page);

  await expect(page.getByTestId('graph-error-message')).toHaveCount(0);
  await expect
    .poll(async () => (await readGraphClickProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThan(0);
});
