import { parseToTree, type PathSeg, type SnapshotId } from '@core-wasm/index';
import fuzzysort from 'fuzzysort';
import { PathSegTag, SemType, TreeKind, type TreeNode } from '@core-wasm/index'
import { postOk } from './logging';
import { type WorkerContext, type WorkerRequest } from './protocol';
import type { GraphState } from './graph-state-service';
import type { GraphSearchItem, GraphSearchResult, SearchIndexEntry } from './graph-search-types';
import { buildPathKey, buildPathText, createPathResolver, resolveLazyPath, resolveSearchRevealTarget, toPathSeg } from './tree-path';

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

export type { GraphSearchItem, GraphSearchResult, SearchIndexEntry } from './graph-search-types';

type GraphSearchAnalysisRuntime = {
  searchIndexByDocumentKey: Map<string, SearchIndexEntry>;
};

export function collectSearchItems(node: TreeNode, path: PathSeg[], items: GraphSearchItem[]): void {
  if (!node) return;
  const children = Array.isArray(node.children) ? node.children : [];
  const isMapping = node.kind === TreeKind.MAPPING || node.semType === SemType.MAP;
  const isSequence = node.kind === TreeKind.SEQUENCE || node.semType === SemType.SEQ;
  if (isMapping) {
    for (let i = 0; i < children.length; i += 2) {
      const keyNode = children[i];
      const valueNode = children[i + 1];
      const keyText = String(keyNode?.value ?? '').trim();
      if (keyText) {
        const nextPath = [...path, toPathSeg(PathSegTag.KEY, keyText, 0)];
        items.push({
          path: nextPath,
          pathKey: buildPathKey(nextPath),
          pathText: buildPathText(nextPath),
          label: keyText,
          keyText,
          valueText: String(valueNode?.value ?? ''),
          target: 'key',
        });
        if (valueNode) collectSearchItems(valueNode, nextPath, items);
      } else if (valueNode) {
        collectSearchItems(valueNode, path, items);
      }
    }
    return;
  }
  if (isSequence) {
    children.forEach((child, index) => {
      const nextPath = [...path, toPathSeg(PathSegTag.INDEX, '', index)];
      const valueText = String(child?.value ?? '').trim();
      if (valueText) {
        items.push({
          path: nextPath,
          pathKey: buildPathKey(nextPath),
          pathText: buildPathText(nextPath),
          label: valueText,
          keyText: String(index),
          valueText,
          target: 'value',
        });
      }
      if (child) collectSearchItems(child, nextPath, items);
    });
    return;
  }
  const valueText = String(node.value ?? '').trim();
  if (!valueText) return;
  items.push({
    path,
    pathKey: buildPathKey(path),
    pathText: buildPathText(path),
    label: valueText,
    keyText: '',
    valueText,
    target: 'value',
  });
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
    const label = String(cell.text ?? cell.value ?? '').trim();
    const valueText = String(cell.value ?? cell.text ?? '').trim();
    if (cell.isIndex || options?.skipIndex) return;
    if (cell.valueType === 'array' || cell.valueType === 'object') return;
    if (label === '[]' || label === '{}' || valueText === '[]' || valueText === '{}') return;
    if (!label && !valueText) return;
    const path = Array.isArray(cell.path) ? cell.path : [];
    items.push({
      path,
      pathKey: buildPathKey(path),
      pathText: buildPathText(path),
      label: label || valueText,
      keyText: '',
      valueText: valueText || label,
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
  language: string,
  text: string,
  nest: boolean,
): Promise<GraphSearchItem[]> {
  const { searchIndexByDocumentKey } = runtime;
  const cached = searchIndexByDocumentKey.get(documentKey);
  if (cached && cached.text === text) {
    const items = [...cached.items];
    collectViewTableItems(graphStateByDocumentKey, documentKey, items);
    return items;
  }
  const root = await parseToTree(language, text, { nest }) as unknown as TreeNode | null;
  if (!root) {
    searchIndexByDocumentKey.set(documentKey, { text, items: [], pathMap: undefined });
    return [];
  }
  const items: GraphSearchItem[] = [];
  collectSearchItems(root, [], items);
  collectViewTableItems(graphStateByDocumentKey, documentKey, items);
  searchIndexByDocumentKey.set(documentKey, { text, items, pathMap: undefined });
  return items;
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
  if (cached?.text === text && cached.pathMap) {
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
  if (cached && cached.text === text) {
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
    postOk(ctx, message.id, []);
    return;
  }
  const nest = message.nest;
  const items = await getSearchItems(runtime, graphStateByDocumentKey, message.documentKey, message.language, message.text, nest);
  const snapshotId = message.snapshotId ?? null;
  const pathMap = await buildGraphPathMap(runtime, graphStateByDocumentKey, message.documentKey, message.language, message.text, snapshotId);
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
          path = await resolveLazyPath(message.documentKey, message.language, message.text, item.lazy ?? {}, snapshotId);
        }
        const pathKey = buildPathKey(path);
        if (!pathKey) return null;
        const nodeId = pathMap.get(pathKey);
        const resolvedTarget = await resolveSearchRevealTarget(
          message.documentKey,
          message.language,
          message.text,
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
  postOk(ctx, message.id, [...uniqueResults.values()]);
}
