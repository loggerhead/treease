import { expect, test, type Page } from './fixtures';
import {
  chooseFile,
  evaluateTreease,
  getMonacoInlineClassColor,
  getMonacoRenderedTokenColor,
  getMonacoValue,
  readEditorState,
  readEditorWorkspace,
  setEditorContent,
  setMonacoValue,
  waitForEditorReady,
  waitForSettingsReady,
} from './utils';

async function openTextMode(page: Page) {
  await page.getByRole('button', { name: 'Text mode', exact: true }).click();
  await expect(page.getByTestId('monaco-right-editor')).toBeVisible({ timeout: 5_000 });
}

async function readSidecarTabSource(page: Page) {
  const workspace = await readEditorWorkspace(page);
  const sidecarId = workspace.paneTabIds.right;
  return sidecarId ? workspace.tabsById[sidecarId]?.sourceText : '';
}

test.beforeEach(async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForSettingsReady(page);
});

test('right editor is a sidecar tab that does not replace primary authority', async ({ page }) => {
  await setEditorContent(page, {
    language: 'json',
    sourceText: '{"primary":true}',
  });
  const before = await readEditorState(page);

  await openTextMode(page);
  await setMonacoValue(page, 'right-editor', '{"sidecar":true}');

  await expect
    .poll(async () => JSON.stringify(JSON.parse(await readSidecarTabSource(page))), { timeout: 5_000 })
    .toBe('{"sidecar":true}');

  const after = await readEditorState(page);
  expect(after.sourceText).toBe('{"primary":true}');
  expect(after.documentKey).toBe(before.documentKey);
});

test('right editor receives semantic token colors after auto-formatted full-edit intake', async ({ page }) => {
  await evaluateTreease(page, async (treease) => {
    await treease.settings.save({
      formatting: {
        indent: 2,
        smart: true,
        maxLineLength: 100,
        maxInlineComplexity: 1,
        maxArrayInlineItems: 6,
        alignObjectArrays: true,
      },
    });
  });
  await waitForSettingsReady(page);
  await setEditorContent(page, {
    language: 'yaml',
    sourceText: 'primary: true\n',
  });

  await openTextMode(page);
  await chooseFile(page, {
    triggerLabel: 'Load compare file',
    inputLabel: 'Right panel file input',
    fileName: 'sidecar.json',
    content: '{"sidecar":{"nested":true},"count":2}',
    mimeType: 'application/json',
  });

  await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: 5_000 }).toContain('\n  "sidecar"');
  const formattedRightText = await getMonacoValue(page, 'right-editor');
  const countLineNumber = formattedRightText.split('\n').findIndex((line) => line.includes('"count"')) + 1;
  expect(countLineNumber).toBeGreaterThan(0);
  await expect
    .poll(async () => getMonacoRenderedTokenColor(page, 'right-editor', '"sidecar"', 2), { timeout: 5_000 })
    .toBe('rgb(163, 21, 21)');
  await expect
    .poll(async () => getMonacoRenderedTokenColor(page, 'right-editor', '2', countLineNumber), { timeout: 5_000 })
    .toBe('rgb(9, 134, 88)');

  const state = await readEditorState(page);
  const workspace = await readEditorWorkspace(page);
  expect(state.sourceText).toBe('primary: true\n');
  expect(workspace.tabsById['tab-sidecar'].languageId).toBe('json');
  expect(workspace.tabsById['tab-sidecar'].sourceText).toContain('\n  "sidecar"');
});

test('right editor preserves semantic token colors after manual edits', async ({ page }) => {
  await setEditorContent(page, {
    language: 'json',
    sourceText: '{"primary":true}',
  });

  const initialRightText = `{
  "object": {
    "float": 0.125,
    "bool": true
  }
}`;

  await openTextMode(page);
  await chooseFile(page, {
    triggerLabel: 'Load compare file',
    inputLabel: 'Right panel file input',
    fileName: 'sidecar.json',
    content: initialRightText,
    mimeType: 'application/json',
  });

  await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: 5_000 }).toContain('0.125');
  const loadedRightText = await getMonacoValue(page, 'right-editor');
  const initialFloatLineNumber = loadedRightText.split('\n').findIndex((line) => line.includes('"float"')) + 1;
  expect(initialFloatLineNumber).toBeGreaterThan(0);
  await expect
    .poll(async () => getMonacoRenderedTokenColor(page, 'right-editor', '"float"', initialFloatLineNumber), { timeout: 5_000 })
    .toBe('rgb(163, 21, 21)');
  await expect
    .poll(async () => getMonacoRenderedTokenColor(page, 'right-editor', '0.125', initialFloatLineNumber), { timeout: 5_000 })
    .toBe('rgb(9, 134, 88)');

  const floatLineText = loadedRightText.split('\n')[initialFloatLineNumber - 1] ?? '';
  const numberColumn = floatLineText.indexOf('0.125') + 1;
  expect(numberColumn).toBeGreaterThan(0);
  const editedRightText = loadedRightText.replace('0.125', '0.1');
  await evaluateTreease(
    page,
    (treease, payload: { lineNumber: number; startColumn: number; oldText: string; newText: string }) => {
      treease.editor.applyEdits('right-editor', [
        {
          range: {
            startLineNumber: payload.lineNumber,
            startColumn: payload.startColumn,
            endLineNumber: payload.lineNumber,
            endColumn: payload.startColumn + payload.oldText.length,
          },
          text: payload.newText,
        },
      ]);
    },
    {
      lineNumber: initialFloatLineNumber,
      startColumn: numberColumn,
      oldText: '0.125',
      newText: '0.1',
    },
  );
  await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: 5_000 }).toBe(editedRightText);
  const editedFloatLineNumber = editedRightText.split('\n').findIndex((line) => line.includes('"float"')) + 1;
  expect(editedFloatLineNumber).toBeGreaterThan(0);

  await expect
    .poll(async () => getMonacoRenderedTokenColor(page, 'right-editor', '"float"', editedFloatLineNumber), { timeout: 5_000 })
    .toBe('rgb(163, 21, 21)');
  await expect
    .poll(async () => getMonacoRenderedTokenColor(page, 'right-editor', '0.1', editedFloatLineNumber), { timeout: 5_000 })
    .toBe('rgb(9, 134, 88)');

  const state = await readEditorState(page);
  const workspace = await readEditorWorkspace(page);
  expect(state.sourceText).toBe('{"primary":true}');
  expect(workspace.tabsById['tab-sidecar'].sourceText).toContain('"float": 0.1');
});

test('right editor string highlighting matches the left editor', async ({ page }) => {
  await setEditorContent(page, {
    language: 'json',
    sourceText: '"left-string"',
  });

  await expect
    .poll(async () => getMonacoInlineClassColor(page, 'source-editor', 'treease-root-scalar-str'), { timeout: 5_000 })
    .toBe('rgb(4, 81, 165)');

  await openTextMode(page);
  await chooseFile(page, {
    triggerLabel: 'Load compare file',
    inputLabel: 'Right panel file input',
    fileName: 'sidecar.json',
    content: '"right-string"',
    mimeType: 'application/json',
  });

  await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: 5_000 }).toBe('"right-string"');
  await expect
    .poll(async () => getMonacoInlineClassColor(page, 'right-editor', 'treease-root-scalar-str'), { timeout: 5_000 })
    .toBe('rgb(4, 81, 165)');
});
