import { writable, get, derived, type Readable } from 'svelte/store';
import { openDB } from 'idb';
import {
  defaultSettings,
  mergeSettings,
  mergeSettingsDocument,
  sanitizeSettingsDocument,
  type Settings,
  type SettingsDocument
} from './ui-settings';
import {
  getColumnNavigatorHeight,
  getEditorSplitRatio,
  mergeEditorLayoutState,
  omitEditorLayoutState,
  withColumnNavigatorHeight,
  withEditorSplitRatio,
} from './editor-layout-state';
import { clearEditorSplitRatioCookie, writeEditorSplitRatioCookie } from './editor-layout-cookie';
import { handleError } from '../utils/error-handler';

const DB_NAME = 'treease-settings';
const STORE_NAME = 'settings';
const SETTINGS_KEY = 'user';
let settingsDbPromise: ReturnType<typeof openDB> | null = null;

export type SettingsStatus = 'idle' | 'loading' | 'ready' | 'error';

type SettingsState = {
  document: SettingsDocument;
  settings: Settings;
  status: SettingsStatus;
};

const initialSettingsState: SettingsState = {
  document: mergeSettings(defaultSettings, {}),
  settings: mergeSettings(defaultSettings, {}),
  status: 'idle'
};

async function getDb() {
  if (!settingsDbPromise) {
    const dbPromise = openDB(DB_NAME, 1, {
      upgrade(db) {
        if (!db.objectStoreNames.contains(STORE_NAME)) {
          db.createObjectStore(STORE_NAME);
        }
      }
    });
    settingsDbPromise = dbPromise;
    void dbPromise.then((db) => {
      db.onversionchange = () => {
        db.close();
        if (settingsDbPromise === dbPromise) settingsDbPromise = null;
      };
    }).catch(() => {
      if (settingsDbPromise === dbPromise) settingsDbPromise = null;
    });
  }
  return settingsDbPromise;
}

async function closePersistence(): Promise<void> {
  const dbPromise = settingsDbPromise;
  settingsDbPromise = null;
  if (!dbPromise) return;
  try {
    (await dbPromise).close();
  } catch {
    // The connection failure is handled by the operation that opened it.
  }
}

const internalStore = writable<SettingsState>(initialSettingsState);

function migrateLegacySettingsDocument(document: SettingsDocument): SettingsDocument {
  if (!document || typeof document !== 'object' || Array.isArray(document)) return document;
  const current = document as Record<string, any>;
  const semanticTypeColors = current.editor?.semanticTypeColors;
  if (!semanticTypeColors || typeof semanticTypeColors !== 'object') return document;

  let changed = false;
  const nextSemanticTypeColors = { ...semanticTypeColors };

  if (semanticTypeColors.boolean === '#a31515') {
    nextSemanticTypeColors.boolean = defaultSettings.editor.semanticTypeColors.boolean;
    changed = true;
  }
  if (semanticTypeColors.nil === '#d1004d') {
    nextSemanticTypeColors.nil = defaultSettings.editor.semanticTypeColors.nil;
    changed = true;
  }
  if (!changed) return document;

  return {
    ...current,
    editor: {
      ...current.editor,
      semanticTypeColors: nextSemanticTypeColors,
    },
  };
}

function createSettingsStore() {
  return {
    subscribe: internalStore.subscribe,
    closePersistence,
    actions: {
      setLoading: () => internalStore.update(s => ({ ...s, status: 'loading' })),
      setReady: () => internalStore.update(s => ({ ...s, status: 'ready' })),
      setError: () => internalStore.update(s => ({ ...s, status: 'error' })),
      setSettings: (settings: Settings) => internalStore.update(s => ({ ...s, document: settings, settings, status: 'ready' })),
      updateSettings: (partial: Partial<Settings>) => internalStore.update(s => ({
        ...s,
        document: mergeSettingsDocument(s.document, partial),
        settings: sanitizeSettingsDocument(mergeSettingsDocument(s.document, partial))
      }))
    },
    load: async () => {
      internalStore.update(s => ({ ...s, status: 'loading' }));
      try {
        const db = await getDb();
        const stored = await db.get(STORE_NAME, SETTINGS_KEY);
        const document = migrateLegacySettingsDocument(stored ?? mergeSettings(defaultSettings, {}));
        const settings = sanitizeSettingsDocument(document);
        internalStore.set({ document, settings, status: 'ready' });
      } catch (error) {
        handleError(error, { component: 'SettingsStore', operation: 'load' });
        internalStore.set({
          document: mergeSettings(defaultSettings, {}),
          settings: mergeSettings(defaultSettings, {}),
          status: 'error'
        });
      }
    },
    save: async (next: Partial<Settings>) => {
      const currentState = get(internalStore);
      try {
        const db = await getDb();
        const stored = await db.get(STORE_NAME, SETTINGS_KEY);
        const document = migrateLegacySettingsDocument(mergeSettingsDocument(stored ?? currentState.document, next));
        const settings = sanitizeSettingsDocument(document);
        internalStore.set({ document, settings, status: currentState.status });
        await db.put(STORE_NAME, document, SETTINGS_KEY);
        internalStore.update(s => ({ ...s, status: 'ready' }));
      } catch (error) {
        handleError(error, {
          component: 'SettingsStore',
          operation: 'save',
          metadata: { settingsKeys: Object.keys(next) }
        });
        internalStore.update(s => ({ ...s, status: 'error' }));
      }
    },
    saveDocument: async (document: SettingsDocument) => {
      const nextDocument = migrateLegacySettingsDocument(document);
      const settings = sanitizeSettingsDocument(nextDocument);
      const currentState = get(internalStore);
      internalStore.set({ document: nextDocument, settings, status: currentState.status });
      try {
        const db = await getDb();
        await db.put(STORE_NAME, nextDocument, SETTINGS_KEY);
        internalStore.update(s => ({ ...s, status: 'ready' }));
      } catch (error) {
        handleError(error, { component: 'SettingsStore', operation: 'saveDocument' });
        internalStore.update(s => ({ ...s, status: 'error' }));
      }
    },
    saveSettingsDialogDocument: async (document: SettingsDocument) => {
      const currentState = get(internalStore);
      await settingsStore.saveDocument(mergeEditorLayoutState(document, currentState.document));
    },
    saveEditorSplitRatio: async (splitRatio: number) => {
      const currentState = get(internalStore);
      const document = withEditorSplitRatio(currentState.document, splitRatio);
      writeEditorSplitRatioCookie(splitRatio);
      if (document === currentState.document) return;
      await settingsStore.saveDocument(document);
    },
    getEditorSplitRatio: () => getEditorSplitRatio(get(internalStore).document),
    saveColumnNavigatorHeight: async (heightPx: number) => {
      const currentState = get(internalStore);
      const document = withColumnNavigatorHeight(currentState.document, heightPx);
      if (document === currentState.document) return;
      await settingsStore.saveDocument(document);
    },
    getColumnNavigatorHeight: () => getColumnNavigatorHeight(get(internalStore).document),
    reset: async () => {
      const document = mergeSettings(defaultSettings, {});
      internalStore.set({ document, settings: document, status: 'ready' });
      clearEditorSplitRatioCookie();
      try {
        const db = await getDb();
        await db.put(STORE_NAME, document, SETTINGS_KEY);

      } catch (error) {
        handleError(error, { component: 'SettingsStore', operation: 'reset' });
        internalStore.update(s => ({ ...s, status: 'error' }));
      }
    },
    get: () => get(internalStore)
  };
}

export const settingsStore = createSettingsStore();

export const settingsDocument: Readable<SettingsDocument> = derived(internalStore, $s => $s.document);
export const settingsDialogDocument: Readable<SettingsDocument> = derived(internalStore, $s => omitEditorLayoutState($s.document));
export const settings: Readable<Settings> = derived(internalStore, $s => $s.settings);
export const settingsStatus: Readable<SettingsStatus> = derived(internalStore, $s => $s.status);
