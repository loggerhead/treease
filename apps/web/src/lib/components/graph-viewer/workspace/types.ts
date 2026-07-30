import type { ValueType } from '@treease/graph-viewer-runtime';
import type { PathSeg } from '../../../store/tree-path';

export type SubgraphWorkspaceContentState = {
  tabId: string;
  tabName: string;
  sourceText: string;
  valueType: ValueType;
  /** Main-editor semantic tokens projected from this snapshot's exact source span. */
  semanticTokens: ArrayBuffer;
  /** Snapshot used to read this scalar; reuse it when planning its edit. */
  snapshotId: number | null;
};

export type SubgraphWorkspaceColumnItem = {
  path: PathSeg[];
  pathKey: string;
  label: string;
  preview: string;
  valueType: ValueType;
  semType: number | null;
  isContainer: boolean;
};

export type SubgraphWorkspacePaneState = {
  requestId?: number;
  path: PathSeg[];
  pathKey: string;
  title: string;
  kind: 'column' | 'content';
  items: SubgraphWorkspaceColumnItem[];
  content: SubgraphWorkspaceContentState | null;
  status: 'loading' | 'ready' | 'empty' | 'error';
  error?: string;
};

export type VisibleSubgraphWorkspacePaneState = SubgraphWorkspacePaneState & {
  visibleIndex: number;
  absoluteIndex: number;
};

export type SubgraphWorkspaceState = {
  open: boolean;
  activePath: PathSeg[];
  chain: SubgraphWorkspacePaneState[];
  visiblePanes: VisibleSubgraphWorkspacePaneState[];
  canGoBack: boolean;
  canGoForward: boolean;
  heightPx: number;
  isDraggingDivider: boolean;
};
