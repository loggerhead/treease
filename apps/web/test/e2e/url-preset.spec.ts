import { expect, test } from './fixtures';

import { evaluateTreease, getMonacoValue, readEditorState, waitForEditorReady, waitForMonacoHook } from './utils';

test.describe.configure({ timeout: 20_000 });

test('viewer-only preset still initializes hidden editor and executes commands', async ({ page }) => {
  await page.goto('/editor?ui=viewer&text=%7B%22b%22%3A2,%22a%22%3A1%7D&command=format');

  await waitForMonacoHook(page, 'source-editor');
  await expect
    .poll(async () => (await readEditorState(page)).sourceText.includes('\n'))
    .toBe(true);

  const presetState = await evaluateTreease(page, (treease) => treease.test.getUrlPresetState());
  expect(presetState?.finalUi).toEqual({
    editor: false,
    viewer: true,
    topbar: false,
    bottombar: false,
  });
  await expect(page.getByTestId('left-pane')).toHaveCount(0);
  await expect(page.getByTestId('right-pane')).toHaveCount(1);
});

test('rightText preset forces viewer text mode and populates the right editor', async ({ page }) => {
  await page.goto('/editor?ui=editor&text=%7B%22left%22%3A1%7D&rightText=%7B%22right%22%3A2%7D');

  await waitForMonacoHook(page, 'right-editor');
  await expect.poll(async () => getMonacoValue(page, 'right-editor')).toContain('"right": 2');

  const presetState = await evaluateTreease(page, (treease) => treease.test.getUrlPresetState());
  expect(presetState?.finalUi.viewer).toBe(true);
  expect(presetState?.viewerMode).toBe('text');
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
