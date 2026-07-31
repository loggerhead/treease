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
  waitForGraphRendered,
  waitForColumnNavigatorSettled,
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

  const refreshedProbes = await readGraphClickProbes(page);
  const rowProbe = refreshedProbes.find(
    (probe) => probe.isTableCell && probe.path.join('.') === 'rows.[0]' && probe.target !== 'node' && probe.coord,
  );
  expect(rowProbe).toBeTruthy();
  if (!rowProbe?.coord) throw new Error('rows[0] probe missing');

  await clickGraphProbeAt(page, rowProbe.coord);
  await waitForColumnNavigatorSettled(page, 'k:rows|i:0');

  const rowPane = workspace.locator('[data-column-navigator-path-key="k:rows|i:0"]');
  await expect(rowPane.locator('.column-navigator-pane__header')).toHaveCount(0);
  await expect(rowPane.locator('.column-navigator-pane__items')).toBeVisible();
  await expect(workspace.locator('[data-column-navigator-content-path-key="k:rows|i:0"]')).toBeVisible();
  await expect
    .poll(async () => getMonacoValue(page, 'column-navigator-content:k:rows|i:0'), { timeout: 5_000 })
    .toBe('{"title":"one","done":false}');
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

  await clickGraphProbeAt(page, nilProbe.coord);
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

  await clickGraphProbeAt(page, nameProbe.coord);
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

  const probe = (await readGraphClickProbes(page)).find(
    (candidate) => candidate.target === 'value' && candidate.path.join('.') === 'user' && candidate.coord,
  );
  expect(probe).toBeTruthy();
  if (!probe?.coord) throw new Error('user object probe missing');
  await clickGraphProbeAt(page, probe.coord);
  await waitForColumnNavigatorSettled(page, 'k:user');

  const navigator = page.getByTestId('column-navigator-graph');
  await expect(navigator).toBeVisible();
  await navigator.getByRole('button', { name: 'Close column navigator' }).click();
  await expect(navigator).toHaveCount(0);
  await expect(page.getByTestId('graph-viewer-canvas')).toBeVisible();

  await clickGraphProbeAt(page, probe.coord);
  await waitForColumnNavigatorSettled(page, 'k:user');
  await expect(page.getByTestId('column-navigator-graph')).toBeVisible();
});
