import { expect, test, type Page } from './fixtures';
import {
  clickGraphProbeAt,
  readGraphClickProbes,
  setEditorContent,
  waitForColumnNavigatorSettled,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

async function countYellowCellRegions(page: Page): Promise<number> {
  const viewport = page.getByTestId('graph-viewer-canvas');
  const clip = await viewport.boundingBox();
  if (!clip) throw new Error('graph viewport is not visible');
  const screenshot = await page.screenshot({ clip });
  return page.evaluate(async (dataUrl) => {
    const image = new Image();
    image.src = dataUrl;
    await image.decode();
    const canvas = document.createElement('canvas');
    canvas.width = image.width;
    canvas.height = image.height;
    const context = canvas.getContext('2d');
    if (!context) return 0;
    context.drawImage(image, 0, 0);
    const { data, width, height } = context.getImageData(0, 0, canvas.width, canvas.height);
    const isYellow = (offset: number) =>
      data[offset] >= 245 && data[offset + 1] >= 210 && data[offset + 1] <= 240 && data[offset + 2] >= 90 && data[offset + 2] <= 150;
    const seen = new Uint8Array(width * height);
    let regions = 0;
    for (let start = 0; start < seen.length; start += 1) {
      if (seen[start] || !isYellow(start * 4)) continue;
      let size = 0;
      const queue = [start];
      seen[start] = 1;
      for (let cursor = 0; cursor < queue.length; cursor += 1) {
        const pixel = queue[cursor]!;
        size += 1;
        const x = pixel % width;
        const y = Math.floor(pixel / width);
        for (const neighbor of [pixel - 1, pixel + 1, pixel - width, pixel + width]) {
          if (neighbor < 0 || neighbor >= seen.length) continue;
          const neighborX = neighbor % width;
          const neighborY = Math.floor(neighbor / width);
          if (Math.abs(neighborX - x) + Math.abs(neighborY - y) !== 1 || seen[neighbor] || !isYellow(neighbor * 4)) continue;
          seen[neighbor] = 1;
          queue.push(neighbor);
        }
      }
      // Ignore antialiased glyph fragments; a cell background is much larger.
      if (size >= 100) regions += 1;
    }
    return regions;
  }, `data:image/png;base64,${screenshot.toString('base64')}`);
}

test('rapid Column Navigator ArrowDown leaves one yellow graph cell', async ({ page }) => {
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
  await workspace.focus();
  await page.keyboard.press('ArrowRight');
  await waitForColumnNavigatorSettled(page, 'k:table|i:0', 20_000);
  await expect.poll(() => countYellowCellRegions(page), { timeout: 5_000 }).toBe(1);

  // Do not await a settle between presses: this matches OS key-repeat faster
  // than async projection and graph-reveal completion.
  await workspace.evaluate((element) => {
    for (let index = 0; index < 30; index += 1) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    }
  });
  await waitForColumnNavigatorSettled(page, undefined, 20_000);
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));

  await expect.poll(() => countYellowCellRegions(page), { timeout: 5_000 }).toBe(1);
});
