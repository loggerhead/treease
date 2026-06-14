import { expect, test } from './fixtures';
import { readEditorState, setEditorContent } from './utils';

test('loads app and runs editor→worker→graph smoke chain', async ({ page }) => {
  await page.goto('/editor');

  await setEditorContent(page, {
    sourceText: '{"user":{"name":"Alice","active":true},"items":[1,2,3]}'
  });

  await expect(page.getByTestId('monaco-source-editor')).toBeVisible();
  await expect
    .poll(async () => {
      if (await page.getByRole('button', { name: 'Graph mode', exact: true }).count()) return 'graph';
      if (await page.getByRole('button', { name: 'Text mode', exact: true }).count()) return 'text';
      return '';
    }, { timeout: 5_000 })
    .not.toBe('');
  await expect(page.getByRole('button', { name: 'Search graph', exact: true })).toBeVisible();

  await expect
    .poll(async () => (await readEditorState(page)).sourceText.includes('Alice'), { timeout: 5_000 })
    .toBe(true);

  await page.getByTestId('monaco-source-editor').click();
  await page.keyboard.press('ArrowRight');
  await expect
    .poll(async () => (await readEditorState(page)).tempModel.cursor, { timeout: 5_000 })
    .toContain('Ln ');
});
