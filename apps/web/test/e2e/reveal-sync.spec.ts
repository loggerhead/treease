import { readFileSync } from 'node:fs';
import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import type { TreeaseRuntimePathSeg } from '../../src/lib/test-bridge/types';
import {
  clickGraphProbe,
  clickGraphProbeAt,
  clearGraphLastReveal,
  getLatestGraphProbes,
  installGraphEditEventCapture,
  readEditorState,
  readGraphClickProbes,
  readGraphHighlight,
  readGraphHighlightRect,
  readGraphHighlightWorld,
  readGraphHitResult,
  readGraphLastReveal,
  readGraphRevealProbe,
  setEditorContent,
  setMonacoPositionByText,
  waitForEditorReady,
  waitForGraphRendered,
  waitForSettingsReady,
} from './utils';

type LargeFixtureRow = {
  name: string;
  language: string;
  id: string;
  bio: string;
  version: number;
};

const oneMbMinJsonRows = JSON.parse(
  readFileSync(new URL('../../../../test/fixtures/json/1MB-min.1.json', import.meta.url), 'utf8'),
) as LargeFixtureRow[];

const oneMbFixtureSampleRows = oneMbMinJsonRows.slice(0, 140);

const oneMbNoHeaderRowsSourceText = JSON.stringify({
  rows: oneMbFixtureSampleRows.map((row) => row.name),
});

const oneMbHeaderTableSourceText = JSON.stringify({
  table_with_header: oneMbFixtureSampleRows,
});

function getVisibleRowValueProbes(
  probes: Array<{
    target?: 'key' | 'value' | 'node';
    path: string[];
    rawPath: TreeaseRuntimePathSeg[];
    coord: { x: number; y: number } | null;
    text: string;
  }>,
) {
  const visible: Array<{
    rowIndex: number;
    path: string[];
    rawPath: TreeaseRuntimePathSeg[];
    coord: { x: number; y: number } | null;
    text: string;
  }> = [];
  for (const probe of probes) {
    if (probe.target !== 'value') continue;
    if (probe.path[0] !== 'rows') continue;
    if (!probe.coord) continue;
    const match = /^\[(\d+)\]$/.exec(probe.path[1] ?? '');
    if (!match) continue;
    visible.push({
      rowIndex: Number(match[1]),
      path: probe.path,
      rawPath: probe.rawPath,
      coord: probe.coord,
      text: probe.text,
    });
  }
  return visible;
}

async function revealGraphSearchResult(page: Page, query: string, resultName: string) {
  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill(query);

  const result = page.getByRole('button', { name: resultName, exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await clearGraphLastReveal(page);
  await result.click();
}

test('graph click updates tree path and selects editor text from emitted reveal payload', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphEditEventCapture(page);

  await setEditorContent(page, {
    sourceText: '{"user":{"name":"Alice","role":"admin"},"count":42}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 }).toBeGreaterThan(1);
  const clickProbes = await readGraphClickProbes(page);
  const probeCount = clickProbes.length;
  let revealPayload = null as Awaited<ReturnType<typeof readGraphLastReveal>>;
  for (let probeIndex = 0; probeIndex < probeCount; probeIndex += 1) {
    const probe = clickProbes[probeIndex];
    if (!probe?.id) continue;
    const expectedProbe = await readGraphRevealProbe(page, probe.id);
    if (!expectedProbe || expectedProbe.path.length <= 1 || expectedProbe.target === 'node') continue;
    await clearGraphLastReveal(page);
    await clickGraphProbe(page, probeIndex);
    try {
      await expect
        .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 1_500 })
        .toEqual(expect.arrayContaining(expectedProbe.path));
      revealPayload = (await readGraphLastReveal(page)) ?? expectedProbe;
      if (revealPayload) break;
    } catch {}
  }
  if (!revealPayload) throw new Error('graph reveal payload missing');
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expect.arrayContaining(revealPayload.path));
  await expect.poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 }).toBeGreaterThan(0);
});

test('graph click reveals header-table cell paths back to editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: JSON.stringify({
      table_with_header: [
        { h1: 11, h2: 12, h3: 13 },
        { h1: 21, h2: 22, h3: 23 },
      ],
    }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const clickProbes = await readGraphClickProbes(page);
  const probeIndex = clickProbes.findIndex(
    (probe) =>
      probe.isTableCell &&
      !probe.isHeader &&
      probe.target === 'value' &&
      probe.text === '11' &&
      probe.path.join('.') === 'table_with_header.[0].h1',
  );
  expect(probeIndex).toBeGreaterThanOrEqual(0);

  const expectedProbe = clickProbes[probeIndex];
  if (!expectedProbe) throw new Error('table value probe missing');

  await clearGraphLastReveal(page);
  await clickGraphProbe(page, probeIndex);

  await expect
    .poll(async () => await readGraphLastReveal(page), { timeout: 5_000 })
    .toEqual(
      expect.objectContaining({
        path: ['$', 'table_with_header', '[0]', 'h1'],
        target: 'value',
      }),
    );
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expect.arrayContaining(['$', 'table_with_header', '[0]', 'h1']));
});

test('graph search selection reveals a long indexed array item', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = JSON.stringify({
    rows: Array.from({ length: 200 }, (_, index) => `item-${String(index).padStart(3, '0')}`),
  });

  await setEditorContent(page, {
    sourceText,
    language: 'json',
  });
  await waitForGraphRendered(page);

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('item-172');

  const result = page.getByRole('button', { name: 'Graph search result $.rows[172]', exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await result.click();

  await expect
    .poll(async () => await readGraphLastReveal(page), { timeout: 5_000 })
    .toEqual(expect.objectContaining({ path: ['$', 'rows', '[172]'], target: 'value' }));

  await expect
    .poll(async () => await readGraphHighlight(page), { timeout: 5_000 })
    .toEqual(expect.objectContaining({ path: ['$', 'rows', '[172]'], target: 'value' }));

  await expect
    .poll(
      async () =>
        (await readGraphClickProbes(page)).some(
          (probe) => probe.path.join('.') === 'rows.[172]' && probe.text === 'item-172',
        ),
      {
        timeout: 5_000,
      },
    )
    .toBe(true);
});

test('graph wheel scroll keeps table rows rendered', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = JSON.stringify({
    rows: Array.from({ length: 240 }, (_, index) => `item-${String(index).padStart(3, '0')}`),
  });

  await setEditorContent(page, {
    sourceText,
    language: 'json',
  });
  await waitForGraphRendered(page);

  await expect
    .poll(async () => getVisibleRowValueProbes(await readGraphClickProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThanOrEqual(3);

  const beforeRows = getVisibleRowValueProbes(await readGraphClickProbes(page));
  const wheelTarget = beforeRows.find((row) => row.coord)?.coord;
  expect(wheelTarget).not.toBeNull();
  if (!wheelTarget) throw new Error('missing initial visible row probe');

  await clickGraphProbeAt(page, wheelTarget);
  await page.mouse.wheel(0, 1200);

  await expect
    .poll(async () => getVisibleRowValueProbes(await readGraphClickProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThanOrEqual(3);
  await expect
    .poll(async () => getVisibleRowValueProbes(await readGraphClickProbes(page)).some((row) => row.rowIndex >= 40), {
      timeout: 5_000,
    })
    .toBe(true);

  const finalRows = getVisibleRowValueProbes(await readGraphClickProbes(page));
  const afterMaxRow = Math.max(...finalRows.map((row) => row.rowIndex));
  expect(afterMaxRow).toBeGreaterThanOrEqual(40);
  expect(finalRows.every((row) => row.text.startsWith('item-'))).toBe(true);
});

test('graph runtime hit testing stays aligned with visible table probes after wheel scroll', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = JSON.stringify({
    rows: Array.from({ length: 240 }, (_, index) => `item-${String(index).padStart(3, '0')}`),
  });

  await setEditorContent(page, {
    sourceText,
    language: 'json',
  });
  await waitForGraphRendered(page);

  await expect
    .poll(async () => getVisibleRowValueProbes(await readGraphClickProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThanOrEqual(3);

  const beforeRows = getVisibleRowValueProbes(await readGraphClickProbes(page));
  const wheelTarget = beforeRows.find((row) => row.coord)?.coord;
  expect(wheelTarget).not.toBeNull();
  if (!wheelTarget) throw new Error('missing initial visible row probe');

  await clickGraphProbeAt(page, wheelTarget);
  await page.mouse.wheel(0, 1200);

  await expect
    .poll(async () => getVisibleRowValueProbes(await readGraphClickProbes(page)).some((row) => row.rowIndex >= 40), {
      timeout: 5_000,
    })
    .toBe(true);

  const finalRows = getVisibleRowValueProbes(await readGraphClickProbes(page));
  const targetRow = finalRows.find((row) => row.coord && row.rowIndex >= 40);
  expect(targetRow).toBeTruthy();
  if (!targetRow?.coord) throw new Error('missing scrolled row probe');

  const hit = await readGraphHitResult(page, targetRow.coord);
  expect(hit).toEqual(
    expect.objectContaining({
      scope: 'root',
      hit: expect.objectContaining({
        target: 'value',
        path: ['$', 'rows', `[${targetRow.rowIndex}]`],
        text: targetRow.text,
      }),
    }),
  );
});

test('1MB fixture no-header table row 100 click selects the matching editor text', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: oneMbNoHeaderRowsSourceText,
    language: 'json',
  });
  await waitForGraphRendered(page, 5_000);

  const targetValue = oneMbFixtureSampleRows[100]?.name;
  expect(targetValue).toBeTruthy();
  if (!targetValue) throw new Error('fixture row 100 name missing');

  await revealGraphSearchResult(page, targetValue, 'Graph search result $.rows[100]');

  const targetPath = 'rows.[100]';
  await expect
    .poll(
      async () => (await readGraphClickProbes(page)).some((probe) => !!probe.coord && probe.path.join('.') === targetPath && probe.text === targetValue),
      { timeout: 5_000 },
    )
    .toBe(true);
  const targetProbe = (await readGraphClickProbes(page)).find(
    (probe) => !!probe.coord && probe.path.join('.') === targetPath && probe.text === targetValue,
  );
  expect(targetProbe?.coord).toBeTruthy();
  if (!targetProbe?.coord) throw new Error(`graph probe ${targetPath} missing after reveal`);

  await clearGraphLastReveal(page);
  await clickGraphProbeAt(page, targetProbe.coord);

  const expectedPath = ['$', 'rows', '[100]'];
  await expect
    .poll(async () => await readGraphLastReveal(page), { timeout: 5_000 })
    .toEqual(
      expect.objectContaining({
        path: expectedPath,
        target: 'value',
      }),
    );
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expectedPath);

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 })
    .toBeGreaterThan(0);
});

test('1MB fixture header table row 100 click selects the matching editor text', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: oneMbHeaderTableSourceText,
    language: 'json',
  });
  await waitForGraphRendered(page, 5_000);

  const targetValue = oneMbFixtureSampleRows[100]?.id;
  expect(targetValue).toBeTruthy();
  if (!targetValue) throw new Error('fixture row 100 id missing');

  await revealGraphSearchResult(page, targetValue, 'Graph search result $.table_with_header[100].id');

  const targetPath = 'table_with_header.[100].id';
  await expect
    .poll(
      async () => (await readGraphClickProbes(page)).some((probe) => !!probe.coord && probe.path.join('.') === targetPath && probe.text === targetValue),
      { timeout: 5_000 },
    )
    .toBe(true);
  const targetProbe = (await readGraphClickProbes(page)).find(
    (probe) => !!probe.coord && probe.path.join('.') === targetPath && probe.text === targetValue,
  );
  expect(targetProbe?.coord).toBeTruthy();
  if (!targetProbe?.coord) throw new Error(`graph probe ${targetPath} missing after reveal`);

  await clearGraphLastReveal(page);
  await clickGraphProbeAt(page, targetProbe.coord);

  const expectedPath = ['$', 'table_with_header', '[100]', 'id'];
  await expect
    .poll(async () => await readGraphLastReveal(page), { timeout: 5_000 })
    .toEqual(
      expect.objectContaining({
        path: expectedPath,
        target: 'value',
      }),
    );
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expectedPath);

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 })
    .toBeGreaterThan(0);
});

test('sync scroll toggle also gates editor and graph reveal synchronization', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForSettingsReady(page);

  await setEditorContent(page, {
    sourceText: '{"user":{"name":"Alice","role":"admin"},"count":42}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  await setMonacoPositionByText(page, 'source-editor', '"role":');
  await expect
    .poll(async () => readGraphHighlight(page), { timeout: 5_000 })
    .toEqual(
      expect.objectContaining({
        path: ['$', 'user', 'role'],
      }),
    );

  await page.getByTestId('sync-scroll-toggle').click();
  await expect(page.getByRole('button', { name: 'Enable synchronized scrolling', exact: true })).toBeVisible();

  await setMonacoPositionByText(page, 'source-editor', '"count":');
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(['$', 'count']);
  expect(await readGraphHighlight(page)).toEqual(
    expect.objectContaining({
      path: ['$', 'user', 'role'],
    }),
  );

  const nameProbe = (await readGraphClickProbes(page)).find(
    (probe) => !!probe.coord && probe.target === 'value' && probe.path.join('.') === 'user.name' && probe.text === 'Alice',
  );
  expect(nameProbe?.coord).toBeTruthy();
  if (!nameProbe?.coord) throw new Error('graph probe user.name missing');

  await clickGraphProbeAt(page, nameProbe.coord);
  await page.waitForTimeout(150);
  expect((await readEditorState(page)).tempModel.treePath).toEqual(['$', 'count']);

  await page.getByTestId('sync-scroll-toggle').click();
  await expect(page.getByRole('button', { name: 'Disable synchronized scrolling', exact: true })).toBeVisible();

  await clickGraphProbeAt(page, nameProbe.coord);
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(['$', 'user', 'name']);
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 })
    .toBeGreaterThan(0);
});

test('graph search closes on a single Escape and outside click', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: '{"previewSamples":["#4f46e5"],"exampleCount":4}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await page.keyboard.press('Escape');
  await expect(input).toBeHidden({ timeout: 5_000 });

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  await expect(input).toBeVisible();

  await page.mouse.click(8, 8);
  await expect(input).toBeHidden({ timeout: 5_000 });
});

test('graph search selection reveals the target graph node and editor path state', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphEditEventCapture(page);

  await setEditorContent(page, {
    sourceText: '{"user":{"name":"Alice","role":"admin"},"items":[1,2,3]}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('Alice');

  const result = page.getByRole('button', { name: 'Graph search result $.user.name', exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await clearGraphLastReveal(page);
  await result.click();

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(['$', 'user', 'name']);
  await expect.poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 }).toBeGreaterThan(0);
  await expect
    .poll(async () => await readGraphLastReveal(page), { timeout: 5_000 })
    .toEqual(
      expect.objectContaining({
        path: ['$', 'user', 'name'],
        target: 'value',
      }),
    );
});

test('editor first click after graph search reveal moves caret from external range selection', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForGraphRendered(page);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toContain('"unicode": "你好"');
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toContain('"color": "#4f46e5"');

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('你好');

  const result = page.getByRole('button', { name: 'Graph search result $.preview.unicode', exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await result.click();

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(['$', 'preview', 'unicode']);
  await expect.poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 }).toBeGreaterThan(0);

  await page.locator('[data-testid="monaco-source-editor"]').getByText('#4f46e5').first().click();

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(['$', 'preview', 'color']);
  await expect.poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 }).toBe(0);
  await expect.poll(async () => (await readEditorState(page)).tempModel.cursor, { timeout: 5_000 }).toMatch(/^Ln 16,/);
});

test('graph search does not expose empty collections as revealable child nodes', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: '{"holder":{"emptyObj":{},"emptyArr":[],"filledObj":{"name":"Alice"},"filledArr":["item-1"]}}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await input.fill('{}');
  await expect(page.getByRole('button', { name: 'Graph search result $.holder.emptyObj', exact: true })).toBeHidden();

  await input.fill('[]');
  await expect(page.getByRole('button', { name: 'Graph search result $.holder.emptyArr', exact: true })).toBeHidden();

  await input.fill('Alice');
  await expect(
    page.getByRole('button', { name: 'Graph search result $.holder.filledObj.name', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });

  await input.fill('item-1');
  await expect(
    page.getByRole('button', { name: 'Graph search result $.holder.filledArr[0]', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });
});

test('graph search filters structural collection results for example json', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText:
      '{"samples":{"identity":{"encodedContacts":{"uriSamples":["https%3A%2F%2Ftreease.dev%2Fpreview","https://example.com/path?redirect=http%3A%2F%2Ftreease.dev"]}}}}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await input.fill('urisample');
  await expect(
    page
      .getByRole('button', { name: 'Graph search result $.samples.identity.encodedContacts.uriSamples', exact: true })
      .first(),
  ).toBeVisible({ timeout: 5_000 });
  await expect(page.locator('button').filter({ hasText: '[]' })).toHaveCount(0);
  await expect(page.locator('button').filter({ hasText: /^1$/ })).toHaveCount(0);
});

test('graph search updates results cleanly as query narrows', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText:
      '{"previewSamples":["#4f46e5"],"exampleCount":4,"examples":{"identity":{"encodedContacts":{"uriSamples":["%E4%BD%A0%E5%A5%BD"]}}}}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await input.fill('4');
  await expect(
    page.getByRole('button', { name: 'Graph search result $.previewSamples[0]', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });
  await expect(
    page.getByRole('button', { name: 'Graph search result $.exampleCount', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });

  await input.fill('4f');
  await expect(
    page.getByRole('button', { name: 'Graph search result $.previewSamples[0]', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });
  await expect(page.getByRole('button', { name: 'Graph search result $.exampleCount', exact: true })).toHaveCount(0);
  await expect(
    page.getByRole('button', {
      name: 'Graph search result $.examples.identity.encodedContacts.uriSamples[0]',
      exact: true,
    }),
  ).toHaveCount(0);
});

test('graph search keeps reveal working after switching queries', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphEditEventCapture(page);

  await waitForGraphRendered(page);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toContain('"object": {');
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toContain('"uris"');

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await input.fill('redirect=');
  await expect(
    page.getByRole('button', { name: 'Graph search result $.preview.uris[1]', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });
  await clearGraphLastReveal(page);
  await page.keyboard.press('Enter');

  await expect
    .poll(async () => await readGraphLastReveal(page), { timeout: 5_000 })
    .toEqual(expect.objectContaining({ path: ['$', 'preview', 'uris', '[1]'] }));
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(['$', 'preview', 'uris', '[1]']);
  await expect
    .poll(async () => readGraphHighlight(page), { timeout: 5_000 })
    .toEqual(
      expect.objectContaining({
        path: ['$', 'preview', 'uris', '[1]'],
      }),
    );
  await expect.poll(async () => await readGraphHighlightRect(page), { timeout: 5_000 }).not.toBeNull();
  await expect.poll(async () => await readGraphHighlightWorld(page), { timeout: 5_000 }).not.toBeNull();
  const uriHighlightRect = await readGraphHighlightRect(page);
  const uriHighlightWorld = await readGraphHighlightWorld(page);
  expect(uriHighlightRect).not.toBeNull();
  expect(uriHighlightWorld).not.toBeNull();
  if (uriHighlightWorld) {
    expect(Math.abs(uriHighlightWorld.highlight.x - uriHighlightWorld.viewportCenter.x)).toBeLessThan(80);
    expect(Math.abs(uriHighlightWorld.highlight.y - uriHighlightWorld.viewportCenter.y)).toBeLessThan(80);
  }

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  await expect(input).toBeVisible();
  await input.clear();
  await input.fill('42');
  await expect(page.getByRole('button', { name: 'Graph search result $.object.int', exact: true }).first()).toBeVisible(
    { timeout: 5_000 },
  );
  await clearGraphLastReveal(page);
  await page.keyboard.press('Enter');

  await expect
    .poll(async () => await readGraphLastReveal(page), { timeout: 5_000 })
    .toEqual(expect.objectContaining({ path: ['$', 'object', 'int'] }));
  const lastReveal = await readGraphLastReveal(page);
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(['$', 'object', 'int']);
  await expect
    .poll(async () => readGraphHighlight(page), { timeout: 5_000 })
    .toEqual(
      expect.objectContaining({
        path: ['$', 'object', 'int'],
      }),
    );
  await expect.poll(async () => await readGraphHighlightRect(page), { timeout: 5_000 }).not.toBeNull();
  await expect.poll(async () => await readGraphHighlightWorld(page), { timeout: 5_000 }).not.toBeNull();
  const countHighlightRect = await readGraphHighlightRect(page);
  const countHighlightWorld = await readGraphHighlightWorld(page);
  expect(countHighlightRect).not.toBeNull();
  expect(countHighlightWorld).not.toBeNull();
  expect(countHighlightRect).not.toEqual(uriHighlightRect);
  if (countHighlightWorld) {
    expect(Math.abs(countHighlightWorld.highlight.x - countHighlightWorld.viewportCenter.x)).toBeLessThan(80);
    expect(Math.abs(countHighlightWorld.highlight.y - countHighlightWorld.viewportCenter.y)).toBeLessThan(80);
  }
  if (lastReveal) {
    await expect.poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 }).toBeGreaterThan(0);
  }
});

test('clearing editor removes graph probes and clears highlight state', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: '{"user":{"name":"Alice"},"items":[1,2,3]}',
    language: 'json',
  });
  await waitForGraphRendered(page);
  await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 }).toBeGreaterThan(1);

  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('Alice');
  const result = page.getByRole('button', { name: 'Graph search result $.user.name', exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await clearGraphLastReveal(page);
  await result.click();
  await expect.poll(async () => readGraphHighlight(page), { timeout: 5_000 }).not.toBeNull();

  await setEditorContent(page, { sourceText: '' });
  await waitForGraphRendered(page);

  await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 }).toBe(0);
  await expect.poll(async () => readGraphHighlight(page), { timeout: 5_000 }).toBeNull();

  await setEditorContent(page, {
    sourceText: '{"next":{"value":42}}',
    language: 'json',
  });
  await waitForGraphRendered(page);
  await expect.poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 }).toBeGreaterThan(0);
});
