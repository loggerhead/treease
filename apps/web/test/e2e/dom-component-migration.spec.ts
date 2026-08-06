import { expect, test, type Page } from './fixtures';
import {
  clearGraphLastReveal,
  clickGraphProbe,
  getLatestGraphProbes,
  getMonacoRenderedTokenColorAtPosition,
  installClipboardCapture,
  readEditorState,
  readGraphClickProbes,
  readGraphHighlightWorld,
  readGraphLastReveal,
  readGraphViewportState,
  readTempGraphSelection,
  readClipboardWrites,
  setEditorContent,
  setMonacoPosition,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

async function settleGraphHover(page: Page) {
  await page.evaluate(
    () => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))),
  );
}

async function hoverGraphProbeAt(page: Page, probe: { x: number; y: number }) {
  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');
  await page.mouse.move(box.x + probe.x, box.y + probe.y);
  await settleGraphHover(page);
}

async function hoverGraphProbe(page: Page, probeIndex = 0) {
  await expect
    .poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThan(probeIndex);
  const probe = (await getLatestGraphProbes(page))[probeIndex];
  if (!probe) throw new Error(`graph probe ${probeIndex} missing`);
  await hoverGraphProbeAt(page, probe);
}

async function movePointerOffGraph(page: Page) {
  const zoomIn = page.getByRole('button', { name: 'Zoom in', exact: true });
  await expect(zoomIn).toBeVisible({ timeout: 5_000 });
  await zoomIn.hover();
}

function resolvePosition(sourceText: string, marker: string): { lineNumber: number; column: number } {
  const offset = sourceText.indexOf(marker);
  if (offset < 0) {
    throw new Error(`Marker "${marker}" not found`);
  }
  const before = sourceText.slice(0, offset);
  const lines = before.split('\n');
  return {
    lineNumber: lines.length,
    column: (lines.at(-1)?.length ?? 0) + 1,
  };
}

async function readEditorTokenColorAtMarker(page: Page, sourceText: string, marker: string): Promise<string | null> {
  const position = resolvePosition(sourceText, marker);
  return getMonacoRenderedTokenColorAtPosition(page, 'source-editor', position.lineNumber, position.column, marker);
}

async function expectEditorTokenColorAtMarker(page: Page, sourceText: string, marker: string, color: string) {
  await expect.poll(() => readEditorTokenColorAtMarker(page, sourceText, marker), { timeout: 5_000 }).toBe(color);
}

test('tree path breadcrumb copies the full path and crumb click reveals parent path', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installClipboardCapture(page);

  await setEditorContent(page, {
    sourceText: '{\n  "user": {\n    "profile-name": {\n      "first": "Alice"\n    }\n  }\n}',
    language: 'json',
  });
  await setMonacoPosition(page, 'source-editor', 4, 8);

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expect.arrayContaining(['$', 'user', 'profile-name', 'first']));

  await page.getByTitle('Copy tree path').click();
  await expect
    .poll(async () => (await readClipboardWrites(page)).at(-1), { timeout: 5_000 })
    .toBe('$.user["profile-name"].first');
});

test('command search supports click open, execute, and outside-click close', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"object":{"int":42},"table_without_header":["a","b"],"table_with_header":[{"h1":11}],"preview":{"color":"#4f46e5"}}',
    language: 'json',
  });

  await page.getByTestId('command-search-button').click();
  const commandList = page.locator('.command-search-list');
  await expect(commandList).toBeVisible({ timeout: 5_000 });
  await expect(commandList.getByText('Format', { exact: true })).toBeVisible({ timeout: 5_000 });
  const commandOptions = commandList.getByRole('option');
  const selectedCommandOptions = commandList.locator('[role="option"][data-selected]');
  const commandInput = page.getByTestId('command-search-input');
  await expect(selectedCommandOptions).toHaveCount(1);
  await expect(commandOptions.first()).toHaveAttribute('data-selected', '');
  await commandInput.press('ArrowUp');
  await expect(selectedCommandOptions).toHaveCount(1);
  await expect(commandOptions.last()).toHaveAttribute('data-selected', '');
  await expect(commandInput).toBeFocused();

  const compactInfo = page.getByRole('option', { name: 'Compact', exact: true }).locator('.ui-tooltip');
  await compactInfo.hover();
  const compactTooltip = page.getByRole('tooltip', {
    name: 'Recursively remove zero-valued object entries and array elements, including null, false, zero, empty strings, empty arrays, and empty objects.',
    exact: true,
  });
  await expect(compactTooltip).toBeVisible();
  await expect.poll(() => compactTooltip.evaluate((tooltip) => {
    let panel = document.querySelector('.command-search-list');
    while (panel && getComputedStyle(panel).zIndex === 'auto') panel = panel.parentElement;
    return Number(getComputedStyle(tooltip).zIndex) > Number(getComputedStyle(panel!).zIndex);
  })).toBe(true);

  await commandInput.fill('sort');
  await commandInput.evaluate((input: HTMLInputElement) => input.setSelectionRange(2, 2));
  await commandInput.press('ArrowLeft');
  await expect.poll(() => commandInput.evaluate((input: HTMLInputElement) => input.selectionStart)).toBe(1);
  await commandInput.press('ArrowRight');
  await expect.poll(() => commandInput.evaluate((input: HTMLInputElement) => input.selectionStart)).toBe(2);
  await expect(commandInput).toBeFocused();
  await expect(commandList.getByText('Sort', { exact: true })).toBeVisible();
  await expect(commandList.getByText('Format', { exact: true })).toHaveCount(0);
  await commandInput.press('Enter');

  await expect
    .poll(async () => Object.keys(JSON.parse((await readEditorState(page)).sourceText)).join(','), { timeout: 5_000 })
    .toBe('object,preview,table_with_header,table_without_header');

  await page.getByTestId('command-search-button').click();
  await expect(commandList).toBeVisible({ timeout: 5_000 });
  await page.locator('main').click({ position: { x: 8, y: 8 } });
  await expect(page.locator('.command-search-list')).toHaveCount(0);
});

test('graph search supports shortcut open, keyboard selection, and escape close', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  const sourceText = '{"user":{"name":"Alice","role":"admin"},"items":[1,2,3]}';
  await setEditorContent(page, {
    sourceText,
    language: 'json',
  });
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe(sourceText);
  await waitForGraphRendered(page);

  await page.getByTestId('graph-search-trigger').click();
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible({ timeout: 5_000 });
  await expect(page.getByText('No results.').first()).toBeVisible({ timeout: 5_000 });

  await input.fill('Alice');
  const result = page.getByRole('option', { name: 'Graph search result $.user.name', exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await input.press('Enter');

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expect.arrayContaining(['$', 'user']));
  await page.keyboard.press('Escape');
  await expect(result).toHaveCount(0);
});

test('graph search loops keyboard selection through results', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  const sourceText = '{"first":"needle","second":"needle","third":"needle","fourth":"needle","fifth":"needle","sixth":"needle","seventh":"needle","eighth":"needle"}';
  await setEditorContent(page, {
    sourceText,
    language: 'json',
  });
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe(sourceText);
  await waitForGraphRendered(page);

  await page.getByTestId('graph-search-trigger').click();
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await input.fill('needle');

  const results = page.getByRole('option', { name: /Graph search result/ });
  await expect.poll(() => results.count(), { timeout: 10_000 }).toBeGreaterThan(1);
  await expect(results.nth(0)).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('[role="option"][aria-selected="true"]')).toHaveCount(1);

  await input.press('ArrowDown');
  await expect(results.nth(1)).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('[role="option"][aria-selected="true"]')).toHaveCount(1);
  await expect(input).toBeFocused();
  await input.evaluate((element: HTMLInputElement) => element.setSelectionRange(3, 3));
  await input.press('ArrowLeft');
  await expect.poll(() => input.evaluate((element: HTMLInputElement) => element.selectionStart)).toBe(2);
  await input.press('ArrowRight');
  await expect.poll(() => input.evaluate((element: HTMLInputElement) => element.selectionStart)).toBe(3);
  await expect.poll(() => readGraphHighlightWorld(page), { timeout: 5_000 }).not.toBeNull();
  await input.press('ArrowUp');
  await expect(results.nth(0)).toHaveAttribute('aria-selected', 'true');
  await input.press('ArrowUp');
  await expect(results.last()).toHaveAttribute('aria-selected', 'true');
  await expect
    .poll(() => page.locator('.graph-search-list').evaluate((element) => element.scrollTop), { timeout: 5_000 })
    .toBeGreaterThan(0);
  await expect.poll(() => readTempGraphSelection(page), { timeout: 5_000 }).not.toBeNull();

  await input.press('Escape');
  await expect.poll(() => readTempGraphSelection(page), { timeout: 5_000 }).toBeNull();
  await expect.poll(() => readGraphHighlightWorld(page), { timeout: 5_000 }).toBeNull();
});

test('graph search hover does not take control of manual result scrolling', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  const sourceText = JSON.stringify(
    Object.fromEntries(Array.from({ length: 20 }, (_, index) => [`field_${index}`, 'needle'])),
  );
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  await page.getByTestId('graph-search-trigger').click();
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await input.fill('needle');
  const results = page.getByRole('option', { name: /Graph search result/ });
  await expect.poll(() => results.count(), { timeout: 10_000 }).toBeGreaterThan(5);

  const list = page.locator('.graph-search-list');
  const manualScrollTop = await list.evaluate((element) => {
    element.scrollTop = element.scrollHeight - element.clientHeight;
    return element.scrollTop;
  });
  expect(manualScrollTop).toBeGreaterThan(0);

  await results.first().evaluate((element) => {
    element.dispatchEvent(new MouseEvent('mouseenter', { bubbles: true }));
  });
  await expect.poll(() => list.evaluate((element) => element.scrollTop)).toBe(manualScrollTop);
  await expect(page.locator('[role="option"][aria-selected="true"]')).toHaveCount(1);
  await expect(results.first()).toHaveAttribute('aria-selected', 'true');

  await input.press('ArrowDown');
  await expect(page.locator('[role="option"][aria-selected="true"]')).toHaveCount(1);
  await expect(results.nth(1)).toHaveAttribute('aria-selected', 'true');
  await expect(results.first()).toHaveAttribute('aria-selected', 'false');
});

test('graph search hover highlights without moving when Navigation sync is off, while click still reveals', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await page.evaluate(async () => {
    const current = window._treease?.settings.getState().settings;
    if (!current) throw new Error('settings bridge unavailable');
    await window._treease.settings.save({
      interaction: { ...current.interaction, enableSyncScroll: false },
    });
  });
  const sourceText = '{"first":"needle","second":"needle","third":"needle","fourth":"needle","fifth":"needle","sixth":"needle","seventh":"needle","eighth":"needle"}';
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  const initialViewport = await readGraphViewportState(page);
  expect(initialViewport).not.toBeNull();
  await page.getByTestId('graph-search-trigger').click();
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await input.fill('needle');
  const results = page.getByRole('option', { name: /Graph search result/ });
  await expect.poll(() => results.count(), { timeout: 10_000 }).toBeGreaterThan(1);
  await expect.poll(() => readGraphHighlightWorld(page), { timeout: 5_000 }).not.toBeNull();
  await expect.poll(() => readGraphViewportState(page), { timeout: 5_000 }).toEqual(initialViewport);

  await results.nth(1).click();
  await expect.poll(() => readGraphHighlightWorld(page), { timeout: 5_000 }).not.toBeNull();
  await expect
    .poll(async () => {
      const current = await readGraphViewportState(page);
      return current && JSON.stringify(current) !== JSON.stringify(initialViewport);
    }, { timeout: 5_000 })
    .toBe(true);
});

test('cancelling graph search restores viewport and graph selection', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  const sourceText = '{"alpha":{"needle":"one"},"beta":{"needle":"two"},"gamma":{"needle":"three"}}';
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  const initialViewport = await readGraphViewportState(page);
  expect(initialViewport).not.toBeNull();
  await page.getByTestId('graph-search-trigger').click();
  const input = page.getByRole('combobox', { name: 'Search graph', exact: true });
  await input.fill('needle');
  const results = page.getByRole('option', { name: /Graph search result/ });
  await expect.poll(() => results.count(), { timeout: 10_000 }).toBeGreaterThan(1);
  await input.press('ArrowDown');

  await expect.poll(() => readTempGraphSelection(page), { timeout: 5_000 }).not.toBeNull();
  await expect
    .poll(async () => {
      const current = await readGraphViewportState(page);
      return current && JSON.stringify(current) !== JSON.stringify(initialViewport);
    }, { timeout: 5_000 })
    .toBe(true);

  await input.press('Escape');
  await expect.poll(() => readTempGraphSelection(page), { timeout: 5_000 }).toBeNull();
  await expect.poll(() => readGraphViewportState(page), { timeout: 5_000 }).toEqual(initialViewport);
});

test('editor semantic colors keep key and value tokens aligned with current theme', async ({ page }) => {
  const sourceText = '{\n  "previewEnabled": true,\n  "exampleCount": 42,\n  "emptyValue": null\n}';
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText,
    language: 'json',
  });

  await expectEditorTokenColorAtMarker(page, sourceText, '"previewEnabled"', 'rgb(163, 21, 21)');
  await expectEditorTokenColorAtMarker(page, sourceText, 'true', 'rgb(4, 81, 165)');
  await expectEditorTokenColorAtMarker(page, sourceText, '42', 'rgb(9, 134, 88)');
  await expectEditorTokenColorAtMarker(page, sourceText, 'null', 'rgb(4, 81, 165)');
});

test('yaml editor semantic colors keep key and value tokens aligned with current theme', async ({ page }) => {
  const sourceText = 'previewEnabled: true\nemptyValue: null\n';
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText,
    language: 'yaml',
  });

  await expectEditorTokenColorAtMarker(page, sourceText, 'previewEnabled', 'rgb(163, 21, 21)');
  await expectEditorTokenColorAtMarker(page, sourceText, 'true', 'rgb(4, 81, 165)');
  await expectEditorTokenColorAtMarker(page, sourceText, 'null', 'rgb(4, 81, 165)');
});

test('javascript editor semantic colors keep key and value tokens aligned with current theme', async ({ page }) => {
  const sourceText = '({ previewEnabled: true, exampleCount: 42, emptyValue: null })';
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText,
    language: 'javascript',
  });

  await expectEditorTokenColorAtMarker(page, sourceText, 'previewEnabled', 'rgb(163, 21, 21)');
  await expectEditorTokenColorAtMarker(page, sourceText, 'true', 'rgb(4, 81, 165)');
  await expectEditorTokenColorAtMarker(page, sourceText, '42', 'rgb(9, 134, 88)');
  await expectEditorTokenColorAtMarker(page, sourceText, 'null', 'rgb(4, 81, 165)');
});

test('python editor semantic colors keep key and value tokens aligned with current theme', async ({ page }) => {
  const sourceText = '{"previewEnabled": True, "emptyValue": None}';
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText,
    language: 'python',
  });

  await expectEditorTokenColorAtMarker(page, sourceText, '"previewEnabled"', 'rgb(163, 21, 21)');
  await expectEditorTokenColorAtMarker(page, sourceText, 'True', 'rgb(4, 81, 165)');
  await expectEditorTokenColorAtMarker(page, sourceText, 'None', 'rgb(4, 81, 165)');
});


test('graph mode supports export image', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"user":{"name":"Alice","role":"admin"}}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  const clickProbes = await readGraphClickProbes(page);
  const targetIndex = clickProbes.findIndex(
    (probe) =>
      probe.nodeType === 'Text' &&
      probe.target === 'value' &&
      probe.path.join('.') === 'user' &&
      (probe.valueType === 'object' || probe.valueType === 'array'),
  );
  expect(targetIndex).toBeGreaterThanOrEqual(0);

  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export image', exact: true }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toMatch(/\.png$/i);
});

test('graph mode does not render empty collections as separate child nodes', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"holder":{"emptyObj":{},"emptyArr":[],"filledObj":{"a":1},"filledArr":[1]}}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  const clickProbes = await readGraphClickProbes(page);
  const valueTexts = clickProbes
    .filter((probe) => probe.target === 'value')
    .map((probe) => probe.text);
  const nodeMetaTexts = clickProbes
    .filter((probe) => probe.target === 'node')
    .map((probe) => probe.text);

  expect(valueTexts).toContain('{}');
  expect(valueTexts).toContain('[]');
  expect(valueTexts).toContain('{1}');
  expect(valueTexts).toContain('[1]');
  expect(nodeMetaTexts).not.toContain('{}');
  expect(nodeMetaTexts).not.toContain('[]');
});

test('graph mode renders no-header array through table runtime with click reveal', async ({
  page,
}) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"items":[1,{"name":"bob"},[3,4]]}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  const clickProbes = await readGraphClickProbes(page);
  expect(
    clickProbes.some(
      (probe) =>
        probe.nodeType === 'Text' &&
        probe.target === 'value' &&
        probe.valueType === 'array' &&
        probe.text === '[3]' &&
        probe.path.join('.') === 'items' &&
        !probe.isTableCell,
    ),
  ).toBe(true);
  expect(clickProbes.some((probe) => probe.isTableCell)).toBe(true);
  const isItemsIndexProbe = (probe: (typeof clickProbes)[number], index: number) =>
    probe.rawPath.length === 2 &&
    probe.rawPath[0]?.key === 'items' &&
    probe.rawPath[1]?.index === index;
  const keyProbeIndex = clickProbes.findIndex(
    (probe) =>
      probe.nodeType === 'Text' &&
      probe.target === 'key' &&
      probe.text === '1' &&
      isItemsIndexProbe(probe, 1),
  );
  const objectProbeIndex = clickProbes.findIndex(
    (probe) =>
      probe.nodeType === 'Text' &&
      probe.target === 'value' &&
      probe.valueType === 'object' &&
      probe.text === '{1}' &&
      isItemsIndexProbe(probe, 1),
  );
  const arrayProbeIndex = clickProbes.findIndex(
    (probe) =>
      probe.nodeType === 'Text' &&
      probe.target === 'value' &&
      probe.valueType === 'array' &&
      probe.text === '[2]' &&
      isItemsIndexProbe(probe, 2),
  );

  expect(keyProbeIndex).toBeGreaterThanOrEqual(0);
  expect(objectProbeIndex).toBeGreaterThanOrEqual(0);
  expect(arrayProbeIndex).toBeGreaterThanOrEqual(0);
  expect(clickProbes[objectProbeIndex]?.isTableCell).toBe(true);
  expect(clickProbes[objectProbeIndex]?.isHeader).toBe(false);
  expect(clickProbes[arrayProbeIndex]?.isTableCell).toBe(true);
  expect(clickProbes[arrayProbeIndex]?.isHeader).toBe(false);

  await clearGraphLastReveal(page);
  await clickGraphProbe(page, keyProbeIndex);
  await expect
    .poll(async () => readGraphLastReveal(page), { timeout: 5_000 })
    .toEqual(
      expect.objectContaining({
        path: ['$', 'items', '[1]'],
        target: 'key',
      }),
    );
  await hoverGraphProbe(page, objectProbeIndex);
  await hoverGraphProbe(page, arrayProbeIndex);
  await movePointerOffGraph(page);
});

test('graph mode does not open hover subgraph for structured cells in scrollable no-header array table', async ({
  page,
}) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  const items = Array.from({ length: 60 }, (_, index) => {
    if (index === 1) return { name: 'bob' };
    if (index === 2) return [3, 4];
    return index;
  });
  await setEditorContent(page, {
    sourceText: JSON.stringify({ items }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const clickProbes = await readGraphClickProbes(page);
  const isItemsIndexProbe = (probe: (typeof clickProbes)[number], index: number) =>
    probe.rawPath.length === 2 &&
    probe.rawPath[0]?.key === 'items' &&
    probe.rawPath[1]?.index === index;
  const objectProbeIndex = clickProbes.findIndex(
    (probe) =>
      probe.nodeType === 'Text' &&
      probe.target === 'value' &&
      probe.valueType === 'object' &&
      probe.text === '{1}' &&
      isItemsIndexProbe(probe, 1),
  );
  const arrayProbeIndex = clickProbes.findIndex(
    (probe) =>
      probe.nodeType === 'Text' &&
      probe.target === 'value' &&
      probe.valueType === 'array' &&
      probe.text === '[2]' &&
      isItemsIndexProbe(probe, 2),
  );

  expect(objectProbeIndex).toBeGreaterThanOrEqual(0);
  expect(arrayProbeIndex).toBeGreaterThanOrEqual(0);

  await hoverGraphProbe(page, objectProbeIndex);
  await movePointerOffGraph(page);
  await hoverGraphProbe(page, arrayProbeIndex);
});

test('graph mode keeps non-table array value hover side-effect free', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText:
      '{"title":"Example","count":42,"ratio":0.125,"active":true,"nothing":null,"tags":["alpha","beta","gamma"],"meta":{"nested":{"id":"item-001","flags":[true,false,true],"scores":[1,2,3,4],"profile":{"name":"demo"}}}}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  const clickProbes = await readGraphClickProbes(page);
  const targetIndex = clickProbes.findIndex(
    (probe) =>
      probe.nodeType === 'Text' &&
      probe.target === 'value' &&
      probe.valueType === 'array' &&
      probe.text === '[3]' &&
      probe.path.join('.') === 'meta.nested.flags',
  );
  expect(targetIndex).toBeGreaterThanOrEqual(0);

  expect(clickProbes[targetIndex]?.isTableCell).toBe(false);

  await hoverGraphProbe(page, targetIndex);
  await movePointerOffGraph(page);
});
