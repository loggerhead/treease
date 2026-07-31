import { serializePath } from '../../../shared/document-anchor-utils';
import { callSharedWasmWorker } from '../../wasm/wasm-worker-singleton';
import type { DocumentProjectionDelta, SnapshotId, SnapshotReadResult } from '@core-wasm/index';
import type { GraphViewerConfig } from '../../settings/ui-settings';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import {
  buildReadablePath,
  isPathSegIndex,
  isPathSegKey,
  pathSegKeyValue,
  type PathSeg,
} from '../../store/tree-path';
import type { GraphCell, GraphEdge, GraphNode, ValueType } from '@treease/graph-viewer-runtime';
import type { ColumnNavigatorGraphData } from './column-navigator-types';
import { isRawGraphDelta } from '../../../shared/worker-protocol/graph-delta-normalize';
import { normalizeGraphDelta } from '../../../shared/worker-protocol/graph-stream-event-codec';
import { buildPathKey } from '../../graph/graph-viewer-path';
import type { ColumnNavigatorColumnItem } from './column-navigator/types';

type GraphCacheEntry = {
  signature: string;
  accessOrder: number;
  graph?: ColumnNavigatorGraphData | null;
  promise?: Promise<ColumnNavigatorGraphData | null>;
};

type GraphCacheDeps = {
  getActiveSnapshotId: () => SnapshotId | null;
  getDocumentKey: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  getRevision: () => number;
  getEnableNest: () => boolean;
  getRenderConfig: () => GraphViewerConfig;
  inferGraphPaths: (nodes: GraphNode[], edges: GraphEdge[]) => void;
};

const graphCacheLimit = 24;

function buildMetaPathText(path: PathSeg[]): string {
  const readable = buildReadablePath(path);
  if (readable === '$') return readable;
  if (readable.startsWith('$.')) return readable.slice(2);
  if (readable.startsWith('$[')) return readable.slice(1);
  return readable;
}

export function rebaseColumnNavigatorPath(basePath: PathSeg[], path: PathSeg[]): PathSeg[] {
  if (!basePath.length) return path;
  if (!path.length) return basePath;
  const pathKey = buildWorkspacePathKey(path);
  const basePathKey = buildWorkspacePathKey(basePath);
  return pathKey === basePathKey || pathKey.startsWith(`${basePathKey}|`) ? path : [...basePath, ...path];
}

export function formatColumnNavigatorPath(path: PathSeg[]): string {
  return buildMetaPathText(path);
}

export function shouldIgnoreSubgraphOpenCell(cell: GraphCell | null | undefined): boolean {
  return cell?.isMissing === true;
}

export function shouldOpenColumnNavigatorContent(value: {
  valueType?: string | null;
  displayText?: string | null;
}): boolean {
  if (value.valueType !== 'object' && value.valueType !== 'array') return true;
  return value.displayText === '{}' || value.displayText === '[]';
}

export function buildWorkspacePathKey(path: PathSeg[]): string {
  if (!path.length) return '';
  return path.map((segment) => (isPathSegKey(segment) ? `k:${pathSegKeyValue(segment)}` : `i:${segment.index}`)).join('|');
}

function pathsEqual(left: PathSeg[], right: PathSeg[]): boolean {
  return buildWorkspacePathKey(left) === buildWorkspacePathKey(right);
}

function itemLabel(path: PathSeg[]): string {
  const segment = path.at(-1);
  if (!segment) return '$';
  return isPathSegIndex(segment) ? `${segment.index}` : pathSegKeyValue(segment);
}

function itemPreview(cell: GraphCell, childCount: number): string {
  // Derive container cardinality from the projected child paths. Core's
  // display text may be the key (`object`/`preview`) rather than a count.
  if (cell.valueType === 'object' || cell.valueType === 'array') {
    const prefix = cell.valueType === 'array' ? '[' : '{';
    const suffix = cell.valueType === 'array' ? ']' : '}';
    return childCount === 0 ? `${prefix}${suffix}` : `${prefix}${childCount}${suffix}`;
  }
  if (cell.valueType === 'null') return 'null';
  return cell.value ?? cell.text ?? '';
}

function collectGraphCells(graph: ColumnNavigatorGraphData): GraphCell[] {
  const cells: GraphCell[] = [];
  for (const node of graph.nodes) {
    if (node.meta) cells.push(node.meta);
    const rows = node.kind === 'table' ? (node.table?.rows ?? []) : node.rows;
    for (const row of rows) cells.push(...row.cells);
  }
  return cells;
}

/**
 * A column is a snapshot projection flattened to exactly one path level.
 * Rebase first because Core may return either projection-relative or absolute cell paths.
 */
export function buildColumnNavigatorColumnItems(
  graph: ColumnNavigatorGraphData,
  containerPath: PathSeg[],
): ColumnNavigatorColumnItem[] {
  const byPath = new Map<string, ColumnNavigatorColumnItem>();
  const cells = collectGraphCells(graph);
  const directChildCounts = new Map<string, Set<string>>();
  for (const cell of cells) {
    const absolutePath = rebaseColumnNavigatorPath(containerPath, cell.path ?? []);
    if (absolutePath.length <= containerPath.length + 1) continue;
    const parentPath = absolutePath.slice(0, -1);
    const parentKey = buildWorkspacePathKey(parentPath);
    const childKey = buildWorkspacePathKey(absolutePath);
    const children = directChildCounts.get(parentKey) ?? new Set<string>();
    children.add(childKey);
    directChildCounts.set(parentKey, children);
  }
  for (const cell of cells) {
    if (cell.isMissing) continue;
    const absolutePath = rebaseColumnNavigatorPath(containerPath, cell.path ?? []);
    if (absolutePath.length !== containerPath.length + 1) continue;
    if (!pathsEqual(absolutePath.slice(0, containerPath.length), containerPath)) continue;
    const pathKey = buildPathKey(absolutePath);
    if (!pathKey) continue;
    const next: ColumnNavigatorColumnItem = {
      path: absolutePath,
      pathKey,
      label: itemLabel(absolutePath),
      preview: itemPreview(cell, directChildCounts.get(pathKey)?.size ?? 0),
      valueType: cell.valueType as ValueType,
      semType: cell.semType ?? null,
      isContainer:
        (cell.valueType === 'object' || cell.valueType === 'array') && (directChildCounts.get(pathKey)?.size ?? 0) > 0,
    };
    const current = byPath.get(pathKey);
    const hasBetterValuePreview =
      current != null &&
      current.preview === current.label &&
      next.preview !== next.label;
    const hasSemanticValue =
      current != null &&
      current.semType == null &&
      next.semType != null;
    if (!current || next.isContainer || hasBetterValuePreview || hasSemanticValue || (!current.preview && next.preview)) {
      byPath.set(pathKey, next);
    }
  }
  return [...byPath.values()];
}

export function buildColumnNavigatorRenderSignature(renderConfig: GraphViewerConfig): string {
  return [
    renderConfig.columns.keyColumnMaxWidth,
    renderConfig.columns.valueColumnMaxWidth,
    renderConfig.layout.rowHeight,
    renderConfig.layout.layerGapX,
    renderConfig.layout.layerGapY,
    renderConfig.layout.tableMaxHeight,
    renderConfig.layout.tableRowHeight,
    renderConfig.layout.tableHeaderHeight,
  ].join('|');
}

function buildGraphCacheSignature(deps: GraphCacheDeps): string {
  const snapshotId = deps.getActiveSnapshotId();
  return [
    deps.getDocumentKey(),
    snapshotId ?? 'no-snapshot',
    deps.getLanguageId(),
    deps.getRevision(),
    deps.getEnableNest() ? 'nest' : 'flat',
    buildColumnNavigatorRenderSignature(deps.getRenderConfig()),
  ].join('|');
}

export function normalizeWorkspaceGraphEdgeRows(nodes: GraphNode[], edges: GraphEdge[]): GraphEdge[] {
  if (!nodes.length || !edges.length) return edges;
  const headeredTableHandles = new Set(
    nodes
      .filter((node) => node.kind === 'table' && node.table && (node.table.headerHeight ?? 0) > 0 && (node.table.rows?.length ?? 0) > 0)
      .map((node) => node.renderHandle),
  );
  if (!headeredTableHandles.size) return edges;

  const collectHandlesNeedingOffset = (
    readHandle: (edge: GraphEdge) => number,
    readRow: (edge: GraphEdge) => number,
  ): Set<number> =>
    new Set(
      edges
        .filter((edge) => readRow(edge) === 0 && headeredTableHandles.has(readHandle(edge)))
        .map((edge) => readHandle(edge)),
    );

  const normalizeSourceHandles = collectHandlesNeedingOffset(
    (edge) => edge.fromRenderHandle,
    (edge) => edge.fromRow,
  );
  const normalizeTargetHandles = collectHandlesNeedingOffset(
    (edge) => edge.toRenderHandle,
    (edge) => edge.toRow,
  );
  if (!normalizeSourceHandles.size && !normalizeTargetHandles.size) return edges;

  return edges.map((edge) => ({
    ...edge,
    fromRow: normalizeSourceHandles.has(edge.fromRenderHandle) ? edge.fromRow + 1 : edge.fromRow,
    toRow: normalizeTargetHandles.has(edge.toRenderHandle) ? edge.toRow + 1 : edge.toRow,
  }));
}

function buildWorkspaceGraphData(
  deps: GraphCacheDeps,
  path: PathSeg[],
  pathKey: string,
  result: { nodes?: GraphNode[]; edges?: GraphEdge[] },
): ColumnNavigatorGraphData | null {
  const nodes = result.nodes ?? [];
  const edges = normalizeWorkspaceGraphEdgeRows(nodes, result.edges ?? []);
  if (!nodes.length) return null;
  deps.inferGraphPaths(nodes, edges);
  const minX = Math.min(...nodes.map((node) => node.boxArgs.x));
  const minY = Math.min(...nodes.map((node) => node.boxArgs.y));
  const maxX = Math.max(...nodes.map((node) => node.boxArgs.x + node.boxArgs.width));
  const maxY = Math.max(...nodes.map((node) => node.boxArgs.y + node.boxArgs.height));
  return {
    path,
    pathKey,
    nodes,
    edges,
    minX,
    minY,
    width: Math.max(0, maxX - minX),
    height: Math.max(0, maxY - minY),
  };
}

export function createColumnNavigatorGraphCache(deps: GraphCacheDeps) {
  const graphCache = new Map<string, GraphCacheEntry>();
  let graphCacheSignature = '';
  let graphCacheAccessOrder = 0;

  function ensureGraphCacheSignature(): void {
    const signature = buildGraphCacheSignature(deps);
    if (graphCacheSignature === signature) return;
    graphCache.clear();
    graphCacheSignature = signature;
  }

  function touchGraphCacheEntry(entry: GraphCacheEntry): void {
    graphCacheAccessOrder += 1;
    entry.accessOrder = graphCacheAccessOrder;
  }

  function trimGraphCache(): void {
    if (graphCache.size <= graphCacheLimit) return;
    const removable = [...graphCache.entries()]
      .filter(([, entry]) => !entry.promise)
      .sort((left, right) => left[1].accessOrder - right[1].accessOrder);
    while (graphCache.size > graphCacheLimit && removable.length) {
      const next = removable.shift();
      if (!next) break;
      graphCache.delete(next[0]);
    }
  }

  async function prepareGraph(path: PathSeg[]): Promise<ColumnNavigatorGraphData | null> {
    ensureGraphCacheSignature();
    const pathKey = buildWorkspacePathKey(path);
    const signature = graphCacheSignature;
    const current = graphCache.get(pathKey);
    if (current && current.signature === signature) {
      touchGraphCacheEntry(current);
      if (current.graph !== undefined) return current.graph;
      if (current.promise) return current.promise;
    }
    const entry: GraphCacheEntry = { signature, accessOrder: 0 };
    touchGraphCacheEntry(entry);
    entry.promise = Promise.resolve().then(async () => {
      const snapshotId = deps.getActiveSnapshotId();
      if (snapshotId == null) return null;
      const projection = await callSharedWasmWorker<SnapshotReadResult<DocumentProjectionDelta>>('buildHoverSubgraphProjection', {
        documentKey: deps.getDocumentKey(),
        snapshotId,
        path: serializePath(path),
      });
      if (projection.status !== 'ready' || !projection.data.graphData) return null;
      const delta = { ...projection.data.graphData, clear: projection.data.clear ? 1 : 0 };
      if (!isRawGraphDelta(delta)) {
        throw new Error('column navigator projection decode failed');
      }
      const normalized = normalizeGraphDelta(delta);
      return buildWorkspaceGraphData(deps, path, pathKey, {
        nodes: normalized.nodesAdded as unknown as GraphNode[],
        edges: normalized.edgesAdded as unknown as GraphEdge[],
      });
    }).then((graph) => {
      const cached = graphCache.get(pathKey);
      if (!cached || cached.signature !== signature) return graph;
      cached.graph = graph;
      cached.promise = undefined;
      touchGraphCacheEntry(cached);
      trimGraphCache();
      return graph;
    }).catch((error) => {
      graphCache.delete(pathKey);
      throw error;
    });
    graphCache.set(pathKey, entry);
    trimGraphCache();
    return entry.promise;
  }

  return {
    clear: () => {
      graphCache.clear();
      graphCacheSignature = '';
    },
    prepareGraph,
  };
}
