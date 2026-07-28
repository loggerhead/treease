// Compile-time boundary for the Web GraphViewer source imported by Vite. Runtime
// resolution remains the explicit @treease-web aliases in vite.config.ts; keeping
// this declaration local prevents the extension's standalone typecheck from
// typechecking every unrelated Svelte-Web module.
export type GraphCell = {
  text: string;
  value: string;
  valueType: 'string' | 'number' | 'boolean' | 'null' | 'object' | 'array';
  path: Array<{ tag: number; key: unknown; index: number }>;
  editable: boolean;
  boxArgs: { x: number; y: number; width: number; height: number; cornerRadius: number };
  textArgs: { x: number; y: number; width: number; height: number; text: string; textAlign: 'left' | 'center' | 'right'; verticalAlign: 'top' | 'middle' | 'bottom'; editable: boolean };
};
export type GraphCellKind = 'meta' | 'key' | 'value' | 'header';
export type GraphNode = {
  renderHandle: number;
  kind: 'scalar' | 'object' | 'table';
  depth: number;
  path: GraphCell['path'];
  boxArgs: GraphCell['boxArgs'];
  meta: GraphCell;
  rows: Array<{ boxArgs: GraphCell['boxArgs']; cellBoxArgs: GraphCell['boxArgs']; cells: GraphCell[] }>;
  table?: { columns: GraphCell[]; rows: Array<{ boxArgs: GraphCell['boxArgs']; cellBoxArgs: GraphCell['boxArgs']; cells: GraphCell[] }>; headerHeight: number };
};
export type GraphEdge = {
  fromRenderHandle: number;
  fromRow: number;
  toRenderHandle: number;
  toRow: number;
  bezierArgs: { fromX: number; fromY: number; c1x: number; c1y: number; c2x: number; c2y: number; toX: number; toY: number };
};
export declare const graphViewerConfig: any;
export declare function renderGraphEdges(input: any): GraphEdge[];
export declare function renderGraphNodes(input: any): void;
export declare function renderGraphNode(input: any): { nodeBox: any; tableRuntime?: { destroy?: () => void } };
export declare function createGraphPointerController(input: any): {
  bindPointerClick(target: any, handler: (event: unknown) => void | Promise<void>): void;
  bindPointerDown(target: any, handler: (event: unknown) => void | Promise<void>): () => void;
  bindVerticalScrollGesture(target: any, handler: (gesture: { event: unknown; deltaY: number; moveType?: string; stop: () => void; stopNow: () => void }) => void): () => void;
  getPointFromEvent(hostApp: any, target: any, event: unknown, space: 'client' | 'box' | 'local' | 'world'): { x: number; y: number } | null;
};
