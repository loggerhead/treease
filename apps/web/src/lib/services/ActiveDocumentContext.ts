import type * as Monaco from 'monaco-editor';
import type { SnapshotId } from '@core-wasm/index';
import { editorStore } from '../store/editor-store';
import type { SupportedEditorLanguageId } from '../monaco/language-support';

export type ActiveDocumentTextSource = 'monacoModel' | 'editorIO' | 'workspaceTab' | 'store';

export type ActiveDocumentContext = {
  documentKey: string;
  languageId: SupportedEditorLanguageId;
  revision: number;
  snapshotId: SnapshotId | null;
  text: string;
  textSource: ActiveDocumentTextSource;
  model: Monaco.editor.ITextModel | null;
};

export function getActiveDocumentContext(): ActiveDocumentContext {
  const state = editorStore.get();
  const activeTab = state.workspace.tabsById[state.workspace.activeTabId] ?? null;
  const documentKey = activeTab?.documentKey ?? state.documentKey;
  const snapshotId = activeTab?.snapshotId ?? state.workspace.snapshotBindingsByDocumentKey[documentKey]?.snapshotId ?? null;
  const model = state.editorIO?.getModel() ?? null;
  const modelText = model?.getValue();
  if (modelText != null) {
    return {
      documentKey,
      languageId: activeTab?.languageId ?? state.languageId,
      revision: state.editorRevision,
      snapshotId,
      text: modelText,
      textSource: 'monacoModel',
      model,
    };
  }
  const ioText = state.editorIO?.getText?.();
  if (ioText != null) {
    return {
      documentKey,
      languageId: state.editorIO?.getLanguage?.() ?? activeTab?.languageId ?? state.languageId,
      revision: state.editorRevision,
      snapshotId,
      text: ioText,
      textSource: 'editorIO',
      model: null,
    };
  }
  if (activeTab) {
    return {
      documentKey: activeTab.documentKey,
      languageId: activeTab.languageId,
      revision: state.editorRevision,
      snapshotId: activeTab.snapshotId,
      text: activeTab.sourceText,
      textSource: 'workspaceTab',
      model: null,
    };
  }
  return {
    documentKey: state.documentKey,
    languageId: state.languageId,
    revision: state.editorRevision,
    snapshotId: state.workspace.snapshotBindingsByDocumentKey[state.documentKey]?.snapshotId ?? null,
    text: state.sourceText,
    textSource: 'store',
    model: null,
  };
}

export function getActiveDocumentText(): string {
  return getActiveDocumentContext().text;
}
