import { expect, test } from '@playwright/test';

import { evaluateTreease, getMonacoValue, readEditorState, waitForMonacoHook } from './utils';

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
