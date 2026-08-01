import { expect, test, type Page } from './fixtures';
import { waitForEditorReady, waitForGraphRendered } from './utils';

type WorkspaceSession = {
  version: 1;
  activeTabIndex: number;
  tabs: Array<{
    name: string;
    languageId: string;
    sourceText: string;
    origin: 'user';
  }>;
};

async function seedWorkspaceSession(page: Page, session: WorkspaceSession) {
  await page.evaluate(async (value: WorkspaceSession) => {
    const database = await new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open('treease-workspace', 1);
      request.onupgradeneeded = () => {
        if (!request.result.objectStoreNames.contains('sessions')) {
          request.result.createObjectStore('sessions', { keyPath: 'id' });
        }
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction('sessions', 'readwrite');
      transaction.objectStore('sessions').put({ id: 'current', session: value });
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
    database.close();
  }, session);
}

test('restored compact source still lets editor and graph finish startup', async ({ page }) => {
  await page.goto('/editor');
  await waitForEditorReady(page);

  await seedWorkspaceSession(page, {
    version: 1,
    activeTabIndex: 0,
    tabs: [{
      name: 'Recovered compact JSON',
      languageId: 'json',
      sourceText: '{"a":1}',
      origin: 'user',
    }],
  });

  await page.reload();

  // On origin/main the initial full-edit returns SnapshotReady, then startup
  // reports stale and leaves the editor behind its loading overlay. The
  // fixtures also fail on the resulting pageerror.
  await waitForEditorReady(page);
  await Promise.all([
    expect(page.getByRole('status', { name: 'Editor loading status' })).toHaveCount(0),
    waitForGraphRendered(page),
  ]);
});
