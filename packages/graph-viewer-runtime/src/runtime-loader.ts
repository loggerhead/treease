export type LeaferView = {
  parentElement?: Element | null;
  addEventListener?: (type: string, listener: EventListenerOrEventListenerObject, options?: boolean) => void;
  removeEventListener?: (type: string, listener: EventListenerOrEventListenerObject, options?: boolean) => void;
};

export type LeaferNode = {
  zoomLayer?: LeaferNode;
  view?: LeaferView;
  x?: number;
  y?: number;
  renderHandle?: number;
  add: (child: unknown) => void;
  removeAll: (destroy?: boolean) => void;
  destroy?: () => void;
  resize?: (size: { width: number; height: number }) => void;
  update?: () => void;
  forceRender?: () => void;
  getWorldPointByClient?: (point: { x: number; y: number }) => { x: number; y: number } | null;
  getClientPointByWorld?: (point: { x: number; y: number }) => { x: number; y: number } | null;
  on?: (event: string, callback: (event?: unknown) => void) => void;
  selected?: boolean;
  selectedStyle?: unknown;
};

type LeaferConstructor = new (config: Record<string, unknown>) => LeaferNode;
type LeaferPen = {
  setStyle: (style: { stroke: string; strokeWidth: number }) => void;
  moveTo: (x: number, y: number) => void;
  bezierCurveTo: (c1x: number, c1y: number, c2x: number, c2y: number, toX: number, toY: number) => void;
  remove?: () => void;
  destroy?: () => void;
};
type LeaferEventNames = Record<string, string | undefined>;

export type LeaferRuntimeModules = {
  App?: LeaferConstructor;
  Leafer: LeaferConstructor;
  Box: LeaferConstructor;
  Text: LeaferConstructor;
  Pen: new () => LeaferPen;
  MoveEvent?: LeaferEventNames;
  ZoomEvent?: LeaferEventNames;
  DragEvent?: LeaferEventNames;
  LeaferEvent?: LeaferEventNames;
  PointerEvent?: LeaferEventNames;
};

export type GraphRuntimeLoaderOptions = {
  host: HTMLElement;
  preload?: () => Promise<void>;
  preferApp?: boolean;
  leaferOptions?: Record<string, unknown>;
  onResize?: (app: LeaferNode) => void;
};

export type LoadedGraphRuntime = {
  app: LeaferNode;
  modules: LeaferRuntimeModules;
  setResizeHandler: (handler: ((app: LeaferNode) => void) | undefined) => void;
  destroy: () => void;
};

/** The one Leafer load/resize/destroy owner for every graph surface. */
export async function loadGraphViewerRuntime(options: GraphRuntimeLoaderOptions): Promise<LoadedGraphRuntime> {
  await import('@leafer-in/viewport');
  await import('@leafer-in/color');
  await import('@leafer-in/animate');
  await import('@leafer-in/view');
  await options.preload?.();
  const modules = await import('leafer-ui') as unknown as LeaferRuntimeModules;
  const RuntimeCtor = options.preferApp && modules.App ? modules.App : modules.Leafer;
  const app = new RuntimeCtor({
    view: options.host,
    type: 'viewport',
    ...options.leaferOptions,
    move: { drag: false, holdSpaceKey: true, holdRightKey: true, scroll: true },
    zoom: { disabled: false },
    wheel: { zoomMode: false },
    multiTouch: { disabled: false },
  });
  let resizeHandler = options.onResize;
  const resize = () => {
    const bounds = options.host.getBoundingClientRect();
    if (bounds.width > 0 && bounds.height > 0) app.resize?.({ width: bounds.width, height: bounds.height });
    resizeHandler?.(app);
  };
  resize();
  const observer = new ResizeObserver(resize);
  observer.observe(options.host);
  return { app, modules, setResizeHandler: (handler) => { resizeHandler = handler; resize(); }, destroy: () => { observer.disconnect(); app.destroy?.(); } };
}
