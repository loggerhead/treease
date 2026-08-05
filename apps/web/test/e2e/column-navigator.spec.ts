import { readFileSync } from 'node:fs';
import { expect, test } from './fixtures';
import {
  applyMonacoEdits,
  clickGraphProbeAt,
  clickColumnNavigatorProbeAt,
  getMonacoRenderedTokenColor,
  getMonacoValue,
  readEditorState,
  readGraphClickProbes,
  readColumnNavigatorClickProbes,
  setEditorContent,
  setMonacoValue,
  waitForEditorReady,
  waitForEditorRuntimeReady,
  waitForGraphRendered,
  waitForImportSettled,
  waitForColumnNavigatorSettled,
} from './utils';

const promptDiffFixture = readFileSync(
  new URL('../../../../test/fixtures/json/prompt_diff_events.1.json', import.meta.url),
  'utf8',
);

const semanticPalette = {
  map: '#9b1c31',
  key: '#7c2d12',
  seq: '#7e22ce',
  str: '#0369a1',
  int: '#15803d',
  float: '#b45309',
  boolean: '#4338ca',
  nil: '#be123c',
};

test('graph cells and Monaco use the identical Core semantic color across supported languages', async ({ page }, testInfo) => {
  test.setTimeout(30_000);
  // The test app loads the production Turnstile script, which is outside this
  // editor scenario and rejects its invisible test configuration in Chromium.
  testInfo.annotations.push({ type: 'allow-browser-error', description: '[Cloudflare Turnstile]' });
  testInfo.annotations.push({ type: 'allow-browser-error', description: 'challenges.cloudflare.com/cdn-cgi' });
  await page.goto('/editor');
  await waitForEditorReady(page);
  await page.evaluate(async (colors) => {
    const current = window._treease?.settings.getState().settings;
    if (!current) throw new Error('settings bridge unavailable');
    await window._treease.settings.save({ editor: { ...current.editor, semanticTypeColors: colors } });
  }, semanticPalette);

  const cases = [
    { language: 'json', sourceText: '{"value":null}', text: 'null', color: semanticPalette.nil },
    { language: 'yaml', sourceText: 'value: 1.0', text: '1.0', color: semanticPalette.float },
    { language: 'toml', sourceText: 'value = 42', text: '42', color: semanticPalette.int },
    { language: 'python', sourceText: '{"value": None}', text: 'None', color: semanticPalette.nil },
  ] as const;

  for (const entry of cases) {
    await setEditorContent(page, { sourceText: entry.sourceText, language: entry.language });
    await waitForGraphRendered(page);
    const probe = (await readGraphClickProbes(page)).find(
      (candidate) =>
        candidate.target === 'value' &&
        candidate.nodeType === 'Text' &&
        candidate.path.join('.') === 'value' &&
        candidate.coord,
    );
    expect(probe, `${entry.language}: graph value probe`).toBeTruthy();
    expect(probe?.textColor, `${entry.language}: final graph text fill`).toBe(entry.color);
    if (!probe?.coord) throw new Error(`${entry.language}: graph value probe missing`);
    const expectedColor = await page.evaluate((color) => {
      const sample = document.createElement('span');
      sample.style.color = color;
      document.body.append(sample);
      const resolved = getComputedStyle(sample).color;
      sample.remove();
      return resolved;
    }, entry.color);
    await expect
      .poll(() => getMonacoRenderedTokenColor(page, 'source-editor', entry.text), { timeout: 5_000 })
      .toBe(expectedColor);

    await clickGraphProbeAt(page, probe.coord);
    await waitForColumnNavigatorSettled(page, 'k:value');
    const hookId = 'column-navigator-content:k:value';
    await expect
      .poll(() => getMonacoRenderedTokenColor(page, hookId, entry.text), { timeout: 5_000 })
      .toBe(expectedColor);
  }
});

test('structured content is an editable main-editor projection without reformatting source', async ({ page }) => {
  test.setTimeout(30_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  const sourceText = `{
  "object": {
    "int": 42,
    "bool": true,
    "arr0": [],
    "obj0": {}
  },
  "table_without_header": ["a", "b", "c"]
}`;
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  const objectProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'object' && probe.coord,
  );
  expect(objectProbe).toBeTruthy();
  if (!objectProbe?.coord) throw new Error('object probe missing');
  await clickGraphProbeAt(page, objectProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:object');

  const hookId = 'column-navigator-content:k:object';
  await expect.poll(() => getMonacoValue(page, hookId), { timeout: 5_000 }).toBe(`{
    "int": 42,
    "bool": true,
    "arr0": [],
    "obj0": {}
  }`);
  await expect
    .poll(() => getMonacoRenderedTokenColor(page, 'source-editor', '"int"', 3), { timeout: 5_000 })
    .not.toBe('rgb(15, 23, 42)');
  const mainKeyColor = await getMonacoRenderedTokenColor(page, 'source-editor', '"int"', 3);
  await expect.poll(() => getMonacoRenderedTokenColor(page, hookId, '"int"', 2), { timeout: 5_000 }).toBe(mainKeyColor);
  await expect
    .poll(() => getMonacoRenderedTokenColor(page, 'source-editor', '42', 3), { timeout: 5_000 })
    .not.toBe('rgb(15, 23, 42)');
  const mainValueColor = await getMonacoRenderedTokenColor(page, 'source-editor', '42', 3);
  await expect.poll(() => getMonacoRenderedTokenColor(page, hookId, '42', 2), { timeout: 5_000 }).toBe(mainValueColor);

  const workspace = page.getByTestId('column-navigator-graph');
  const longKey = workspace.locator('[data-column-navigator-item-path-key="k:table_without_header"]');
  await expect(longKey).toHaveAttribute('data-column-navigator-item-preview', '[3]');
  expect(await longKey.locator('.column-navigator-item__label').evaluate((element) => element.getBoundingClientRect().width))
    .toBeGreaterThan(0);
  await expect(workspace.locator('[data-column-navigator-item-path-key="k:object|k:arr0"]')).toHaveAttribute(
    'data-column-navigator-item-preview',
    '[]',
  );
  await expect(workspace.locator('[data-column-navigator-item-path-key="k:object|k:obj0"]')).toHaveAttribute(
    'data-column-navigator-item-preview',
    '{}',
  );
  await expect
    .poll(() =>
      longKey.locator('.column-navigator-item__preview').evaluate((element) => getComputedStyle(element).color),
    )
    .toBe('rgb(93, 114, 130)');

  await applyMonacoEdits(page, hookId, [
    {
      range: { startLineNumber: 2, startColumn: 12, endLineNumber: 2, endColumn: 14 },
      text: '43',
    },
  ]);
  const expectedText = sourceText.replace('"int": 42', '"int": 43');
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe(expectedText);
});

function parseSourceText(sourceText: string): any {
  return JSON.parse(sourceText);
}

test('column navigator column detail editor uses monaco editor and syncs edits back to editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: JSON.stringify({
      user: { name: 'Alice' },
      rows: [{ title: 'one', done: false }],
    }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const probes = await readGraphClickProbes(page);
  const keyProbe = probes.find(
    (probe) => probe.target === 'key' && probe.path.join('.') === 'user.name' && probe.text === 'name' && probe.coord,
  );
  expect(keyProbe).toBeTruthy();
  if (!keyProbe?.coord) throw new Error('user.name key probe missing');

  await clickGraphProbeAt(page, keyProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:user|k:name');

  const workspace = page.getByTestId('column-navigator-graph');
  const contentPane = workspace.getByTestId('column-navigator-content-pane');
  const pane = contentPane.locator('..');
  await expect(workspace).toBeVisible();
  await expect(contentPane).toBeVisible();
  await expect(pane).toHaveAttribute('data-column-navigator-content-path-key', 'k:user|k:name');
  await expect(pane.getByTestId('column-navigator-key-input')).toHaveCount(0);
  const monacoHost = pane.getByTestId('column-navigator-monaco-editor');
  await expect(monacoHost).toBeVisible();
  await expect
    .poll(async () => getMonacoValue(page, 'column-navigator-content:k:user|k:name'), { timeout: 5_000 })
    .toBe('"Alice"');

  await monacoHost.click();
  await setMonacoValue(page, 'column-navigator-content:k:user|k:name', 'Bob');

  await expect
    .poll(async () => parseSourceText((await readEditorState(page)).sourceText), { timeout: 5_000 })
    .toMatchObject({
      user: { name: 'Bob' },
      rows: [{ title: 'one', done: false }],
    });
  await page.getByTestId('monaco-source-editor').click();
  await waitForImportSettled(page);
  await waitForGraphRendered(page);

  await workspace.locator('[data-column-navigator-item-path-key="k:rows"]').click();
  await expect(workspace.locator('[data-column-navigator-path-key="k:rows"]')).toBeVisible();
  await workspace.locator('[data-column-navigator-item-path-key="k:rows|i:0"]').click();
  await expect(workspace.locator('[data-column-navigator-path-key="k:rows|i:0"]')).toBeVisible();

  const rowPane = workspace.locator('[data-column-navigator-path-key="k:rows|i:0"]');
  await expect(rowPane.locator('.column-navigator-pane__header')).toHaveCount(0);
  await expect(rowPane.locator('.column-navigator-pane__items')).toBeVisible();
  await expect(workspace.locator('[data-column-navigator-content-path-key="k:rows|i:0"]')).toBeVisible();
  await expect
    .poll(async () => getMonacoValue(page, 'column-navigator-content:k:rows|i:0'), { timeout: 5_000 })
    .toBe('{"title":"one","done":false}');
});

test('column navigator detail editor resizes and collapses at the right edge', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: JSON.stringify({ user: { name: 'Alice' } }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const probe = (await readGraphClickProbes(page)).find(
    (candidate) => candidate.target === 'value' && candidate.path.join('.') === 'user.name' && candidate.coord,
  );
  expect(probe).toBeTruthy();
  if (!probe?.coord) throw new Error('user.name probe missing');
  await clickGraphProbeAt(page, probe.coord);
  await waitForColumnNavigatorSettled(page, 'k:user|k:name');

  const workspace = page.getByTestId('column-navigator-graph');
  const divider = page.getByTestId('column-navigator-detail-divider');
  const columns = workspace.locator('.column-navigator-graph__track');
  const trailingSpace = page.getByTestId('column-navigator-trailing-space');
  const detail = workspace.locator('.column-navigator-detail');
  const initialWidth = (await detail.boundingBox())?.width ?? 0;
  const dividerBox = await divider.boundingBox();
  const workspaceBox = await workspace.boundingBox();
  expect(dividerBox).toBeTruthy();
  expect(workspaceBox).toBeTruthy();
  if (!dividerBox || !workspaceBox) throw new Error('column navigator detail drag geometry missing');
  await expect.poll(async () => {
    const trailingSpaceBox = await trailingSpace.boundingBox();
    return Math.round(trailingSpaceBox?.width ?? 0);
  }).toBe(Math.round(workspaceBox.width - 288));
  await expect.poll(async () => {
    const columnsBox = await columns.boundingBox();
    return Math.round((columnsBox?.width ?? 0) - workspaceBox.width);
  }).toBe(0);

  await page.mouse.move(dividerBox.x + dividerBox.width / 2, dividerBox.y + dividerBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(workspaceBox.x + workspaceBox.width - 2, dividerBox.y + dividerBox.height / 2);
  await page.mouse.up();

  await expect(detail).toHaveClass(/column-navigator-detail--collapsed/);
  const expand = page.getByTestId('column-navigator-detail-expand');
  await expect(expand).toBeVisible();
  expect((await detail.boundingBox())?.width ?? 0).toBeLessThanOrEqual(1);
  expect((await detail.boundingBox())?.width).toBeLessThan(initialWidth);
  await expect.poll(async () => {
    const expandBox = await expand.boundingBox();
    const dividerMetrics = await divider.evaluate((element) => ({
      lineRight: getComputedStyle(element, '::after').right,
      lineLeft: getComputedStyle(element, '::after').left,
    }));
    return {
      expandRightInset: Math.round((workspaceBox.x + workspaceBox.width) - (expandBox?.x ?? 0) - (expandBox?.width ?? 0)),
      ...dividerMetrics,
    };
  }).toEqual({ expandRightInset: 10, lineRight: '0px', lineLeft: '8px' });

  await expand.click();
  await expect(detail).not.toHaveClass(/column-navigator-detail--collapsed/);
  await expect.poll(async () => (await detail.boundingBox())?.width ?? 0).toBeGreaterThan(200);

  const restoredDividerBox = await divider.boundingBox();
  if (!restoredDividerBox) throw new Error('column navigator detail divider missing after expansion');
  await page.mouse.move(restoredDividerBox.x + restoredDividerBox.width / 2, restoredDividerBox.y + restoredDividerBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(workspaceBox.x + 2, restoredDividerBox.y + restoredDividerBox.height / 2);
  await page.mouse.up();

  const expandColumns = page.getByRole('button', { name: 'Expand column navigator columns', exact: true });
  await expect(expandColumns).toBeVisible();
  await expandColumns.click();
  await expect(expandColumns).toHaveCount(0);
});

test('prompt diff lane_name detail edit repaints only the affected graph region', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, { sourceText: promptDiffFixture, language: 'json' });
  await waitForGraphRendered(page, 30_000);

  const rootItem = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.length === 1 && probe.path[0] === '[1]' && probe.coord,
  );
  expect(rootItem, 'foo root array item probe').toBeTruthy();
  if (!rootItem?.coord) throw new Error('foo root array item probe is missing a coordinate');
  await clickGraphProbeAt(page, rootItem.coord);
  await waitForColumnNavigatorSettled(page, 'i:1', 30_000);

  const workspace = page.getByTestId('column-navigator-graph');
  await workspace.locator('[data-column-navigator-item-path-key="i:1|k:value"]').click();
  await waitForColumnNavigatorSettled(page, 'i:1|k:value', 30_000);
  await workspace.locator('[data-column-navigator-item-path-key="i:1|k:value|k:lane_name"]').click();
  await waitForColumnNavigatorSettled(page, 'i:1|k:value|k:lane_name', 30_000);

  const hookId = 'column-navigator-content:i:1|k:value|k:lane_name';
  await expect.poll(() => getMonacoValue(page, hookId), { timeout: 10_000 }).toBe('"zff_fb_jm_nzta"');

  await page.evaluate(() => {
    const refs = window._treease?.graph.refs as { leafer?: { forceRender?: (...args: unknown[]) => void } } | undefined;
    const leafer = refs?.leafer;
    if (!leafer?.forceRender) throw new Error('Leafer forceRender is unavailable');
    const original = leafer.forceRender.bind(leafer);
    const calls: unknown[] = [];
    leafer.forceRender = (...args: unknown[]) => {
      calls.push(args[0] ?? null);
      original(...args);
    };
    Object.assign(window, { __treeaseForceRenderCalls: calls });
  });

  await setMonacoValue(page, hookId, 'ppe_test_next');
  await expect
    .poll(async () => {
      const source = JSON.parse((await readEditorState(page)).sourceText);
      return source[1]?.value?.lane_name;
    }, { timeout: 30_000 })
    .toBe('ppe_test_next');
  await waitForGraphRendered(page, 30_000);

  const renderState = await page.evaluate(() => {
    const graph = window._treease?.graph.getLastGraphData();
    const nodes = (graph?.nodes ?? []) as Array<{ boxArgs: { x: number; y: number; width: number; height: number } }>;
    const graphBounds = nodes.reduce(
      (bounds, node) => ({
        left: Math.min(bounds.left, node.boxArgs.x),
        top: Math.min(bounds.top, node.boxArgs.y),
        right: Math.max(bounds.right, node.boxArgs.x + node.boxArgs.width),
        bottom: Math.max(bounds.bottom, node.boxArgs.y + node.boxArgs.height),
      }),
      { left: Number.POSITIVE_INFINITY, top: Number.POSITIVE_INFINITY, right: Number.NEGATIVE_INFINITY, bottom: Number.NEGATIVE_INFINITY },
    );
    return {
      calls: (window as unknown as { __treeaseForceRenderCalls?: unknown[] }).__treeaseForceRenderCalls ?? [],
      readiness: window._treease?.graph.getRuntimeReadiness(),
      graphBounds,
    };
  });
  expect(renderState.readiness?.graph.settled).toBe(true);
  const dirtyBounds = renderState.calls.filter(
    (bounds): bounds is { left: number; top: number; width: number; height: number } =>
      bounds != null && typeof bounds === 'object',
  );
  expect(dirtyBounds.length).toBeGreaterThan(0);
  expect(
    dirtyBounds.some(
      (bounds) =>
        bounds.width < renderState.graphBounds.right - renderState.graphBounds.left ||
        bounds.height < renderState.graphBounds.bottom - renderState.graphBounds.top,
    ),
  ).toBe(true);
});

test('column navigator highlights null roots and keeps string editing caret stable', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: JSON.stringify({
      object: { nil: null },
      user: { name: 'A' },
    }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const probes = await readGraphClickProbes(page);
  const nilProbe = probes.find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'object.nil' && probe.coord,
  );
  expect(nilProbe).toBeTruthy();
  if (!nilProbe?.coord) throw new Error('object.nil probe missing');

  // The assertion covers Graph navigation → Column Navigator. The Leafer
  // canvas has no stable DOM hit target after its first pane reflow, so invoke
  // the registered Graph probe instead of coupling this data-flow test to a
  // transformed client coordinate.
  await page.evaluate(async (probeId) => {
    await window._treease?.graph.activateProbe(probeId);
  }, nilProbe.id);
  await waitForColumnNavigatorSettled(page, 'k:object|k:nil');

  await expect
    .poll(async () => getMonacoValue(page, 'column-navigator-content:k:object|k:nil'), { timeout: 5_000 })
    .toBe('null');

  const refreshedProbes = await readGraphClickProbes(page);
  const nameProbe = refreshedProbes.find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'user.name' && probe.coord,
  );
  expect(nameProbe).toBeTruthy();
  if (!nameProbe?.coord) throw new Error('user.name probe missing');

  await page.evaluate(async (probeId) => {
    await window._treease?.graph.activateProbe(probeId);
  }, nameProbe.id);
  await waitForColumnNavigatorSettled(page, 'k:user|k:name');

  await applyMonacoEdits(page, 'column-navigator-content:k:user|k:name', [
    {
      range: { startLineNumber: 1, startColumn: 3, endLineNumber: 1, endColumn: 3 },
      text: 'B',
    },
  ]);
  await applyMonacoEdits(page, 'column-navigator-content:k:user|k:name', [
    {
      range: { startLineNumber: 1, startColumn: 4, endLineNumber: 1, endColumn: 4 },
      text: 'C',
    },
  ]);

  await expect
    .poll(async () => getMonacoValue(page, 'column-navigator-content:k:user|k:name'), { timeout: 5_000 })
    .toBe('"ABC"');
  await expect
    .poll(async () => parseSourceText((await readEditorState(page)).sourceText).user.name, { timeout: 5_000 })
    .toBe('ABC');
});

test('column navigator rebases nested click paths before opening the next pane', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setEditorContent(page, {
    sourceText: JSON.stringify({
      preview: {
        uris: ['https://a.example.com', 'https://treease.com/path?redirect=1'],
      },
      object: {
        int: 42,
        float: 0.125,
        bool: true,
        nil: null,
        arr0: [],
        obj0: {},
      },
    }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const rootProbes = await readGraphClickProbes(page);
  const urisProbe = rootProbes.find(
    (probe) => probe.target === 'value' && probe.valueType === 'array' && probe.path.join('.') === 'preview.uris' && probe.coord,
  );
  expect(urisProbe).toBeTruthy();
  if (!urisProbe?.coord) throw new Error('preview.uris probe missing');

  await clickGraphProbeAt(page, urisProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:preview|k:uris');

  const workspaceProbes = await readColumnNavigatorClickProbes(page);
  const uriItemProbe = workspaceProbes.find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'preview.uris.[1]' && probe.coord,
  );
  expect(uriItemProbe, JSON.stringify(workspaceProbes, null, 2)).toBeTruthy();
  if (!uriItemProbe?.coord) throw new Error('preview.uris[1] workspace probe missing');

  await clickColumnNavigatorProbeAt(page, uriItemProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:preview|k:uris|i:1');

  const indexItem = page.locator('[data-column-navigator-item-index="true"]').first();
  await expect(indexItem.locator('.column-navigator-item__dot')).toHaveCount(1);
  await expect(indexItem.locator('svg')).toHaveCount(0);

  await expect(page.locator('[data-column-navigator-item-path-key="k:preview"]')).toHaveAttribute(
    'data-column-navigator-path-ancestor',
    'true',
  );
  await expect(page.locator('[data-column-navigator-item-path-key="k:preview|k:uris"]')).toHaveAttribute(
    'data-column-navigator-path-ancestor',
    'true',
  );

  const panes = page.getByTestId('column-navigator-pane');
  await expect(panes).toHaveCount(4);
  await expect(panes.nth(3)).toHaveAttribute('data-column-navigator-content-path-key', 'k:preview|k:uris|i:1');
  await expect
    .poll(async () => getMonacoValue(page, 'column-navigator-content:k:preview|k:uris|i:1'), { timeout: 5_000 })
    .toBe('"https://treease.com/path?redirect=1"');
  await expect(page.getByText('Reveal failed')).toHaveCount(0);
});

test('workspace keyboard navigation is focus-bounded and Monaco keeps arrow keys', async ({ page }) => {
  test.setTimeout(30_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: JSON.stringify({
      root: {
        first: { leaf: 'one' },
        second: { leaf: 'two' },
      },
      outside: true,
    }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const rootProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'root' && probe.coord,
  );
  expect(rootProbe).toBeTruthy();
  if (!rootProbe?.coord) throw new Error('root probe missing');
  await clickGraphProbeAt(page, rootProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:root', 20_000);

  const workspace = page.getByTestId('column-navigator-graph');
  await workspace.focus();
  await page.keyboard.press('ArrowRight');
  await waitForColumnNavigatorSettled(page, 'k:root|k:first', 20_000);
  await expect(workspace.locator('[data-column-navigator-item-path-key="k:root|k:first"]')).toHaveAttribute('aria-pressed', 'true');

  await page.keyboard.press('ArrowDown');
  await waitForColumnNavigatorSettled(page, 'k:root|k:second', 20_000);
  await expect(workspace.locator('[data-column-navigator-item-path-key="k:root|k:second"]')).toHaveAttribute('aria-pressed', 'true');

  await page.getByTestId('monaco-source-editor').click();
  await page.keyboard.press('ArrowUp');
  await expect(workspace.locator('[data-column-navigator-item-path-key="k:root|k:second"]')).toHaveAttribute('aria-pressed', 'true');

  await workspace.locator('[data-column-navigator-item-path-key="k:root|k:second|k:leaf"]').click();
  await waitForColumnNavigatorSettled(page, 'k:root|k:second|k:leaf', 20_000);
  const detailMonaco = workspace.getByTestId('column-navigator-monaco-editor');
  await detailMonaco.click();
  await page.keyboard.press('ArrowLeft');
  await expect
    .poll(() => getMonacoValue(page, 'column-navigator-content:k:root|k:second|k:leaf'))
    .toBe('"two"');
  await expect(workspace.locator('[data-column-navigator-selected="true"]')).toHaveAttribute(
    'data-column-navigator-item-path-key',
    'k:root|k:second|k:leaf',
  );
});

test('returning to a parent collapses its child preview with one rightward rail movement', async ({ page }) => {
  test.setTimeout(30_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: JSON.stringify({
      root: {
        branch: {
          child: {
            grandchild: { leaf: 'value' },
          },
        },
      },
    }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const rootProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'root' && probe.coord,
  );
  expect(rootProbe).toBeTruthy();
  if (!rootProbe?.coord) throw new Error('root probe missing');
  await clickGraphProbeAt(page, rootProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:root', 20_000);

  const workspace = page.getByTestId('column-navigator-graph');
  await workspace.focus();
  for (const pathKey of [
    'k:root|k:branch',
    'k:root|k:branch|k:child',
    'k:root|k:branch|k:child|k:grandchild',
  ]) {
    await page.keyboard.press('ArrowRight');
    await waitForColumnNavigatorSettled(page, pathKey, 20_000);
  }

  const rail = workspace.locator('.column-navigator-graph__track');
  const before = await rail.evaluate((element) => element.scrollLeft);
  await rail.evaluate((element) => {
    element.dataset.previewScrollEvents = '0';
    element.addEventListener('scroll', () => {
      element.dataset.previewScrollEvents = String(
        Number(element.dataset.previewScrollEvents ?? '0') + 1,
      );
    });
  });
  await page.keyboard.press('ArrowLeft');
  await waitForColumnNavigatorSettled(page, 'k:root|k:branch|k:child', 20_000);
  const result = await workspace.evaluate((root) => {
    const railElement = root.querySelector<HTMLElement>('.column-navigator-graph__track')!;
    const activeColumn = root.querySelector<HTMLElement>(
      '[data-column-navigator-item-path-key="k:root|k:branch|k:child"]',
    )!.closest<HTMLElement>('[data-testid="column-navigator-pane"]')!;
    const visibleRight = root.querySelector<HTMLElement>('.column-navigator-detail-divider')?.offsetLeft
      ?? railElement.clientWidth;
    const visibleLeft = railElement.scrollLeft;
    const activeLeft = activeColumn.offsetLeft;
    const activeRight = activeLeft + activeColumn.offsetWidth;
    return {
      scrollLeft: railElement.scrollLeft,
      scrollEvents: Number(railElement.dataset.previewScrollEvents ?? '0'),
      expectedActiveVisibleWidth: Math.min(activeColumn.offsetWidth, visibleRight / 2),
      activeVisibleWidth: Math.min(activeRight, visibleLeft + visibleRight) - Math.max(activeLeft, visibleLeft),
    };
  });

  expect(result.scrollLeft).toBeLessThan(before);
  expect(result.scrollEvents).toBeLessThanOrEqual(1);
  expect(result.activeVisibleWidth).toBeGreaterThan(0);
  expect(result.activeVisibleWidth).toBeCloseTo(result.expectedActiveVisibleWidth, 0);
});

test('sibling preview changes keep the rail stable after the level uses its scroll opportunity', async ({ page }) => {
  test.setTimeout(30_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: JSON.stringify({
      root: {
        container: { child: { leaf: 'value' } },
        scalar: 1,
      },
    }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const rootProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'root' && probe.coord,
  );
  expect(rootProbe).toBeTruthy();
  if (!rootProbe?.coord) throw new Error('root probe missing');
  await clickGraphProbeAt(page, rootProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:root', 20_000);

  const workspace = page.getByTestId('column-navigator-graph');
  const rail = workspace.locator('.column-navigator-graph__track');
  await workspace.focus();
  const beforeExpansion = await rail.evaluate((element) => {
    element.dataset.previewScrollEvents = '0';
    element.addEventListener('scroll', () => {
      element.dataset.previewScrollEvents = String(
        Number(element.dataset.previewScrollEvents ?? '0') + 1,
      );
    });
    return element.scrollLeft;
  });
  await page.keyboard.press('ArrowRight');
  await waitForColumnNavigatorSettled(page, 'k:root|k:container', 20_000);
  const afterExpansion = await rail.evaluate((element) => element.scrollLeft);
  const expansion = await workspace.evaluate((root) => {
    const railElement = root.querySelector<HTMLElement>('.column-navigator-graph__track')!;
    const activeColumn = root.querySelector<HTMLElement>(
      '[data-column-navigator-path-key="k:root"]',
    )!;
    const previewColumn = root.querySelector<HTMLElement>(
      '[data-column-navigator-path-key="k:root|k:container"]',
    )!;
    const visibleWidth = root.querySelector<HTMLElement>('.column-navigator-detail-divider')?.offsetLeft
      ?? railElement.clientWidth;
    const visibleLeft = railElement.scrollLeft;
    const visibleRight = visibleLeft + visibleWidth;
    const visiblePart = (element: HTMLElement) => {
      const left = element.offsetLeft;
      const right = left + element.offsetWidth;
      return Math.min(right, visibleRight) - Math.max(left, visibleLeft);
    };
    return {
      scrollEvents: Number(railElement.dataset.previewScrollEvents ?? '0'),
      activeVisibleWidth: visiblePart(activeColumn),
      previewVisibleWidth: visiblePart(previewColumn),
      expectedPreviewVisibleWidth: Math.min(previewColumn.offsetWidth, visibleWidth / 2),
    };
  });
  expect(afterExpansion).toBeGreaterThanOrEqual(beforeExpansion);
  expect(expansion.scrollEvents).toBeLessThanOrEqual(1);
  expect(expansion.activeVisibleWidth).toBeGreaterThan(0);
  expect(expansion.previewVisibleWidth).toBeCloseTo(expansion.expectedPreviewVisibleWidth, 0);

  await page.keyboard.press('ArrowDown');
  await waitForColumnNavigatorSettled(page, 'k:root|k:scalar', 20_000);
  expect(await rail.evaluate((element) => element.scrollLeft)).toBe(afterExpansion);

  await page.keyboard.press('ArrowUp');
  await waitForColumnNavigatorSettled(page, 'k:root|k:container', 20_000);
  expect(await rail.evaluate((element) => element.scrollLeft)).toBe(afterExpansion);
});

test('column navigator introduces its keyboard controls and supports history shortcuts', async ({ page }) => {
  test.setTimeout(30_000);
  await page.addInitScript(() => localStorage.removeItem('treease:column-navigator-keyboard-hint-seen'));
  await page.goto('/editor');
  await waitForEditorRuntimeReady(page);
  await setEditorContent(page, {
    sourceText: JSON.stringify({ root: { first: { leaf: 'one' } } }),
    language: 'json',
  });
  await waitForGraphRendered(page);

  const rootProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'root' && probe.coord,
  );
  expect(rootProbe).toBeTruthy();
  if (!rootProbe?.coord) throw new Error('root probe missing');

  await clickGraphProbeAt(page, rootProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:root', 20_000);

  const workspace = page.getByTestId('column-navigator-graph');
  const hint = page.getByTestId('column-navigator-keyboard-hint');
  await expect(hint).toBeVisible();
  await expect(hint).toContainText('Browse nodes with');
  await expect(hint).toContainText('move through history');

  await workspace.focus();
  await page.keyboard.press('ArrowRight');
  await waitForColumnNavigatorSettled(page, 'k:root|k:first', 20_000);
  await expect(hint).toHaveCount(1);

  await page.keyboard.press('[');
  await waitForColumnNavigatorSettled(page, 'k:root', 20_000);
  await page.keyboard.press(']');
  await waitForColumnNavigatorSettled(page, 'k:root|k:first', 20_000);

  await page.getByTestId('column-navigator-collapse').click();
  await page.getByTestId('graph-viewer-dropzone').focus();
  await page.keyboard.press('[');
  await waitForColumnNavigatorSettled(page, 'k:root', 20_000);
  await expect(page.getByTestId('column-navigator-graph')).toHaveCount(0);

  await expect.poll(() => page.evaluate(() => localStorage.getItem('treease:column-navigator-keyboard-hint-seen'))).toBe('1');
});

test('full active paths remain horizontally browsable with independent native column scrolling', async ({ page }) => {
  test.setTimeout(30_000);
  await page.goto('/editor');
  await waitForEditorReady(page);
  const deepValue = {
    level1: {
      level2: {
        level3: {
          level4: {
            level5: {
              level6: Array.from({ length: 40 }, (_, index) => ({ index, value: `row-${index}` })),
            },
          },
        },
      },
    },
  };
  await setEditorContent(page, { sourceText: JSON.stringify(deepValue), language: 'json' });
  await waitForGraphRendered(page);

  const rootProbe = (await readGraphClickProbes(page)).find(
    (probe) => probe.path.join('.') === 'level1' && probe.target === 'value' && probe.coord,
  );
  expect(rootProbe).toBeTruthy();
  if (!rootProbe?.coord) throw new Error('deep root probe missing');
  await clickGraphProbeAt(page, rootProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:level1', 20_000);

  const workspace = page.getByTestId('column-navigator-graph');
  const pathKeys = [
    'k:level1|k:level2',
    'k:level1|k:level2|k:level3',
    'k:level1|k:level2|k:level3|k:level4',
    'k:level1|k:level2|k:level3|k:level4|k:level5',
    'k:level1|k:level2|k:level3|k:level4|k:level5|k:level6',
    'k:level1|k:level2|k:level3|k:level4|k:level5|k:level6|i:39',
    'k:level1|k:level2|k:level3|k:level4|k:level5|k:level6|i:39|k:value',
  ];
  for (const nextPathKey of pathKeys) {
    await workspace.locator(`[data-column-navigator-item-path-key="${nextPathKey}"]`).click();
    await waitForColumnNavigatorSettled(page, nextPathKey, 20_000);
  }
  await waitForColumnNavigatorSettled(
    page,
    'k:level1|k:level2|k:level3|k:level4|k:level5|k:level6|i:39|k:value',
    20_000,
  );

  const metrics = await workspace.evaluate((root) => {
    const rail = root.querySelector<HTMLElement>('.column-navigator-graph__track');
    const columns = [...root.querySelectorAll<HTMLElement>('.column-navigator-pane')];
    const detail = root.querySelector<HTMLElement>('.column-navigator-detail');
    return {
      railOverflowX: rail ? getComputedStyle(rail).overflowX : '',
      railScrollWidth: rail?.scrollWidth ?? 0,
      railClientWidth: rail?.clientWidth ?? 0,
      railScrollLeft: rail?.scrollLeft ?? 0,
      columnWidths: columns.map((column) => column.getBoundingClientRect().width),
      columnOverflowY: columns.map((column) =>
        getComputedStyle(column.querySelector<HTMLElement>('.column-navigator-pane__items')!).overflowY),
      detailOverflow: detail ? getComputedStyle(detail).overflow : '',
    };
  });

  expect(metrics.railOverflowX).toBe('auto');
  expect(metrics.railScrollWidth).toBeGreaterThan(metrics.railClientWidth);
  expect(metrics.railScrollLeft).toBeGreaterThan(0);
  expect(metrics.columnWidths.every((width) => Math.round(width) === 288)).toBe(true);
  expect(metrics.columnOverflowY.every((overflow) => overflow === 'auto')).toBe(true);
});

test('column navigator can close and reopen without losing the main document view', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, {
    sourceText: JSON.stringify({ user: { name: 'Alice' }, keep: true }),
    language: 'json',
  });
  await waitForGraphRendered(page);
  await expect(page.getByTestId('graph-bottom-surfaces')).toBeVisible();

  const probe = (await readGraphClickProbes(page)).find(
    (candidate) => candidate.target === 'value' && candidate.path.join('.') === 'user' && candidate.coord,
  );
  expect(probe).toBeTruthy();
  if (!probe?.coord) throw new Error('user object probe missing');
  await clickGraphProbeAt(page, probe.coord);
  await waitForColumnNavigatorSettled(page, 'k:user');

  const navigator = page.getByTestId('column-navigator-graph');
  await expect(navigator).toBeVisible();
  const collapseButton = page.getByTestId('column-navigator-collapse');
  await collapseButton.click();
  await expect(navigator).toHaveCount(0);
  await expect(page.getByTestId('graph-viewer-canvas')).toBeVisible();
  await expect(page.getByTestId('graph-bottom-surfaces')).toBeVisible();
  const expandButton = page.getByTestId('column-navigator-expand');
  const backButton = page.getByTestId('column-navigator-back');
  const forwardButton = page.getByTestId('column-navigator-forward');
  await expect(expandButton).toBeVisible();
  await expect(page.getByTestId('column-navigator-pin-collapsed')).toBeDisabled();
  await expect(backButton).toBeVisible();
  await expect(forwardButton).toBeVisible();
  expect((await expandButton.boundingBox())?.x).toBeGreaterThan((await forwardButton.boundingBox())?.x ?? 0);

  await clickGraphProbeAt(page, probe.coord);
  await waitForColumnNavigatorSettled(page, 'k:user');
  await expect(navigator).toBeVisible();
  await page.getByTestId('column-navigator-collapse').click();

  await page.getByTestId('column-navigator-expand').click();
  await expect(page.getByTestId('column-navigator-graph')).toBeVisible();

  await expect(page.getByTestId('column-navigator-pin-collapsed')).toBeEnabled();
  await page.getByTestId('column-navigator-pin-collapsed').click();
  await expect(navigator).toHaveCount(0);
  await clickGraphProbeAt(page, probe.coord);
  await waitForColumnNavigatorSettled(page, 'k:user');
  await expect(navigator).toHaveCount(0);

  await page.getByTestId('column-navigator-expand').click();
  await expect(navigator).toBeVisible();
});
