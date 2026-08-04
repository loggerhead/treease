import { expect, test } from './fixtures';

import { evaluateTreease, getMonacoValue, readEditorState, waitForEditorReady, waitForMonacoHook } from './utils';

test.describe.configure({ timeout: 20_000 });

test('viewer-only preset still initializes hidden editor and executes commands', async ({ page }) => {
  await page.goto('/editor?ui=viewer&text=%7B%22b%22%3A2,%22a%22%3A1%7D&command=format');

  await waitForMonacoHook(page, 'source-editor');
  await waitForMonacoHook(page, 'right-editor');
  await expect
    .poll(async () => (await readEditorState(page)).sourceText.includes('\n'))
    .toBe(true);

  const presetState = await evaluateTreease(page, (treease) => treease.test.getUrlPresetState());
  expect(presetState?.finalUi).toEqual({
    editor: false,
    viewer: true,
    topbar: false,
  });
  await expect(page.getByTestId('left-pane')).toHaveCount(0);
  await expect(page.getByTestId('right-pane')).toHaveCount(1);
});

test('editor and viewer preset keeps the split workspace visible', async ({ page }) => {
  await page.goto('/editor?ui=editor,viewer');

  await waitForEditorReady(page);
  await expect(page.getByTestId('left-pane')).toBeVisible();
  await expect(page.getByTestId('right-pane')).toBeVisible();
  await expect(page.getByTestId('splitter-divider')).toBeVisible();
  await expect(page.getByTestId('monaco-source-editor')).toBeVisible();
});

test('editor-only preset does not keep waiting for graph runtime', async ({ page }) => {
  await page.goto('/editor?ui=editor');

  await waitForMonacoHook(page, 'source-editor');
  await expect(page.getByTestId('left-pane')).toBeVisible();
  await expect(page.getByTestId('right-pane')).toHaveCount(0);
  await expect(page.getByTestId('monaco-source-editor')).toBeVisible();
  await expect(page.getByText('Waiting for graph runtime...')).toHaveCount(0);
});

test('rightText preset forces viewer text mode and populates the right editor', async ({ page }) => {
  await page.goto('/editor?ui=editor&text=%7B%22left%22%3A1%7D&rightText=%7B%22right%22%3A2%7D');

  await waitForMonacoHook(page, 'right-editor');
  await expect.poll(async () => getMonacoValue(page, 'right-editor')).toContain('"right": 2');
  await expect
    .poll(async () => JSON.parse((await readEditorState(page)).tempModel.scratchText))
    .toEqual({ right: 2 });

  const presetState = await evaluateTreease(page, (treease) => treease.test.getUrlPresetState());
  expect(presetState?.finalUi.viewer).toBe(true);
  expect(presetState?.viewerMode).toBe('text');
});

test('url compare command reads the right side from workspace state', async ({ page }) => {
  await page.goto('/editor?text=%7B%22service%22%3A%7B%22port%22%3A8080%7D%7D&rightText=%7B%0A%20%20%22service%22%3A%20%7B%22port%22%3A%209090%7D%0A%7D&command=compare');

  await waitForMonacoHook(page, 'right-editor');
  await expect(page.getByText('Compare completed (differences found)')).toBeVisible();
  await expect.poll(async () => (await readEditorState(page)).tempModel.scratchText).toContain('"port": 9090');
  await expect
    .poll(async () => Number((await page.getByTestId('right-panel-dropzone').getAttribute('data-compare-highlight-count')) ?? '0'))
    .toBeGreaterThan(0);
});

test('url compare waits for right-side inline decorations before completing', async ({ page }) => {
  await page.goto('/editor?text=%7B%0A%20%20%22service%22%3A%20%7B%22name%22%3A%20%22api%22%2C%20%22port%22%3A%208080%7D%2C%0A%20%20%22features%22%3A%20%5B%22graph%22%2C%20%22compare%22%5D%0A%7D&rightText=%7B%0A%20%20%22features%22%3A%20%5B%22graph%22%2C%20%22compare%22%2C%20%22export%22%5D%2C%0A%20%20%22service%22%3A%20%7B%22name%22%3A%20%22api%22%2C%20%22port%22%3A%209090%7D%0A%7D&command=compare');

  await waitForMonacoHook(page, 'right-editor');
  await expect(page.getByText('Compare completed (differences found)')).toBeVisible();
  const rightEditor = page.getByTestId('monaco-right-editor');
  await expect(rightEditor.locator('.diff-inline-ins').filter({ hasText: '"export"' })).toHaveCount(1);
  await expect(rightEditor.locator('.diff-line-ins')).toHaveCount(2);
});

test('textUrl preset fetches same-origin json into the source editor', async ({ page }) => {
  await page.goto('/editor?textUrl=%2Furl-preset%2Fsource.json');

  await waitForEditorReady(page);
  await expect.poll(async () => (await readEditorState(page)).sourceText).toContain('"remote": true');

  const presetState = await evaluateTreease(page, (treease) => treease.test.getUrlPresetState());
  expect(presetState?.recognized.textUrlPresent).toBe(true);
  expect(presetState?.recognized.textUrlEffective).toBe(true);
  expect(await getMonacoValue(page, 'source-editor')).toContain('"items": [');
});

test('missing textUrl shows a toast instead of crashing the page', async ({ page }, testInfo) => {
  testInfo.annotations.push({
    type: 'allow-browser-error',
    description: 'Failed to load resource: the server responded with a status of 404 (Not Found)',
  });
  testInfo.annotations.push({
    type: 'allow-browser-error',
    description: '[editor] failed to apply url preset',
  });

  await page.goto('/editor');
  await waitForEditorReady(page);
  const initialSourceText = (await readEditorState(page)).sourceText;

  await page.goto('/editor?textUrl=%2Furl-preset%2Fmissing.json');

  await waitForEditorReady(page);
  await expect(page.getByText(/Editor URL preset failed:/)).toBeVisible();
  await expect.poll(async () => (await readEditorState(page)).sourceText).toBe(initialSourceText);
});
