import { expect, test } from './fixtures';
import {
  applyMonacoEdits,
  clickGraphProbeAt,
  clickSubgraphWorkspaceProbeAt,
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
  const pane = workspace.getByTestId('graph-subgraph-pane').first();
  await expect(workspace).toBeVisible();
  await expect(pane.getByTestId('graph-subgraph-content-pane')).toBeVisible();
  await expect(pane.locator('.graph-subgraph-pane__header')).toHaveText('user.name');
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

  const rowPane = workspace.getByTestId('graph-subgraph-pane').first();
  await expect(rowPane.locator('.graph-subgraph-pane__header')).toHaveText('rows[0]');
  await expect(rowPane.locator('.graph-subgraph-pane__canvas')).toBeVisible();
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

  const panes = page.getByTestId('graph-subgraph-pane');
  await expect(panes).toHaveCount(2);
  await expect(panes.nth(1).locator('.graph-subgraph-pane__header')).toHaveText('preview.uris[1]');
  await expect
    .poll(async () => getMonacoValue(page, 'subgraph-content:k:preview|k:uris|i:1'), { timeout: 5_000 })
    .toBe('"https://treease.com/path?redirect=1"');
  await expect(page.getByText('Reveal failed')).toHaveCount(0);
});
