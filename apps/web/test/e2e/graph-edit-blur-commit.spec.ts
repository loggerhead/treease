import { expect, test, type Page } from './fixtures';
import {
  getLatestGraphProbes,
  installGraphEditEventCapture,
  readEditorState,
  readGraphClickProbes,
  readGraphRevealProbe,
  readTempGraphSelection,
  setTempGraphSelection,
  commitGraphValueViaProbes,
  setEditorContent,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

async function ensureGraphMode(page: Page) {
  const graphModeButton = page.getByTestId('graph-surface-graph');
  if (await graphModeButton.isVisible().catch(() => false)) {
    if ((await graphModeButton.getAttribute('aria-selected')) !== 'true') await graphModeButton.click();
  }
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

test('graph value edit preserves graph click highlight', async ({ page }) => {
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
  let rawSelectedPath: any[] | null = null;
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
  await expect
    .poll(async () => readTempGraphSelection(page), { timeout: 5_000 })
    .toEqual(expect.objectContaining({ path: selectedPath, source: 'graph' }));
});
