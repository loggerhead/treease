import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDbStore: Record<string, any> = {};
const mockDb = {
  get: vi.fn(async (_store: string, key: string) => mockDbStore[key]),
  put: vi.fn(async (_store: string, value: any, key: string) => { mockDbStore[key] = value; }),
};

vi.mock('idb', () => ({
  openDB: vi.fn(async () => mockDb),
}));

vi.mock('../utils/error-handler', () => ({
  handleError: vi.fn(),
}));

import {
  settingsStore,
  settings,
  settingsDialogDocument,
  settingsDocument,
  settingsStatus,
} from './settings-store';
import { defaultSettings } from './ui-settings';

describe('settings-store', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const key of Object.keys(mockDbStore)) delete mockDbStore[key];
  });

  it('initial state has idle status and default settings', () => {
    const state = settingsStore.get();
    expect(state.status).toBe('idle');
    expect(state.document).toEqual(defaultSettings);
    expect(state.settings).toEqual(defaultSettings);
  });

  it('actions.setLoading sets status to loading', () => {
    settingsStore.actions.setLoading();
    expect(settingsStore.get().status).toBe('loading');
  });

  it('actions.setReady sets status to ready', () => {
    settingsStore.actions.setReady();
    expect(settingsStore.get().status).toBe('ready');
  });

  it('actions.setError sets status to error', () => {
    settingsStore.actions.setError();
    expect(settingsStore.get().status).toBe('error');
  });

  it('actions.setSettings replaces settings and sets ready', () => {
    const custom = {
      ...defaultSettings,
      parser: {
        enableNest: false
      }
    };
    settingsStore.actions.setSettings(custom);
    const state = settingsStore.get();
    expect(state.status).toBe('ready');
    expect(state.document).toEqual(custom);
    expect(state.settings.parser.enableNest).toBe(false);
  });

  it('actions.updateSettings merges partial settings into effective state and document', () => {
    settingsStore.actions.updateSettings({ formatting: { indent: 4 } } as any);
    const state = settingsStore.get();
    expect(state.document).toEqual(expect.objectContaining({
      formatting: expect.objectContaining({ indent: 4 })
    }));
    expect(state.settings.formatting.indent).toBe(4);
  });

  it('load preserves raw document and sanitizes effective settings', async () => {
    mockDbStore['user'] = { formatting: { indent: 'bad' } };
    await settingsStore.load();
    const state = settingsStore.get();
    expect(state.status).toBe('ready');
    expect(state.document).toEqual({ formatting: { indent: 'bad' } });
    expect(state.settings.formatting.indent).toBe(defaultSettings.formatting.indent);
    expect(mockDb.get).toHaveBeenCalled();
  });

  it('loads a persisted editor split ratio without adding it to effective settings', async () => {
    mockDbStore['user'] = { __treeaseEditorLayout: { splitRatio: 0.42 } };
    await settingsStore.load();
    expect(settingsStore.getEditorSplitRatio()).toBe(0.42);
    expect(settingsStore.get().settings).not.toHaveProperty('__treeaseEditorLayout');
  });

  it('load migrates legacy boolean and nil editor colors', async () => {
    mockDbStore['user'] = {
      editor: {
        semanticTypeColors: {
          boolean: '#a31515',
          nil: '#d1004d'
        }
      }
    };
    await settingsStore.load();
    const state = settingsStore.get();
    expect(state.document).toEqual({
      editor: {
        semanticTypeColors: {
          boolean: defaultSettings.editor.semanticTypeColors.boolean,
          nil: defaultSettings.editor.semanticTypeColors.nil
        }
      }
    });
    expect(state.settings.editor.semanticTypeColors.boolean).toBe(defaultSettings.editor.semanticTypeColors.boolean);
    expect(state.settings.editor.semanticTypeColors.nil).toBe(defaultSettings.editor.semanticTypeColors.nil);
  });

  it('load falls back to defaults when db returns nothing', async () => {
    await settingsStore.load();
    const state = settingsStore.get();
    expect(state.status).toBe('ready');
    expect(state.document).toEqual(defaultSettings);
    expect(state.settings).toEqual(defaultSettings);
  });

  it('load sets error status when db throws', async () => {
    mockDb.get.mockRejectedValueOnce(new Error('DB error'));
    await settingsStore.load();
    const state = settingsStore.get();
    expect(state.status).toBe('error');
  });

  it('save persists correct value to IndexedDB and updates store', async () => {
    await settingsStore.save({
      viewer: {
        graphViewer: {
          layout: {
            baseFontSize: 16
          }
        }
      } as any
    });
    expect(mockDb.put).toHaveBeenCalled();
    const storedValue = mockDb.put.mock.calls[0][1];
    expect(storedValue.viewer.graphViewer.layout.baseFontSize).toBe(16);
    const state = settingsStore.get();
    expect(state.status).toBe('ready');
    expect(state.document).toEqual(storedValue);
    expect(state.settings.viewer.graphViewer.layout.baseFontSize).toBe(16);
  });

  it('save sets error status when db throws', async () => {
    mockDb.put.mockRejectedValueOnce(new Error('write fail'));
    await settingsStore.save({ parser: { enableNest: false } });
    const state = settingsStore.get();
    expect(state.status).toBe('error');
  });

  it('saveDocument preserves invalid raw values while applying defaults effectively', async () => {
    await settingsStore.saveDocument({ formatting: { indent: 'bad' } });
    const state = settingsStore.get();
    expect(state.document).toEqual({ formatting: { indent: 'bad' } });
    expect(state.settings.formatting.indent).toBe(defaultSettings.formatting.indent);
    expect(mockDbStore.user).toEqual({ formatting: { indent: 'bad' } });
  });

  it('saveDocument migrates legacy boolean and nil editor colors', async () => {
    await settingsStore.saveDocument({
      editor: {
        semanticTypeColors: {
          boolean: '#a31515',
          nil: '#d1004d'
        }
      }
    });
    const state = settingsStore.get();
    expect(state.document).toEqual({
      editor: {
        semanticTypeColors: {
          boolean: defaultSettings.editor.semanticTypeColors.boolean,
          nil: defaultSettings.editor.semanticTypeColors.nil
        }
      }
    });
    expect(mockDbStore.user).toEqual({
      editor: {
        semanticTypeColors: {
          boolean: defaultSettings.editor.semanticTypeColors.boolean,
          nil: defaultSettings.editor.semanticTypeColors.nil
        }
      }
    });
  });

  it('save merges partial settings into an existing raw document', async () => {
    await settingsStore.saveDocument({ formatting: { indent: 'bad' }, customFlag: true });
    await settingsStore.save({ parser: { enableNest: false } });
    const state = settingsStore.get();
    expect(state.document).toEqual({
      formatting: { indent: 'bad' },
      customFlag: true,
      parser: { enableNest: false }
    });
    expect(state.settings.formatting.indent).toBe(defaultSettings.formatting.indent);
    expect(state.settings.parser.enableNest).toBe(false);
  });

  it('persists an editor split ratio without applying it as a user setting', async () => {
    await settingsStore.saveEditorSplitRatio(0.42);
    const state = settingsStore.get();
    expect(settingsStore.getEditorSplitRatio()).toBe(0.42);
    expect(state.document).toEqual(expect.objectContaining({
      __treeaseEditorLayout: { splitRatio: 0.42 }
    }));
    expect(state.settings).not.toHaveProperty('__treeaseEditorLayout');
    expect(mockDbStore.user).toEqual(expect.objectContaining({
      __treeaseEditorLayout: { splitRatio: 0.42 }
    }));
  });

  it('keeps editor layout state out of the Settings dialog document and preserves it on dialog save', async () => {
    await settingsStore.saveEditorSplitRatio(0.42);
    let dialogDocument: unknown;
    const unsubscribe = settingsDialogDocument.subscribe((value) => { dialogDocument = value; });
    expect(dialogDocument).not.toHaveProperty('__treeaseEditorLayout');
    unsubscribe();

    await settingsStore.saveSettingsDialogDocument({ parser: { enableNest: false } });
    expect(settingsStore.getEditorSplitRatio()).toBe(0.42);
    expect(settingsStore.get().document).toEqual(expect.objectContaining({
      parser: { enableNest: false },
      __treeaseEditorLayout: { splitRatio: 0.42 }
    }));
  });

  it('reset saves defaultSettings to IndexedDB and resets store to defaults', async () => {
    await settingsStore.saveDocument({ formatting: { indent: 'bad' } });
    mockDb.put.mockClear();

    await settingsStore.reset();
    expect(mockDb.put).toHaveBeenCalled();
    const storedValue = mockDb.put.mock.calls[0][1];
    expect(storedValue).toEqual(defaultSettings);
    const state = settingsStore.get();
    expect(state.status).toBe('ready');
    expect(state.document).toEqual(defaultSettings);
    expect(state.settings).toEqual(defaultSettings);
  });

  it('reset sets error status when db throws', async () => {
    mockDb.put.mockRejectedValueOnce(new Error('reset fail'));
    await settingsStore.reset();
    const state = settingsStore.get();
    expect(state.status).toBe('error');
  });

  it('derived settings store reflects internal state', () => {
    settingsStore.actions.setSettings({
      ...defaultSettings,
      parser: {
        enableNest: false
      }
    });
    let val: typeof defaultSettings | undefined;
    const unsub = settings.subscribe((v) => { val = v; });
    expect(val?.parser.enableNest).toBe(false);
    unsub();
  });

  it('derived settingsDocument reflects raw state', () => {
    settingsStore.actions.setSettings({
      ...defaultSettings,
      interaction: {
        enableSyncScroll: false,
        autoSave: 'off',
      }
    });
    let val: unknown;
    const unsub = settingsDocument.subscribe((v) => { val = v; });
    expect(val).toEqual(expect.objectContaining({
      interaction: expect.objectContaining({ enableSyncScroll: false })
    }));
    unsub();
  });

  it('derived settingsStatus reflects status', () => {
    settingsStore.actions.setLoading();
    let val: any;
    const unsub = settingsStatus.subscribe((v) => { val = v; });
    expect(val).toBe('loading');
    unsub();
  });
});
