<script lang="ts">
  import type * as Monaco from 'monaco-editor';
  import { createEventDispatcher } from 'svelte';
  import type { SupportedEditorLanguageId } from '../../monaco/language-support';
  import type { TempModel } from '../../store/graph-selection-store';
  import type { EditorModelWithDocumentKey, EditorTab, TabSummary } from './types';
  import { EDITOR_CONFIG } from '../../config/constants';
  import { ensureModelDocumentKey, rotateModelDocumentKey } from './document-key';
import { createDefaultTabName } from './tab-name';
import type { DocumentOrigin } from '../../document-origin';

  export let monaco: typeof Monaco;
  export let maxTabs = EDITOR_CONFIG.maxTabs;
  export let initialLanguageId: SupportedEditorLanguageId;
  export let initialCode: string;

  const dispatch = createEventDispatcher<{
    tabChange: { tab: EditorTab };
    tabClose: { id: string };
    tabAdd: { tab: EditorTab };
  }>();

  let tabs: EditorTab[] = [];
  let tabSummaries: TabSummary[] = [];
  let activeTabId = '';
  let tabCounter = 1;
  const tempModels = new Map<string, TempModel>();

  function createTempModel(): TempModel {
    return {
      diffInputText: '',
      scratchText: '',
      commandQuery: '',
      status: 'Ready',
      error: '',
      cursor: 'Ln 1, Col 1',
      selectionLength: 0,
      treePath: [],
      graphHighlight: null,
      diagnostics: [],
    };
  }

  function createTab(languageId: SupportedEditorLanguageId, text: string, origin: DocumentOrigin): EditorTab {
    const id = `tab-${Date.now()}-${tabCounter}`;
    const name = createDefaultTabName(tabCounter);
    tabCounter += 1;
    const uri = monaco.Uri.parse(`inmemory://model/${id}`);
    tempModels.set(id, createTempModel());
    const model = monaco.editor.createModel(text, languageId, uri) as EditorModelWithDocumentKey;
    const documentKey = ensureModelDocumentKey(model, id);
    return { id, name, languageId, origin, documentKey, model };
  }

  function syncTabSummaries(): void {
    tabSummaries = tabs.map((tab) => ({ id: tab.id, name: tab.name, languageId: tab.languageId }));
  }

  export function initTabs(): EditorTab {
    const firstTab = createTab(initialLanguageId, initialCode, 'example');
    tabs = [firstTab];
    syncTabSummaries();
    return firstTab;
  }

  export function addTab(languageId: SupportedEditorLanguageId, text: string, origin: DocumentOrigin = 'example'): EditorTab | null {
    if (tabs.length >= maxTabs) return null;
    const tab = createTab(languageId, text, origin);
    tabs = [...tabs, tab];
    syncTabSummaries();
    dispatch('tabAdd', { tab });
    return tab;
  }

  export function closeTab(id: string, fallbackLanguageId: SupportedEditorLanguageId, fallbackText: string): EditorTab | null {
    const index = tabs.findIndex((tab) => tab.id === id);
    if (index < 0) return null;
    const tab = tabs[index];
    tab.model.dispose();
    tempModels.delete(id);
    const nextTabs = tabs.filter((item) => item.id !== id);
    tabs = nextTabs;
    syncTabSummaries();

    if (nextTabs.length === 0) {
      const fallback = createTab(fallbackLanguageId, fallbackText, 'example');
      tabs = [fallback];
      syncTabSummaries();
      return fallback;
    }

    if (activeTabId === id) {
      return nextTabs[Math.max(0, index - 1)] ?? nextTabs[0];
    }
    return null;
  }

  export function activateTab(id: string): EditorTab | undefined {
    return tabs.find((item) => item.id === id);
  }

  export function getActiveTab(): EditorTab | undefined {
    return tabs.find((tab) => tab.id === activeTabId);
  }

  export function setActiveTabId(id: string): void {
    activeTabId = id;
  }

  export function getActiveTabId(): string {
    return activeTabId;
  }

  export function getTabSummaries(): TabSummary[] {
    return tabSummaries;
  }

  export function getTempModel(id: string): TempModel | undefined {
    return tempModels.get(id);
  }

  export function setTempModel(id: string, model: TempModel): void {
    tempModels.set(id, model);
  }

  export function updateTabLanguage(id: string, languageId: SupportedEditorLanguageId): void {
    const index = tabs.findIndex((tab) => tab.id === id);
    if (index >= 0 && tabs[index].languageId !== languageId) {
      tabs = tabs.map((tab, i) => (i === index ? { ...tab, languageId } : tab));
      syncTabSummaries();
    }
  }

  export function setTabName(id: string, name: string): void {
    const index = tabs.findIndex((tab) => tab.id === id);
    if (index < 0 || tabs[index].name === name) return;
    tabs = tabs.map((tab, currentIndex) => (currentIndex === index ? { ...tab, name } : tab));
    syncTabSummaries();
  }

  export function setTabDocumentKey(id: string, documentKey: string): void {
    const index = tabs.findIndex((tab) => tab.id === id);
    if (index < 0) return;
    tabs = tabs.map((tab, i) => (i === index ? { ...tab, documentKey } : tab));
    syncTabSummaries();
  }

  export function rotateDocumentKey(id: string): string | null {
    const index = tabs.findIndex((tab) => tab.id === id);
    if (index < 0) return null;
    const nextDocumentKey = rotateModelDocumentKey(tabs[index].model, id);
    const nextTabs = tabs.map((tab, i) => {
      if (i !== index) return tab;
      return { ...tab, documentKey: nextDocumentKey };
    });
    tabs = nextTabs;
    syncTabSummaries();
    return nextDocumentKey;
  }

  export function disposeAll(): void {
    tabs.forEach((tab) => tab.model.dispose());
    tabs = [];
    tabSummaries = [];
    tempModels.clear();
  }

  export function getTabs(): EditorTab[] {
    return tabs;
  }
</script>
