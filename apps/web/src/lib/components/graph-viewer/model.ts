import type { Box, Text } from 'leafer-ui';
import type { GraphCell, GraphCellKind, GraphNode } from '@treease/graph-viewer-runtime';
import type { PathSeg } from '../../store/tree-path';

export type LeaferInteractiveTarget = {
  __graphCell?: GraphCell;
  __graphCellKind?: GraphCellKind;
  __graphNodeKind?: GraphNode['kind'];
  __treeaseClickTargetId?: string;
  parent?: LeaferInteractiveTarget | null;
  text?: unknown;
};

export type LeaferBox = Box & LeaferInteractiveTarget;

export type LeaferText = Text & LeaferInteractiveTarget;

export type LeaferEditor = {
  getInnerEditor?: (name: string) => { config?: { selectAll?: boolean } } | undefined;
  innerEditor?: { config?: { selectAll?: boolean } };
  openInnerEditor?: (target?: LeaferBox | LeaferText, name?: string, selectTarget?: unknown) => void;
  closeInnerEditor?: (skipUpdate?: boolean) => void;
  on?: (event: string, callback: (event: unknown) => void) => void;
};

export type LeaferEditorHost = {
  editor?: LeaferEditor;
};

export type LeaferAppLike = {
  zoomLayer?: unknown;
  destroy?: () => void;
  resize?: (options: { width: number; height: number }) => void;
  updateClientBounds?: () => void;
  update?: () => void;
  on?: (event: string, callback: (event: unknown) => void) => void;
  getClientPointByWorld?: (point: { x: number; y: number }) => { x?: number; y?: number } | null;
};

export type ScrollableBox = LeaferBox & {
  scrollTo?: (options: { x: number; y: number } | number, y?: number) => void;
  scrollY?: number;
};

export type CellBoxEntry = {
  key?: LeaferBox;
  value?: LeaferBox;
  row?: LeaferBox;
  scrollOwner?: ScrollableBox;
  bodyHeight?: number;
  contentHeight?: number;
  cell?: GraphCell;
};

export type GraphViewerClickTarget = {
  id: string;
  box: LeaferBox;
  cell: GraphCell;
  target: 'key' | 'value' | 'node';
};

export type GraphViewerClickTargetStore = Record<string, GraphViewerClickTarget>;

export type ColumnNavigatorRuntime = {
  host: HTMLElement;
  mount: HTMLDivElement;
  view: HTMLDivElement;
  app: LeaferAppLike;
  edgeLayer: LeaferBox;
  nodeLayer: LeaferBox;
  pathKey: string;
  path: PathSeg[];
  clickTargetsById: GraphViewerClickTargetStore;
  clickTargetIdByTarget?: WeakMap<object, string>;
  clickBoundTargets?: WeakSet<object>;
  cellBoxByPathMap: Map<string, CellBoxEntry>;
  nodeBoxMap: Map<number, LeaferBox>;
  tableRuntimes: Array<{ destroy: () => void }>;
  editor: LeaferEditor | null;
  dispose?: () => void;
};

export type GraphSceneLayers = {
  edgeLayer: LeaferBox | null;
  nodeLayer: LeaferBox | null;
  overlayLayer: LeaferBox | null;
};
