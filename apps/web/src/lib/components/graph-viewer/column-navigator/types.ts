import type { ValueType } from '@treease/graph-viewer-runtime';
import type { PathSeg } from '../../../store/tree-path';

export type ColumnNavigatorContentState = {
  tabId: string;
  tabName: string;
  sourceText: string;
  valueType: ValueType;
  /** Main-editor semantic tokens projected from this snapshot's exact source span. */
  semanticTokens: ArrayBuffer;
  /** Snapshot used to read this scalar; reuse it when planning its edit. */
  snapshotId: number | null;
};

export type ColumnNavigatorColumnItem = {
  path: PathSeg[];
  pathKey: string;
  label: string;
  preview: string;
  valueType: ValueType;
  semType: number | null;
  isContainer: boolean;
};

export type ColumnNavigatorPaneState = {
  requestId?: number;
  path: PathSeg[];
  pathKey: string;
  title: string;
  kind: 'column' | 'content';
  items: ColumnNavigatorColumnItem[];
  content: ColumnNavigatorContentState | null;
  status: 'loading' | 'ready' | 'empty' | 'error';
  error?: string;
};

export type VisibleColumnNavigatorPaneState = ColumnNavigatorPaneState & {
  visibleIndex: number;
  absoluteIndex: number;
};

export type ColumnNavigatorState = {
  open: boolean;
  /** A new path is being prepared while the last complete workspace stays visible. */
  isLoading: boolean;
  activePath: PathSeg[];
  chain: ColumnNavigatorPaneState[];
  visiblePanes: VisibleColumnNavigatorPaneState[];
  canGoBack: boolean;
  canGoForward: boolean;
  heightPx: number;
  isDraggingDivider: boolean;
};
