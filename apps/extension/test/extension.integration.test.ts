import { chromium, expect, test } from '@playwright/test';
import http from 'node:http';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const testDir = path.dirname(fileURLToPath(import.meta.url));
const extensionDir = path.resolve(testDir, '../dist');
const fixturePath = path.resolve(testDir, 'fixtures/json-page.html');
const rawJson = JSON.stringify([{ id: 'V59FY2YF62HFY0', payload: 'x'.repeat(128 * 1024) }]);

async function expectPaintedGraph(panel: import('@playwright/test').Page): Promise<void> {
  const canvas = panel.locator('.graph-host canvas');
  await expect(canvas).toBeVisible();
  const nonWhitePixels = await canvas.evaluate((element) => {
    const graphCanvas = element as HTMLCanvasElement;
    const context = graphCanvas.getContext('2d');
    if (!context) return 0;
    const { data } = context.getImageData(0, 0, graphCanvas.width, graphCanvas.height);
    let count = 0;
    for (let index = 0; index < data.length; index += 4) {
      if (data[index + 3] !== 0 && (data[index] < 245 || data[index + 1] < 245 || data[index + 2] < 245)) count += 1;
    }
    return count;
  });
  expect(nonWhitePixels).toBeGreaterThan(0);
}

test('renders Graphs for a highlighted GitHub-style cell and a 128 KB raw JSON document', async () => {
  const fixture = await readFile(fixturePath, 'utf8');
  const highlightedFixture = `<!doctype html><title>Highlighted JSON</title><table><tbody><tr><td class="blob-code">[{"id":"<span id="highlighted-token">V59FY2YF62HFY0</span>","ok":true}]</td></tr></tbody></table>`;
  const server = http.createServer((request, response) => {
    if (request.url === '/raw/128KB-min.json') {
      response.setHeader('content-type', 'text/plain; charset=utf-8');
      response.end(rawJson);
      return;
    }
    response.setHeader('content-type', 'text/html');
    response.end(request.url === '/highlighted' ? highlightedFixture : fixture);
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = (server.address() as { port: number }).port;
  const origin = `http://127.0.0.1:${port}`;
  const profileDir = test.info().outputPath('chrome-profile');
  const context = await chromium.launchPersistentContext(profileDir, {
    headless: false,
    executablePath: process.env.TREEASE_CHROME_EXECUTABLE,
    ignoreDefaultArgs: ['--disable-extensions'],
    args: [`--disable-extensions-except=${extensionDir}`, `--load-extension=${extensionDir}`],
  });
  try {
    const worker = context.serviceWorkers().find((item) => item.url().endsWith('/background.js'))
      ?? await context.waitForEvent('serviceworker', { predicate: (item) => item.url().endsWith('/background.js') });
    const extensionId = new URL(worker.url()).host;
    const panel = await context.newPage();
    await panel.goto(`chrome-extension://${extensionId}/sidepanel.html`);
    await expect(panel.getByRole('button', { name: 'Enable local webpage listening' })).toBeVisible();
    await panel.getByRole('button', { name: 'Enable local webpage listening' }).click();

    const page = await context.newPage();
    await page.goto(`${origin}/highlighted`);
    await page.bringToFront();
    await page.waitForTimeout(250);
    await page.locator('#highlighted-token').click();
    await expect(panel.locator('.document-head')).toHaveCount(0);
    await page.locator('#highlighted-token').click({ modifiers: ['Meta'] });
    await expect(panel.locator('.source-header')).toBeVisible();
    await expect(panel.getByText('Host', { exact: true })).toBeVisible();
    await expect(panel.getByText('DOM path', { exact: true })).toBeVisible();
    await expect(panel.getByText('Raw source', { exact: true })).toHaveCount(0);
    await expect(panel.getByText('Starting Treease GraphViewer…', { exact: true })).toHaveCount(0);
    await expect(panel.getByRole('button', { name: 'Copy JSONPath' })).toHaveCount(0);
    await expect(panel.getByRole('button', { name: 'Open Treease Web' })).toHaveCount(0);
    await expect(panel.getByText('Highlighted JSON', { exact: true })).toHaveCount(0);
    await expectPaintedGraph(panel);
    await expect(panel.locator('[data-treease-open-hint]')).toHaveCount(0);

    await page.goto(`${origin}/raw/128KB-min.json`);
    await page.bringToFront();
    await page.waitForTimeout(250);
    const rawPre = page.locator('pre');
    await expect(rawPre).toHaveCount(1);
    await expect(panel.locator('.source-header')).toBeVisible();
    await expectPaintedGraph(panel);
    await expect(panel.locator('text=TOO LARGE')).toHaveCount(0);
    const graphCanvas = panel.locator('.graph-host canvas');
    const graphBounds = await graphCanvas.boundingBox();
    if (!graphBounds) throw new Error('Missing graph canvas bounds.');
    // A table cell is an interactive graph surface. This deliberately clicks the
    // rendered canvas, rather than an implementation-only workspace probe.
    await graphCanvas.click({ position: { x: 96, y: 64 } });
    await expect(panel.locator('#selected-path')).not.toHaveText('$');
    await expect(panel.locator('#subgraph-workspace')).toBeVisible();
    await expect(panel.getByText('Subgraph workspace', { exact: true })).toBeVisible();
    const panelScrollBefore = await panel.evaluate(() => document.scrollingElement?.scrollTop ?? 0);
    await graphCanvas.hover({ position: { x: 80, y: 80 } });
    await panel.mouse.wheel(0, 240);
    await expect.poll(() => panel.evaluate(() => document.scrollingElement?.scrollTop ?? 0)).toBe(panelScrollBefore);
  } finally {
    await context.close();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});
