import { expect, test, type Page } from './fixtures';
import {
  evaluateTreease,
  getMonacoValue,
  readEditorState,
  readEditorWorkspace,
  setEditorContent,
  setMonacoValue,
  waitForEditorReady,
  waitForMonacoHook,
  waitForSettingsReady,
} from './utils';

async function activeWorkspaceTabId(page: Page): Promise<string> {
  const workspace = await readEditorWorkspace(page);
  return workspace.activeTabId;
}

async function leftTabIds(page: Page): Promise<string[]> {
  const workspace = await readEditorWorkspace(page);
  return workspace.tabOrder;
}

async function openTab(page: Page, tabId: string): Promise<void> {
  await page.getByTestId(`tab-open-${tabId}`).click();
}

async function waitForActiveLeftTabReady(page: Page, tabId: string): Promise<void> {
  await expect.poll(async () => activeWorkspaceTabId(page), { timeout: 5_000 }).toBe(tabId);
  await waitForMonacoHook(page, 'source-editor', 10_000);
  await expect.poll(async () => {
    const fullEditUiState = await evaluateTreease(page, (treease) => treease.editor.getState().fullEditUiState);
    return { active: fullEditUiState.active, phase: fullEditUiState.phase };
  }, { timeout: 5_000 }).toEqual({ active: false, phase: 'idle' });
}

test.beforeEach(async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);
  await waitForSettingsReady(page);
  await evaluateTreease(page, async (treease) => {
    const formatting = treease.settings.getState().settings.formatting;
    if (!formatting.smart) return;
    await treease.settings.save({
      formatting: {
        ...formatting,
        smart: false,
      },
    });
  });
  await waitForSettingsReady(page);
});

test('left editor tabs preserve text and mirror only the active tab into primary authority', async ({ page }) => {
  await setEditorContent(page, {
    language: 'json',
    sourceText: '{"tab":"one"}',
  });
  const [firstTabId] = await leftTabIds(page);
  await expect(page.getByTestId('editor-tab-strip')).toBeVisible({ timeout: 5_000 });
  await expect(page.getByTestId(`tab-open-${firstTabId}`)).toBeVisible({ timeout: 5_000 });

  await page.getByTestId('new-tab-button').click();
  await expect.poll(async () => (await leftTabIds(page)).length, { timeout: 5_000 }).toBe(2);
  const [, secondTabId] = await leftTabIds(page);
  await waitForActiveLeftTabReady(page, secondTabId);

  await setEditorContent(page, {
    language: 'yaml',
    sourceText: 'tab: two\n',
  });
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe('tab: two\n');
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('yaml');

  await openTab(page, firstTabId);
  await waitForActiveLeftTabReady(page, firstTabId);
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe('{"tab":"one"}');
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe('{"tab":"one"}');
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('json');
  await expect
    .poll(async () => (await readEditorWorkspace(page)).tabsById[secondTabId].languageId, { timeout: 5_000 })
    .toBe('yaml');

  await openTab(page, secondTabId);
  await waitForActiveLeftTabReady(page, secondTabId);
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe('tab: two\n');
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe('tab: two\n');
  await expect.poll(async () => (await readEditorState(page)).languageId, { timeout: 5_000 }).toBe('yaml');
  await expect
    .poll(async () => (await readEditorWorkspace(page)).tabsById[firstTabId].languageId, { timeout: 5_000 })
    .toBe('json');
});

test('closing tabs removes only left tabs and never lists the right sidecar tab', async ({ page }) => {
  await setEditorContent(page, {
    language: 'json',
    sourceText: '{"left":1}',
  });
  await page.getByTestId('new-tab-button').click();
  await expect.poll(async () => (await leftTabIds(page)).length, { timeout: 5_000 }).toBe(2);
  const [firstTabId, secondTabId] = await leftTabIds(page);

  await page.getByRole('button', { name: 'Text mode', exact: true }).click();
  await expect(page.getByTestId('monaco-right-editor')).toBeVisible({ timeout: 5_000 });
  await setMonacoValue(page, 'right-editor', '{"right":true}');

  await expect.poll(async () => {
    const workspace = await readEditorWorkspace(page);
    return {
      leftTabs: workspace.tabOrder,
      right: workspace.paneTabIds.right,
      sidecarRole: workspace.paneTabIds.right ? workspace.tabsById[workspace.paneTabIds.right]?.role : null,
    };
  }, { timeout: 5_000 }).toEqual({
    leftTabs: [firstTabId, secondTabId],
    right: 'tab-sidecar',
    sidecarRole: 'sidecar',
  });
  await expect(page.getByTestId('tab-open-tab-sidecar')).toHaveCount(0);

  page.once('dialog', (dialog) => {
    expect(dialog.message()).toContain('without saving local changes');
    void dialog.accept();
  });
  await page.getByTestId(`tab-close-${firstTabId}`).click();
  await expect.poll(async () => await leftTabIds(page), { timeout: 5_000 }).toEqual([secondTabId]);
  await expect.poll(async () => activeWorkspaceTabId(page), { timeout: 5_000 }).toBe(secondTabId);

  const state = await readEditorState(page);
  expect(state.sourceText).not.toBe('{"right":true}');
  expect((await readEditorWorkspace(page)).tabsById['tab-sidecar'].sourceText).toBe('{"right":true}');
});

test('formatting applies to the active left tab without mutating inactive tabs', async ({ page }) => {
  await setEditorContent(page, {
    language: 'json',
    sourceText: '{"first":{"compact":true}}',
  });
  const [firstTabId] = await leftTabIds(page);

  await page.getByTestId('new-tab-button').click();
  await expect.poll(async () => (await leftTabIds(page)).length, { timeout: 5_000 }).toBe(2);
  const [, secondTabId] = await leftTabIds(page);
  await waitForActiveLeftTabReady(page, secondTabId);

  await setEditorContent(page, {
    language: 'json',
    sourceText: '{"second":{"compact":true}}',
  });
  await page.getByRole('button', { name: 'Format', exact: true }).click();
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toContain('\n  "second"');

  await openTab(page, firstTabId);
  await waitForActiveLeftTabReady(page, firstTabId);
  await expect.poll(async () => (await readEditorState(page)).sourceText, { timeout: 5_000 }).toBe('{"first":{"compact":true}}');

  await openTab(page, secondTabId);
  await waitForActiveLeftTabReady(page, secondTabId);
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toContain('\n  "second"');
  await expect.poll(async () => evaluateTreease(page, (treease) => treease.editor.getWorkspace().activeTabId), {
    timeout: 5_000,
  }).toBe(secondTabId);
});
