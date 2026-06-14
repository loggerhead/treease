import type { GraphViewerConfig } from '../../settings/ui-settings';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import type { SnapshotId } from '@core-wasm/index';
import { buildPathKey } from '../../graph/graph-viewer-path';
import type { GraphCell, GraphNode } from '../../graph/graph-viewer-render';
import { isPathSegIndex, type PathSeg } from '../../store/tree-path';
import type { GraphHighlightTarget } from '../../store/editor-store';
import { resolveTreePathFromText } from '../../services/TreePathService';
import type { CellBoxEntry, GraphViewerClickTarget, LeaferBox } from './model';
import { getCellEntry, getHighlightTarget, getScrollContext } from './graph-anchor-index';

export type GraphTextLinkageSearchResult = {
  target: 'key' | 'value' | 'node';
  path: PathSeg[];
};

type GraphTextLinkageControllerDeps = {
  getDocumentKey: () => string;
  getSourceText: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  getActiveSnapshotId: () => SnapshotId | null;
  getEnableNest: () => boolean;
  getRenderConfig: () => GraphViewerConfig;
  getNodeDataMap: () => Map<number, GraphNode>;
  getNodeBoxMap: () => Map<number, LeaferBox>;
  getCellBoxByPathMap: () => Map<string, CellBoxEntry>;
  getPathKeyToRenderHandleMap: () => Map<string, number>;
  getClickTargetProbes: () => GraphViewerClickTarget[];
  setGraphHighlightTestState: (path: PathSeg[] | null, target?: GraphHighlightTarget, box?: LeaferBox | null) => void;
  setGraphRevealTestState: (path: PathSeg[] | null, target?: GraphHighlightTarget) => void;
  setGraphRowScrollTestState: (path: PathSeg[] | null, scrollY?: number) => void;
  buildPathSegFromCell: (cell: GraphCell | undefined, rowIndex: number) => PathSeg | null;
  upsertCellEntry: (map: Map<string, CellBoxEntry>, cell: GraphCell, updater: (entry: CellBoxEntry) => void) => void;
  centerOnBox: (box: LeaferBox) => boolean;
  centerOnNode: (node: GraphNode) => void;
  updateLeafer: () => void;
  updateActiveTempModel: (updater: (current: any) => any) => void;
  getEditorRevision: () => number;
  getGraphAppliedRevision: () => number;
  getEnableRevealSync?: () => boolean;
  dispatchReveal: (path: PathSeg[], target?: GraphHighlightTarget, trigger?: string) => void;
  handleError: (
    error: unknown,
    context: { component: string; operation: string; metadata?: Record<string, unknown> },
  ) => void;
};


export function createGraphTextLinkageController(deps: GraphTextLinkageControllerDeps) {
  let revealPathToken = 0;
  let activeSearchHighlights: Array<{ target: LeaferBox; fill?: string; stroke?: string }> = [];
  let activeHighlightState: { path: PathSeg[]; target?: GraphHighlightTarget } | null = null;

  function clearSearchHighlight(): void {
    const renderConfig = deps.getRenderConfig();
    const cellBoxByPathMap = deps.getCellBoxByPathMap();
    activeSearchHighlights.forEach((entry) => {
      if (entry.fill !== undefined) entry.target.fill = entry.fill;
      if (entry.stroke !== undefined) entry.target.stroke = entry.stroke;
    });
    cellBoxByPathMap.forEach((entry) => {
      if (entry.row) entry.row.fill = renderConfig.colors.table.rowBackground;
      if (entry.key) entry.key.fill = 'transparent';
      if (entry.value) entry.value.fill = 'transparent';
    });
    activeSearchHighlights = [];
    activeHighlightState = null;
    deps.setGraphHighlightTestState(null);
  }

  function applySearchHighlight(target: LeaferBox | null, style: { fill?: string; stroke?: string }): void {
    if (!target) return;
    activeSearchHighlights.push({
      target,
      fill: target.fill as string | undefined,
      stroke: target.stroke as string | undefined,
    });
    if (style.fill) target.fill = style.fill;
    if (style.stroke) target.stroke = style.stroke;
  }

  async function resolveTreePathByPosition(row: number, column: number): Promise<PathSeg[]> {
    const documentKey = deps.getDocumentKey();
    if (!documentKey) return [];
    const text = deps.getSourceText();
    if (!text) return [];
    const snapshotId = deps.getActiveSnapshotId();
    if (snapshotId == null) return [];
    return resolveTreePathFromText(text, row, column, documentKey, deps.getLanguageId(), deps.getEnableNest(), 'auto', snapshotId).catch((error) => {
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'resolveTreePath',
        metadata: { documentKey, row, column },
      });
      return [];
    });
  }


  async function ensurePathIndex(path: PathSeg[]): Promise<void> {
    const pathKeyToRenderHandleMap = deps.getPathKeyToRenderHandleMap();
    const cellBoxByPathMap = deps.getCellBoxByPathMap();
    const nodeDataMap = deps.getNodeDataMap();
    const targetKey = buildPathKey(path);
    if (!targetKey) return;
    if (pathKeyToRenderHandleMap.has(targetKey) || cellBoxByPathMap.has(targetKey)) {
      return;
    }
    for (const node of nodeDataMap.values()) {
      const nodePath = Array.isArray(node.path) ? node.path : [];
      const nodeKey = buildPathKey(nodePath);
      if (nodeKey) pathKeyToRenderHandleMap.set(nodeKey, node.renderHandle);
      if (nodeKey === targetKey) {
        return;
      }
    }
    for (const entry of new Set(cellBoxByPathMap.values())) {
      const cell = entry.cell;
      if (!cell) continue;
      const cellPath = Array.isArray(cell.path) ? cell.path : [];
      if (!cellPath.length) continue;
      deps.upsertCellEntry(cellBoxByPathMap, cell, (nextEntry) => {
        Object.assign(nextEntry, entry, { cell });
      });
      if (buildPathKey(cellPath) === targetKey) {
        return;
      }
    }
  }

  async function hydrateResolvedGraphPaths(nodes: GraphNode[], _text: string): Promise<void> {
    for (const node of nodes) {
      if ((!Array.isArray(node.meta?.path) || node.meta.path.length === 0) && Array.isArray(node.path) && node.path.length > 0) {
        node.meta.path = [...node.path];
      }
      const shouldInferRowPath = node.kind === 'table' && (node.meta?.valueType === 'object' || node.meta?.valueType === 'array');
      const rows = node.kind === 'table' ? (node.table?.rows ?? []) : (node.rows ?? []);
      for (const [rowIndex, row] of rows.entries()) {
        const rowKeyCell = row.cells?.[0];
        if (node.kind === 'object' && row.cells?.length >= 2 && Array.isArray(rowKeyCell?.path) && rowKeyCell.path.length > 0) {
          row.cells[1].path = [...rowKeyCell.path];
        }
        const inferredSeg = shouldInferRowPath && rowKeyCell ? deps.buildPathSegFromCell(rowKeyCell, rowIndex) : null;
        const inferredRowPath = inferredSeg ? [...(Array.isArray(node.path) ? node.path : []), inferredSeg] : [];
        for (const cell of row.cells ?? []) {
          if (Array.isArray(cell.path) && cell.path.length > 0) continue;
          if (inferredRowPath.length) {
            cell.path = inferredRowPath;
          }
        }
      }
    }
  }

  function syncTreeSelection(path: PathSeg[], target?: GraphHighlightTarget, trigger?: string): void {
    if (!path?.length || deps.getEnableRevealSync?.() === false) return;
    const source = trigger === 'search' ? 'search' : 'graph';
    deps.updateActiveTempModel((current) => ({
      ...current,
      treePath: path,
      graphHighlight: {
        path,
        target,
        revision: Math.max(deps.getEditorRevision(), deps.getGraphAppliedRevision()),
        source,
      },
    }));
  }

  function emitReveal(path: PathSeg[], target?: GraphHighlightTarget, trigger?: string): void {
    if (!path?.length) return;
    syncTreeSelection(path, target, trigger);
    deps.setGraphRevealTestState(path, target);
    deps.dispatchReveal(path, target, trigger);
  }

  function resolveNodeForPath(path: PathSeg[]): { renderHandle?: number; node: GraphNode | null } {
    const pathKeyToRenderHandleMap = deps.getPathKeyToRenderHandleMap();
    const nodeDataMap = deps.getNodeDataMap();
    const pathKey = buildPathKey(path);
    let renderHandle = pathKey ? pathKeyToRenderHandleMap.get(pathKey) : undefined;
    if (renderHandle == null) {
      for (let i = path.length - 1; i > 0; i -= 1) {
        const parentKey = buildPathKey(path.slice(0, i));
        if (!parentKey) continue;
        const parentRenderHandle = pathKeyToRenderHandleMap.get(parentKey);
        if (parentRenderHandle != null) {
          renderHandle = parentRenderHandle;
          break;
        }
      }
    }
    return { renderHandle, node: renderHandle != null ? nodeDataMap.get(renderHandle) ?? null : null };
  }

  function clampScrollY(
    scrollContext: NonNullable<ReturnType<typeof getScrollContext>>,
    targetY: number,
  ): number {
    const maxScroll = Math.max(0, scrollContext.contentHeight - scrollContext.bodyHeight);
    return Math.max(0, Math.min(targetY, maxScroll));
  }

  function applyScrollY(
    path: PathSeg[],
    scrollContext: NonNullable<ReturnType<typeof getScrollContext>>,
    scrollY: number,
    operations: { primary: string; fallback: string },
  ): void {
    if (typeof scrollContext.scrollOwner.scrollTo === 'function') {
      try {
        scrollContext.scrollOwner.scrollTo({ x: 0, y: scrollY });
      } catch (error) {
        deps.handleError(error, { component: 'GraphViewer', operation: operations.primary, metadata: { scrollY } });
        try {
          scrollContext.scrollOwner.scrollTo(0, scrollY);
        } catch (fallbackError) {
          deps.handleError(fallbackError, { component: 'GraphViewer', operation: operations.fallback });
        }
      }
    } else if ('scrollY' in scrollContext.scrollOwner) {
      scrollContext.scrollOwner.scrollY = scrollY;
    }
    deps.setGraphRowScrollTestState(path, scrollY);
    deps.updateLeafer();
  }

  function scrollRowIntoView(path: PathSeg[], entry: ReturnType<typeof getCellEntry>): void {
    const scrollContext = getScrollContext(entry);
    if (!scrollContext) return;
    const targetY = (scrollContext.row.y ?? 0) + (scrollContext.row.height ?? 0) / 2 - scrollContext.bodyHeight / 2;
    applyScrollY(path, scrollContext, clampScrollY(scrollContext, targetY), {
      primary: 'scrollTo',
      fallback: 'scrollToFallback',
    });
  }

  function getIndexedTableTarget(path: PathSeg[]): { tablePath: PathSeg[]; tablePathKey: string; rowIndex: number; rowSegIndex: number } | null {
    for (let index = path.length - 1; index >= 0; index -= 1) {
      const segment = path[index];
      if (!segment || !isPathSegIndex(segment)) continue;
      const tablePath = path.slice(0, index);
      const tablePathKey = buildPathKey(tablePath);
      if (!tablePathKey) return null;
      return {
        tablePath,
        tablePathKey,
        rowIndex: segment.index,
        rowSegIndex: index,
      };
    }
    return null;
  }

  function findTableScrollFallback(path: PathSeg[]): {
    scrollContext: NonNullable<ReturnType<typeof getScrollContext>>;
    rowIndex: number;
    rowOffsetY?: number;
    rowHeight?: number;
  } | null {
    const indexedTarget = getIndexedTableTarget(path);
    const uniqueEntries = new Set(deps.getCellBoxByPathMap().values());
    if (indexedTarget) {
      const { node } = resolveNodeForPath(indexedTarget.tablePath);
      const targetRow = node?.table?.rows?.[indexedTarget.rowIndex];
      const firstRow = node?.table?.rows?.[0];
      const rowOffsetY =
        targetRow && firstRow ? Math.max(0, (targetRow.boxArgs.y ?? 0) - (firstRow.boxArgs.y ?? 0)) : undefined;
      const rowHeight =
        targetRow?.boxArgs.height && targetRow.boxArgs.height > 0
          ? Number(targetRow.boxArgs.height)
          : node?.table?.rowHeight && node.table.rowHeight > 0
            ? Number(node.table.rowHeight)
            : undefined;
      for (const candidate of uniqueEntries) {
        const candidatePath = candidate.cell?.path;
        if (!Array.isArray(candidatePath) || candidatePath.length <= indexedTarget.rowSegIndex) continue;
        const candidatePrefixKey = buildPathKey(candidatePath.slice(0, indexedTarget.rowSegIndex));
        if (candidatePrefixKey !== indexedTarget.tablePathKey) continue;
        if (typeof candidatePath[indexedTarget.rowSegIndex]?.index !== 'number') continue;
        const scrollContext = getScrollContext(candidate);
        if (!scrollContext) continue;
        return { scrollContext, rowIndex: indexedTarget.rowIndex, rowOffsetY, rowHeight };
      }
    }

    const { node } = resolveNodeForPath(path);
    if (!node?.table?.rows?.length || !Array.isArray(node.path) || node.path.length === 0) return null;
    const tablePathKey = buildPathKey(node.path);
    if (!tablePathKey) return null;

    let rowIndex = -1;
    const targetPathKey = buildPathKey(path);
    if (!targetPathKey) return null;
    const tablePath = node.path;
    for (const [index, row] of node.table.rows.entries()) {
      const keyCell = row.cells?.[0];
      if (!keyCell) continue;
      const inferredSeg = keyCell.isIndex ? deps.buildPathSegFromCell(keyCell, index) : null;
      const rowPath =
        Array.isArray(keyCell.path) && keyCell.path.length > 0
          ? keyCell.path
          : inferredSeg
            ? [...tablePath, inferredSeg]
            : [];
      const rowPathKey = buildPathKey(rowPath);
      if (!rowPathKey) continue;
      if (rowPathKey === targetPathKey || targetPathKey.startsWith(`${rowPathKey}.`)) {
        rowIndex = index;
        break;
      }
    }
    if (rowIndex < 0) return null;

    const targetRow = node.table.rows[rowIndex];
    const firstRow = node.table.rows[0];
    const rowOffsetY =
      targetRow && firstRow ? Math.max(0, (targetRow.boxArgs.y ?? 0) - (firstRow.boxArgs.y ?? 0)) : undefined;
    const rowHeight =
      targetRow?.boxArgs.height && targetRow.boxArgs.height > 0
        ? Number(targetRow.boxArgs.height)
        : node.table.rowHeight && node.table.rowHeight > 0
          ? Number(node.table.rowHeight)
          : undefined;

    for (const candidate of uniqueEntries) {
      const candidatePath = candidate.cell?.path;
      if (!Array.isArray(candidatePath) || candidatePath.length === 0) continue;
      const candidatePathKey = buildPathKey(candidatePath);
      if (!candidatePathKey || (candidatePathKey !== tablePathKey && !candidatePathKey.startsWith(`${tablePathKey}.`))) {
        continue;
      }
      const scrollContext = getScrollContext(candidate);
      if (!scrollContext) continue;
      return { scrollContext, rowIndex, rowOffsetY, rowHeight };
    }
    return null;
  }

  function scrollRowIntoViewFromFallback(path: PathSeg[]): void {
    const fallback = findTableScrollFallback(path);
    if (!fallback) return;
    const { scrollContext, rowIndex } = fallback;
    const rowHeight =
      typeof fallback.rowHeight === 'number' && fallback.rowHeight > 0
        ? fallback.rowHeight
        : typeof scrollContext.row.height === 'number' && scrollContext.row.height > 0
          ? Number(scrollContext.row.height)
          : Math.max(1, scrollContext.contentHeight / Math.max(1, rowIndex + 1));
    const rowOffsetY =
      typeof fallback.rowOffsetY === 'number' && fallback.rowOffsetY >= 0 ? fallback.rowOffsetY : rowIndex * rowHeight;
    const targetY = rowOffsetY + rowHeight / 2 - scrollContext.bodyHeight / 2;
    applyScrollY(path, scrollContext, clampScrollY(scrollContext, targetY), {
      primary: 'scrollToFallbackRow',
      fallback: 'scrollToFallbackRowLegacy',
    });
  }

  function revealPathInternal(path: PathSeg[], options?: { target?: 'key' | 'value' | 'node'; navigate?: boolean }): void {
    const cellBoxByPathMap = deps.getCellBoxByPathMap();
    const nodeBoxMap = deps.getNodeBoxMap();
    const renderConfig = deps.getRenderConfig();
    if (!path || path.length === 0) return;
    const { renderHandle, node } = resolveNodeForPath(path);
    let entry = getCellEntry(cellBoxByPathMap, path);
    const hasRenderableEntry = (candidate: ReturnType<typeof getCellEntry>): boolean =>
      !!(candidate?.row || candidate?.key || candidate?.value);
    clearSearchHighlight();
    activeHighlightState = { path: [...path], target: options?.target };
    if (options?.navigate) {
      if (!hasRenderableEntry(entry)) {
        scrollRowIntoViewFromFallback(path);
        entry = getCellEntry(cellBoxByPathMap, path);
      }
      scrollRowIntoView(path, entry);
    }
    const cellHighlight = renderConfig.colors.table.hoverCellBackground;
    const rowHighlight = renderConfig.colors.table.hoverRowBackground;
    if (entry?.row) applySearchHighlight(entry.row, { fill: rowHighlight });
    const { target: resolvedTarget, box: highlightBox } = getHighlightTarget(entry, options?.target);
    if ((resolvedTarget === 'key' || resolvedTarget === 'value') && highlightBox) {
      applySearchHighlight(highlightBox, { fill: cellHighlight });
    }
    const focusBox = highlightBox ?? entry?.row ?? null;
    deps.setGraphHighlightTestState(path, resolvedTarget, focusBox ?? entry?.row ?? null);
    if (options?.navigate) {
      deps.setGraphRevealTestState(path, resolvedTarget);
      const centeredOnBox = focusBox ? deps.centerOnBox(focusBox) : false;
      if (!centeredOnBox && node) deps.centerOnNode(node);
      deps.setGraphHighlightTestState(path, resolvedTarget, focusBox ?? entry?.row ?? null);
    } else if (renderHandle != null) {
      const nodeBox = nodeBoxMap.get(renderHandle);
      if (nodeBox) applySearchHighlight(nodeBox, { fill: rowHighlight });
    }
  }

  function revealSearchResult(result: GraphTextLinkageSearchResult): void {
    if (!result?.path?.length) return;
    const token = (revealPathToken += 1);
    const run = async () => {
      await ensurePathIndex(result.path);
      if (token !== revealPathToken) return;
      revealPathInternal(result.path, { target: result.target, navigate: true });
      await Promise.resolve();
      await ensurePathIndex(result.path);
      if (token !== revealPathToken) return;
      revealPathInternal(result.path, { target: result.target, navigate: false });
      emitReveal(result.path, result.target, 'search');
    };
    void run();
  }

  function revealPath(path: PathSeg[], options?: { target?: 'key' | 'value' | 'node'; navigate?: boolean }): void {
    if (!path || path.length === 0) return;
    const token = (revealPathToken += 1);
    const run = async () => {
      await ensurePathIndex(path);
      if (token !== revealPathToken) return;
      revealPathInternal(path, options);
      if (!options?.navigate) return;
      await Promise.resolve();
      await ensurePathIndex(path);
      if (token !== revealPathToken) return;
      revealPathInternal(path, { ...options, navigate: false });
    };
    void run();
  }

  function refreshActiveHighlight(): void {
    if (!activeHighlightState?.path?.length) return;
    revealPathInternal(activeHighlightState.path, { target: activeHighlightState.target, navigate: false });
  }

  return {
    clearSearchHighlight,
    resolveTreePathByPosition,
    ensurePathIndex,
    hydrateResolvedGraphPaths,
    emitReveal,
    revealSearchResult,
    revealPath,
    refreshActiveHighlight,
  };
}
