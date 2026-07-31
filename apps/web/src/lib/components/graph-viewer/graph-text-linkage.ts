import type { GraphViewerConfig } from "../../settings/ui-settings";
import type { SupportedEditorLanguageId } from "../../monaco/language-support";
import type { SnapshotId } from "@core-wasm/index";
import { buildPathKey } from "../../graph/graph-viewer-path";
import type { GraphCell, GraphNode } from '@treease/graph-viewer-runtime';
import { isPathSegIndex, type PathSeg } from "../../store/tree-path";
import type { GraphHighlightTarget } from "../../store/graph-selection-store";
import { resolveTreePathFromTextResult } from "../../services/TreePathService";
import { queryPathValue } from "../../services/SnapshotProjectionService";
import type { CellBoxEntry, GraphViewerClickTarget, LeaferBox } from "./model";
import {
  getCellEntry,
  getHighlightTarget,
  getScrollContext,
} from "./graph-anchor-index";
import type { TableCellAnchor } from "./graph-table-anchor-index";

export type GraphTextLinkageSearchResult = {
  target: "key" | "value" | "node";
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
  getTableCellAnchorMap?: () => Map<string, TableCellAnchor>;
  getPathKeyToRenderHandleMap: () => Map<string, number>;
  materializeTarget?: (renderHandle: number) => Promise<boolean>;
  getClickTargetProbes: () => GraphViewerClickTarget[];
  setGraphHighlightTestState: (
    path: PathSeg[] | null,
    target?: GraphHighlightTarget,
    box?: LeaferBox | null,
  ) => void;
  setGraphRevealTestState: (
    path: PathSeg[] | null,
    target?: GraphHighlightTarget,
  ) => void;
  setGraphRowScrollTestState: (
    path: PathSeg[] | null,
    scrollY?: number,
  ) => void;
  scrollTableCellAnchorIntoView?: (anchor: TableCellAnchor) => boolean;
  buildPathSegFromCell: (
    cell: GraphCell | undefined,
    rowIndex: number,
  ) => PathSeg | null;
  upsertCellEntry: (
    map: Map<string, CellBoxEntry>,
    cell: GraphCell,
    updater: (entry: CellBoxEntry) => void,
  ) => void;
  centerOnBox: (box: LeaferBox) => boolean;
  centerOnNode: (node: GraphNode) => void;
  updateLeafer: () => void;
  updateActiveTempModel: (updater: (current: any) => any) => void;
  getEditorRevision: () => number;
  getGraphAppliedRevision: () => number;
  getEnableRevealSync?: () => boolean;
  dispatchReveal: (
    path: PathSeg[],
    target?: GraphHighlightTarget,
    trigger?: string,
  ) => void;
  handleError: (
    error: unknown,
    context: {
      component: string;
      operation: string;
      metadata?: Record<string, unknown>;
    },
  ) => void;
};

type RevealTarget = "key" | "value" | "node";
type RevealOptions = {
  target?: RevealTarget;
  navigate?: boolean;
};

type TableRowMetrics = {
  rowOffsetY?: number;
  rowHeight?: number;
};

export function createGraphTextLinkageController(
  deps: GraphTextLinkageControllerDeps,
) {
  let revealPathToken = 0;
  let activeHighlightState: {
    path: PathSeg[];
    target?: GraphHighlightTarget;
  } | null = null;
  let highlightValidationToken = 0;

  function clearRenderedSearchHighlights(): void {
    deps.setGraphHighlightTestState(null);
  }

  function clearSearchHighlight(): void {
    clearRenderedSearchHighlights();
    activeHighlightState = null;
  }

  async function resolveTreePathByPosition(
    row: number,
    column: number,
  ): Promise<PathSeg[]> {
    const documentKey = deps.getDocumentKey();
    if (!documentKey) return [];
    const text = deps.getSourceText();
    if (!text) return [];
    const snapshotId = deps.getActiveSnapshotId();
    if (snapshotId == null) return [];
    try {
      const result = await resolveTreePathFromTextResult(
        text,
        row,
        column,
        documentKey,
        deps.getLanguageId(),
        deps.getEnableNest(),
        "auto",
        snapshotId,
      );
      return result.status === "ready" ? result.data : [];
    } catch (error) {
      deps.handleError(error, {
        component: "GraphViewer",
        operation: "resolveTreePath",
        metadata: { documentKey, row, column },
      });
      return [];
    }
  }

  async function ensurePathIndex(path: PathSeg[]): Promise<void> {
    const pathKeyToRenderHandleMap = deps.getPathKeyToRenderHandleMap();
    const cellBoxByPathMap = deps.getCellBoxByPathMap();
    const nodeDataMap = deps.getNodeDataMap();
    const targetKey = buildPathKey(path);
    if (!targetKey) return;
    if (
      pathKeyToRenderHandleMap.has(targetKey) ||
      cellBoxByPathMap.has(targetKey)
    ) {
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

  async function hydrateResolvedGraphPaths(
    nodes: GraphNode[],
    _text: string,
  ): Promise<void> {
    for (const node of nodes) {
      if (
        (!Array.isArray(node.meta?.path) || node.meta.path.length === 0) &&
        Array.isArray(node.path) &&
        node.path.length > 0
      ) {
        node.meta.path = [...node.path];
      }
      const shouldInferRowPath =
        node.kind === "table" &&
        (node.meta?.valueType === "object" || node.meta?.valueType === "array");
      const rows =
        node.kind === "table" ? (node.table?.rows ?? []) : (node.rows ?? []);
      for (const [rowIndex, row] of rows.entries()) {
        const rowKeyCell = row.cells?.[0];
        if (
          node.kind === "object" &&
          row.cells?.length >= 2 &&
          Array.isArray(rowKeyCell?.path) &&
          rowKeyCell.path.length > 0
        ) {
          row.cells[1].path = [...rowKeyCell.path];
        }
        const inferredSeg =
          shouldInferRowPath && rowKeyCell
            ? deps.buildPathSegFromCell(rowKeyCell, rowIndex)
            : null;
        const inferredRowPath = inferredSeg
          ? [...(Array.isArray(node.path) ? node.path : []), inferredSeg]
          : [];
        for (const cell of row.cells ?? []) {
          if (Array.isArray(cell.path) && cell.path.length > 0) continue;
          if (inferredRowPath.length) {
            cell.path = inferredRowPath;
          }
        }
      }
    }
  }

  function syncTreeSelection(
    path: PathSeg[],
    target?: GraphHighlightTarget,
    trigger?: string,
  ): void {
    if (!path?.length || deps.getEnableRevealSync?.() === false) return;
    const source = trigger === 'search' || trigger === 'breadcrumb' ? trigger : 'graph';
    deps.updateActiveTempModel((current) => ({
      ...current,
      treePath: path,
      graphHighlight: {
        path,
        target,
        revision: Math.max(
          deps.getEditorRevision(),
          deps.getGraphAppliedRevision(),
        ),
        source,
      },
    }));
  }

  function emitReveal(
    path: PathSeg[],
    target?: GraphHighlightTarget,
    trigger?: string,
  ): void {
    if (!path?.length) return;
    syncTreeSelection(path, target, trigger);
    deps.setGraphRevealTestState(path, target);
    deps.dispatchReveal(path, target, trigger);
  }

  function resolveNodeForPath(path: PathSeg[]): {
    renderHandle?: number;
    node: GraphNode | null;
  } {
    const pathKeyToRenderHandleMap = deps.getPathKeyToRenderHandleMap();
    const nodeDataMap = deps.getNodeDataMap();
    const pathKey = buildPathKey(path);
    let renderHandle = pathKey
      ? pathKeyToRenderHandleMap.get(pathKey)
      : undefined;
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
    return {
      renderHandle,
      node:
        renderHandle != null ? (nodeDataMap.get(renderHandle) ?? null) : null,
    };
  }

  function clampScrollY(
    scrollContext: NonNullable<ReturnType<typeof getScrollContext>>,
    targetY: number,
  ): number {
    const maxScroll = Math.max(
      0,
      scrollContext.contentHeight - scrollContext.bodyHeight,
    );
    return Math.max(0, Math.min(targetY, maxScroll));
  }

  function applyScrollY(
    path: PathSeg[],
    scrollContext: NonNullable<ReturnType<typeof getScrollContext>>,
    scrollY: number,
    operations: { primary: string; fallback: string },
  ): void {
    if (typeof scrollContext.scrollOwner.scrollTo === "function") {
      try {
        scrollContext.scrollOwner.scrollTo({ x: 0, y: scrollY });
      } catch (error) {
        deps.handleError(error, {
          component: "GraphViewer",
          operation: operations.primary,
          metadata: { scrollY },
        });
        try {
          scrollContext.scrollOwner.scrollTo(0, scrollY);
        } catch (fallbackError) {
          deps.handleError(fallbackError, {
            component: "GraphViewer",
            operation: operations.fallback,
          });
        }
      }
    } else if ("scrollY" in scrollContext.scrollOwner) {
      scrollContext.scrollOwner.scrollY = scrollY;
    }
    deps.setGraphRowScrollTestState(path, scrollY);
    deps.updateLeafer();
  }

  function scrollRowIntoView(
    path: PathSeg[],
    entry: ReturnType<typeof getCellEntry>,
  ): void {
    const scrollContext = getScrollContext(entry);
    if (!scrollContext) return;
    const targetY =
      (scrollContext.row.y ?? 0) +
      (scrollContext.row.height ?? 0) / 2 -
      scrollContext.bodyHeight / 2;
    applyScrollY(path, scrollContext, clampScrollY(scrollContext, targetY), {
      primary: "scrollTo",
      fallback: "scrollToFallback",
    });
  }

  function getIndexedTableTarget(
    path: PathSeg[],
  ): {
    tablePath: PathSeg[];
    tablePathKey: string;
    rowIndex: number;
    rowSegIndex: number;
  } | null {
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

  function getTableRowMetrics(
    targetRow: { boxArgs?: { y?: number; height?: number } } | undefined,
    firstRow: { boxArgs?: { y?: number } } | undefined,
    fallbackRowHeight?: number,
  ): TableRowMetrics {
    const rowOffsetY =
      targetRow && firstRow
        ? Math.max(0, (targetRow.boxArgs?.y ?? 0) - (firstRow.boxArgs?.y ?? 0))
        : undefined;
    const rowHeight =
      targetRow?.boxArgs?.height && targetRow.boxArgs.height > 0
        ? Number(targetRow.boxArgs.height)
        : fallbackRowHeight && fallbackRowHeight > 0
          ? Number(fallbackRowHeight)
          : undefined;
    return { rowOffsetY, rowHeight };
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
      const { rowOffsetY, rowHeight } = getTableRowMetrics(
        targetRow,
        firstRow,
        node?.table?.rowHeight,
      );
      for (const candidate of uniqueEntries) {
        const candidatePath = candidate.cell?.path;
        if (
          !Array.isArray(candidatePath) ||
          candidatePath.length <= indexedTarget.rowSegIndex
        )
          continue;
        const candidatePrefixKey = buildPathKey(
          candidatePath.slice(0, indexedTarget.rowSegIndex),
        );
        if (candidatePrefixKey !== indexedTarget.tablePathKey) continue;
        if (typeof candidatePath[indexedTarget.rowSegIndex]?.index !== "number")
          continue;
        const scrollContext = getScrollContext(candidate);
        if (!scrollContext) continue;
        return {
          scrollContext,
          rowIndex: indexedTarget.rowIndex,
          rowOffsetY,
          rowHeight,
        };
      }
    }

    const { node } = resolveNodeForPath(path);
    if (
      !node?.table?.rows?.length ||
      !Array.isArray(node.path) ||
      node.path.length === 0
    )
      return null;
    const tablePathKey = buildPathKey(node.path);
    if (!tablePathKey) return null;

    let rowIndex = -1;
    const targetPathKey = buildPathKey(path);
    if (!targetPathKey) return null;
    const tablePath = node.path;
    for (const [index, row] of node.table.rows.entries()) {
      const keyCell = row.cells?.[0];
      if (!keyCell) continue;
      const inferredSeg = keyCell.isIndex
        ? deps.buildPathSegFromCell(keyCell, index)
        : null;
      const rowPath =
        Array.isArray(keyCell.path) && keyCell.path.length > 0
          ? keyCell.path
          : inferredSeg
            ? [...tablePath, inferredSeg]
            : [];
      const rowPathKey = buildPathKey(rowPath);
      if (!rowPathKey) continue;
      if (
        rowPathKey === targetPathKey ||
        targetPathKey.startsWith(`${rowPathKey}.`)
      ) {
        rowIndex = index;
        break;
      }
    }
    if (rowIndex < 0) return null;

    const targetRow = node.table.rows[rowIndex];
    const firstRow = node.table.rows[0];
    const { rowOffsetY, rowHeight } = getTableRowMetrics(
      targetRow,
      firstRow,
      node.table.rowHeight,
    );

    for (const candidate of uniqueEntries) {
      const candidatePath = candidate.cell?.path;
      if (!Array.isArray(candidatePath) || candidatePath.length === 0) continue;
      const candidatePathKey = buildPathKey(candidatePath);
      if (
        !candidatePathKey ||
        (candidatePathKey !== tablePathKey &&
          !candidatePathKey.startsWith(`${tablePathKey}.`))
      ) {
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
      typeof fallback.rowHeight === "number" && fallback.rowHeight > 0
        ? fallback.rowHeight
        : typeof scrollContext.row.height === "number" &&
            scrollContext.row.height > 0
          ? Number(scrollContext.row.height)
          : Math.max(
              1,
              scrollContext.contentHeight / Math.max(1, rowIndex + 1),
            );
    const rowOffsetY =
      typeof fallback.rowOffsetY === "number" && fallback.rowOffsetY >= 0
        ? fallback.rowOffsetY
        : rowIndex * rowHeight;
    const targetY = rowOffsetY + rowHeight / 2 - scrollContext.bodyHeight / 2;
    applyScrollY(path, scrollContext, clampScrollY(scrollContext, targetY), {
      primary: "scrollToFallbackRow",
      fallback: "scrollToFallbackRowLegacy",
    });
  }

  function hasRenderableEntry(
    candidate: ReturnType<typeof getCellEntry>,
  ): boolean {
    return !!(candidate?.row || candidate?.key || candidate?.value);
  }

  function revealPathInternal(
    path: PathSeg[],
    options?: RevealOptions,
  ): boolean {
    const cellBoxByPathMap = deps.getCellBoxByPathMap();
    const nodeBoxMap = deps.getNodeBoxMap();
    if (!path || path.length === 0) return false;
    const { renderHandle, node } = resolveNodeForPath(path);
    let entry = getCellEntry(cellBoxByPathMap, path);
    if (options?.navigate && !entry) {
      const anchor = deps.getTableCellAnchorMap?.().get(buildPathKey(path));
      if (anchor && deps.scrollTableCellAnchorIntoView?.(anchor)) {
        entry = getCellEntry(cellBoxByPathMap, path);
      }
    }
    let missingRenderableContext =
      !hasRenderableEntry(entry) && renderHandle == null && !node;
    if (missingRenderableContext && options?.navigate) {
      scrollRowIntoViewFromFallback(path);
      entry = getCellEntry(cellBoxByPathMap, path);
      missingRenderableContext =
        !hasRenderableEntry(entry) && renderHandle == null && !node;
    }
    if (missingRenderableContext) {
      clearRenderedSearchHighlights();
      activeHighlightState = { path: [...path], target: options?.target };
      return false;
    }
    clearSearchHighlight();
    activeHighlightState = { path: [...path], target: options?.target };
    if (options?.navigate) {
      if (!hasRenderableEntry(entry)) {
        scrollRowIntoViewFromFallback(path);
        entry = getCellEntry(cellBoxByPathMap, path);
      }
      scrollRowIntoView(path, entry);
    }
    const { target: resolvedTarget, box: highlightBox } = getHighlightTarget(
      entry,
      options?.target,
    );
    const focusBox = highlightBox ?? entry?.row ?? null;
    deps.setGraphHighlightTestState(
      path,
      resolvedTarget,
      focusBox ?? entry?.row ?? null,
    );
    if (options?.navigate) {
      deps.setGraphRevealTestState(path, resolvedTarget);
      const centeredOnBox = focusBox ? deps.centerOnBox(focusBox) : false;
      if (!centeredOnBox && node) deps.centerOnNode(node);
      deps.setGraphHighlightTestState(
        path,
        resolvedTarget,
        focusBox ?? entry?.row ?? null,
      );
    }
    return true;
  }

  async function runRevealSequence(
    path: PathSeg[],
    options?: RevealOptions,
    afterStable?: () => void,
  ): Promise<boolean> {
    const token = (revealPathToken += 1);
    await ensurePathIndex(path);
    if (token !== revealPathToken) return false;
    const target = resolveNodeForPath(path);
    if (
      options?.navigate &&
      target.renderHandle != null &&
      deps.materializeTarget
    ) {
      const materialized = await deps.materializeTarget(target.renderHandle);
      if (materialized === false || token !== revealPathToken) return false;
    }
    const firstReveal = revealPathInternal(path, options);
    if (!options?.navigate) return firstReveal;
    await Promise.resolve();
    await ensurePathIndex(path);
    if (token !== revealPathToken) return false;
    const stableReveal = revealPathInternal(path, {
      ...options,
      navigate: false,
    });
    if (stableReveal) afterStable?.();
    return stableReveal;
  }

  function revealSearchResult(result: GraphTextLinkageSearchResult): void {
    if (!result?.path?.length) return;
    runRevealSequence(
      result.path,
      { target: result.target, navigate: true },
      () => {
        emitReveal(result.path, result.target, "search");
      },
    );
  }

  function revealPath(
    path: PathSeg[],
    options?: RevealOptions,
  ): Promise<boolean> {
    if (!path || path.length === 0) return Promise.resolve(false);
    return runRevealSequence(path, options);
  }

  function refreshActiveHighlight(): void {
    if (!activeHighlightState?.path?.length) return;
    revealPathInternal(activeHighlightState.path, {
      target: activeHighlightState.target,
      navigate: false,
    });
  }

  /**
   * Render bindings are deliberately not used to decide whether a selection
   * still exists: a scene replacement temporarily has no bindings.  The
   * snapshot projection is the authority for clearing a retained selection.
   */
  async function reconcileActiveHighlight(options: {
    documentKey: string;
    snapshotId: SnapshotId | null;
    graphAppliedRevision: number;
  }): Promise<void> {
    const active = activeHighlightState;
    if (!active?.path.length || !options.documentKey || options.snapshotId == null) return;

    const token = ++highlightValidationToken;
    const result = await queryPathValue({
      documentKey: options.documentKey,
      snapshotId: options.snapshotId,
      path: active.path,
    });
    if (
      token !== highlightValidationToken ||
      activeHighlightState !== active ||
      deps.getDocumentKey() !== options.documentKey ||
      deps.getActiveSnapshotId() !== options.snapshotId ||
      deps.getGraphAppliedRevision() !== options.graphAppliedRevision
    ) {
      return;
    }
    // Unready and failed reads are not proof that the path disappeared.
    if (result.status !== "ready" || result.data != null) return;

    clearSearchHighlight();
    const activePathKey = buildPathKey(active.path);
    deps.updateActiveTempModel((current) => {
      const highlight = current.graphHighlight;
      if (!highlight || buildPathKey(highlight.path) !== activePathKey) return current;
      const treePathMatches = Array.isArray(current.treePath) &&
        buildPathKey(current.treePath) === activePathKey;
      return {
        ...current,
        ...(treePathMatches ? { treePath: [] } : {}),
        graphHighlight: null,
      };
    });
  }

  return {
    clearSearchHighlight,
    clearRenderedSearchHighlights,
    resolveTreePathByPosition,
    ensurePathIndex,
    hydrateResolvedGraphPaths,
    emitReveal,
    revealSearchResult,
    revealPath,
    refreshActiveHighlight,
    reconcileActiveHighlight,
  };
}
