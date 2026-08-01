import { getLanguageExample } from '../monaco/language-examples';
import {
  editorLanguageFallback,
  supportedEditorLanguageSet,
  type SupportedEditorLanguageId,
} from '../monaco/language-support';
import type { EditorWorkspaceState, WorkspaceEditorTabInput } from '../store/editor-workspace';
import type { WorkspaceSession } from './contract';

export type WorkspaceSessionValidationResult =
  | { kind: 'valid'; session: WorkspaceSession }
  | { kind: 'invalid'; reason: string };

function isOptionalString(value: unknown): value is string | undefined {
  return value === undefined || typeof value === 'string';
}

export function validateWorkspaceSession(value: unknown): WorkspaceSessionValidationResult {
  if (!value || typeof value !== 'object') return { kind: 'invalid', reason: 'Session must be an object.' };
  const candidate = value as Partial<WorkspaceSession>;
  if (candidate.version !== 1) return { kind: 'invalid', reason: 'Unsupported workspace session version.' };
  if (!Number.isInteger(candidate.activeTabIndex) || (candidate.activeTabIndex ?? -1) < 0) {
    return { kind: 'invalid', reason: 'Workspace session activeTabIndex is invalid.' };
  }
  if (!Array.isArray(candidate.tabs)) return { kind: 'invalid', reason: 'Workspace session tabs are invalid.' };
  for (const [index, tab] of candidate.tabs.entries()) {
    if (!tab || typeof tab !== 'object') return { kind: 'invalid', reason: `Workspace session tab ${index} is invalid.` };
    if (
      typeof tab.name !== 'string' ||
      typeof tab.languageId !== 'string' ||
      typeof tab.sourceText !== 'string' ||
      !isOptionalString(tab.savedText) ||
      !isOptionalString(tab.linkedFileName) ||
      (tab.origin !== undefined && tab.origin !== 'example' && tab.origin !== 'user' && tab.origin !== 'import')
    ) {
      return { kind: 'invalid', reason: `Workspace session tab ${index} has invalid fields.` };
    }
  }
  return { kind: 'valid', session: candidate as WorkspaceSession };
}

export function workspaceSessionFromWorkspace(workspace: EditorWorkspaceState): WorkspaceSession {
  return {
    version: 1,
    activeTabIndex: Math.max(0, workspace.tabOrder.indexOf(workspace.activeTabId)),
    tabs: workspace.tabOrder.flatMap((tabId) => {
      const tab = workspace.tabsById[tabId];
      return tab && tab.role !== 'sidecar'
        ? [{
            name: tab.name,
            languageId: tab.languageId,
            sourceText: tab.sourceText,
            origin: tab.origin,
            savedText: tab.savedText,
            linkedFileName: tab.fileLinkedDocument?.name,
          }]
        : [];
    }),
  };
}

function normalizeSessionLanguage(languageId: string): SupportedEditorLanguageId {
  return supportedEditorLanguageSet.has(languageId as SupportedEditorLanguageId)
    ? languageId as SupportedEditorLanguageId
    : editorLanguageFallback;
}

export function workspaceTabInputFromSession(
  tab: WorkspaceSession['tabs'][number],
  id: string,
): WorkspaceEditorTabInput {
  const languageId = normalizeSessionLanguage(tab.languageId);
  return {
    id,
    name: tab.name,
    documentKey: `${id}:0`,
    languageId,
    sourceText: tab.sourceText,
    origin: tab.origin ?? (tab.sourceText === getLanguageExample(languageId) ? 'example' : 'user'),
    savedText: tab.savedText,
  };
}
