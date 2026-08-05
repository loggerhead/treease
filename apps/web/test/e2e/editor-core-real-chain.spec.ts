import { expect, test } from './fixtures';
import {
  dropFile,
  evaluateTreease,
  getMonacoMarkers,
  getMonacoRenderedTokenColor,
  getMonacoValue,
  readEditorState,
  readGraphClickProbes,
  readRuntimeReadiness,
  setEditorContent,
  setMonacoValue,
  waitForGraphRendered,
  waitForEditorReady,
} from './utils';

const EDITOR_LANGUAGES = ['json', 'yaml', 'toml', 'python', 'javascript'] as const;

test('renders the initial JSON example with syntax highlighting through the real full-edit chain', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('json');
  await expect
    .poll(
      async () => ({
        keyColor: await getMonacoRenderedTokenColor(page, 'source-editor', '"object"', 2),
        numberColor: await getMonacoRenderedTokenColor(page, 'source-editor', '42', 3),
      }),
      { timeout: 5_000 },
    )
    .toEqual({
      keyColor: expect.not.stringMatching(/^rgb\(15,\s*23,\s*42\)$/),
      numberColor: expect.not.stringMatching(/^rgb\(15,\s*23,\s*42\)$/),
    });
});

test('restores JSON syntax highlighting after switching through Python', async ({ page }) => {
  test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
  await page.goto('/editor');
  await waitForEditorReady(page);

  await evaluateTreease(page, (treease) => treease.editor.setLanguageId('python'));
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('python');
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.diagnostics.length, { timeout: 5_000 })
    .toBeGreaterThan(0);

  await evaluateTreease(page, (treease) => treease.editor.setLanguageId('json'));
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('json');
  await expect
    .poll(
      async () => ({
        keyColor: await getMonacoRenderedTokenColor(page, 'source-editor', '"object"', 2),
        numberColor: await getMonacoRenderedTokenColor(page, 'source-editor', '42', 3),
      }),
      { timeout: 5_000 },
    )
    .toEqual({
      keyColor: expect.not.stringMatching(/^rgb\(15,\s*23,\s*42\)$/),
      numberColor: expect.not.stringMatching(/^rgb\(15,\s*23,\s*42\)$/),
    });
});


test('imports through the TopBar drop target and exports using the real EditorCore chain', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await page.getByTestId('topbar-import-button').click();
  await dropFile(page, {
    targetTestId: 'import-drop-trigger',
    fileName: 'sample.json',
    content: '{"user":{"name":"Alice"},"items":[1,2,3]}',
    mimeType: 'application/json',
  });

  await expect
    .poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 })
    .toContain('"Alice"');

  await page.getByRole('button', { name: 'Export', exact: true }).click();
  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Download export file', exact: true }).click();
  const download = await downloadPromise;
  const stream = await download.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream!) {
    chunks.push(Buffer.from(chunk));
  }
  const content = Buffer.concat(chunks).toString('utf8');

  expect(content).toContain('"Alice"');
  expect(content).toContain('"items"');
});

test('imports json into the selected toml language without switching languages', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, { sourceText: 'title = "ready"\n', language: 'toml' });

  await page.getByTestId('topbar-import-button').click();
  await dropFile(page, {
    targetTestId: 'import-drop-trigger',
    fileName: 'sample.json',
    content: '{"user":{"name":"Alice"}}',
    mimeType: 'application/json',
  });

  await expect(page.getByText('Imported sample.json')).toBeVisible();
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('toml');
  await expect
    .poll(async () => {
      const text = await getMonacoValue(page, 'source-editor');
      return text.includes('Alice') && text.includes('=') && !text.trimStart().startsWith('{');
    }, { timeout: 5_000 })
    .toBe(true);
});

test('keeps dropped file content when switching language after drag import', async ({ page }) => {
  test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
  const jsonText = '{"library":{"book":"Alice"}}\n';

  await page.goto('/editor');
  await waitForEditorReady(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: 'sample.json',
    content: jsonText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toContain('Alice');
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('json');

  await evaluateTreease(page, (treease) => {
    treease.editor.setLanguageId('toml');
  });

  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('toml');
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toContain('Alice');
});

test('keeps the loaded example content when switching language', async ({ page }) => {
  test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });

  await page.goto('/editor');
  await waitForEditorReady(page);

  const exampleText = await getMonacoValue(page, 'source-editor');
  expect(exampleText.trim()).not.toBe('');

  await evaluateTreease(page, (treease) => {
    treease.editor.setLanguageId('toml');
  });

  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('toml');
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe(exampleText);
});

test('uses the full-replacement chain when a language switch preserves incompatible text', async ({ page }) => {
  test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
  await page.goto('/editor');
  await waitForEditorReady(page);

  const sourceText = await getMonacoValue(page, 'source-editor');
  await evaluateTreease(page, (treease) => treease.editor.setLanguageId('toml'));
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('toml');
  await expect.poll(async () => (await readEditorState(page)).tempModel.diagnostics.length, { timeout: 5_000 }).toBeGreaterThan(0);

  const switchedDiagnostics = (await readEditorState(page)).tempModel.diagnostics;
  const switchedMarkers = await getMonacoMarkers(page, 'source-editor');
  expect(switchedDiagnostics).toEqual(expect.arrayContaining([expect.objectContaining({ message: 'Syntax error' })]));
  expect(switchedMarkers.map((marker) => marker.message)).toContain('Syntax error');
  await expect.poll(async () => (await readGraphClickProbes(page)).length, { timeout: 5_000 }).toBe(0);
});

test('switches the JSON example through every language and reports editor/graph syntax state', async ({ page }) => {
  test.info().annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
  await page.goto('/editor');
  await waitForEditorReady(page);

  const jsonExample = await getMonacoValue(page, 'source-editor');
  await setEditorContent(page, { sourceText: jsonExample, language: 'json' });
  await waitForGraphRendered(page);

  for (const language of EDITOR_LANGUAGES) {
    const before = await readEditorState(page);
    await evaluateTreease(page, (treease, nextLanguage) => {
      treease.editor.setLanguageId(nextLanguage);
    }, language);

    await expect
      .poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 })
      .toBe(language);
    const afterLanguageChange = await readEditorState(page);
    expect(afterLanguageChange.sourceText).toBe(jsonExample);

    const target = {
      documentKey: afterLanguageChange.documentKey,
      revision: afterLanguageChange.editorRevision,
    };
    await expect
      .poll(
        async () => {
          const [readiness, color] = await Promise.all([
            readRuntimeReadiness(page),
            getMonacoRenderedTokenColor(page, 'source-editor', '"object"', 2),
          ]);
          return {
            color,
            analysisSettled:
              readiness.documentKey === target.documentKey &&
              readiness.graph.requestedRevision >= target.revision &&
              (readiness.graph.settled || (await readEditorState(page)).tempModel.diagnostics.length > 0),
          };
        },
        { timeout: 5_000 },
      )
      .toEqual(expect.objectContaining({ analysisSettled: true }));

    const diagnostics = (await readEditorState(page)).tempModel.diagnostics;
    if (diagnostics.length === 0) {
      await waitForGraphRendered(page, 5_000, target);
      await expect
        .poll(async () => (await readGraphClickProbes(page)).length, { timeout: 5_000 })
        .toBeGreaterThan(0);
      await expect(page.getByTestId('graph-diagnostic-syntax-error')).toHaveCount(0);
    } else {
      expect(diagnostics).toEqual(expect.arrayContaining([expect.objectContaining({ message: 'Syntax error' })]));
      await expect(page.getByTestId('graph-diagnostic-syntax-error').first()).toBeVisible({ timeout: 5_000 });
    }

    expect((await readEditorState(page)).editorRevision).toBeGreaterThanOrEqual(before.editorRevision);
  }
});

test('surfaces diagnostics for invalid editor input through the real EditorCore chain', async ({ page }, testInfo) => {
  testInfo.annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setMonacoValue(page, 'source-editor', '{"invalid":');
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.diagnostics.length, { timeout: 5_000 })
    .toBeGreaterThan(0);
  await expect(page.getByText(/OperationFailed/i)).toHaveCount(0);
});
