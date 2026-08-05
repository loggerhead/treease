import { readFileSync } from 'node:fs';
import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import {
  clickGraphProbe,
  clickGraphProbeAt,
  clearGraphLastReveal,
  getLatestGraphProbes,
  getMonacoVisibleStartLine,
  installGraphEditEventCapture,
  readEditorState,
  readEditorWorkspace,
  readGraphClickProbes,
  readGraphHighlight,
  readGraphHighlightRect,
  readGraphHighlightWorld,
  readGraphLastReveal,
  readGraphRevealProbe,
  setEditorContent,
  setMonacoPositionByText,
  setMonacoScroll,
  waitForEditorReady,
  waitForGraphRendered,
  waitForColumnNavigatorSettled,
  waitForSettingsReady,
} from './utils';

const trajectoryFixture = readFileSync(
  new URL('../../../../test/fixtures/json/trajectory.1.json', import.meta.url),
  'utf8',
);

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

async function revealGraphSearchResult(page: Page, query: string, resultName: string) {
  await page.getByRole('button', { name: 'Search graph', exact: true }).click();
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill(query);

  const result = page.getByRole('option', { name: resultName, exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await clearGraphLastReveal(page);
  await result.click();
}

async function readActiveSidecarTreePath(page: Page): Promise<string[]> {
  const workspace = await readEditorWorkspace(page);
  const sidecarId = workspace.tabsById[workspace.activeTabId]?.sidecarTabId;
  const path = sidecarId ? workspace.tabsById[sidecarId]?.tempModel.treePath ?? [] : [];
  return [
    '$',
    ...path.map((segment) => typeof segment.key === 'string' ? segment.key : `[${segment.index}]`),
  ];
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
  let revealPayload = null as Awaited<ReturnType<typeof readGraphLastReveal>>;
  for (const probe of clickProbes) {
    if (!probe?.id) continue;
    const expectedProbe = await readGraphRevealProbe(page, probe.id);
    if (!expectedProbe || expectedProbe.path.length <= 1 || expectedProbe.target === 'node') continue;
    // A click can re-render the graph and reorder/remove sibling probes.
    // Resolve the initial stable id again instead of carrying a transient
    // index into the next interaction.
    const currentProbe = (await readGraphClickProbes(page)).find((candidate) => candidate.id === probe.id);
    if (!currentProbe?.coord) continue;
    await clearGraphLastReveal(page);
    await page.evaluate(async (probeId) => {
      await window._treease?.graph.activateProbe(probeId);
    }, currentProbe.id);
    try {
      await expect
        .poll(async () => readActiveSidecarTreePath(page), { timeout: 1_500 })
        .toEqual(expect.arrayContaining(expectedProbe.path));
      revealPayload = (await readGraphLastReveal(page)) ?? expectedProbe;
      if (revealPayload) break;
    } catch {}
  }
  if (!revealPayload) throw new Error('graph reveal payload missing');
  await expect
    .poll(async () => readActiveSidecarTreePath(page), { timeout: 5_000 })
    .toEqual(expect.arrayContaining(revealPayload.path));
  await expect.poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 5_000 }).toBeGreaterThan(0);
});

test('editor scrolling does not create a navigation target', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForSettingsReady(page);
  await page.getByTestId('graph-surface-graph').click();

  const fields = Array.from(
    { length: 240 },
    (_, index) => `  "field_${String(index).padStart(3, '0')}": ${index}`,
  );
  await setEditorContent(page, {
    sourceText: `{\n${fields.join(',\n')}\n}`,
    language: 'json',
  });
  await waitForGraphRendered(page);

  await setMonacoScroll(page, 'source-editor', 1_600);
  const visibleStartLine = await getMonacoVisibleStartLine(page, 'source-editor');
  expect(visibleStartLine).toBeGreaterThan(2);
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 2_000 })
    .toEqual([]);
  await expect(page.getByTestId('graph-bottom-surfaces').getByTestId('tree-path-crumb-1')).toHaveCount(0);
});

test('editor cursor selects the matching Column Navigator path', async ({ page }) => {
  test.setTimeout(30_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForSettingsReady(page);
  await page.evaluate(async () => {
    const current = window._treease?.settings.getState().settings;
    if (!current) throw new Error('settings bridge unavailable');
    await window._treease.settings.save({ interaction: { ...current.interaction, enableSyncScroll: true } });
  });
  await page.getByTestId('graph-surface-graph').click();
  await setEditorContent(page, {
    sourceText: '{\n  "object": {\n    "int": 42,\n    "float": 0.125\n  },\n  "table_with_header": [{ "h1": 11, "h2": 12 }]\n}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  const tableProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.path.join('.') === 'table_with_header' && probe.target === 'key' && probe.coord,
  );
  expect(tableProbe?.coord).toBeTruthy();
  if (!tableProbe?.coord) throw new Error('table_with_header graph cell missing');
  await clickGraphProbeAt(page, tableProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:table_with_header');

  const workspace = page.getByTestId('column-navigator-graph');
  await expect(workspace.locator('[data-column-navigator-selected="true"]')).toHaveAttribute(
    'data-column-navigator-item-path-key',
    'k:table_with_header',
  );

  await page.locator('.view-line', { hasText: '"float": 0.125' }).click();

  await waitForColumnNavigatorSettled(page, 'k:object|k:float');
  await expect(workspace.locator('[data-column-navigator-selected="true"]')).toHaveAttribute(
    'data-column-navigator-item-path-key',
    'k:object|k:float',
  );
  const navigationBar = page.getByTestId('tree-path-bar');
  await expect(navigationBar.getByTestId('tree-path-crumb-1')).toHaveText('object');
  const activeBreadcrumb = navigationBar.getByTestId('tree-path-crumb-2');
  await expect(activeBreadcrumb).toHaveText('float');
});

test('editor cursor movement updates the active navigation breadcrumb', async ({ page }) => {
  test.setTimeout(30_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForSettingsReady(page);
  await page.evaluate(async () => {
    const current = window._treease?.settings.getState().settings;
    if (!current) throw new Error('settings bridge unavailable');
    await window._treease.settings.save({ interaction: { ...current.interaction, enableSyncScroll: true } });
  });
  await page.getByTestId('graph-surface-graph').click();
  await setEditorContent(page, {
    sourceText: '{\n  "first": 1,\n  "second": 2\n}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  const secondLine = page.locator('.view-line', { hasText: '"second": 2' }).first();
  await expect(secondLine).toBeVisible();
  await secondLine.click();
  await expect(page.getByTestId('column-navigator-graph')).toBeVisible();
  const navigationBar = page.getByTestId('tree-path-bar');
  const activeCrumb = navigationBar.getByTestId('tree-path-crumb-1');
  await expect(activeCrumb).toHaveText('second', { timeout: 10_000 });
});

test('Column Navigator selects a deep trajectory node and reveals its editor text', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  await page.getByTestId('graph-surface-graph').click();
  await setEditorContent(page, {
    sourceText: trajectoryFixture,
    language: 'json',
  });
  const trajectoryState = await readEditorState(page);
  await waitForGraphRendered(page, 30_000, {
    documentKey: trajectoryState.documentKey,
    revision: trajectoryState.editorRevision,
  });

  const rootProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'key' && probe.path.join('.') === 'root_step' && probe.coord,
  );
  expect(rootProbe?.coord).toBeTruthy();
  if (!rootProbe?.coord) throw new Error('root_step graph cell missing');
  await clickGraphProbeAt(page, rootProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:root_step', 30_000);

  const workspace = page.getByTestId('column-navigator-graph');
  const pathKeys = [
    'k:root_step|k:output',
    'k:root_step|k:output|k:stream',
    'k:root_step|k:output|k:stream|i:11',
    'k:root_step|k:output|k:stream|i:11|k:extra',
    'k:root_step|k:output|k:stream|i:11|k:extra|k:ark-service-tier',
  ];
  for (const pathKey of pathKeys) {
    const item = workspace.locator(`[data-column-navigator-item-path-key="${pathKey}"]`);
    await expect(item).toBeVisible({ timeout: 10_000 });
    await item.click();
    await waitForColumnNavigatorSettled(page, pathKey, 30_000);
  }

  await expect(workspace.locator('[data-column-navigator-selected="true"]')).toHaveAttribute(
    'data-column-navigator-item-path-key',
    pathKeys.at(-1)!,
  );
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.selectionLength, { timeout: 10_000 })
    .toBeGreaterThan(0);
});

test('graph click highlight survives leaving the hovered value cell', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForGraphRendered(page);

  const boolProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'object.bool' && probe.text === 'true' && probe.coord,
  );
  expect(boolProbe?.coord).toBeTruthy();
  if (!boolProbe?.coord) throw new Error('example JSON boolean graph cell missing');

  await clickGraphProbeAt(page, boolProbe.coord);
  const canvas = page.getByTestId('graph-viewer-canvas');
  const canvasBounds = await canvas.boundingBox();
  if (!canvasBounds) throw new Error('graph canvas bounds missing');
  await page.mouse.move(canvasBounds.x + 20, canvasBounds.y + 20);

  await expect
    .poll(async () => readGraphHighlight(page), { timeout: 5_000 })
    .toEqual(expect.objectContaining({ path: ['$', 'object', 'bool'], target: 'value' }));
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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('item-172');

  const result = page.getByRole('option', { name: 'Graph search result $.rows[172]', exact: true }).first();
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

test('sync scroll toggle gates programmatic editor navigation without creating graph highlights', async ({ page }) => {
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
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(['$', 'user', 'role']);
  await expect.poll(async () => readGraphHighlight(page), { timeout: 5_000 }).toBeNull();

  const navigationSync = page.getByRole('button', { name: 'Navigation sync', exact: true });
  await navigationSync.click();
  await expect(navigationSync).toHaveAttribute('aria-pressed', 'false');

  await setMonacoPositionByText(page, 'source-editor', '"count":');
  await page.waitForTimeout(150);
  expect((await readEditorState(page)).tempModel.treePath).toEqual(['$', 'count']);
  await expect.poll(async () => readGraphHighlight(page), { timeout: 5_000 }).toBeNull();

  const nameProbe = (await readGraphClickProbes(page)).find(
    (probe) => !!probe.coord && probe.target === 'value' && probe.path.join('.') === 'user.name' && probe.text === 'Alice',
  );
  expect(nameProbe?.coord).toBeTruthy();
  if (!nameProbe?.coord) throw new Error('graph probe user.name missing');

  await clickGraphProbeAt(page, nameProbe.coord);
  await page.waitForTimeout(150);
  expect((await readEditorState(page)).tempModel.treePath).toEqual(['$', 'user', 'name']);

  await navigationSync.click();
  await expect(navigationSync).toHaveAttribute('aria-pressed', 'true');

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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('Alice');

  const result = page.getByRole('option', { name: 'Graph search result $.user.name', exact: true }).first();
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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('你好');

  const result = page.getByRole('option', { name: 'Graph search result $.preview.unicode', exact: true }).first();
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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await input.fill('{}');
  await expect(page.getByRole('option', { name: 'Graph search result $.holder.emptyObj', exact: true })).toBeHidden();

  await input.fill('[]');
  await expect(page.getByRole('option', { name: 'Graph search result $.holder.emptyArr', exact: true })).toBeHidden();

  await input.fill('Alice');
  await expect(
    page.getByRole('option', { name: 'Graph search result $.holder.filledObj.name', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });

  await input.fill('item-1');
  await expect(
    page.getByRole('option', { name: 'Graph search result $.holder.filledArr[0]', exact: true }).first(),
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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await input.fill('urisample');
  await expect(
    page
      .getByRole('option', { name: 'Graph search result $.samples.identity.encodedContacts.uriSamples', exact: true })
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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await input.fill('4');
  await expect(
    page.getByRole('option', { name: 'Graph search result $.previewSamples[0]', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });
  await expect(
    page.getByRole('option', { name: 'Graph search result $.exampleCount', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });

  await input.fill('4f');
  await expect(
    page.getByRole('option', { name: 'Graph search result $.previewSamples[0]', exact: true }).first(),
  ).toBeVisible({ timeout: 5_000 });
  await expect(page.getByRole('option', { name: 'Graph search result $.exampleCount', exact: true })).toHaveCount(0);
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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();

  await input.fill('redirect=');
  await expect(
    page.getByRole('option', { name: 'Graph search result $.preview.uris[1]', exact: true }).first(),
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
  await expect(page.getByRole('option', { name: 'Graph search result $.object.int', exact: true }).first()).toBeVisible(
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
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible();
  await input.fill('Alice');
  const result = page.getByRole('option', { name: 'Graph search result $.user.name', exact: true }).first();
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
