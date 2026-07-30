import { expect, test } from './fixtures';
import {
  applyMonacoEdits,
  clickGraphProbeAt,
  clickSubgraphWorkspaceProbeAt,
  getMonacoRenderedTokenColor,
  getMonacoValue,
  readEditorState,
  readGraphClickProbes,
  readSubgraphWorkspaceClickProbes,
  setEditorContent,
  setMonacoValue,
  waitForEditorReady,
  waitForGraphRendered,
  waitForSubgraphSettled,
} from './utils';

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
    await waitForSubgraphSettled(page, 'k:value');
    const hookId = 'subgraph-content:k:value';
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
  await waitForSubgraphSettled(page, 'k:object');

  const hookId = 'subgraph-content:k:object';
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

  const workspace = page.getByTestId('graph-subgraph-workspace');
  const longKey = workspace.locator('[data-subgraph-item-path-key="k:table_without_header"]');
  await expect(longKey).toHaveAttribute('data-subgraph-item-preview', '[3]');
  expect(await longKey.locator('.graph-subgraph-item__label').evaluate((element) => element.getBoundingClientRect().width))
    .toBeGreaterThanOrEqual(120);
  await expect(workspace.locator('[data-subgraph-item-path-key="k:object|k:arr0"]')).toHaveAttribute(
    'data-subgraph-item-preview',
    '[]',
  );
  await expect(workspace.locator('[data-subgraph-item-path-key="k:object|k:obj0"]')).toHaveAttribute(
    'data-subgraph-item-preview',
    '{}',
  );
  await expect
    .poll(() =>
      longKey.locator('.graph-subgraph-item__preview').evaluate((element) => getComputedStyle(element).color),
    )
    .toBe('rgb(107, 114, 128)');

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

test('subgraph workspace content pane uses monaco editor and syncs edits back to editor', async ({ page }) => {
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
  await waitForSubgraphSettled(page, 'k:user|k:name');

  const workspace = page.getByTestId('graph-subgraph-workspace');
  const contentPane = workspace.getByTestId('graph-subgraph-content-pane');
  const pane = contentPane.locator('..');
  await expect(workspace).toBeVisible();
  await expect(contentPane).toBeVisible();
  await expect(pane).toHaveAttribute('data-content-path-key', 'k:user|k:name');
  await expect(pane.getByTestId('graph-subgraph-key-input')).toHaveCount(0);
  const monacoHost = pane.getByTestId('graph-subgraph-monaco-editor');
  await expect(monacoHost).toBeVisible();
  await expect
    .poll(async () => getMonacoValue(page, 'subgraph-content:k:user|k:name'), { timeout: 5_000 })
    .toBe('"Alice"');

  await monacoHost.click();
  await setMonacoValue(page, 'subgraph-content:k:user|k:name', 'Bob');

  await expect
    .poll(async () => parseSourceText((await readEditorState(page)).sourceText), { timeout: 5_000 })
    .toMatchObject({
      user: { name: 'Bob' },
      rows: [{ title: 'one', done: false }],
    });

  const refreshedProbes = await readGraphClickProbes(page);
  const rowProbe = refreshedProbes.find(
    (probe) => probe.isTableCell && probe.path.join('.') === 'rows.[0]' && probe.target !== 'node' && probe.coord,
  );
  expect(rowProbe).toBeTruthy();
  if (!rowProbe?.coord) throw new Error('rows[0] probe missing');

  await clickGraphProbeAt(page, rowProbe.coord);
  await waitForSubgraphSettled(page, 'k:rows|i:0');

  const rowPane = workspace.locator('[data-column-path-key="k:rows|i:0"]');
  await expect(rowPane.locator('.graph-subgraph-pane__header')).toHaveCount(0);
  await expect(rowPane.locator('.graph-subgraph-pane__items')).toBeVisible();
  await expect(workspace.locator('[data-content-path-key="k:rows|i:0"]')).toBeVisible();
  await expect
    .poll(async () => getMonacoValue(page, 'subgraph-content:k:rows|i:0'), { timeout: 5_000 })
    .toBe('{"title":"one","done":false}');
});

test('subgraph workspace highlights null roots and keeps string editing caret stable', async ({ page }) => {
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

  await clickGraphProbeAt(page, nilProbe.coord);
  await waitForSubgraphSettled(page, 'k:object|k:nil');

  await expect
    .poll(async () => getMonacoValue(page, 'subgraph-content:k:object|k:nil'), { timeout: 5_000 })
    .toBe('null');

  const refreshedProbes = await readGraphClickProbes(page);
  const nameProbe = refreshedProbes.find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'user.name' && probe.coord,
  );
  expect(nameProbe).toBeTruthy();
  if (!nameProbe?.coord) throw new Error('user.name probe missing');

  await clickGraphProbeAt(page, nameProbe.coord);
  await waitForSubgraphSettled(page, 'k:user|k:name');

  await applyMonacoEdits(page, 'subgraph-content:k:user|k:name', [
    {
      range: { startLineNumber: 1, startColumn: 3, endLineNumber: 1, endColumn: 3 },
      text: 'B',
    },
  ]);
  await applyMonacoEdits(page, 'subgraph-content:k:user|k:name', [
    {
      range: { startLineNumber: 1, startColumn: 4, endLineNumber: 1, endColumn: 4 },
      text: 'C',
    },
  ]);

  await expect
    .poll(async () => getMonacoValue(page, 'subgraph-content:k:user|k:name'), { timeout: 5_000 })
    .toBe('"ABC"');
  await expect
    .poll(async () => parseSourceText((await readEditorState(page)).sourceText).user.name, { timeout: 5_000 })
    .toBe('ABC');
});

test('subgraph workspace rebases nested click paths before opening the next pane', async ({ page }) => {
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
  await waitForSubgraphSettled(page, 'k:preview|k:uris');

  const workspaceProbes = await readSubgraphWorkspaceClickProbes(page);
  const uriItemProbe = workspaceProbes.find(
    (probe) => probe.target === 'value' && probe.path.join('.') === 'preview.uris.[1]' && probe.coord,
  );
  expect(uriItemProbe, JSON.stringify(workspaceProbes, null, 2)).toBeTruthy();
  if (!uriItemProbe?.coord) throw new Error('preview.uris[1] workspace probe missing');

  await clickSubgraphWorkspaceProbeAt(page, uriItemProbe.coord);
  await waitForSubgraphSettled(page, 'k:preview|k:uris|i:1');

  const indexItem = page.locator('[data-subgraph-item-index="true"]').first();
  await expect(indexItem.locator('.graph-subgraph-item__dot')).toHaveCount(1);
  await expect(indexItem.locator('svg')).toHaveCount(0);

  await expect(page.locator('[data-subgraph-item-path-key="k:preview"]')).toHaveAttribute(
    'data-subgraph-path-ancestor',
    'true',
  );
  await expect(page.locator('[data-subgraph-item-path-key="k:preview|k:uris"]')).toHaveAttribute(
    'data-subgraph-path-ancestor',
    'true',
  );

  const panes = page.getByTestId('graph-subgraph-pane');
  await expect(panes).toHaveCount(4);
  await expect(panes.nth(3)).toHaveAttribute('data-content-path-key', 'k:preview|k:uris|i:1');
  await expect
    .poll(async () => getMonacoValue(page, 'subgraph-content:k:preview|k:uris|i:1'), { timeout: 5_000 })
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
  await waitForSubgraphSettled(page, 'k:root', 20_000);

  const workspace = page.getByTestId('graph-subgraph-workspace');
  await workspace.focus();
  await page.keyboard.press('ArrowRight');
  await waitForSubgraphSettled(page, 'k:root|k:first', 20_000);
  await expect(workspace.locator('[data-subgraph-item-path-key="k:root|k:first"]')).toHaveAttribute('aria-pressed', 'true');

  await page.keyboard.press('ArrowDown');
  await waitForSubgraphSettled(page, 'k:root|k:second', 20_000);
  await expect(workspace.locator('[data-subgraph-item-path-key="k:root|k:second"]')).toHaveAttribute('aria-pressed', 'true');

  await page.getByTestId('monaco-source-editor').click();
  await page.keyboard.press('ArrowUp');
  await expect(workspace.locator('[data-subgraph-item-path-key="k:root|k:second"]')).toHaveAttribute('aria-pressed', 'true');

  await workspace.locator('[data-subgraph-item-path-key="k:root|k:second|k:leaf"]').click();
  await waitForSubgraphSettled(page, 'k:root|k:second|k:leaf', 20_000);
  const detailMonaco = workspace.getByTestId('graph-subgraph-monaco-editor');
  await detailMonaco.click();
  await page.keyboard.press('ArrowLeft');
  await expect
    .poll(() => getMonacoValue(page, 'subgraph-content:k:root|k:second|k:leaf'))
    .toBe('"two"');
  await expect(workspace.locator('[data-subgraph-selected="true"]')).toHaveAttribute(
    'data-subgraph-item-path-key',
    'k:root|k:second|k:leaf',
  );
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
  await waitForSubgraphSettled(page, 'k:level1', 20_000);

  const workspace = page.getByTestId('graph-subgraph-workspace');
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
    await workspace.locator(`[data-subgraph-item-path-key="${nextPathKey}"]`).click();
    await waitForSubgraphSettled(page, nextPathKey, 20_000);
  }
  await waitForSubgraphSettled(
    page,
    'k:level1|k:level2|k:level3|k:level4|k:level5|k:level6|i:39|k:value',
    20_000,
  );

  const metrics = await workspace.evaluate((root) => {
    const rail = root.querySelector<HTMLElement>('.graph-subgraph-workspace__track');
    const columns = [...root.querySelectorAll<HTMLElement>('.graph-subgraph-pane')];
    const detail = root.querySelector<HTMLElement>('.graph-subgraph-detail');
    return {
      railOverflowX: rail ? getComputedStyle(rail).overflowX : '',
      railScrollWidth: rail?.scrollWidth ?? 0,
      railClientWidth: rail?.clientWidth ?? 0,
      railScrollLeft: rail?.scrollLeft ?? 0,
      columnWidths: columns.map((column) => column.getBoundingClientRect().width),
      columnOverflowY: columns.map((column) =>
        getComputedStyle(column.querySelector<HTMLElement>('.graph-subgraph-pane__items')!).overflowY),
      detailOverflow: detail ? getComputedStyle(detail).overflow : '',
    };
  });

  expect(metrics.railOverflowX).toBe('auto');
  expect(metrics.railScrollWidth).toBeGreaterThan(metrics.railClientWidth);
  expect(metrics.railScrollLeft).toBeGreaterThan(0);
  expect(metrics.columnWidths.every((width) => Math.round(width) === 288)).toBe(true);
  expect(metrics.columnOverflowY.every((overflow) => overflow === 'auto')).toBe(true);
});
