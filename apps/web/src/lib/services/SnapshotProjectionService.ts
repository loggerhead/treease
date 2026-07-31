import type {
  DocumentNodePreview,
  DocumentPathValue,
  DocumentDirectChild,
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
  queryKind: 'rootValueKind' | 'nodePreview' | 'pathValue' | 'directChildren' | 'fieldLabels';
  path?: PathSeg[];
}): Promise<SnapshotReadResult<QueryResult>> {
  if (!options.documentKey || options.snapshotId == null) return { status: 'snapshotNotReady' };
  return callSharedWasmWorker<SnapshotReadResult<QueryResult>>('querySnapshot', {
    documentKey: options.documentKey,
    snapshotId: options.snapshotId,
    queryKind: options.queryKind,
    pathPattern: options.path ? serializePath(options.path) : undefined,
  });
}

export async function queryRootValueKind(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
}): Promise<SnapshotReadResult<RootValueKind | null>> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'rootValueKind' });
  if (result.status !== 'ready') return result;
  return { status: 'ready', data: (result.data.rootValueKind as RootValueKind | undefined) ?? null };
}

export async function queryNodePreview(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
  path: PathSeg[];
}): Promise<SnapshotReadResult<DocumentNodePreview | null>> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'nodePreview' });
  if (result.status !== 'ready') return result;
  return { status: 'ready', data: result.data.nodePreview ?? null };
}

export async function queryPathValue(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
  path: PathSeg[];
}): Promise<SnapshotReadResult<DocumentPathValue | null>> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'pathValue' });
  if (result.status !== 'ready') return result;
  return { status: 'ready', data: result.data.pathValue ?? null };
}

export async function queryDirectChildren(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
  path: PathSeg[];
}): Promise<SnapshotReadResult<DocumentDirectChild[]>> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'directChildren' });
  if (result.status !== 'ready') return result;
  return { status: 'ready', data: result.data.directChildren ?? [] };
}

export async function queryFieldLabels(options: {
  documentKey: string;
  snapshotId: SnapshotId | null;
}): Promise<SnapshotReadResult<string[]>> {
  const result = await querySnapshotProjection({ ...options, queryKind: 'fieldLabels' });
  if (result.status !== 'ready') return result;
  return { status: 'ready', data: result.data.fieldLabels };
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
