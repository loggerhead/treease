import { expect, test } from './fixtures';
import { waitForEditorRuntimeReady, waitForGraphRendered } from './utils';

const CANVAS_HINT_STORAGE_KEY = 'treease:canvas-drag-hint-seen';

test('graph canvas drag hint stays open during navigation and supports manual dismissal', async ({ page }) => {
  test.setTimeout(30_000);
  await page.addInitScript((key) => localStorage.removeItem(key), CANVAS_HINT_STORAGE_KEY);
  await page.goto('/editor');
  await waitForEditorRuntimeReady(page);
  await waitForGraphRendered(page);

  const hint = page.getByTestId('canvas-drag-hint');
  await expect(hint).toBeVisible();
  await expect(hint).toContainText('Hold Space and drag to move the canvas.');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), CANVAS_HINT_STORAGE_KEY)).toBe('1');

  await page.getByTestId('graph-viewer-dropzone').focus();
  await page.keyboard.press('Space');
  await expect(hint).toBeVisible();

  await hint.getByRole('button', { name: 'Dismiss canvas navigation hint' }).click();
  await expect(hint).toHaveCount(0);

  await page.reload();
  await waitForEditorRuntimeReady(page);
  await waitForGraphRendered(page);
  await expect(hint).toBeVisible();
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), CANVAS_HINT_STORAGE_KEY)).toBe('1');
});
