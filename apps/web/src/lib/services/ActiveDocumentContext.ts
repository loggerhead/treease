import type * as Monaco from 'monaco-editor';
import type { SnapshotId } from '@core-wasm/index';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import { getDocumentSessionState } from '../store/document-session-store';
import { getWorkspaceState } from '../store/workspace-store';
import {
  getActiveDocumentCommitBaseSnapshotId,
  getActiveDocumentSuccessfulSnapshotId,
} from '../store/active-document-semantic-state';
import { getWorkspaceSnapshotId } from '../store/workspace-snapshot-bindings';

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

export function resolveReadableSnapshotId(documentKey: string, revision: number, fallbackSnapshotId?: SnapshotId | null): SnapshotId | null {
  return getActiveDocumentSuccessfulSnapshotId(documentKey, revision) ?? fallbackSnapshotId ?? getWorkspaceSnapshotId(documentKey);
}

export function resolveCommitBaseSnapshotId(documentKey: string, fallbackSnapshotId?: SnapshotId | null): SnapshotId | null {
  return getActiveDocumentCommitBaseSnapshotId(documentKey) ?? fallbackSnapshotId ?? getWorkspaceSnapshotId(documentKey);
}

export function getActiveDocumentContext(): ActiveDocumentContext {
  const session = getDocumentSessionState();
  const workspace = getWorkspaceState();
  const activeTab = workspace.tabsById[workspace.activeTabId] ?? null;
  const documentKey = activeTab?.documentKey ?? session.documentKey;
  const snapshotId = resolveReadableSnapshotId(
    documentKey,
    session.editorRevision,
    activeTab?.snapshotId ?? workspace.snapshotBindingsByDocumentKey[documentKey]?.snapshotId ?? null,
  );
  const model = session.editorIO?.getModel() ?? null;
  const modelText = model?.getValue();
  if (modelText != null) {
    return {
      documentKey,
      languageId: activeTab?.languageId ?? session.languageId,
      revision: session.editorRevision,
      snapshotId,
      text: modelText,
      textSource: 'monacoModel',
      model,
    };
  }
  const ioText = session.editorIO?.getText?.();
  if (ioText != null) {
    return {
      documentKey,
      languageId: session.editorIO?.getLanguage?.() ?? activeTab?.languageId ?? session.languageId,
      revision: session.editorRevision,
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
      revision: session.editorRevision,
      snapshotId,
      text: activeTab.sourceText,
      textSource: 'workspaceTab',
      model: null,
    };
  }
  return {
    documentKey: session.documentKey,
    languageId: session.languageId,
    revision: session.editorRevision,
    snapshotId,
    text: session.sourceText,
    textSource: 'store',
    model: null,
  };
}

export function getActiveDocumentText(): string {
  return getActiveDocumentContext().text;
}
