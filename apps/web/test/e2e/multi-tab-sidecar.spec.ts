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
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe('');

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

test('tab rename keeps the display width and exposes active editing state', async ({ page }) => {
  const [tabId] = await leftTabIds(page);
  const tab = page.locator(`[data-testid="editor-tab"][data-tab-id="${tabId}"]`);
  const before = await tab.boundingBox();
  expect(before).not.toBeNull();

  await page.getByTestId(`tab-open-${tabId}`).dblclick();
  const renameInput = page.getByTestId(`tab-rename-${tabId}`);
  await expect(renameInput).toBeVisible();
  await expect(tab).toHaveAttribute('data-active', 'true');
  await expect(tab).toHaveAttribute('data-renaming', 'true');

  const after = await tab.boundingBox();
  expect(after).not.toBeNull();
  expect(after?.width).toBeCloseTo(before?.width ?? 0, 0);
  await renameInput.press('Escape');
});

test('bottom file menu exposes rename and close actions', async ({ page }) => {
  const [firstTabId] = await leftTabIds(page);
  await page.getByTestId('tab-switcher').click();
  await page.getByTestId(`editor-tab-actions-${firstTabId}`).click();
  await expect(page.getByTestId(`editor-tab-actions-menu-${firstTabId}`)).toBeVisible();
  await page.getByTestId(`editor-tab-action-rename-${firstTabId}`).click();

  const renameInput = page.getByTestId(`editor-tab-rename-${firstTabId}`);
  await renameInput.fill('Renamed from file menu');
  await renameInput.press('Enter');
  await expect.poll(async () => (await readEditorWorkspace(page)).tabsById[firstTabId]?.name, { timeout: 5_000 }).toBe('Renamed from file menu');

  await page.getByTestId('new-tab-button').click();
  await expect.poll(async () => (await leftTabIds(page)).length, { timeout: 5_000 }).toBe(2);
  const [, secondTabId] = await leftTabIds(page);
  await page.getByTestId('tab-switcher').click();
  await page.getByTestId(`editor-tab-actions-${secondTabId}`).click();
  page.once('dialog', (dialog) => void dialog.accept());
  await page.getByTestId(`editor-tab-action-close-${secondTabId}`).click();
  await expect.poll(async () => await leftTabIds(page), { timeout: 5_000 }).toEqual([firstTabId]);
});

test('immediately closing the active new tab keeps header, editor, and authority on its successor', async ({ page }) => {
  await setEditorContent(page, { language: 'json', sourceText: '{"tab":"one"}' });
  const [firstTabId] = await leftTabIds(page);

  await page.getByTestId('new-tab-button').click();
  await expect.poll(async () => (await leftTabIds(page)).length, { timeout: 5_000 }).toBe(2);
  const [, secondTabId] = await leftTabIds(page);
  await page.getByTestId(`tab-close-${secondTabId}`).click();

  await expect.poll(async () => await leftTabIds(page), { timeout: 5_000 }).toEqual([firstTabId]);
  await expect.poll(async () => activeWorkspaceTabId(page), { timeout: 5_000 }).toBe(firstTabId);
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe('{"tab":"one"}');
  await expect.poll(async () => {
    const [workspace, state] = await Promise.all([readEditorWorkspace(page), readEditorState(page)]);
    return state.documentKey === workspace.tabsById[workspace.activeTabId]?.documentKey;
  }, { timeout: 5_000 }).toBe(true);
});

test('restores a persisted multi-tab session before the editor accepts tab commands', async ({ page }) => {
  await page.evaluate(async () => {
    const database = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open('treease-workspace', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('sessions', { keyPath: 'id' });
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction('sessions', 'readwrite');
      transaction.objectStore('sessions').put({
        id: 'current',
        session: {
          version: 1,
          activeTabIndex: 1,
          tabs: [
            { name: 'first.json', languageId: 'json', sourceText: '{"restored":1}', origin: 'user' },
            { name: 'second.json', languageId: 'json', sourceText: '{"restored":2}', origin: 'user' },
          ],
        },
      });
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
    });
    database.close();
  });

  // Leave the first page before opening the recovered workspace so its pending
  // debounced save cannot overwrite the explicitly seeded host session.
  await page.goto('about:blank');
  await page.goto('/editor');
  await waitForEditorReady(page);
  await expect(page.getByTestId('editor-tab-strip')).toBeVisible({ timeout: 5_000 });
  await expect.poll(async () => readEditorWorkspace(page), { timeout: 5_000 }).toMatchObject({
    tabOrder: ['session-tab-0', 'session-tab-1'],
    activeTabId: 'session-tab-1',
    primaryTabId: 'session-tab-1',
  });
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe('{"restored":2}');
  await expect.poll(async () => {
    const [workspace, state] = await Promise.all([readEditorWorkspace(page), readEditorState(page)]);
    return state.documentKey === workspace.tabsById[workspace.activeTabId]?.documentKey;
  }, { timeout: 5_000 }).toBe(true);
});

test('closing tabs removes only left tabs and never lists the right sidecar tab', async ({ page }) => {
  await setEditorContent(page, {
    language: 'json',
    sourceText: '{"left":1}',
  });
  await page.getByTestId('new-tab-button').click();
  await expect.poll(async () => (await leftTabIds(page)).length, { timeout: 5_000 }).toBe(2);
  const [firstTabId, secondTabId] = await leftTabIds(page);

  await page.getByTestId('graph-surface-compare').click();
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

test('closing the last left tab creates a new blank primary and retains the sidecar', async ({ page }) => {
  await setEditorContent(page, { language: 'json', sourceText: '{"left":1}' });
  const [closedTabId] = await leftTabIds(page);
  await page.getByTestId('graph-surface-compare').click();
  await expect(page.getByTestId('monaco-right-editor')).toBeVisible({ timeout: 5_000 });
  await setMonacoValue(page, 'right-editor', '{"right":true}');

  page.once('dialog', (dialog) => void dialog.accept());
  await page.getByTestId(`tab-close-${closedTabId}`).click();
  await expect.poll(async () => leftTabIds(page), { timeout: 5_000 }).toHaveLength(1);
  const [replacementId] = await leftTabIds(page);
  const workspace = await readEditorWorkspace(page);
  expect(replacementId).not.toBe(closedTabId);
  expect(workspace.activeTabId).toBe(replacementId);
  expect(workspace.primaryTabId).toBe(replacementId);
  expect(workspace.tabsById[replacementId]).toMatchObject({ sourceText: '', role: 'primary' });
  expect(workspace.paneTabIds.right).toBe('tab-sidecar');
  expect(workspace.tabsById['tab-sidecar']).toMatchObject({ role: 'sidecar', sourceText: '{"right":true}' });
  await expect.poll(async () => getMonacoValue(page, 'source-editor'), { timeout: 5_000 }).toBe('');
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
