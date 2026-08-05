import type { EditorWorkspaceState, EditorWorkspaceTab } from './editor-workspace';

/**
 * One identity for every workspace tab entity. Role and pane are projections,
 * not separate target types.
 */
export type TabTarget = Readonly<{
  tabId: string;
  documentKey: string;
  generation: number;
  revision: number;
}>;

export type TabTargetStatus = 'current' | 'stale' | 'closed';

export function targetOfTab(tab: EditorWorkspaceTab): TabTarget {
  return {
    tabId: tab.id,
    documentKey: tab.documentKey,
    generation: tab.generation ?? 0,
    revision: tab.revision,
  };
}

export function tabTargetStatus(workspace: EditorWorkspaceState, target: TabTarget): TabTargetStatus {
  const tab = workspace.tabsById[target.tabId];
  if (!tab) return 'closed';
  return tab.documentKey === target.documentKey
    && (tab.generation ?? 0) === target.generation
    && tab.revision === target.revision
    ? 'current'
    : 'stale';
}

export function isVisibleTabTarget(workspace: EditorWorkspaceState, target: TabTarget): boolean {
  const active = workspace.tabsById[workspace.activeTabId];
  return active?.id === target.tabId || active?.sidecarTabId === target.tabId;
}
