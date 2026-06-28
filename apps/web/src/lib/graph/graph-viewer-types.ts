export type ValueType = 'string' | 'number' | 'boolean' | 'null' | 'object' | 'array';

export type GraphBoxArgs = {
  x: number;
  y: number;
  width: number;
  height: number;
  cornerRadius: number;
};

export type GraphTextArgs = {
  x: number;
  y: number;
  width: number;
  height: number;
  text: string;
  textAlign: 'left' | 'center' | 'right';
  verticalAlign: 'top' | 'middle' | 'bottom';
  editable: boolean;
};

export type GraphCellKind = 'meta' | 'key' | 'value' | 'header';

export type GraphCell = {
  text: string;
  value: string;
  isMissing?: boolean;
  isTableCell?: boolean;
  isHeader?: boolean;
  isIndex?: boolean;
  isHeaderlessTable?: boolean;
  /** True when this cell belongs to a table body with vertical scroll. */
  isScrollableTable?: boolean;
  valueType: ValueType;
  path: any[];
  editable: boolean;
  boxArgs: GraphBoxArgs;
  textArgs: GraphTextArgs;
};

export type GraphRow = {
  boxArgs: GraphBoxArgs;
  cellBoxArgs: GraphBoxArgs;
  cells: GraphCell[];
};

export type GraphTable = {
  /** Canonical PathKey for the table node. */
  key?: string;
  columns: GraphCell[];
  rows: GraphRow[];
  headerHeight: number;
  totalHeight?: number;
  viewHeight?: number;
  rowHeight?: number;
};

export type GraphNodeKey = {
  kind: 'scalar' | 'object' | 'table';
  path: any[];
  pathKey: string;
  stableId: string;
};

export type GraphNode = {
  /** Local rendering/layout handle. Use key for business identity. */
  renderHandle: number;
  key?: GraphNodeKey;
  kind: 'scalar' | 'object' | 'table';
  depth: number;
  boxArgs: GraphBoxArgs;
  path: any[];
  meta: GraphCell;
  rows: GraphRow[];
  table?: GraphTable;
};

export type GraphEdge = {
  fromRenderHandle: number;
  from?: GraphNodeKey;
  fromRow: number;
  toRenderHandle: number;
  to?: GraphNodeKey;
  toRow: number;
  bezierArgs: {
    fromX: number;
    fromY: number;
    c1x: number;
    c1y: number;
    c2x: number;
    c2y: number;
    toX: number;
    toY: number;
  };
};

export type TableRowBinding = {
  cell: GraphCell;
  kind: GraphCellKind;
  box: any;
  text: any;
};

export type TableRowRenderEntry = {
  rowBox: any;
  cellContainer: any;
  cellBoxes: any[];
  borderBoxes: any[];
  textNodes: any[];
  rowIndex: number | null;
  bindings: TableRowBinding[];
};

export type TableRuntime = {
  nodeId: number;
  node: GraphNode;
  nodeBox: any;
  bodyViewport: any;
  bodyContent: any;
  bodyHeight: number;
  contentHeight: number;
  rowHeight: number;
  scrollY: number;
  visibleRange: { start: number; end: number };
  renderedRows: Map<number, TableRowRenderEntry>;
  renderedSlots: Map<number, TableRowRenderEntry>;
  rowPool: TableRowRenderEntry[];
  layoutSignature: string;
  dataSignature: string;
  updateWindow: (options?: { forceRebindVisibleRows?: boolean }) => void;
  destroy: () => void;
};

export type DrawContext = {
  nodeLayer: any;
  styleConfig: any;
  languageIdValue: string;
  fontSize: number;
  textEditInnerName?: string;
  BoxCtor: any;
  TextCtor: any;
  PenCtor: any;
  valueTypeToSemType: Record<ValueType, string>;
  editable?: boolean;
  registerCellBox: (cell: GraphCell, kind: GraphCellKind, box: any) => void;
  unregisterCellBox?: (cell: GraphCell, kind: GraphCellKind, box: any) => void;
  registerRowBox: (
    cell: GraphCell,
    rowBox: any,
    scrollOwner?: any,
    bodyHeight?: number,
    contentHeight?: number,
  ) => void;
  unregisterRowBox?: (cell: GraphCell, rowBox: any) => void;
  registerClickTarget: (
    target: any,
    cell: GraphCell,
    kind: GraphCellKind,
    nodeKind?: GraphNode['kind'],
  ) => string;
  requestRender?: () => void;
  bindVerticalScrollGesture?: (
    target: any,
    handler: (gesture: {
      event: unknown;
      deltaY: number;
      moveType?: string;
      stop: () => void;
      stopNow: () => void;
    }) => void,
  ) => (() => void) | void;
  bindPointerDown?: (target: any, handler: (event: unknown) => void | Promise<void>) => (() => void) | void;
  getPointFromEvent?: (
    hostApp: any,
    target: any,
    event: unknown,
    space: 'client' | 'box' | 'local' | 'world',
  ) => { x: number; y: number } | null;
  refreshActiveHighlight?: () => void;
};
