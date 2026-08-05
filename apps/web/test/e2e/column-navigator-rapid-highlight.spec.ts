import { expect, test, type Page } from './fixtures';
import {
  clickGraphProbeAt,
  readGraphClickProbes,
  setEditorContent,
  waitForColumnNavigatorSettled,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

async function readRuntimeHighlightPathKey(page: Page): Promise<string | null> {
  return page.evaluate(() => {
    const path = window._treease?.graph.getHighlightTarget()?.path;
    if (!path?.length) return null;
    return path
      .map((segment) => segment.tag === 1 ? `i:${segment.index}` : `k:${segment.key}`)
      .join('|');
  });
}

test('rapid Column Navigator ArrowDown leaves the graph highlight on the selected path', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: JSON.stringify({
      table: [
        { h1: 11, h2: 12, h3: 13 },
        { h1: 21, h2: 22, h3: 23 },
        { h1: 31, h2: 32, h3: 33 },
      ],
    }),
    language: 'json',
  });
  await waitForGraphRendered(page, 30_000);

  const tableProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'table' && probe.coord,
  );
  expect(tableProbe, 'table graph cell').toBeTruthy();
  if (!tableProbe?.coord) throw new Error('table graph cell has no coordinate');
  await clickGraphProbeAt(page, tableProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:table', 20_000);

  const workspace = page.getByTestId('column-navigator-graph');
  // Move the pointer off the root graph so this test measures the shared
  // graphHighlight decoration rather than an unrelated pointer-hover fill.
  await workspace.hover();
  await workspace.focus();
  await page.keyboard.press('ArrowRight');
  await waitForColumnNavigatorSettled(page, 'k:table|i:0', 20_000);
  await expect.poll(() => readRuntimeHighlightPathKey(page)).toBe('k:table|i:0');

  // Do not await a settle between presses: this matches OS key-repeat faster
  // than async projection and graph-reveal completion.
  await workspace.evaluate((element) => {
    for (let index = 0; index < 30; index += 1) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    }
  });
  await waitForColumnNavigatorSettled(page, undefined, 20_000);
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));

  const selectedPathKey = await workspace.locator('[data-column-navigator-selected="true"]')
    .getAttribute('data-column-navigator-item-path-key');
  expect(selectedPathKey).toBeTruthy();
  await expect.poll(() => readRuntimeHighlightPathKey(page)).toBe(selectedPathKey);
});
