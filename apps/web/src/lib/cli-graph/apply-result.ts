import { editorStore, editorRevision, graphAppliedRevision } from '../store/editor-store';
import {
  editorLanguageFallback,
  supportedEditorLanguageSet,
  type SupportedEditorLanguageId,
} from '../monaco/language-support';
import type { CliGraphResult } from './result-client';

export function resolveCliGraphLanguage(language: string): SupportedEditorLanguageId {
  return supportedEditorLanguageSet.has(language as SupportedEditorLanguageId)
    ? (language as SupportedEditorLanguageId)
    : editorLanguageFallback;
}

export function buildCliGraphDocumentKey(token: string): string {
  return `cli:${token}`;
}

export function applyCliGraphResultToEditorStore(token: string, result: CliGraphResult): void {
  const language = resolveCliGraphLanguage(result.language);
  const documentKey = buildCliGraphDocumentKey(token);
  const tabName = result.sourceLabel || 'CLI result';

  editorStore.reset();
  editorStore.actions.setDocumentKey(documentKey);
  editorStore.actions.setLanguageId(language);
  editorStore.actions.setSourceText(result.text);
  editorRevision.set(1);
  graphAppliedRevision.set(0);
  editorStore.actions.initWorkspaceFromPrimaryTab({ id: 'cli-graph', name: tabName });
}
