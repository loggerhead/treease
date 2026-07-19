import type { LeaferAppLike, LeaferBox } from './model';

export type LeaferEventTarget = {
  on?: (event: string, callback: (event?: unknown) => void) => void;
};

type PointerEventCtorLike = {
  TAP?: string;
  CLICK?: string;
  DOWN?: string;
  MOVE?: string;
  UP?: string;
};

type MoveEventCtorLike = {
  BEFORE_MOVE?: string;
  MOVE?: string;
};

type CreateGraphPointerControllerOptions = {
  getPointerEventCtor: () => PointerEventCtorLike | undefined;
  getMoveEventCtor: () => MoveEventCtorLike | undefined;
  getActiveApp: () => LeaferAppLike | null;
};

export function createGraphPointerController(options: CreateGraphPointerControllerOptions) {
  function getClientPointFromEvent(event?: unknown): { x: number; y: number } | null {
    const point = event as {
      origin?: { x?: number; y?: number } | null;
      clientX?: number;
      clientY?: number;
      x?: number;
      y?: number;
    } | null;
    const origin = point?.origin;
    if (origin && Number.isFinite(origin.x) && Number.isFinite(origin.y)) {
      return { x: Number(origin.x), y: Number(origin.y) };
    }
    if (point && Number.isFinite(point.clientX) && Number.isFinite(point.clientY)) {
      return { x: Number(point.clientX), y: Number(point.clientY) };
    }
    if (point && Number.isFinite(point.x) && Number.isFinite(point.y)) {
      return { x: Number(point.x), y: Number(point.y) };
    }
    return null;
  }

  function getPointerEventName(type: 'click' | 'down' | 'move' | 'up'): string {
    const PointerEventCtor = options.getPointerEventCtor();
    if (!PointerEventCtor) return type;
    if (type === 'click') return PointerEventCtor.TAP ?? PointerEventCtor.CLICK ?? type;
    if (type === 'down') return PointerEventCtor.DOWN ?? type;
    if (type === 'move') return PointerEventCtor.MOVE ?? type;
    return PointerEventCtor.UP ?? type;
  }

  function getPointerClickEventNames(): string[] {
    const PointerEventCtor = options.getPointerEventCtor();
    const names = [PointerEventCtor?.TAP, PointerEventCtor?.CLICK, 'click'].filter(
      (name): name is string => typeof name === 'string' && name.length > 0,
    );
    return [...new Set(names)];
  }

  function bindPointerClick(target: LeaferEventTarget, handler: (event: unknown) => void | Promise<void>): void {
    if (!target?.on) return;
    let lastHandledAt = 0;
    for (const eventName of getPointerClickEventNames()) {
      target.on(eventName, (event: unknown) => {
        const now = Date.now();
        if (now - lastHandledAt < 16) return;
        lastHandledAt = now;
        const graphCell = (target as { __graphCell?: { path?: Array<{ key?: string }> } }).__graphCell;
        if (graphCell?.path?.[0]?.key === 'object') {
          console.debug('[DEBUG-graph-highlight-race]', 'pointer.accepted', { eventName, now });
        }
        void handler(event);
      });
    }
  }

  function bindPointerMove(target: LeaferEventTarget, handler: (event: unknown) => void | Promise<void>): void {
    if (!target?.on) return;
    const eventName = getPointerEventName('move');
    let lastHandledAt = 0;
    target.on(eventName, (event: unknown) => {
      const now = Date.now();
      if (now - lastHandledAt < 16) return;
      lastHandledAt = now;
      void handler(event);
    });
  }

  function bindPointerDown(target: LeaferEventTarget, handler: (event: unknown) => void | Promise<void>): () => void {
    if (!target?.on) return () => {};
    const eventName = getPointerEventName('down');
    target.on(eventName, (event: unknown) => {
      void handler(event);
    });
    return () => {};
  }

  function bindVerticalScrollGesture(
    target: LeaferEventTarget,
    handler: (gesture: {
      event: unknown;
      deltaY: number;
      moveType?: string;
      stop: () => void;
      stopNow: () => void;
    }) => void,
  ): () => void {
    if (!target?.on) return () => {};
    const MoveEventCtor = options.getMoveEventCtor();
    const eventName = (MoveEventCtor?.BEFORE_MOVE ?? MoveEventCtor?.MOVE ?? 'move') as string;
    target.on(eventName, (event: unknown) => {
      const moveEvent = event as {
        moveType?: string;
        y?: number;
        moveY?: number;
        getInnerMove?: (target: LeaferEventTarget) => { y?: number } | null;
        stop?: () => void;
        stopNow?: () => void;
      } | null;
      const innerMove = typeof moveEvent?.getInnerMove === 'function' ? moveEvent.getInnerMove(target) : null;
      const deltaY = Number(innerMove?.y ?? moveEvent?.moveY ?? moveEvent?.y ?? 0);
      if (!Number.isFinite(deltaY) || deltaY === 0) return;
      handler({
        event,
        deltaY,
        moveType: typeof moveEvent?.moveType === 'string' ? moveEvent.moveType : undefined,
        stop: () => moveEvent?.stop?.(),
        stopNow: () => moveEvent?.stopNow?.() ?? moveEvent?.stop?.(),
      });
    });
    return () => {};
  }

  function getPointFromEvent(
    hostApp: LeaferAppLike | null,
    target: LeaferBox,
    event: unknown,
    space: 'client' | 'box' | 'local' | 'world',
  ): { x: number; y: number } | null {
    const activeApp = hostApp ?? options.getActiveApp();
    const activeView = (activeApp as { view?: { parentElement?: Element | null } | null })?.view;
    if (!activeView || !activeView.parentElement) return null;
    const clientPoint = getClientPointFromEvent(event);
    if (!clientPoint) return null;
    if (space === 'client') return clientPoint;
    const appLike = activeApp as
      | (LeaferAppLike & {
          getWorldPointByClient?: (point: { x: number; y: number }) => { x?: number; y?: number } | null;
        })
      | null;
    const targetBox = target as LeaferBox & {
      getWorldPointByBox?: (point: { x: number; y: number }) => { x?: number; y?: number } | null;
    };
    const worldPoint =
      typeof appLike?.getWorldPointByClient === 'function' ? appLike.getWorldPointByClient(clientPoint) : null;
    if (space === 'world' && worldPoint && Number.isFinite(worldPoint.x) && Number.isFinite(worldPoint.y)) {
      return { x: Number(worldPoint.x), y: Number(worldPoint.y) };
    }
    if (typeof targetBox.getWorldPointByBox !== 'function' || typeof activeApp?.getClientPointByWorld !== 'function') {
      return null;
    }
    const worldOrigin = targetBox.getWorldPointByBox({ x: 0, y: 0 });
    const worldUnitX = targetBox.getWorldPointByBox({ x: 1, y: 0 });
    const worldUnitY = targetBox.getWorldPointByBox({ x: 0, y: 1 });
    if (!worldOrigin || !worldUnitX || !worldUnitY) return null;
    const clientOrigin = activeApp.getClientPointByWorld(worldOrigin);
    const clientUnitX = activeApp.getClientPointByWorld(worldUnitX);
    const clientUnitY = activeApp.getClientPointByWorld(worldUnitY);
    if (!clientOrigin || !clientUnitX || !clientUnitY) return null;
    const scaleX = Number(clientUnitX.x) - Number(clientOrigin.x);
    const scaleY = Number(clientUnitY.y) - Number(clientOrigin.y);
    if (!Number.isFinite(scaleX) || !Number.isFinite(scaleY) || scaleX === 0 || scaleY === 0) return null;
    return {
      x: (clientPoint.x - Number(clientOrigin.x)) / scaleX,
      y: (clientPoint.y - Number(clientOrigin.y)) / scaleY,
    };
  }

  return {
    getClientPointFromEvent,
    getPointerEventName,
    getPointerClickEventNames,
    bindPointerClick,
    bindPointerMove,
    bindPointerDown,
    bindVerticalScrollGesture,
    getPointFromEvent,
  };
}
