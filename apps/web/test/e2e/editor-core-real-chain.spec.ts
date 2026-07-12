import { expect, test } from './fixtures';
import {
  dropFile,
  evaluateTreease,
  getMonacoRenderedTokenColor,
  getMonacoValue,
  readEditorState,
  setEditorContent,
  setMonacoValue,
  waitForEditorReady,
} from './utils';

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

test('surfaces diagnostics for invalid editor input through the real EditorCore chain', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await setMonacoValue(page, 'source-editor', '{"invalid":');
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.diagnostics.length, { timeout: 5_000 })
    .toBeGreaterThan(0);
  await expect(page.getByText(/OperationFailed/i)).toHaveCount(0);
});
