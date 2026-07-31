import {
  WorkspaceHostUnavailableError,
  type FileAccessGrant,
  type WorkspaceHost,
  type WorkspaceOpenFileOptions,
  type WorkspaceSession,
  type WorkspaceSaveTextOptions,
} from './contract';

const WORKSPACE_DB_NAME = 'treease-workspace';
const WORKSPACE_DB_VERSION = 1;
const WORKSPACE_STORE_NAME = 'sessions';
const WORKSPACE_SESSION_KEY = 'current';

type StoredWorkspaceSession = {
  id: string;
  session: WorkspaceSession;
};

function openWorkspaceDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(WORKSPACE_DB_NAME, WORKSPACE_DB_VERSION);
    request.onupgradeneeded = () => {
      request.result.createObjectStore(WORKSPACE_STORE_NAME, { keyPath: 'id' });
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('Failed to open workspace database.'));
  });
}

async function saveBrowserWorkspaceSession(session: WorkspaceSession): Promise<void> {
  const database = await openWorkspaceDatabase();
  await new Promise<void>((resolve, reject) => {
    const transaction = database.transaction(WORKSPACE_STORE_NAME, 'readwrite');
    transaction.objectStore(WORKSPACE_STORE_NAME).put({ id: WORKSPACE_SESSION_KEY, session } satisfies StoredWorkspaceSession);
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error('Failed to save workspace session.'));
    transaction.onabort = () => reject(transaction.error ?? new Error('Workspace session save was aborted.'));
  });
  database.close();
}

async function loadBrowserWorkspaceSession(): Promise<WorkspaceSession | null> {
  const database = await openWorkspaceDatabase();
  const stored = await new Promise<StoredWorkspaceSession | undefined>((resolve, reject) => {
    const transaction = database.transaction(WORKSPACE_STORE_NAME, 'readonly');
    const request = transaction.objectStore(WORKSPACE_STORE_NAME).get(WORKSPACE_SESSION_KEY);
    request.onsuccess = () => resolve(request.result as StoredWorkspaceSession | undefined);
    request.onerror = () => reject(request.error ?? new Error('Failed to load workspace session.'));
  });
  database.close();
  return stored?.session ?? null;
}

function chooseBrowserFile(options: WorkspaceOpenFileOptions): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = options.accept.join(',');
    input.addEventListener('change', () => resolve(input.files?.[0] ?? null), { once: true });
    input.click();
  });
}

function downloadBrowserText({ fileName, text, mimeType }: WorkspaceSaveTextOptions): void {
  const url = URL.createObjectURL(new Blob([text], { type: mimeType }));
  const link = document.createElement('a');
  link.href = url;
  link.download = fileName;
  link.click();
  URL.revokeObjectURL(url);
}

export const browserWorkspaceHost: WorkspaceHost = {
  surface: 'browser',
  openFile: chooseBrowserFile,
  async saveText(options) {
    downloadBrowserText(options);
  },
  async readFile(_grant: FileAccessGrant) {
    throw new Error('Browser files cannot be read after their initial selection.');
  },
  async saveFile(_grant: FileAccessGrant, _text: string) {
    throw new Error('Browser files must be exported through a download.');
  },
  async saveFileAs(options) {
    downloadBrowserText(options);
    return null;
  },
  async watchFile() {
    return () => {};
  },
  async listRecentFiles() {
    return [];
  },
  async openRecentFile() {
    return null;
  },
  async clearRecentFiles() {},
  async onFilesDropped() {
    return () => {};
  },
  async takeStartupFiles() {
    return [];
  },
  async saveSession(session) {
    await saveBrowserWorkspaceSession(session);
  },
  async loadSession() {
    return loadBrowserWorkspaceSession();
  },
  async onCommand() {
    return () => {};
  },
  async storeRefreshToken() {
    throw new WorkspaceHostUnavailableError('Desktop credential storage');
  },
  async hasRefreshToken() {
    return false;
  },
  async refreshSession() {
    throw new WorkspaceHostUnavailableError('Desktop credential refresh');
  },
  async checkForUpdate() {
    return null;
  },
  async installCheckedUpdate() {
    throw new WorkspaceHostUnavailableError('Desktop updater');
  },
  async clearRefreshToken() {},
  async openExternal(url) {
    window.open(url, '_blank', 'noopener,noreferrer');
  },
  async getInitialDeepLinks() {
    return [];
  },
  async onDeepLinks() {
    return () => {};
  },
};
