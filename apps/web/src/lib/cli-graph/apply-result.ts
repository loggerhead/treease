import {
  editorRevision,
  graphAppliedRevision,
  resetDocumentSession,
  setDocumentKey,
  setLanguageId,
  setSourceText,
} from '../store/document-session-store';
import { initWorkspaceFromPrimaryTab } from '../store/workspace-store';
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

  resetDocumentSession();
  setDocumentKey(documentKey);
  setLanguageId(language);
  setSourceText(result.text);
  editorRevision.set(1);
  graphAppliedRevision.set(0);
  initWorkspaceFromPrimaryTab({ id: 'cli-graph', name: tabName });
}
