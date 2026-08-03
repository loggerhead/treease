export type MinimapBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type MinimapNode = MinimapBounds & {
  id: number | string;
  kind?: string;
  label?: string;
};

export type MinimapEdge = {
  fromX: number;
  fromY: number;
  c1x: number;
  c1y: number;
  c2x: number;
  c2y: number;
  toX: number;
  toY: number;
};

export type MinimapViewData = {
  nodes: MinimapNode[];
  edges: MinimapEdge[];
};

export type MinimapConstructors = {
  BoxCtor: new (props?: Record<string, unknown>) => any;
  PenCtor: new () => any;
  TextCtor?: new (props?: Record<string, unknown>) => any;
};

export type MinimapEventNames = {
  move?: string;
  zoom?: string;
  dragStart?: string;
  drag?: string;
  dragEnd?: string;
  pointerDown?: string;
};

export type MinimapColors = {
  background?: string;
  node?: string;
  tableNode?: string;
  scalarNode?: string;
  edge?: string;
  viewportFill?: string;
  viewportStroke?: string;
};

export type MinimapBoxLike = {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  fill?: unknown;
  stroke?: unknown;
  strokeWidth?: number;
  strokeAlign?: string;
  scaleX?: number;
  scaleY?: number;
  cornerRadius?: number;
  opacity?: number;
  visible?: boolean;
  hittable?: boolean;
  hitChildren?: boolean;
  draggable?: boolean;
  cursor?: string;
  children?: unknown[];
  add?: (child: unknown) => void;
  removeAll?: (destroy?: boolean) => void;
  remove?: () => void;
  destroy?: () => void;
  on?: (event: string, handler: (event?: unknown) => void) => unknown;
  off?: (event: string, handler: (event?: unknown) => void) => void;
};

export type MinimapPenLike = {
  setStyle?: (style: Record<string, unknown>) => void;
  moveTo?: (x: number, y: number) => void;
  bezierCurveTo?: (c1x: number, c1y: number, c2x: number, c2y: number, toX: number, toY: number) => void;
  remove?: () => void;
};

export type MinimapZoomLayerLike = {
  x?: number;
  y?: number;
  scaleX?: number;
  scaleY?: number;
  scale?: number | { x?: number; y?: number };
  __?: {
    scaleX?: number;
    scaleY?: number;
    scale?: number | { x?: number; y?: number };
  };
};

export type MinimapAppLike = {
  width?: number;
  height?: number;
  zoomLayer?: MinimapZoomLayerLike;
  sky?: { add?: (child: unknown) => void };
  add?: (child: unknown) => void;
  resize?: (options: { width: number; height: number }) => void;
  on?: (event: string, handler: (event?: unknown) => void) => unknown;
  off?: (event: string, handler: (event?: unknown) => void) => void;
  on_?: (event: string, handler: (event?: unknown) => void, bind?: unknown) => unknown;
  off_?: (ids: unknown[]) => void;
  update?: () => void;
};

export type LeaferMinimapPluginOptions = {
  app: MinimapAppLike;
  mountApp?: MinimapAppLike;
  mountContainer?: HTMLElement;
  container: HTMLElement;
  constructors: MinimapConstructors;
  events?: MinimapEventNames;
  getViewData: () => MinimapViewData | null;
  width?: number;
  height?: number;
  padding?: number;
  contentPadding?: number;
  colors?: MinimapColors;
  requestViewport?: (view: MinimapBounds) => void;
};

export type LeaferMinimapPluginHandle = {
  update: () => void;
  updateLayout: () => void;
  updateViewport: () => void;
  destroy: () => void;
};
