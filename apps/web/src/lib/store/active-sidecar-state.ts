import { derived, type Readable } from 'svelte/store';

import type { TempModel } from './editor-store-types';
import { initialTempModel } from './graph-selection-store';
import { editorWorkspace } from './workspace-store';

function cloneTempModel(value: TempModel): TempModel {
  return {
    ...value,
    treePath: value.treePath.map((segment) => ({ ...segment })),
    graphHighlight: value.graphHighlight
      ? { ...value.graphHighlight, path: value.graphHighlight.path.map((segment) => ({ ...segment })) }
      : null,
    diagnostics: value.diagnostics.map((diagnostic) => ({
      ...diagnostic,
      context: diagnostic.context.map((line) => ({ ...line })),
    })),
  };
}

/**
 * Read-only projection for the currently visible right pane. Mutations must
 * go through sidecar-tab-state with an explicit captured TabTarget.
 */
export const activeSidecarTempModel: Readable<TempModel> = derived(editorWorkspace, ($workspace) => {
  const main = $workspace.tabsById[$workspace.activeTabId];
  const sidecar = main?.sidecarTabId ? $workspace.tabsById[main.sidecarTabId] : null;
  return cloneTempModel(sidecar?.role === 'sidecar' && sidecar.ownerMainTabId === main?.id
    ? sidecar.tempModel
    : initialTempModel);
});
