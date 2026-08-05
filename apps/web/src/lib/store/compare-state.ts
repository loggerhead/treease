import { derived, type Readable } from 'svelte/store';
import { editorWorkspace } from './workspace-store';
import type { CompareOutcomeState } from './editor-workspace';

export type CompareState = CompareOutcomeState;

export const initialCompareState: CompareState = { kind: 'none' };

/** Read-only projection of the visible pair. Compare mutations use sidecar-tab-state. */
export const compareState: Readable<CompareState> = derived(editorWorkspace, ($workspace) => {
  const main = $workspace.tabsById[$workspace.activeTabId];
  const sidecar = main?.sidecarTabId ? $workspace.tabsById[main.sidecarTabId] : null;
  return sidecar?.role === 'sidecar' && sidecar.ownerMainTabId === main?.id
    ? sidecar.sidecarState?.compare.outcome ?? initialCompareState
    : initialCompareState;
});
