import type { SnapshotId, TreeNode } from '@core-wasm/index';

import type { TreeSyncSource, TreeSyncState } from '../../../store/editor-store-types';

export function createGraphTreeStateController(options: {
  getToken: () => number;
  setToken: (token: number) => void;
  setTreeState: (state: TreeSyncState) => void;
}) {
  return {
    nextToken(): number {
      const nextToken = options.getToken() + 1;
      options.setToken(nextToken);
      return nextToken;
    },
    publish(
      requestId: number,
      tree: TreeNode | null,
      value: unknown,
      source: TreeSyncSource,
      revision: number,
      snapshotId?: SnapshotId | null,
    ): boolean {
      return publishGraphTreeState(
        requestId,
        options.getToken(),
        options.setTreeState,
        tree,
        value,
        source,
        revision,
        snapshotId,
      );
    },
    clear(
      requestId: number,
      source: TreeSyncSource,
      revision: number,
      snapshotId?: SnapshotId | null,
    ): boolean {
      return clearGraphTreeState(
        requestId,
        options.getToken(),
        options.setTreeState,
        source,
        revision,
        snapshotId,
      );
    },
  };
}

export function publishGraphTreeState(
  requestId: number,
  currentToken: number,
  setTreeState: (state: TreeSyncState) => void,
  _tree: TreeNode | null,
  _value: unknown,
  source: TreeSyncSource,
  revision: number,
  _snapshotId?: SnapshotId | null,
): boolean {
  if (requestId !== currentToken) return false;
  setTreeState({ tree: null, value: null, source, revision });
  return true;
}

export function clearGraphTreeState(
  requestId: number,
  currentToken: number,
  setTreeState: (state: TreeSyncState) => void,
  source: TreeSyncSource,
  revision: number,
  _snapshotId?: SnapshotId | null,
): boolean {
  if (requestId !== currentToken) return false;
  setTreeState({ tree: null, value: null, source, revision });
  return true;
}
