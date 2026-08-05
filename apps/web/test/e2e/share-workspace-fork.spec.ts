import { expect, test, type Page } from './fixtures';
import { evaluateTreease, getMonacoValue, readEditorWorkspace, waitForEditorReady, waitForMonacoHook } from './utils';

const shareId = '7f4f2e7b-2d5d-4b76-8d52-91e8b6b3a201';

const persistedSession = {
  version: 1 as const,
  activeTabIndex: 1,
  tabs: [
    { name: 'first.json', languageId: 'json', sourceText: '{"local":1}', origin: 'user' as const },
    { name: 'second.yaml', languageId: 'yaml', sourceText: 'local: 2\n', origin: 'user' as const },
  ],
};

async function writeSession(page: Page, session = persistedSession): Promise<void> {
  await page.evaluate(async (value) => {
    const database = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open('treease-workspace', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('sessions', { keyPath: 'id' });
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction('sessions', 'readwrite');
      transaction.objectStore('sessions').put({ id: 'current', session: value });
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
    });
    database.close();
  }, session);
}

async function readSession(page: Page): Promise<typeof persistedSession | null> {
  return page.evaluate(async () => {
    const database = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open('treease-workspace', 1);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    const stored = await new Promise<{ session?: typeof persistedSession } | undefined>((resolve, reject) => {
      const transaction = database.transaction('sessions', 'readonly');
      const request = transaction.objectStore('sessions').get('current');
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    database.close();
    return stored?.session ?? null;
  });
}

async function mockShare(page: Page): Promise<void> {
  await page.route('**/v1/public/shares/**', async (route) => {
    await route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify({
        resourceType: 'text_snapshot',
        resourcePayload: {
          schemaVersion: 1,
          left: { text: '{"shared":1}', languageId: 'json' },
          right: null,
          layout: { viewMode: 'graph', activePane: 'left' },
          interaction: { treePath: [], focus: null, columnNavigator: { activePath: [] } },
        },
      }),
    });
  });
}

async function readJsonEditorValue(page: Page): Promise<unknown> {
  return JSON.parse(await getMonacoValue(page, 'source-editor'));
}

test('share browsing leaves the local session untouched', async ({ page }) => {
  await page.goto('/editor');
  await writeSession(page);
  await page.goto('about:blank');
  await mockShare(page);
  await page.goto(`/editor?shareID=${shareId}`);
  await waitForMonacoHook(page, 'source-editor');
  await expect.poll(() => readJsonEditorValue(page)).toEqual({ shared: 1 });

  await page.waitForTimeout(500);
  expect(await readSession(page)).toEqual(persistedSession);
  await expect.poll(async () => (await readEditorWorkspace(page)).tabOrder).toEqual(['primary']);
});

test('first direct edit publishes merged topology once and refresh restores it', async ({ page }) => {
  await page.goto('/editor');
  await writeSession(page);
  await page.goto('about:blank');
  await mockShare(page);
  await page.goto(`/editor?shareID=${shareId}`);
  await waitForMonacoHook(page, 'source-editor');
  await expect.poll(() => readJsonEditorValue(page)).toEqual({ shared: 1 });

  await evaluateTreease(page, (treease) => {
    treease.editor.applyEdits('source-editor', [{
      range: { startLineNumber: 1, startColumn: 12, endLineNumber: 1, endColumn: 13 },
      text: '2',
    }]);
  });

  await expect.poll(async () => await readEditorWorkspace(page)).toMatchObject({
    tabOrder: ['session-tab-0', 'session-tab-1', 'primary'],
    activeTabId: 'primary',
    primaryTabId: 'primary',
  });
  await expect.poll(() => readJsonEditorValue(page)).toEqual({ shared: 2 });
  await expect.poll(() => new URL(page.url()).searchParams.has('shareID')).toBe(false);
  await expect.poll(async () => (await readSession(page))?.tabs.map((tab) => tab.sourceText)).toEqual([
    '{"local":1}',
    'local: 2\n',
    '{"shared": 2}\n',
  ]);

  await page.reload();
  await waitForEditorReady(page);
  await expect.poll(async () => (await readEditorWorkspace(page)).tabOrder).toEqual(['session-tab-0', 'session-tab-1', 'session-tab-2']);
  await expect.poll(() => readJsonEditorValue(page)).toEqual({ shared: 2 });
});
