import type {
  DocumentNodePreview,
  DocumentPathValue,
  QueryResult,
  SnapshotId,
  SnapshotReadResult,
  TreeNode,
} from '@core-wasm/index';
import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import type { PathSeg } from '../store/tree-path';
import { serializePath } from '../../shared/document-anchor-utils';

export type RootValueKind = 'string' | 'int' | 'float' | 'boolean' | 'null' | 'object' | 'array' | 'unknown';

export async function querySnapshotProjection(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
  queryKind: 'rootValueKind' | 'nodePreview' | 'pathValue' | 'fieldLabels';
  path?: PathSeg[];
}): Promise<QueryResult | null> {
  if (!options.documentKey || options.snapshotId == null) return null;
  const result = await callSharedWasmWorker<SnapshotReadResult<QueryResult>>('querySnapshot', {
    documentKey: options.documentKey,
    snapshotId: options.snapshotId,
    queryKind: options.queryKind,
    pathPattern: options.path ? serializePath(options.path) : undefined,
  });
  return result.status === 'ready' ? result.data : null;
}

export async function queryRootValueKind(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
}): Promise<RootValueKind | null> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'rootValueKind' });
  return (result?.rootValueKind as RootValueKind | undefined) ?? null;
}

export async function queryNodePreview(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
  path: PathSeg[];
}): Promise<DocumentNodePreview | null> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'nodePreview' });
  return result?.nodePreview ?? null;
}

export async function queryPathValue(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
  path: PathSeg[];
}): Promise<DocumentPathValue | null> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'pathValue' });
  return result?.pathValue ?? null;
}

export async function queryFieldLabels(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
}): Promise<string[]> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'fieldLabels' });
  return result?.fieldLabels ?? [];
}

export function nodePreviewToTreeNode(preview: DocumentNodePreview): TreeNode {
  return {
    kind: preview.kind,
    semType: preview.semType,
    tag: preview.tag,
    value: preview.value,
    children: [],
  } as TreeNode;
}
