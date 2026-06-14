import { expect, test, type Page } from '@playwright/test';
import {
  readEditorState,
  readGraphClickProbes,
  readGraphHighlight,
  waitForGraphRendered,
} from './utils';

function buildDeepObject(depth: number): unknown {
  let value: unknown = { leaf: true };
  for (let index = depth - 1; index >= 0; index -= 1) {
    value = { [`level${index}`]: value };
  }
  return value;
}

const cliGraphText = JSON.stringify({
  user: { name: 'Alice', role: 'admin' },
  count: 2,
  deep: buildDeepObject(28),
});

async function mockCliResult(page: Page, token: string) {
  let requestCount = 0;
  await page.route('**/cli/result**', async (route) => {
    const url = new URL(route.request().url());
    if (url.searchParams.get('token') !== token) {
      await route.fulfill({ status: 403, body: 'forbidden' });
      return;
    }
    requestCount += 1;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        source_label: 'cli-input.json',
        expression: '.user',
        language: 'json',
        text: cliGraphText,
      }),
    });
  });
  return () => requestCount;
}

test('CLI graph route renders a fullscreen readonly graph from /cli/result', async ({ page }) => {
  const token = 'test-token';
  const getRequestCount = await mockCliResult(page, token);

  await page.goto(`/cli/graph?token=${token}`);
  await expect(page.getByTestId('graph-viewer-canvas')).toBeVisible({ timeout: 5_000 });
  await waitForGraphRendered(page);

  await expect.poll(getRequestCount, { timeout: 5_000 }).toBe(1);
  await expect(page.getByTestId('graph-search-trigger')).toBeVisible();
  await expect(page.getByTestId('zoom-in-button')).toBeVisible();
  await expect(page.getByTestId('zoom-out-button')).toBeVisible();
  await expect(page.getByTestId('graph-viewer-minimap')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Text mode', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Load compare file', exact: true })).toHaveCount(0);
  await expect(page.getByTestId('monaco-source-editor')).toHaveCount(0);

  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toBe(cliGraphText);
  await expect
    .poll(async () => (await readGraphClickProbes(page)).some((probe) => probe.path.join('.') === 'user.name'), {
      timeout: 5_000,
    })
    .toBe(true);

  await page.getByTestId('graph-search-trigger').click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('Alice');
  const result = page.getByRole('button', { name: 'Graph search result $.user.name', exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await result.click();
  await expect
    .poll(async () => await readGraphHighlight(page), { timeout: 5_000 })
    .toEqual(expect.objectContaining({ path: ['$', 'user', 'name'], target: 'value' }));

  const nameProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.path.join('.') === 'user.name' && probe.target === 'value' && probe.coord,
  );
  expect(nameProbe?.coord).toBeTruthy();
  const canvasBox = await page.getByTestId('graph-viewer-canvas').boundingBox();
  expect(canvasBox).toBeTruthy();
  await page.mouse.dblclick(canvasBox!.x + nameProbe!.coord!.x, canvasBox!.y + nameProbe!.coord!.y);
  await expect(page.locator('.leafer-text-editor')).toHaveCount(0);
});
