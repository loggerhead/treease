const SETTINGS_DB_NAME = 'treease-settings';
const WORKSPACE_DB_NAME = 'treease-workspace';
const PRESERVED_INDEXED_DB_NAMES = new Set(['treease-usage']);

export async function resetBrowserLocalState(): Promise<void> {
  const databases = typeof indexedDB.databases === 'function' ? await indexedDB.databases() : [];
  const databaseNames = new Set([
    SETTINGS_DB_NAME,
    WORKSPACE_DB_NAME,
    ...databases.map((database) => database.name).filter((name): name is string => Boolean(name)),
  ]);

  await Promise.all([...databaseNames]
    .filter((name) => !PRESERVED_INDEXED_DB_NAMES.has(name))
    .map((name) => new Promise<void>((resolve, reject) => {
      const request = indexedDB.deleteDatabase(name);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error ?? new Error(`Failed to delete IndexedDB database: ${name}`));
      request.onblocked = () => reject(new Error(`IndexedDB database is still open: ${name}`));
    })));

  window.localStorage.clear();
  window.sessionStorage.clear();
  for (const cookie of document.cookie.split(';')) {
    const name = cookie.split('=', 1)[0]?.trim();
    if (name) document.cookie = `${name}=; Path=/; Max-Age=0; SameSite=Lax`;
  }
}
