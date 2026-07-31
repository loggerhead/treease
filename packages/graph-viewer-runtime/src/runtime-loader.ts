export type LeaferRuntimeModules = {
  App?: any; Leafer: any; Box: any; Text: any; Pen: any;
  MoveEvent?: any; ZoomEvent?: any; DragEvent?: any; LeaferEvent?: any; PointerEvent?: any;
};

export type GraphRuntimeLoaderOptions = {
  host: HTMLElement;
  preload?: () => Promise<void>;
  preferApp?: boolean;
  leaferOptions?: Record<string, unknown>;
  onResize?: (app: any) => void;
};

export type LoadedGraphRuntime = {
  app: any;
  modules: LeaferRuntimeModules;
  setResizeHandler: (handler: ((app: any) => void) | undefined) => void;
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
