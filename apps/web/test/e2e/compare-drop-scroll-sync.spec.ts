import { expect, type Page, test } from './fixtures';
import { chooseFile, getMonacoScroll, getMonacoValue, readEditorState, waitForEditorReady } from './utils';

async function openTextMode(page: Page) {
  await page.getByRole('button', { name: 'Text mode', exact: true }).click();
}

async function dropFileToRightPanel(page: Page, fileName: string, content: string) {
  await chooseFile(page, {
    triggerLabel: 'Load compare file',
    inputLabel: 'Right panel file input',
    fileName,
    content,
    mimeType: 'application/json',
  });
}

async function importToSourceEditor(page: Page, fileName: string, content: string) {
  await chooseFile(page, {
    triggerLabel: 'Import',
    inputLabel: 'Import file input',
    fileName,
    content,
    mimeType: 'application/json',
  });
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toContain('item-0');
}

test('drop-to-compare keeps right panel text editor scroll in sync with source editor', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  const longJson = JSON.stringify(
    {
      lines: Array.from({ length: 40 }, (_, index) => ({
        id: index,
        label: `item-${index}`,
        nested: { enabled: index % 2 === 0 },
      })),
    },
    null,
    2,
  );

  await importToSourceEditor(page, 'source.json', longJson);
  await openTextMode(page);
  await dropFileToRightPanel(page, 'compare.json', longJson.replace(/item-39/, 'item-39-updated'));
  await expect.poll(async () => getMonacoValue(page, 'right-editor')).toContain('item-39-updated');

  await page.getByTestId('monaco-source-editor').hover();
  await page.mouse.wheel(0, 2400);

  await expect.poll(async () => (await getMonacoScroll(page, 'source-editor')).scrollTop, { timeout: 5_000 }).toBeGreaterThan(0);
  await expect.poll(async () => (await getMonacoScroll(page, 'right-editor')).scrollTop, { timeout: 5_000 }).toBeGreaterThan(0);
});
