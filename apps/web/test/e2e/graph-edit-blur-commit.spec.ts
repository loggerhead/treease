import { expect, test, type Page } from './fixtures';
import type { TreeaseRuntimePathSeg } from '../../src/lib/test-bridge/types';
import {
  chooseFile,
  clearGraphLastReveal,
  commitGraphValueViaProbes,
  getLatestGraphProbes,
  installGraphEditEventCapture,
  dropFile,
  readEditorState,
  readGraphHoverPanel,
  readGraphHoverPanelClickProbes,
  readGraphHoverPanelPrewarmState,
  readGraphHoverPreview,
  readGraphClickProbes,
  readGraphLastReveal,
  revealGraphPath,
  readGraphRevealProbe,
  readTempGraphSelection,
  setTempGraphSelection,
  setEditorContent,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

async function ensureGraphMode(page: Page) {
  const graphModeButton = page.getByRole('button', { name: 'Graph mode', exact: true });
  if (await graphModeButton.isVisible().catch(() => false)) {
    await graphModeButton.click();
  } else {
    await expect(page.getByRole('button', { name: 'Text mode', exact: true })).toBeVisible();
  }
}

function buildHoverPanelFixtureText() {
  return JSON.stringify({
    Result: {
      Blocks: Array.from({ length: 7 }, (_, index) => ({
        Id: `block-${index}`,
        Content: index === 0 ? { Text: 'hello', Spans: [{ Start: 0, End: 5, Type: 'text' }] } : { Text: `block ${index}` },
        TaskError: index === 6 ? { Code: 'TASK_FAILED', Message: 'task failed', Retryable: false } : null,
      })),
    },
  });
}

function buildAccountLimitFixtureText() {
  return JSON.stringify({
    ApiList: [
      {
        AccountLevelLimitConf: {
          StrategicAccountLimit: 3,
          SmallMediumAccountLimit: 5,
        },
        AccountLevelTotalLimitConf: {
          StrategicAccountLimit: 11,
          SmallMediumAccountLimit: 5,
        },
      },
    ],
  });
}

test('graph value edit writes back to source editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphEditEventCapture(page);

  const sourceText = '{"user":{"name":"Alice","role":"admin"},"count":42}';
  await setEditorContent(page, { sourceText, language: 'json' });
  await ensureGraphMode(page);
  await waitForGraphRendered(page);

  const committed = await commitGraphValueViaProbes(page, {
    sourceText,
    inputText: 'Carol',
    selectAllModifier: 'Meta',
    matchesOpenEvent: (detail) => {
      const path = detail.path ?? [];
      return detail.kind === 'value' && path.some((segment) => segment?.key === 'name');
    },
    verifyCommitted: (nextSourceText) =>
      /"name"\s*:\s*"Carol"/.test(nextSourceText) && !/"name"\s*:\s*"Alice"/.test(nextSourceText),
  });

  expect(committed).toBe(true);
  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toMatch(/"name"\s*:\s*"Carol"/);
});

test('graph non-string value edit writes back to source editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphEditEventCapture(page);

  const sourceText = '{"object":{"int":42},"count":1}';
  await setEditorContent(page, { sourceText, language: 'json' });
  await ensureGraphMode(page);
  await waitForGraphRendered(page);

  const committed = await commitGraphValueViaProbes(page, {
    sourceText,
    inputText: '43',
    selectAllModifier: 'Meta',
    matchesOpenEvent: (detail) => {
      const path = detail.path ?? [];
      return detail.kind === 'value' && path.length === 2 && path[0]?.key === 'object' && path[1]?.key === 'int';
    },
    verifyCommitted: (nextSourceText) => {
      try {
        return JSON.parse(nextSourceText)?.object?.int === 43;
      } catch {
        return false;
      }
    },
  });

  expect(committed).toBe(true);
  await expect
    .poll(async () => JSON.parse((await readEditorState(page)).sourceText).object?.int, { timeout: 5_000 })
    .toBe(43);
});

test('double-clicking a main-graph scalar cell enters inline edit mode', async ({ page }) => {
  // Regression: initial graph projection must preserve editable flags for main-graph scalar cells.
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphEditEventCapture(page);

  const sourceText = '{"user":{"name":"Alice","role":"admin"},"count":42}';
  await setEditorContent(page, { sourceText, language: 'json' });
  await ensureGraphMode(page);
  await waitForGraphRendered(page);

  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');

  await expect
    .poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThan(0);

  const clickProbes = await readGraphClickProbes(page);
  const probeIndex = clickProbes.findIndex(
    (probe) =>
      probe.nodeType === 'Text' &&
      probe.target === 'value' &&
      probe.valueType === 'string' &&
      probe.text === 'Alice' &&
      probe.path.join('.') === 'user.name',
  );
  expect(probeIndex, JSON.stringify(clickProbes, null, 2)).toBeGreaterThanOrEqual(0);

  const probe = (await getLatestGraphProbes(page))[probeIndex];
  if (!probe) throw new Error('user.name value probe missing');
  const beforeOpenCount = await page.evaluate(() =>
    window._treease?.test
      .getGraphEditEvents()
      .filter((event: { type?: string }) => event.type === 'open').length,
  );

  await page.mouse.dblclick(box.x + probe.x, box.y + probe.y);

  await expect(page.locator('.leafer-text-editor')).toBeVisible({ timeout: 1_000 });
  await expect
    .poll(
      async () =>
        await page.evaluate(() =>
          window._treease?.test
            .getGraphEditEvents()
            .filter((event: { type?: string }) => event.type === 'open').length,
        ),
      { timeout: 1_000 },
    )
    .toBeGreaterThan(beforeOpenCount);
});



test('non-overflow scalar value hover does not open graph hover panel', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = '{"message":"你好","count":1}';
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');

  await expect
    .poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThan(0);

  const clickProbes = await readGraphClickProbes(page);
  const messageProbeIndex = clickProbes.findIndex(
    (probe) => probe.nodeType === 'Text' && probe.target === 'value' && probe.valueType === 'string' && probe.text === '你好' && probe.path.join('.') === 'message',
  );
  expect(messageProbeIndex).toBeGreaterThanOrEqual(0);

  const messageProbe = (await getLatestGraphProbes(page))[messageProbeIndex];
  if (!messageProbe) throw new Error('message probe missing');
  await page.mouse.move(box.x + messageProbe.x, box.y + messageProbe.y);

  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);
});

test('non-table array value hover does not open graph hover panel', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = '{"preview":{"uris":["https://a.example.com","https://b.example.com"]},"count":1}';
  await setEditorContent(page, { sourceText, language: 'json' });
  await waitForGraphRendered(page);

  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');

  await expect
    .poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThan(0);

  const clickProbes = await readGraphClickProbes(page);
  const urisProbeIndex = clickProbes.findIndex(
    (probe) => probe.nodeType === 'Text' && probe.target === 'value' && probe.valueType === 'array' && probe.path.join('.') === 'preview.uris',
  );
  expect(urisProbeIndex, JSON.stringify(clickProbes, null, 2)).toBeGreaterThanOrEqual(0);

  const urisProbe = (await getLatestGraphProbes(page))[urisProbeIndex];
  if (!urisProbe) throw new Error('uris probe missing');
  await page.mouse.move(box.x + urisProbe.x, box.y + urisProbe.y);

  await expect.poll(async () => readGraphHoverPreview(page), { timeout: 2_000 }).toBeNull();
  await expect(page.locator('.leafer-x-tooltip [data-tooltip-panel]')).toHaveCount(0);
});


test('json graph hover panel renders TaskError subtree after drop import', async ({ page }) => {
  const sourceText = buildHoverPanelFixtureText();
  const timeout = 5_000;

  await page.setViewportSize({ width: 2_600, height: 1_400 });
  await page.goto('/editor');
  await waitForEditorReady(page);
  await ensureGraphMode(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '2mb.json',
    content: sourceText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout }).toBe('json');
  await waitForGraphRendered(page, timeout);

  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');

  const findTaskErrorProbe = async () => {
    const probes = await readGraphClickProbes(page);
    const probe = probes.find(
      (probe) =>
        probe.nodeType === 'Text' &&
        probe.target === 'value' &&
        probe.valueType === 'object' &&
        probe.path.join('.') === 'Result.Blocks.[6].TaskError',
    );
    return probe?.coord ? probe : null;
  };

  await expect.poll(findTaskErrorProbe, { timeout }).not.toBeNull();
  const taskErrorProbe = await findTaskErrorProbe();
  if (!taskErrorProbe?.coord) throw new Error('Result.Blocks.[6].TaskError probe missing');

  await page.mouse.move(box.x + taskErrorProbe.coord.x, box.y + taskErrorProbe.coord.y);

  await expect.poll(async () => readGraphHoverPanel(page), { timeout }).toEqual(
    expect.objectContaining({
      visible: true,
      rect: expect.any(Object),
    }),
  );
  await expect.poll(async () => (await readGraphHoverPanelClickProbes(page)).length, { timeout }).toBeGreaterThan(0);
});

test('real graph-table-missing-row fixture keeps tooltip subgraph inline editor aligned to first cell', async ({ page }) => {
  const sourceText = buildAccountLimitFixtureText();
  const timeout = 5_000;

  await page.setViewportSize({ width: 3_600, height: 1_600 });
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphEditEventCapture(page);

  await chooseFile(page, {
    triggerLabel: 'Import',
    inputLabel: 'Import file input',
    fileName: 'graph-table-missing-row.1.json',
    content: sourceText,
    mimeType: 'application/json',
  });

  await expect(page.getByText('Imported graph-table-missing-row.1.json')).toBeVisible();
  await waitForGraphRendered(page, timeout);

  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');

  const findRootProbe = async () => {
    const probes = await readGraphClickProbes(page);
    return (
      probes.find(
        (probe) =>
          probe.nodeType === 'Text' &&
          probe.target === 'value' &&
          probe.valueType === 'object' &&
          probe.path.join('.') === 'ApiList.[0].AccountLevelLimitConf' &&
          probe.coord,
      ) ?? null
    );
  };

  await expect.poll(findRootProbe, { timeout }).not.toBeNull();
  const rootProbe = await findRootProbe();
  if (!rootProbe?.coord) throw new Error('ApiList.[0].AccountLevelLimitConf probe missing');

  await page.mouse.move(box.x + rootProbe.coord.x, box.y + rootProbe.coord.y);

  await expect.poll(async () => readGraphHoverPanel(page), { timeout }).toEqual(
    expect.objectContaining({
      visible: true,
      rect: expect.any(Object),
    }),
  );

  const findFirstCellProbe = async () => {
    const probes = await readGraphHoverPanelClickProbes(page);
    return (
      probes.find(
        (probe) =>
          probe.nodeType === 'Text' &&
          probe.target === 'value' &&
          probe.valueType === 'number' &&
          probe.path.join('.') === 'ApiList.[0].AccountLevelLimitConf.StrategicAccountLimit' &&
          probe.coord &&
          probe.rect,
      ) ?? null
    );
  };

  await expect.poll(findFirstCellProbe, { timeout }).not.toBeNull();
  const firstCellProbe = await findFirstCellProbe();
  if (!firstCellProbe?.coord || !firstCellProbe.rect) {
    throw new Error('AccountLevelLimitConf.StrategicAccountLimit probe missing');
  }

  await page.mouse.dblclick(box.x + firstCellProbe.coord.x, box.y + firstCellProbe.coord.y);

  const tooltipEditor = page.locator('.leafer-text-editor');
  await expect(tooltipEditor).toBeVisible({ timeout: 5_000 });

  const editorRect = await tooltipEditor.evaluate((node) => {
    const bounds = (node as HTMLElement).getBoundingClientRect();
    return {
      left: Number(bounds.left),
      top: Number(bounds.top),
      width: Number(bounds.width),
      height: Number(bounds.height),
    };
  });

  expect(Math.abs(editorRect.left - firstCellProbe.rect.left)).toBeLessThanOrEqual(1);
  expect(Math.abs(editorRect.top - firstCellProbe.rect.top)).toBeLessThanOrEqual(1);
});

test('tooltip subgraph cells reveal and inline edit with absolute document paths after drop import', async ({ page }) => {
  const sourceText = buildAccountLimitFixtureText();
  const timeout = 5_000;
  const consoleErrors: string[] = [];

  page.on('console', (message) => {
    if (message.type() === 'error') {
      consoleErrors.push(message.text());
    }
  });
  page.on('pageerror', (error) => {
    consoleErrors.push(error.message);
  });

  await page.setViewportSize({ width: 3_600, height: 1_600 });
  await page.goto('/editor');
  await waitForEditorReady(page);
  const hasIndexZeroSegment = (path: TreeaseRuntimePathSeg[]) =>
    path.some((segment) => typeof segment?.index === 'number' && segment.index === 0);

  await ensureGraphMode(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: 'graph-table-missing-row.1.json',
    content: sourceText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout }).toBe('json');
  await waitForGraphRendered(page, timeout);

  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');
  await revealGraphPath(
    page,
    [{ key: 'ApiList' }, { index: 0 }, { key: 'AccountLevelTotalLimitConf' }],
    { target: 'value', navigate: true },
  );


  type ProbeSnapshot = Awaited<ReturnType<typeof readGraphClickProbes>>[number];
  const expectedRootPath = 'ApiList.[0].AccountLevelTotalLimitConf';
  const unindexedRootPath = 'ApiList.AccountLevelTotalLimitConf';
  const findRootProbe = async (): Promise<ProbeSnapshot | null> => {
    const probes = await readGraphClickProbes(page);
    return (
      probes.find((probe) => {
        const path = probe.path.join('.');
        return (
          probe.nodeType === 'Text' &&
          probe.target === 'value' &&
          probe.valueType === 'object' &&
          (path === expectedRootPath || path === unindexedRootPath) &&
          probe.coord
        );
      }) ?? null
    );
  };
  await expect.poll(findRootProbe, { timeout }).not.toBeNull();
  const rootProbe = await findRootProbe();
  if (!rootProbe?.coord) throw new Error('ApiList[0].AccountLevelTotalLimitConf probe missing');
  const rootPathHasIndex = hasIndexZeroSegment(rootProbe.rawPath);

  await page.mouse.move(box.x + rootProbe.coord.x, box.y + rootProbe.coord.y);

  await expect.poll(async () => readGraphHoverPanel(page), { timeout }).toEqual(
    expect.objectContaining({
      visible: true,
      rect: expect.any(Object),
    }),
  );

  type PanelProbeSnapshot = Awaited<ReturnType<typeof readGraphHoverPanelClickProbes>>[number];
  const expectedSmallMediumPath = 'ApiList.[0].AccountLevelTotalLimitConf.SmallMediumAccountLimit';
  const unindexedSmallMediumPath = 'ApiList.AccountLevelTotalLimitConf.SmallMediumAccountLimit';
  const findSmallMediumProbe = async (): Promise<PanelProbeSnapshot | null> => {
    const probes = await readGraphHoverPanelClickProbes(page);
    return (
      probes.find((probe) => {
        const path = probe.path.join('.');
        return (
          probe.nodeType === 'Text' &&
          probe.target === 'value' &&
          probe.valueType === 'number' &&
          (path === expectedSmallMediumPath || path === unindexedSmallMediumPath) &&
          probe.coord
        );
      }) ?? null
    );
  };
  await expect.poll(findSmallMediumProbe, { timeout }).not.toBeNull();
  const smallMediumProbe = await findSmallMediumProbe();
  if (!smallMediumProbe?.coord) throw new Error('AccountLevelTotalLimitConf.SmallMediumAccountLimit probe missing');
  const panelPathHasIndex = hasIndexZeroSegment(smallMediumProbe.rawPath);

  await clearGraphLastReveal(page);
  await page.mouse.click(box.x + smallMediumProbe.coord.x, box.y + smallMediumProbe.coord.y);
  const revealed = await expect
    .poll(async () => readGraphLastReveal(page), { timeout: 1_000 })
    .toEqual(
      expect.objectContaining({
        path: ['$', 'ApiList', '[0]', 'AccountLevelTotalLimitConf', 'SmallMediumAccountLimit'],
        target: 'value',
      }),
    )
    .then(() => true)
    .catch(() => false);

  await page.mouse.dblclick(box.x + smallMediumProbe.coord.x, box.y + smallMediumProbe.coord.y);
  const tooltipEditor = page.locator('.leafer-text-editor');
  await expect(tooltipEditor).toBeVisible({ timeout });
  await page.keyboard.press('Meta+A');
  await page.keyboard.type('7');
  await page.getByTestId('monaco-source-editor').click();

  const edited = await expect
    .poll(
      async () => {
        try {
          return JSON.parse((await readEditorState(page)).sourceText)?.ApiList?.[0]?.AccountLevelTotalLimitConf
            ?.SmallMediumAccountLimit;
        } catch {
          return null;
        }
      },
      { timeout: 2_000 },
    )
    .toBe(7)
    .then(() => true)
    .catch(() => false);
  expect({
    rootPathHasIndex,
    panelPathHasIndex,
    revealed,
    edited,
    applyValueEditErrors: consoleErrors.filter((message) => message.includes('applyValueEdit failed')),
  }).toEqual({
    rootPathHasIndex: true,
    panelPathHasIndex: true,
    revealed: true,
    edited: true,
    applyValueEditErrors: [],
  });
});

test('json graph hover panel renders Content subtree after drop import', async ({ page }) => {
  const sourceText = buildHoverPanelFixtureText();
  const timeout = 5_000;

  await page.setViewportSize({ width: 2_600, height: 1_400 });
  await page.goto('/editor');
  await waitForEditorReady(page);
  await ensureGraphMode(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: '2mb.json',
    content: sourceText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout }).toBe('json');
  await waitForGraphRendered(page, timeout);

  const canvas = page.getByTestId('graph-viewer-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('graph-viewer-canvas bounding box missing');

  const findContentProbe = async () => {
    const probes = await readGraphClickProbes(page);
    const probe = probes.find(
      (probe) =>
        probe.nodeType === 'Text' &&
        probe.target === 'value' &&
        probe.valueType === 'object' &&
        probe.path.join('.') === 'Result.Blocks.[0].Content',
    );
    return probe?.coord ? probe : null;
  };

  await expect.poll(findContentProbe, { timeout }).not.toBeNull();
  const contentProbe = await findContentProbe();
  if (!contentProbe?.coord) throw new Error('Result.Blocks.[0].Content probe missing');

  await page.mouse.move(box.x + contentProbe.coord.x, box.y + contentProbe.coord.y);

  await expect.poll(async () => readGraphHoverPanel(page), { timeout }).toEqual(
    expect.objectContaining({
      visible: true,
      rect: expect.any(Object),
    }),
  );
  await expect.poll(async () => (await readGraphHoverPanelClickProbes(page)).length, { timeout }).toBeGreaterThan(0);

  await expect
    .poll(
      async () => page.evaluate(() => window._treease?.graph.getHoverPanelDebugState()?.phase ?? ''),
      { timeout },
    )
    .toBe('panel-ready');
});

test('graph value edit clears graph click highlight', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await installGraphEditEventCapture(page);

  const sourceText = '{"user":{"name":"Alice","role":"admin"},"count":42}';
  await setEditorContent(page, { sourceText, language: 'json' });
  await ensureGraphMode(page);
  await waitForGraphRendered(page);

  await expect
    .poll(async () => (await getLatestGraphProbes(page)).length, { timeout: 5_000 })
    .toBeGreaterThan(0);

  let selectedPath: string[] | null = null;
  let selectedField: 'name' | 'role' | null = null;
  let rawSelectedPath: TreeaseRuntimePathSeg[] | null = null;
  let selectedTarget: 'key' | 'value' | 'node' | undefined = undefined;
  const revealProbes = await readGraphClickProbes(page);
  for (let i = 0; i < Math.min(24, revealProbes.length); i += 1) {
    const probeId = revealProbes[i]?.id;
    if (!probeId) continue;
    const probe = await readGraphRevealProbe(page, probeId);
    const path = probe?.path ?? [];
    if (!path.length) continue;
    if (path.includes('name')) {
      selectedPath = path;
      rawSelectedPath = probe?.rawPath ?? null;
      selectedTarget = probe?.target;
      selectedField = 'name';
      break;
    }
    if (!selectedField && path.includes('role')) {
      selectedPath = path;
      rawSelectedPath = probe?.rawPath ?? null;
      selectedTarget = probe?.target;
      selectedField = 'role';
      break;
    }
  }

  expect(selectedPath).not.toBeNull();
  expect(selectedField).not.toBeNull();
  expect(rawSelectedPath).not.toBeNull();

  await setTempGraphSelection(page, rawSelectedPath!, selectedTarget);

  await expect.poll(async () => readTempGraphSelection(page), { timeout: 5_000 }).not.toBeNull();

  const committed = await commitGraphValueViaProbes(page, {
    sourceText,
    inputText: 'Carol',
    selectAllModifier: 'Meta',
    matchesOpenEvent: (detail) => {
      const path = detail.path ?? [];
      return detail.kind === 'value' && path.some((segment) => segment?.key === selectedField);
    },
    verifyCommitted: (nextSourceText) => {
      if (selectedField === 'name') {
        return /"name"\s*:\s*"Carol"/.test(nextSourceText) && !/"name"\s*:\s*"Alice"/.test(nextSourceText);
      }
      return /"role"\s*:\s*"Carol"/.test(nextSourceText) && !/"role"\s*:\s*"admin"/.test(nextSourceText);
    },
  });

  expect(committed).toBe(true);
  await expect.poll(async () => readTempGraphSelection(page), { timeout: 5_000 }).toBeNull();
});
