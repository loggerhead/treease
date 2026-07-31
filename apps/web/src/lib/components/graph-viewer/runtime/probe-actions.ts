import type { GraphHighlightTarget } from '../../../store/graph-selection-store';
import type { PathSeg } from '../../../store/tree-path';
import type { GraphRuntimeProbeTarget } from './index';
import type { LeaferBox, GraphViewerClickTarget } from '../model';
import type { GraphCell, GraphCellKind, GraphNode } from '@treease/graph-viewer-runtime';

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
  getWorkspaceRoot: () => HTMLElement | null;
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
    getColumnNavigatorProbeTargets: (): GraphRuntimeProbeTarget[] => {
      const workspace = deps.getWorkspaceRoot();
      if (!workspace) return [];
      const workspaceRect = workspace.getBoundingClientRect();
      return [...workspace.querySelectorAll<HTMLElement>('[data-column-navigator-item-path]')].map((element) => {
        const rect = element.getBoundingClientRect();
        const valueType = element.dataset.columnNavigatorItemValueType ?? '';
        const rawPreview = element.dataset.columnNavigatorItemPreview ?? '';
        let preview = rawPreview;
        if (valueType === 'string') {
          try {
            const parsed = JSON.parse(rawPreview);
            if (typeof parsed === 'string') preview = parsed;
          } catch {
            preview = rawPreview;
          }
        }
        let path: PathSeg[] = [];
        try {
          path = JSON.parse(element.dataset.columnNavigatorItemPath ?? '[]') as PathSeg[];
        } catch {
          path = [];
        }
        return {
          scope: 'workspace',
          id: element.dataset.columnNavigatorItemPathKey ?? '',
          target: 'value',
          nodeType: element.tagName,
          coord: {
            x: Math.round(rect.left + rect.width / 2 - workspaceRect.left),
            y: Math.round(rect.top + rect.height / 2 - workspaceRect.top),
          },
          rect: {
            x: rect.x,
            y: rect.y,
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
          },
          worldRect: null,
          textColor: getComputedStyle(element).color,
          cell: {
            text: preview,
            valueType,
            isTableCell: element.dataset.columnNavigatorItemIndex === 'true',
            isHeader: false,
            path,
          },
        };
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
