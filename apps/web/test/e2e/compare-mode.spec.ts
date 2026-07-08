import { expect, test, type Page } from './fixtures';
import { getMonacoValue, readEditorState, setEditorContent, setMonacoValue, waitForEditorReady } from './utils';

async function openTextMode(page: Page) {
  await page.getByRole('button', { name: 'Text mode', exact: true }).click();
  await expect(page.getByTestId('monaco-right-editor')).toBeVisible({ timeout: 5_000 });
  await expect(page.getByRole('button', { name: 'Compare', exact: true })).toBeVisible();
}

async function setRightTextFromStore(page: Page, value: string) {
  await setMonacoValue(page, 'right-editor', value);
}

async function syncRightToSource(page: Page) {
  const source = (await readEditorState(page)).sourceText;
  await setMonacoValue(page, 'right-editor', source);
  await expect
    .poll(async () => {
      const rightText = await getMonacoValue(page, 'right-editor');
      return JSON.stringify(JSON.parse(rightText)) === JSON.stringify(JSON.parse(source));
    })
    .toBe(true);
}

async function runCompare(page: Page) {
  await page.getByRole('button', { name: 'Compare', exact: true }).click();
}

async function readInlineDiffTexts(page: Page, hookId: string, className: string): Promise<string[]> {
  return page.evaluate(
    ({ hookId, className: nextClassName }) => {
      const root = document.querySelector(`[data-testid="monaco-${hookId}"]`);
      if (!root) return [];
      return Array.from(root.querySelectorAll(`.view-lines .${nextClassName}`))
        .map((node) => (node.textContent ?? '').trim())
        .filter((text) => text.length > 0);
    },
    { hookId, className },
  );
}

test('shows equal toast and no decorations when right text equals source', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await openTextMode(page);

  await syncRightToSource(page);
  await runCompare(page);
  await expect(page.getByText('Compare completed (no differences)')).toBeVisible();
  await expect(page.getByTestId('right-panel-dropzone')).toHaveAttribute('data-compare-highlight-count', '0');
});

test('re-compare should show warning toast and render highlight for parseable unequal JSON', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, { sourceText: '{"token":"base"}', language: 'json' });
  await openTextMode(page);

  await syncRightToSource(page);
  await runCompare(page);
  await expect(page.getByTestId('right-panel-dropzone')).toHaveAttribute('data-compare-highlight-count', '0');

  await setMonacoValue(page, 'right-editor', '{"token":"second-token"}');
  await runCompare(page);
  await expect(page.getByText('Compare completed (differences found)')).toBeVisible();
  await expect
    .poll(async () => Number((await page.getByTestId('right-panel-dropzone').getAttribute('data-compare-highlight-count')) ?? '0'))
    .toBeGreaterThan(0);
});

test('editing right Monaco clears previous compare highlights', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await openTextMode(page);

  await setMonacoValue(page, 'right-editor', '{"token":"will-clear"');
  await runCompare(page);
  await expect
    .poll(async () => Number((await page.getByTestId('right-panel-dropzone').getAttribute('data-compare-highlight-count')) ?? '0'))
    .toBeGreaterThan(0);

  await setRightTextFromStore(page, '{"token":"after-edit"}');
  await expect(page.getByTestId('right-panel-dropzone')).toHaveAttribute('data-compare-highlight-count', '0');
});


test('structured compare ignores formatting-only diffs', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, { sourceText: '{"a":1,"b":[true,false],"c":{"x":"y"}}', language: 'json' });
  await openTextMode(page);

  await setMonacoValue(page, 'right-editor', '{\n  "a": 1,\n  "b": [\n    true,\n    false\n  ],\n  "c": {\n    "x": "y"\n  }\n}');
  await runCompare(page);

  await expect(page.getByTestId('right-panel-dropzone')).toHaveAttribute('data-compare-highlight-count', '0');
});

test('structured compare keeps string whitespace diffs', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, { sourceText: '{"msg":"a b"}', language: 'json' });
  await openTextMode(page);

  await setMonacoValue(page, 'right-editor', '{"msg":"a  b"}');
  await runCompare(page);

  await expect
    .poll(async () => Number((await page.getByTestId('right-panel-dropzone').getAttribute('data-compare-highlight-count')) ?? '0'))
    .toBeGreaterThan(0);
});

test('structured compare with unicode strings does not drift into unchanged sibling tokens', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(
    page,
    {
      sourceText: '{"value":{"message":"存在差异：新增 577 行，删除 382 行","type":"info"}}',
      language: 'json',
    },
  );
  await openTextMode(page);

  await setMonacoValue(
    page,
    'right-editor',
    '{"value":{"message":"就你就于：们时 525 有，人那 168 就","type":"dsjk"}}',
  );
  await runCompare(page);

  await expect
    .poll(async () => Number((await page.getByTestId('right-panel-dropzone').getAttribute('data-compare-highlight-count')) ?? '0'))
    .toBeGreaterThan(0);

  await expect
    .poll(async () => readInlineDiffTexts(page, 'right-editor', 'diff-inline-ins'))
    .not.toEqual([]);

  const inlineTexts = await readInlineDiffTexts(page, 'right-editor', 'diff-inline-ins');

  expect(inlineTexts.join('')).not.toContain('type');
  expect(inlineTexts.join('')).not.toContain('}');
  expect(inlineTexts.some((text) => text.includes('168'))).toBe(true);
  expect(inlineTexts.some((text) => text.includes('dsjk'))).toBe(true);
});


test('editing left source clears previous compare highlights', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await setEditorContent(page, { sourceText: '{"token":"base"}', language: 'json' });
  await openTextMode(page);

  await setMonacoValue(page, 'right-editor', '{"token":"changed"}');
  await runCompare(page);
  await expect
    .poll(async () => Number((await page.getByTestId('right-panel-dropzone').getAttribute('data-compare-highlight-count')) ?? '0'))
    .toBeGreaterThan(0);

  await setMonacoValue(page, 'source-editor', '{"token":"edited-left"}');
  await expect(page.getByTestId('right-panel-dropzone')).toHaveAttribute('data-compare-highlight-count', '0');
});
