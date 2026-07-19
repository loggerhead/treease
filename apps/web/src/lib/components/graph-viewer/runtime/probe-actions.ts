import type { GraphHighlightTarget } from '../../../store/graph-selection-store';
import type { PathSeg } from '../../../store/tree-path';
import { getClientProbeCoordFromBoxLike, getClientRectFromBoxLike, getWorldRectFromBoxLike } from '../rendering';
import type { GraphRuntimeProbeTarget } from './index';
import type { LeaferAppLike, LeaferBox, GraphViewerClickTarget } from '../model';
import type { GraphCell, GraphCellKind, GraphNode } from '../../../graph/graph-viewer-render';

type ProbeController = {
  listRootClickTargets: () => GraphViewerClickTarget[];
  setGraphHighlightTestState: (
    path: PathSeg[] | null,
    target?: GraphHighlightTarget,
    box?: LeaferBox | null,
  ) => void;
  setGraphRevealTestState: (path: PathSeg[] | null, target?: GraphHighlightTarget) => void;
  setGraphRowScrollTestState: (path: PathSeg[] | null, scrollY?: number) => void;
  registerRootClickTarget: (
    target: LeaferBox,
    cell: GraphCell,
    kind: GraphCellKind,
    nodeKind?: GraphNode['kind'],
  ) => string;
  getRuntimeProbeTargets: (scope?: 'root') => GraphRuntimeProbeTarget[];
  getRuntimeHighlightTarget: () => unknown;
  getRuntimeRowScrollState: (path?: PathSeg[] | null) => unknown;
  getRuntimeHitResult: (point: { x: number; y: number }) => unknown;
  clearLastReveal: () => void;
  getLastReveal: () => unknown;
  activateRuntimeProbe: (probeId: string) => Promise<void>;
  commitRuntimeProbe: (probeId: string, text: string) => Promise<boolean>;
};

type ProbeActionDeps = {
  getController: () => ProbeController | null | undefined;
  getVisiblePanes: () => Array<{ pathKey: string; path: PathSeg[] }>;
  getWorkspaceRuntime: (pathKey: string) => {
    app: LeaferAppLike | null;
    clickTargetsById?: Record<string, GraphViewerClickTarget>;
  } | null;
  getWorkspaceRect: () => DOMRect | null;
  rebaseWorkspacePath: (basePath: PathSeg[], path: PathSeg[]) => PathSeg[];
  resolveCellText: (entry: GraphViewerClickTarget) => string;
  getLanguageId: () => string;
};

export function createGraphRuntimeProbeActions(deps: ProbeActionDeps) {
  const controller = () => deps.getController();

  return {
    listClickTargetProbes: () => controller()?.listRootClickTargets() ?? [],
    setGraphHighlightTestState: (
      path: PathSeg[] | null,
      target?: GraphHighlightTarget,
      box?: LeaferBox | null,
    ) => controller()?.setGraphHighlightTestState(path, target, box),
    setGraphRevealTestState: (path: PathSeg[] | null, target?: GraphHighlightTarget) =>
      controller()?.setGraphRevealTestState(path, target),
    setGraphRowScrollTestState: (path: PathSeg[] | null, scrollY?: number) =>
      controller()?.setGraphRowScrollTestState(path, scrollY),
    registerClickTarget: (
      target: LeaferBox,
      cell: GraphCell,
      kind: GraphCellKind,
      nodeKind?: GraphNode['kind'],
    ) => controller()?.registerRootClickTarget(target, cell, kind, nodeKind) ?? '',
    getRuntimeProbeTargets: (scope: 'root' = 'root') =>
      controller()?.getRuntimeProbeTargets(scope) ?? [],
    getSubgraphWorkspaceProbeTargets: (): GraphRuntimeProbeTarget[] => {
      const workspaceRect = deps.getWorkspaceRect();
      if (!workspaceRect) return [];
      return deps.getVisiblePanes().flatMap((pane) => {
        const runtime = deps.getWorkspaceRuntime(pane.pathKey);
        if (!runtime) return [];
        const app = runtime.app as LeaferAppLike | null;
        return Object.values(runtime.clickTargetsById ?? {}).map((entry) => {
          const path = deps.rebaseWorkspacePath(pane.path, entry.cell?.path ?? []);
          const point = getClientProbeCoordFromBoxLike(entry.box, app);
          return {
            scope: 'workspace',
            id: entry.id,
            target: entry.target,
            nodeType: String((entry.box as { tag?: string }).tag ?? ''),
            coord:
              point && workspaceRect
                ? {
                    x: Math.round(point.x - workspaceRect.left),
                    y: Math.round(point.y - workspaceRect.top),
                  }
                : null,
            rect: getClientRectFromBoxLike(entry.box, app),
            worldRect: getWorldRectFromBoxLike(entry.box),
            textColor: typeof (entry.box as { fill?: unknown }).fill === 'string' ? (entry.box as { fill: string }).fill : null,
            cell: entry.cell
              ? {
                  text: deps.resolveCellText(entry),
                  valueType: String(entry.cell.valueType ?? ''),
                  isTableCell: !!entry.cell.isTableCell,
                  isHeader: !!entry.cell.isHeader,
                  path,
                }
              : null,
          };
        });
      });
    },
    getRuntimeHighlightTarget: () => controller()?.getRuntimeHighlightTarget() ?? null,
    getRuntimeRowScrollState: (path?: PathSeg[] | null) =>
      controller()?.getRuntimeRowScrollState(path) ?? null,
    getRuntimeHitResult: (point: { x: number; y: number }) =>
      controller()?.getRuntimeHitResult(point) ?? null,
    clearLastReveal: () => controller()?.clearLastReveal(),
    getLastReveal: () => controller()?.getLastReveal() ?? null,
    activateRuntimeProbe: async (probeId: string) => {
      await controller()?.activateRuntimeProbe(probeId);
    },
    commitRuntimeProbe: async (probeId: string, text: string) =>
      (await controller()?.commitRuntimeProbe(probeId, text)) ?? false,
  };
}
