import { LeaferMinimapPlugin, type MinimapViewData } from '../../leafer-minimap';
import type { App as LeaferApp, Box, Leafer, Pen, Text } from 'leafer-ui';

export function createGraphMinimapRuntimeController(options: {
  getViewData: () => MinimapViewData | null;
  onViewportChange: () => void;
}) {
  let plugin: LeaferMinimapPlugin | null = null;
  let mountApp: Leafer | null = null;

  return {
    attach(input: {
      app: LeaferApp | Leafer;
      PlainLeaferCtor: typeof Leafer | undefined;
      minimapHost: HTMLDivElement | undefined;
      container: HTMLDivElement | undefined;
      constructors: { BoxCtor: typeof Box; PenCtor: typeof Pen; TextCtor: typeof Text };
      events: {
        move?: string;
        zoom?: string;
        dragStart?: string;
        drag?: string;
        dragEnd?: string;
        pointerDown?: string;
      };
      width: number;
      height: number;
    }) {
      this.destroy();
      if (!input.PlainLeaferCtor || !input.minimapHost || !input.container) return;
      mountApp = new input.PlainLeaferCtor({
        view: input.minimapHost,
        width: input.width,
        height: input.height,
      } as any);
      plugin = new LeaferMinimapPlugin({
        app: input.app as any,
        mountApp: mountApp as any,
        mountContainer: input.minimapHost,
        container: input.container,
        constructors: input.constructors,
        width: input.width,
        height: input.height,
        events: input.events,
        getViewData: options.getViewData,
        onViewportChange: options.onViewportChange,
      });
    },
    update() {
      plugin?.update();
    },
    updateLayout() {
      plugin?.updateLayout();
    },
    updateViewport() {
      plugin?.updateViewport();
    },
    destroy() {
      plugin?.destroy?.();
      plugin = null;
      mountApp?.destroy?.();
      mountApp = null;
    },
  };
}
