import type { GraphHighlightTarget } from '../../store/editor-store';
import type { GraphCell, GraphCellKind, GraphNode } from '../../graph/graph-viewer-render';
import { resolveGraphCellDisplayText } from '../../graph/literal-display';
import { getCellEntry, getHighlightTarget, getScrollContext } from './graph-anchor-index';
import type { GraphViewerClickTarget, GraphViewerClickTargetStore, LeaferAppLike, LeaferBox } from './model';
import type {
  GraphRuntimeHighlightState,
  GraphRuntimeHitResult,
  GraphRuntimePoint,
  GraphRuntimeProbeTarget,
  GraphRuntimeRect,
  GraphRuntimeRevealState,
  GraphRuntimeRowScrollState,
} from './runtime/scene-types';

type ActiveHighlightState = { path: any[]; target?: GraphHighlightTarget; box: LeaferBox | null } | null;
type RegisteredTargetScope = 'root';

type CreateGraphRuntimeProbeControllerOptions = {
  shouldAttachGraphViewerTestHooks: () => boolean;
  isTextClickTarget: (target: object) => boolean;
  isFullEditStreaming: () => boolean;
  bindPointerClick: (target: LeaferBox, handler: (event: unknown) => void | Promise<void>) => void;
  getContainerRect: () => DOMRect | null;
  getRootClickTargets: () => GraphViewerClickTarget[];
  getRootApp: () => LeaferAppLike | null;
  getLanguageId: () => string;
  getCellBoxByPathMap: () => Map<string, any>;
  buildPathKey: (path: any[]) => string;
  getClientProbeCoordFromBox: (box: LeaferBox, app: LeaferAppLike | null) => GraphRuntimePoint | null;
  getClientRectFromBox: (box: LeaferBox, app: LeaferAppLike | null) => GraphRuntimeRect | null;
  getWorldRectFromBox: (box: LeaferBox) => GraphRuntimeRect | null;
  getClientPointFromWorld: (point: GraphRuntimePoint | null) => GraphRuntimePoint | null;
  getViewportWorldCenter: () => GraphRuntimePoint | null;
  ensurePathIndex: (path: any[]) => Promise<void>;
  resolveTreePathByPosition: (row: number, column: number) => Promise<any[]>;
  resolveInteractiveCellPath: (cell: GraphCell, fallbackPath: any[]) => Promise<any[]>;
  emitReveal: (path: any[], target: 'key' | 'value' | 'node', source: 'click' | 'runtime-query') => void;
  onRegisteredTargetClick?: (payload: { path: any[]; target: 'key' | 'value' | 'node'; cell: GraphCell; scope: 'root' }) => void | Promise<void>;
  commitProbe: (probe: { cell: GraphCell; kind: GraphCellKind }, text: string) => Promise<boolean>;
};

function toRelativeClientPoint(point: GraphRuntimePoint | null, rect: DOMRect | null): GraphRuntimePoint | null {
  if (!point || !rect) return point;
  return {
    x: Math.round(point.x - rect.left),
    y: Math.round(point.y - rect.top),
  };
}

function toGraphClickTarget(kind: GraphCellKind): 'key' | 'value' | 'node' {
  if (kind === 'key') return 'key';
  if (kind === 'value') return 'value';
  return 'node';
}

export function createGraphRuntimeProbeController(options: CreateGraphRuntimeProbeControllerOptions) {
  let nextClickTargetSeq = 0;
  let clickTargetProbesById: GraphViewerClickTargetStore = Object.create(null) as GraphViewerClickTargetStore;
  let clickTargetIdByTarget = new WeakMap<object, string>();
  let clickBoundTargets = new WeakSet<object>();
  let runtimeActiveHighlightState: ActiveHighlightState = null;
  let runtimeLastRevealState: GraphRuntimeRevealState | null = null;
  let runtimeLastRowScrollState: GraphRuntimeRowScrollState | null = null;

  function clearTestState(): void {
    runtimeActiveHighlightState = null;
    runtimeLastRevealState = null;
    runtimeLastRowScrollState = null;
  }

  function resetRootClickTargets(): void {
    clickTargetProbesById = Object.create(null) as GraphViewerClickTargetStore;
    clickTargetIdByTarget = new WeakMap();
    clickBoundTargets = new WeakSet();
  }

  function listRootClickTargets(): GraphViewerClickTarget[] {
    return Object.values(clickTargetProbesById);
  }

  function assignClickTargetIdForStore(target: object, targetIds: WeakMap<object, string>): string {
    const existing = targetIds.get(target);
    if (existing) return existing;
    const prefix = options.isTextClickTarget(target) ? 'text' : 'node';
    const id = `${prefix}-${nextClickTargetSeq++}`;
    targetIds.set(target, id);
    (target as { __treeaseClickTargetId?: string }).__treeaseClickTargetId = id;
    return id;
  }

  function upsertProbe(
    store: GraphViewerClickTargetStore,
    targetIds: WeakMap<object, string>,
    box: LeaferBox,
    cell: GraphCell,
    kind: GraphCellKind,
  ): string {
    const id = assignClickTargetIdForStore(box, targetIds);
    store[id] = { id, box, cell, target: toGraphClickTarget(kind) };
    return id;
  }

  function upsertClickTargetProbe(box: LeaferBox, cell: GraphCell, kind: GraphCellKind): string {
    return upsertProbe(clickTargetProbesById, clickTargetIdByTarget, box, cell, kind);
  }

  async function revealRegisteredTarget(
    target: LeaferBox,
    scope: RegisteredTargetScope,
    optionsOverride?: { waitForPathIndex?: boolean },
  ): Promise<void> {
    if (options.isFullEditStreaming()) return;
    const targetCell = target.__graphCell as GraphCell | undefined;
    const targetKind = toGraphClickTarget((target.__graphCellKind as GraphCellKind | undefined) ?? 'meta');
    if (!targetCell) return;
    const path = targetCell.path ?? [];
    const interactivePath = targetKind === 'node' ? path : await options.resolveInteractiveCellPath(targetCell, path);
    if (!interactivePath.length) return;
    if (optionsOverride?.waitForPathIndex !== false) {
      await options.ensurePathIndex(interactivePath);
    }
    setGraphRevealTestState(interactivePath, targetKind);
    options.emitReveal(interactivePath, targetKind, 'click');
    await options.onRegisteredTargetClick?.({
      path: interactivePath,
      target: targetKind,
      cell: targetCell,
      scope,
    });
  }

  function bindClickReveal(
    target: LeaferBox,
    boundTargets: WeakSet<object>,
    scope: RegisteredTargetScope,
    optionsOverride?: { waitForPathIndex?: boolean },
  ): void {
    if (boundTargets.has(target)) return;
    boundTargets.add(target);
    options.bindPointerClick(target, () => revealRegisteredTarget(target, scope, optionsOverride));
  }

  function registerRootClickTarget(
    target: LeaferBox,
    cell: GraphCell,
    kind: GraphCellKind,
    nodeKind?: GraphNode['kind'],
  ): string {
    if (!target || typeof target.on !== 'function') return '';
    target.__graphCell = cell;
    target.__graphCellKind = kind;
    target.__graphNodeKind = nodeKind;
    const clickTargetId = upsertClickTargetProbe(target, cell, kind);
    bindClickReveal(target, clickBoundTargets, 'root');
    return clickTargetId;
  }

  function setGraphHighlightTestState(path: any[] | null, target?: GraphHighlightTarget, box?: LeaferBox | null): void {
    runtimeActiveHighlightState = path?.length ? { path, target, box: box ?? null } : null;
  }

  function setGraphRevealTestState(path: any[] | null, target?: GraphHighlightTarget): void {
    runtimeLastRevealState = path?.length ? { path, target } : null;
  }

  function clearLastReveal(): void {
    runtimeLastRevealState = null;
  }

  function setGraphRowScrollTestState(path: any[] | null, scrollY?: number): void {
    runtimeLastRowScrollState = path?.length && typeof scrollY === 'number' ? { path, scrollY } : null;
  }

  function getRuntimeProbeTargets(scope: 'root' = 'root'): GraphRuntimeProbeTarget[] {
    const probes = listRootClickTargets();
    const app = options.getRootApp();
    const containerRect = options.getContainerRect();
    return probes.map((entry) => {
      return {
        scope,
        id: entry.id,
        target: entry.target,
        nodeType: String((entry.box as { tag?: string }).tag ?? ''),
        coord: toRelativeClientPoint(options.getClientProbeCoordFromBox(entry.box, app), containerRect),
        rect: options.getClientRectFromBox(entry.box, app),
        worldRect: options.getWorldRectFromBox(entry.box),
        cell: entry.cell
          ? {
              text: resolveGraphCellDisplayText(
                entry.cell.text,
                entry.cell.value,
                String(entry.cell.valueType ?? ''),
                options.getLanguageId(),
              ),
              valueType: String(entry.cell.valueType ?? ''),
              isTableCell: !!entry.cell.isTableCell,
              isHeader: !!entry.cell.isHeader,
              path: entry.cell.path ?? [],
            }
          : null,
      };
    });
  }

  function getRuntimeHighlightTarget(): GraphRuntimeHighlightState | null {
    if (!runtimeActiveHighlightState?.path?.length) return null;
    const pathKey = options.buildPathKey(runtimeActiveHighlightState.path);
    const entry = getCellEntry(options.getCellBoxByPathMap(), runtimeActiveHighlightState.path);
    const resolvedHighlight = getHighlightTarget(entry, runtimeActiveHighlightState.target);
    const resolvedBox = resolvedHighlight.box as LeaferBox | null;
    if (!resolvedBox) return null;
    const matchedProbe = listRootClickTargets().find((probe) => {
      if (options.buildPathKey(probe.cell?.path ?? []) !== pathKey) return false;
      if (!resolvedHighlight.target) return true;
      return probe.target === resolvedHighlight.target;
    });
    const probeCoord = matchedProbe
      ? options.getClientProbeCoordFromBox(matchedProbe.box, options.getRootApp())
      : options.getClientProbeCoordFromBox(resolvedBox, options.getRootApp());
    const rawHighlightRect = options.getClientRectFromBox(resolvedBox, options.getRootApp());
    const highlightRect = rawHighlightRect
      ? {
          left: rawHighlightRect.left + 0.5,
          top: rawHighlightRect.top + 0.5,
          width: Math.max(0, rawHighlightRect.width - 1),
          height: Math.max(0, rawHighlightRect.height - 1),
        }
      : null;
    const worldBox = resolvedBox as LeaferBox & {
      getWorldPointByBox?: (point: { x: number; y: number }) => { x?: number; y?: number } | null;
    };
    const highlightWorld =
      typeof worldBox.getWorldPointByBox === 'function'
        ? worldBox.getWorldPointByBox({
            x: Number(resolvedBox.width ?? 0) / 2,
            y: Number(resolvedBox.height ?? 0) / 2,
          })
        : null;
    const highlightClient = options.getClientPointFromWorld(
      highlightWorld ? { x: Number(highlightWorld.x ?? 0), y: Number(highlightWorld.y ?? 0) } : null,
    );
    const viewportCenter = options.getViewportWorldCenter();
    return {
      path: runtimeActiveHighlightState.path,
      target: resolvedHighlight.target,
      rect: highlightRect,
      probe: probeCoord
        ? {
            x: probeCoord.x,
            y: probeCoord.y,
            source: matchedProbe ? 'matched-probe' : 'highlight-box',
          }
        : null,
      world:
        highlightClient && viewportCenter
          ? {
              highlight: highlightClient,
              viewportCenter,
            }
          : null,
    };
  }

  function getRuntimeRowScrollState(path?: any[] | null): GraphRuntimeRowScrollState | null {
    if (!path?.length) return runtimeLastRowScrollState;
    const entry = getCellEntry(options.getCellBoxByPathMap(), path);
    const scrollContext = getScrollContext(entry);
    if (!scrollContext) return null;
    return {
      path,
      scrollY: Number(scrollContext.scrollOwner?.scrollY ?? 0),
      bodyHeight: typeof scrollContext.bodyHeight === 'number' ? scrollContext.bodyHeight : undefined,
      contentHeight: typeof scrollContext.contentHeight === 'number' ? scrollContext.contentHeight : undefined,
    };
  }

  function getRuntimeHitResult(point: GraphRuntimePoint): GraphRuntimeHitResult {
    const hit =
      getRuntimeProbeTargets().find((entry) => {
        const rect = entry.rect;
        if (!rect) return false;
        return (
          point.x >= rect.left &&
          point.x <= rect.left + rect.width &&
          point.y >= rect.top &&
          point.y <= rect.top + rect.height
        );
      }) ?? null;
    return { scope: 'root', point, hit };
  }

  async function activateRuntimeProbe(probeId: string): Promise<void> {
    const probe = clickTargetProbesById[probeId];
    if (!probe) return;
    const path = probe.cell?.path ?? [];
    const interactivePath = probe.target === 'node' ? path : await options.resolveInteractiveCellPath(probe.cell, path);
    if (!interactivePath.length) return;
    await options.ensurePathIndex(interactivePath);
    options.emitReveal(interactivePath, probe.target, 'runtime-query');
  }

  async function commitRuntimeProbe(probeId: string, text: string): Promise<boolean> {
    const probe = clickTargetProbesById[probeId];
    if (!probe?.cell) {
      return false;
    }
    const kind: GraphCellKind = probe.target === 'key' ? 'key' : probe.target === 'value' ? 'value' : 'meta';
    const applied = await options.commitProbe({ cell: probe.cell, kind }, text);
    return applied;
  }

  return {
    shouldAttachGraphViewerTestHooks: options.shouldAttachGraphViewerTestHooks,
    clearTestState,
    resetRootClickTargets,
    listRootClickTargets,
    getRootStore: () => clickTargetProbesById,
    getClickBoundTargets: () => clickBoundTargets,
    registerRootClickTarget,
    upsertProbe,
    upsertClickTargetProbe,
    setGraphHighlightTestState,
    setGraphRevealTestState,
    setGraphRowScrollTestState,
    clearLastReveal,
    getLastReveal: () => runtimeLastRevealState,
    getRuntimeProbeTargets,
    getRuntimeHighlightTarget,
    getRuntimeRowScrollState,
    getRuntimeHitResult,
    activateRuntimeProbe,
    commitRuntimeProbe,
  };
}
