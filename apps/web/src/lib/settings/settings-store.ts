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

function replaceLegacyDefaultColors(
  colors: Record<string, unknown> | undefined,
  replacements: Readonly<Record<string, readonly [string | readonly string[], string]>>,
): Record<string, unknown> | undefined {
  if (!colors) return colors;
  let nextColors = colors;
  for (const [key, [legacy, replacement]] of Object.entries(replacements)) {
    const legacyValues = Array.isArray(legacy) ? legacy : [legacy];
    if (!legacyValues.includes(colors[key] as string)) continue;
    if (nextColors === colors) nextColors = { ...colors };
    nextColors[key] = replacement;
  }
  return nextColors;
}

function migrateLegacySettingsDocument(document: SettingsDocument): SettingsDocument {
  if (!document || typeof document !== 'object' || Array.isArray(document)) return document;
  const current = document as Record<string, any>;
  const editor = current.editor as Record<string, any> | undefined;
  const viewer = current.viewer as Record<string, any> | undefined;
  const semanticTypeColors = editor?.semanticTypeColors as Record<string, unknown> | undefined;
  const uiColors = editor?.uiColors as Record<string, unknown> | undefined;
  const graphViewer = viewer?.graphViewer as Record<string, any> | undefined;
  const graphColors = graphViewer?.colors as Record<string, any> | undefined;

  const nextSemanticTypeColors = replaceLegacyDefaultColors(semanticTypeColors, {
    boolean: ['#a31515', defaultSettings.editor.semanticTypeColors.boolean],
    nil: ['#d1004d', defaultSettings.editor.semanticTypeColors.nil],
  });
  const nextUiColors = replaceLegacyDefaultColors(uiColors, {
    'editor.background': [['#ffffff', '#fdfcf9'], '#ffffff'],
    'editor.foreground': [['#0f172a', '#1d2735'], '#294c66'],
    'editorLineNumber.foreground': [['#64748b', '#9aa4aa'], '#9caebb'],
    'editorLineNumber.activeForeground': [['#0f172a', '#687181'], '#6d8292'],
    'editorCursor.foreground': [['#0f172a', '#315c94'], '#286b90'],
    'editor.selectionBackground': [['#dbeafe', '#eaf0f8'], '#eaf4fb'],
    'editor.selectionHighlightBackground': [['#dbeafe', '#eaf0f8'], '#eaf4fb'],
    'editorOverviewRuler.background': [['#ffffff', '#fdfcf9'], '#ffffff'],
    'editorOverviewRuler.border': [['#e2e8f0', '#d8d9d6'], '#cbd9e3'],
  });
  const nextNodeColors = replaceLegacyDefaultColors(graphColors?.node, {
    background: [['#ffffff', '#fdfcf9'], '#ffffff'],
    border: [['#00000040', '#bfc2c2'], '#afc4d4'],
  });
  const nextTableColors = replaceLegacyDefaultColors(graphColors?.table, {
    background: [['#ffffff', '#fdfcf9'], '#ffffff'],
    border: [['#00000040', '#bfc2c2'], '#afc4d4'],
    headerBackground: [['#f1f5f9', '#f6f5f1'], '#f6f9fb'],
    headerBorder: [['#00000040', '#bfc2c2'], '#d5e1e9'],
    rowBackground: [['#ffffff', '#fdfcf9'], '#ffffff'],
    rowBorder: [['#00000040', '#d8d9d6'], '#e0e8ee'],
    hoverRowBackground: [['#e6f0ff', '#eaf0f8'], '#eaf4fb'],
    hoverCellBackground: [['#ffe27a', '#f6e6b8'], '#fff2c8'],
    trackBackground: [['#f8fafc', '#f6f5f1'], '#f2f6f9'],
    trackBorder: [['#e2e8f0', '#d8d9d6'], '#cbd9e3'],
    thumbBackground: [['#cbd5e1', '#bfc2c2'], '#9bb0c0'],
  });
  const nextGraphColors = replaceLegacyDefaultColors(graphColors, {
    textMuted: [['#6b7280', '#687181'], '#74899a'],
    edge: [['#cbd5e1', '#c7d1de'], '#aec9dc'],
  });
  const normalizedGraphColors = nextGraphColors === graphColors && nextNodeColors === graphColors?.node && nextTableColors === graphColors?.table
    ? graphColors
    : { ...nextGraphColors, node: nextNodeColors, table: nextTableColors };
  const normalizedEditor = nextSemanticTypeColors === semanticTypeColors && nextUiColors === uiColors
    ? editor
    : {
      ...editor,
      ...(nextSemanticTypeColors !== semanticTypeColors ? { semanticTypeColors: nextSemanticTypeColors } : {}),
      ...(nextUiColors !== uiColors ? { uiColors: nextUiColors } : {}),
    };
  const normalizedViewer = normalizedGraphColors === graphColors
    ? viewer
    : { ...viewer, graphViewer: { ...graphViewer, colors: normalizedGraphColors } };

  if (normalizedEditor === editor && normalizedViewer === viewer) return document;
  return {
    ...current,
    ...(normalizedEditor !== editor ? { editor: normalizedEditor } : {}),
    ...(normalizedViewer !== viewer ? { viewer: normalizedViewer } : {}),
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
