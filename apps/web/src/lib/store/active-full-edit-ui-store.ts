import { derived, get, type Readable } from 'svelte/store';

import { workspaceStore } from './workspace-store';
import { initialFullEditUiState, type FullEditUiState } from './full-edit-ui-state';

function freezeForRead(state: FullEditUiState): FullEditUiState {
  return Object.freeze({ ...state });
}

/** Active-tab projection only; document operations write their target workspace tab. */
export const activeFullEditUiState: Readable<FullEditUiState> = {
  subscribe: (run) =>
    derived(workspaceStore, ($workspace) =>
      freezeForRead($workspace.tabsById[$workspace.activeTabId]?.fullEditUiState ?? initialFullEditUiState),
    ).subscribe(run),
};

export function getActiveFullEditUiState(): FullEditUiState {
  return get(activeFullEditUiState);
}
