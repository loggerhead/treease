import type { GraphCell, ValueType } from '@treease/graph-viewer-runtime';
import type { PathSeg } from '../../../store/tree-path';
import type { SubgraphWorkspaceGraphData } from '../graph-subgraph-workspace-types';

export type SubgraphWorkspaceContentState = {
  tabId: string;
  tabName: string;
  sourceText: string;
  valueType: ValueType;
  /** Exact Core SemType for this scalar content pane; never a Web value-type guess. */
  rootSemType: number | null;
  /** Snapshot used to read this scalar; reuse it when planning its edit. */
  snapshotId: number | null;
};

export type SubgraphWorkspacePaneState = {
  requestId?: number;
  path: PathSeg[];
  pathKey: string;
  title: string;
  kind: 'graph' | 'content';
  graph: SubgraphWorkspaceGraphData | null;
  content: SubgraphWorkspaceContentState | null;
  status: 'loading' | 'ready' | 'empty' | 'error';
  error?: string;
};

export type VisibleSubgraphWorkspacePaneState = SubgraphWorkspacePaneState & {
  visibleIndex: number;
  absoluteIndex: number;
};

export type SubgraphWorkspaceActivatePayload = {
  path: PathSeg[];
  target: 'key' | 'value' | 'node';
  cell: GraphCell;
};

export type SubgraphWorkspaceState = {
  chain: SubgraphWorkspacePaneState[];
  visiblePanes: VisibleSubgraphWorkspacePaneState[];
  heightPx: number;
  isDraggingDivider: boolean;
};
