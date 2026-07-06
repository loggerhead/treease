import type { PathSeg } from '../../../store/tree-path';
import type { SubgraphWorkspacePaneState } from './types';
import type { createSubgraphWorkspaceController } from './controller';

type SubgraphWorkspaceController = ReturnType<typeof createSubgraphWorkspaceController>;

type WorkspaceActionDeps = {
  controller: SubgraphWorkspaceController;
  syncPaneReadiness: (pane: SubgraphWorkspacePaneState | null | undefined) => void;
};

export function createSubgraphWorkspaceActions(deps: WorkspaceActionDeps) {
  const { controller, syncPaneReadiness } = deps;

  return {
    hostAction: controller.hostAction,
    reset: () => {
      controller.reset();
    },
    openPath: async (path: PathSeg[], parentAbsoluteIndex: number): Promise<void> => {
      await controller.openPath(path, parentAbsoluteIndex);
      syncPaneReadiness(controller.getChain().at(-1));
    },
    closePane: (absoluteIndex: number): void => {
      controller.closePane(absoluteIndex);
    },
    commitValueEdit: async (
      pane: SubgraphWorkspacePaneState,
      draft?: string,
    ): Promise<void> => {
      await controller.commitValueEdit(pane, draft);
    },
    startDividerDrag: (clientY: number): void => {
      controller.startDividerDrag(clientY);
    },
    moveDividerDrag: (clientY: number): void => {
      controller.moveDividerDrag(clientY);
    },
    endDividerDrag: (): void => {
      controller.endDividerDrag();
    },
    renderPanes: async (): Promise<void> => {
      await controller.renderPanes();
      for (const pane of controller.getVisiblePanes()) {
        syncPaneReadiness(pane);
      }
    },
    refreshPanes: async (): Promise<void> => {
      await controller.refreshPanes();
      for (const pane of controller.getVisiblePanes()) {
        syncPaneReadiness(pane);
      }
    },
  };
}
