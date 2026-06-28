import { expect, test, type Page } from './fixtures';
import {
  buildGraphTooltipContent,
  clearGraphLastReveal,
  clickGraphProbe,
  getLatestGraphProbes,
  getMonacoRenderedTokenColor,
  installClipboardCapture,
  readEditorState,
  readGraphClickProbes,
  readGraphHoverPreview,
  readGraphLastReveal,
  readClipboardWrites,
  setEditorContent,
  setMonacoPosition,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

const COMMAND_MOD = process.platform === 'darwin' ? 'Meta' : 'Control';

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

async function dispatchWindowShortcut(page: Page, key: string, modifier: 'Meta' | 'Control') {
  await page.evaluate(
    ({ key, modifier }) => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', {
          key,
          bubbles: true,
          metaKey: modifier === 'Meta',
          ctrlKey: modifier === 'Control',
        }),
      );
    },
    { key, modifier },
  );
}

async function readEditorTokenColor(page: Page, tokenText: string, lineNumber?: number): Promise<string | null> {
  return getMonacoRenderedTokenColor(page, 'source-editor', tokenText, lineNumber);
}

async function expectEditorTokenColor(page: Page, tokenText: string, color: string, lineNumber?: number) {
  await expect.poll(() => readEditorTokenColor(page, tokenText, lineNumber), { timeout: 5_000 }).toBe(color);
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

test('command search supports shortcut toggle, execute, and outside-click close', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"object":{"int":42},"table_without_header":["a","b"],"table_with_header":[{"h1":11}],"preview":{"color":"#4f46e5"}}',
    language: 'json',
  });

  await dispatchWindowShortcut(page, 'k', COMMAND_MOD);
  await expect(page.getByText('Format').first()).toBeVisible({ timeout: 5_000 });

  await dispatchWindowShortcut(page, 'k', COMMAND_MOD);
  await expect(page.locator('.command-search-list')).toHaveCount(0);

  const commandInput = page.getByRole('textbox', { name: 'Search command', exact: true });
  await commandInput.press('Enter');
  await expect(page.getByText('Format').first()).toBeVisible({ timeout: 5_000 });
  await commandInput.fill('sort');
  await commandInput.press('Enter');
  await commandInput.press('Enter');

  await expect
    .poll(async () => Object.keys(JSON.parse((await readEditorState(page)).sourceText)).join(','), { timeout: 5_000 })
    .toBe('object,preview,table_with_header,table_without_header');

  await commandInput.press('Enter');
  await expect(page.getByText('Sort').first()).toBeVisible({ timeout: 5_000 });
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
  const input = page.getByRole('textbox', { name: 'Search graph', exact: true });
  await expect(input).toBeVisible({ timeout: 5_000 });
  await expect(page.getByText('No results.').first()).toBeVisible({ timeout: 5_000 });

  await input.fill('Alice');
  const result = page.getByRole('button', { name: 'Graph search result $.user.name', exact: true }).first();
  await expect(result).toBeVisible({ timeout: 5_000 });
  await input.press('Enter');

  await expect
    .poll(async () => (await readEditorState(page)).tempModel.treePath, { timeout: 5_000 })
    .toEqual(expect.arrayContaining(['$', 'user']));
  await page.keyboard.press('Escape');
  await expect(result).toHaveCount(0);
});

test('editor semantic colors keep key and value tokens aligned with current theme', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{\n  "previewEnabled": true,\n  "exampleCount": 42,\n  "emptyValue": null\n}',
    language: 'json',
  });

  await expectEditorTokenColor(page, '"previewEnabled"', 'rgb(163, 21, 21)', 2);
  await expectEditorTokenColor(page, 'true', 'rgb(4, 81, 165)', 2);
  await expectEditorTokenColor(page, '42', 'rgb(9, 134, 88)', 3);
  await expectEditorTokenColor(page, 'null', 'rgb(4, 81, 165)', 4);
});

test('yaml editor semantic colors keep key and value tokens aligned with current theme', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: 'previewEnabled: true\nemptyValue: null\n',
    language: 'yaml',
  });

  await expectEditorTokenColor(page, 'previewEnabled', 'rgb(163, 21, 21)', 1);
  await expectEditorTokenColor(page, 'true', 'rgb(4, 81, 165)', 1);
  await expectEditorTokenColor(page, 'null', 'rgb(4, 81, 165)', 2);
});

test('javascript editor semantic colors keep key and value tokens aligned with current theme', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '({ previewEnabled: true, exampleCount: 42, emptyValue: null })',
    language: 'javascript',
  });

  await expectEditorTokenColor(page, 'previewEnabled', 'rgb(163, 21, 21)');
  await expectEditorTokenColor(page, 'true', 'rgb(4, 81, 165)');
  await expectEditorTokenColor(page, '42', 'rgb(9, 134, 88)');
  await expectEditorTokenColor(page, 'null', 'rgb(4, 81, 165)');
});

test('python editor semantic colors keep key and value tokens aligned with current theme', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"previewEnabled": True, "emptyValue": None}',
    language: 'python',
  });

  await expectEditorTokenColor(page, '"previewEnabled"', 'rgb(163, 21, 21)', 1);
  await expectEditorTokenColor(page, 'True', 'rgb(4, 81, 165)', 1);
  await expectEditorTokenColor(page, 'None', 'rgb(4, 81, 165)', 1);
});


test('graph meta tooltip uses muted path color', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: '{"user":{"name":"Alice","role":"admin"}}',
    language: 'json',
  });
  await waitForGraphRendered(page);

  const tooltipHtml = await buildGraphTooltipContent(page, {
    currentData: { user: { name: 'Alice', role: 'admin' } },
    target: {
      __graphCell: {
        value: '',
        valueType: 'object',
        path: [{ tag: 'KEY', key: 'user', index: 0 }],
      },
      __graphCellKind: 'meta',
    },
    language: 'json',
  });
  const color = await page.evaluate((html) => {
    const host = document.createElement('div');
    host.className = 'leafer-x-tooltip';
    host.innerHTML = html;
    document.body.appendChild(host);
    const node = host.querySelector('.graph-tooltip-meta-path') as HTMLElement | null;
    if (!node) throw new Error('meta tooltip node missing');
    const result = {
      text: node.textContent ?? '',
      color: getComputedStyle(node).color,
    };
    host.remove();
    return result;
  }, tooltipHtml);

  expect(color.text).toBe('$.user');
  expect(color.color).toBe('rgb(107, 114, 128)');
});

test('graph mode resolves tooltip content and supports export image', async ({ page }) => {
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

  const tooltipHtml = await page.evaluate(
    async ({ targetIndex }) => {
      const treease = window._treease;
      if (!treease) throw new Error('window._treease is unavailable');
      const probes = treease.graph.getClickProbeTargets('root') ?? [];
      if (!probes.length) throw new Error('graph runtime unavailable');
      const probe = probes[targetIndex];
      const state = treease.editor.getState();
      return treease.graph.buildTooltipContent({
        currentData: state.treeState?.value ?? null,
        target: { __graphCell: probe?.cell, __graphCellKind: probe?.target },
        language: state.languageId,
      });
    },
    { targetIndex },
  );
  expect(tooltipHtml).toContain('Alice');
  expect(tooltipHtml).toContain('admin');

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

test('graph mode renders no-header array through table runtime with click reveal and no graph hover panel', async ({
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
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);

  await hoverGraphProbe(page, objectProbeIndex);
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);

  await hoverGraphProbe(page, arrayProbeIndex);
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);

  await movePointerOffGraph(page);
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);
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
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);

  await movePointerOffGraph(page);
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 5_000 }).toBeNull();

  await hoverGraphProbe(page, arrayProbeIndex);
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);
});

test('graph mode does not show graph hover panel for non-table array value hover', async ({ page }) => {
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
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);

  await movePointerOffGraph(page);
  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);
});
