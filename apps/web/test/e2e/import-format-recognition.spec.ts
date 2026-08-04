import { expect, test, type Page } from './fixtures';
import {
  dropFile,
  getMonacoValue,
  readEditorState,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

const yamlText = 'user:\n  name: Alice\ncount: 42\n';

async function openRightTextMode(page: Page) {
  await page.getByTestId('graph-surface-compare').click();
  await expect(page.getByTestId('monaco-right-editor')).toBeVisible({ timeout: 5_000 });
}

test('top import drop target loads original file text into source editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await page.getByRole('button', { name: 'Import', exact: true }).click();
  await dropFile(page, {
    targetTestId: 'import-drop-trigger',
    fileName: 'sample.yaml',
    content: yamlText,
    mimeType: 'text/plain',
  });

  await expect(page.getByText('Imported sample.yaml')).toBeVisible();
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe(yamlText);
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe(yamlText);
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('yaml');
});

test('top import drop target converts csv into the active editor language', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await page.getByRole('button', { name: 'Import', exact: true }).click();
  await dropFile(page, {
    targetTestId: 'import-drop-trigger',
    fileName: 'people.csv',
    content: 'name,age\nAlice,18\nBob,20\n',
    mimeType: 'text/csv',
  });

  await expect(page.getByText('Imported people.csv')).toBeVisible();
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('json');
  await expect
    .poll(
      async () => {
        const text = await getMonacoValue(page, 'source-editor');
        return JSON.stringify(JSON.parse(text));
      },
      { timeout: 5_000 },
    )
    .toBe(JSON.stringify([{ name: 'Alice', age: 18 }, { name: 'Bob', age: 20 }]));
  await waitForGraphRendered(page);
});

test('dragging file onto left editor loads original file text', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await dropFile(page, {
    targetTestId: 'source-editor-region',
    fileName: 'drag-left.yaml',
    content: yamlText,
    mimeType: 'text/plain',
  });

  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe(yamlText);
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe(yamlText);
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('yaml');
});


test('dragging file onto right editor loads original file text without changing source language', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await openRightTextMode(page);

  await dropFile(page, {
    targetTestId: 'right-panel-dropzone',
    fileName: 'drag-right.yaml',
    content: yamlText,
    mimeType: 'text/plain',
  });

  await expect.poll(async () => getMonacoValue(page, 'right-editor'), { timeout: 5_000 }).toBe(yamlText);
  await expect.poll(async () => (await readEditorState(page)).tempModel.scratchText, { timeout: 5_000 }).toBe(yamlText);
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('json');
});
