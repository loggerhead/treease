import { querySnapshot, type PathSeg, type QueryResult, type SnapshotId, type SnapshotReadResult } from '@core-wasm/index';
import fuzzysort from 'fuzzysort';
import { postOk } from './logging';
import { type WorkerContext, type WorkerRequest } from './protocol';
import type { GraphState } from './graph-state-service';
import type { GraphSearchItem, GraphSearchReadResult, GraphSearchResult, SearchIndexEntry } from './graph-search-types';
import { buildPathKey, buildPathText, createPathResolver, parseAnchorPath, resolveLazyPath, resolveSearchRevealTarget } from './tree-path';

type TableCell = {
  text?: string;
  value?: string;
  valueType?: string;
  isIndex?: boolean;
  path?: PathSeg[];
};

type TableRow = {
  cells?: TableCell[];
};

type GraphTable = {
  columns?: TableCell[];
  rows?: TableRow[];
};

export type { GraphSearchItem, GraphSearchReadResult, GraphSearchResult, SearchIndexEntry } from './graph-search-types';

type GraphSearchAnalysisRuntime = {
  searchIndexByDocumentKey: Map<string, SearchIndexEntry>;
};

function getSearchableCellText(cell: TableCell): { label: string; valueText: string } {
  const primaryText = String(cell.text ?? cell.value ?? '').trim();
  const fallbackText = String(cell.value ?? cell.text ?? '').trim();
  return {
    label: primaryText || fallbackText,
    valueText: fallbackText || primaryText,
  };
}

export function collectViewTableItems(
  graphStateByDocumentKey: Map<string, GraphState>,
  documentKey: string,
  items: GraphSearchItem[],
): void {
  const state = graphStateByDocumentKey.get(documentKey);
  if (!state) return;
  function pushCell(cell: TableCell, options?: { skipIndex?: boolean }): void {
    if (!cell) return;
    const { label, valueText } = getSearchableCellText(cell);
    if (cell.isIndex || options?.skipIndex) return;
    if (cell.valueType === 'array' || cell.valueType === 'object') return;
    if (label === '[]' || label === '{}' || valueText === '[]' || valueText === '{}') return;
    if (!label && !valueText) return;
    const path = Array.isArray(cell.path) ? cell.path : [];
    items.push({
      path,
      pathKey: buildPathKey(path),
      pathText: buildPathText(path),
      label,
      keyText: '',
      valueText,
      target: 'value',
      lazy: cell,
    });
  }
  state.nodes.forEach((node) => {
    const table = node.table as GraphTable | undefined;
    const nodeMeta = (node.meta ?? {}) as { valueType?: string };
    const isArrayTable = nodeMeta.valueType === 'array';
    table?.columns?.forEach((cell) => pushCell(cell));
    table?.rows?.forEach((row) => {
      row?.cells?.forEach((cell, index) => pushCell(cell, { skipIndex: isArrayTable && index === 0 }));
    });
  });
}

export async function getSearchItems(
  runtime: GraphSearchAnalysisRuntime,
  graphStateByDocumentKey: Map<string, GraphState>,
  documentKey: string,
  snapshotId: SnapshotId | null,
): Promise<SnapshotReadResult<GraphSearchItem[]>> {
  if (snapshotId == null) return { status: 'snapshotNotReady' };
  const { searchIndexByDocumentKey } = runtime;
  const cached = searchIndexByDocumentKey.get(documentKey);
  if (cached && cached.snapshotId === snapshotId) {
    const items = [...cached.items];
    collectViewTableItems(graphStateByDocumentKey, documentKey, items);
    return { status: 'ready', data: items };
  }
  const result = await querySnapshot({
    documentKey,
    snapshotId,
    queryKind: 'searchIndex',
  });
  if (result.status !== 'ready') {
    return { status: 'snapshotNotReady' };
  }
  const queryResult = result.data as QueryResult & {
    searchItems?: Array<{
      path: string;
      pathText: string;
      label: string;
      keyText: string;
      valueText: string;
      target: 'key' | 'value' | 'node';
    }>;
  };
  const items: GraphSearchItem[] = (queryResult.searchItems ?? []).map((item) => {
    const path = parseAnchorPath(item.path);
    return {
      path,
      pathKey: buildPathKey(path),
      pathText: item.pathText || buildPathText(path),
      label: item.label,
      keyText: item.keyText,
      valueText: item.valueText,
      target: item.target,
    };
  });
  collectViewTableItems(graphStateByDocumentKey, documentKey, items);
  searchIndexByDocumentKey.set(documentKey, { snapshotId, items, pathMap: undefined });
  return { status: 'ready', data: items };
}

export async function buildGraphPathMap(
  runtime: GraphSearchAnalysisRuntime,
  graphStateByDocumentKey: Map<string, GraphState>,
  documentKey: string,
  language: string,
  text: string,
  snapshotId: SnapshotId | null,
): Promise<Map<string, number>> {
  const { searchIndexByDocumentKey } = runtime;
  const cached = searchIndexByDocumentKey.get(documentKey);
  if (snapshotId != null && cached?.snapshotId === snapshotId && cached.pathMap) {
    return cached.pathMap;
  }
  const state = graphStateByDocumentKey.get(documentKey);
  const map = new Map<string, number>();
  if (!state) return map;
  const memo = new Map<string, PathSeg[]>();
  const resolve = createPathResolver({
    documentKey,
    language,
    text,
    snapshotId,
    memo,
    updateTarget: true,
  });
  function registerPath(path: PathSeg[] | undefined, nodeId: number): void {
    if (!path) return;
    const pathKey = buildPathKey(path);
    if (!pathKey) return;
    if (!map.has(pathKey)) map.set(pathKey, nodeId);
  }
  async function registerCell(cell: TableCell | undefined, nodeId: number): Promise<void> {
    if (!cell) return;
    let path = Array.isArray(cell.path) ? cell.path : [];
    if (path.length === 0) {
      path = await resolve(cell);
      cell.path = path;
    }
    registerPath(path, nodeId);
  }
  for (const node of state.nodes.values()) {
    let path = Array.isArray(node.path) ? node.path : [];
    if (path.length === 0) {
      path = await resolve(node);
      node.path = path;
    }
    const nodeRenderHandle = node.renderHandle;
    registerPath(path, nodeRenderHandle);
    await registerCell(node.meta as TableCell | undefined, nodeRenderHandle);
    const nodeRows = node.rows as { cells?: TableCell[] }[] | undefined;
    for (const row of nodeRows ?? []) {
      for (const cell of row?.cells ?? []) {
        await registerCell(cell, nodeRenderHandle);
      }
    }
    const table = node.table as GraphTable | undefined;
    for (const cell of table?.columns ?? []) {
      await registerCell(cell, nodeRenderHandle);
    }
    for (const row of table?.rows ?? []) {
      for (const cell of row?.cells ?? []) {
        await registerCell(cell, nodeRenderHandle);
      }
    }
  }
  if (snapshotId != null && cached && cached.snapshotId === snapshotId) {
    cached.pathMap = map;
  }
  return map;
}

export async function handleGraphSearch(
  ctx: WorkerContext,
  runtime: GraphSearchAnalysisRuntime,
  graphStateByDocumentKey: Map<string, GraphState>,
  message: Extract<WorkerRequest, { type: 'graphSearch' }>,
): Promise<void> {
  const query = message.query?.trim();
  if (!query) {
    postOk(ctx, message.id, { status: 'ready', data: [] } satisfies GraphSearchReadResult);
    return;
  }
  const snapshotId = message.snapshotId ?? null;
  const nest = message.nest;
  const itemsResult = await getSearchItems(runtime, graphStateByDocumentKey, message.documentKey, snapshotId);
  if (itemsResult.status !== 'ready') {
    postOk(ctx, message.id, { status: 'snapshotNotReady' } satisfies GraphSearchReadResult);
    return;
  }
  const items = itemsResult.data;
  const pathMap = await buildGraphPathMap(runtime, graphStateByDocumentKey, message.documentKey, message.language, '', snapshotId);
  const resolved = await Promise.all(
    fuzzysort
      .go(query, items as any, {
        limit: 200,
        keys: ['label', 'pathText', 'keyText', 'valueText'],
      } as any)
      .map((result: any) => result.obj as GraphSearchItem)
      .map(async (item) => {
        let path = item.path;
        if (!path || path.length === 0) {
          path = await resolveLazyPath(message.documentKey, message.language, '', item.lazy ?? {}, snapshotId);
        }
        const pathKey = buildPathKey(path);
        if (!pathKey) return null;
        const nodeId = pathMap.get(pathKey);
        const resolvedTarget = await resolveSearchRevealTarget(
          message.documentKey,
          message.language,
          '',
          path,
          item.target,
          nest,
          snapshotId,
        );
        if (!resolvedTarget) return null;
        const pathText = buildPathText(path);
        const data: GraphSearchResult = {
          nodeId,
          target: resolvedTarget,
          label: item.label,
          path,
          pathText,
        };
        return { data, dedupeKey: `${resolvedTarget}:${pathKey}` };
      }),
  );
  const uniqueResults = new Map<string, GraphSearchResult>();
  for (const entry of resolved) {
    if (!entry || uniqueResults.has(entry.dedupeKey)) continue;
    uniqueResults.set(entry.dedupeKey, entry.data);
    if (uniqueResults.size >= 20) break;
  }
  postOk(ctx, message.id, { status: 'ready', data: [...uniqueResults.values()] } satisfies GraphSearchReadResult);
}
