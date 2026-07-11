<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { createViewRuntimeOperation, type ViewRuntimeOperation } from '../../guards/view-runtime-operation';
  import { getSharedWasmWorkerClient } from '../../wasm/wasm-worker-singleton';
  import type {
    App as LeaferApp,
    Box,
    Text,
    Pen,
    MoveEvent,
    ZoomEvent,
    DragEvent,
    LeaferEvent,
    PointerEvent as LeaferPointerEvent,
    Leafer,
  } from 'leafer-ui';

  type LeaferAppOrLeafer = LeaferApp | Leafer;

  export let container: HTMLDivElement | undefined;
  export let minimapHost: HTMLDivElement | undefined;
  export let graphRuntimeReady = false;
  export let errorMessage = '';
  export let leafer: LeaferAppOrLeafer | null = null;
  export let LeaferCtor: typeof LeaferApp | typeof Leafer | undefined;
  export let PlainLeaferCtor: typeof Leafer | undefined;
  export let BoxCtor: typeof Box | undefined;
  export let TextCtor: typeof Text | undefined;
  export let PenCtor: typeof Pen | undefined;
  export let MoveEventCtor: typeof MoveEvent | undefined;
  export let ZoomEventCtor: typeof ZoomEvent | undefined;
  export let DragEventCtor: typeof DragEvent | undefined;
  export let LeaferEventCtor: typeof LeaferEvent | undefined;
  export let PointerEventCtor: typeof LeaferPointerEvent | undefined;
  export let minimapRuntimeController: {
    attach: (input: {
      app: LeaferApp | Leafer;
      PlainLeaferCtor: typeof Leafer | undefined;
      minimapHost: HTMLDivElement | undefined;
      container: HTMLDivElement | undefined;
      constructors: { BoxCtor?: typeof Box; PenCtor?: typeof Pen; TextCtor?: typeof Text };
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
    }) => void;
    updateLayout: () => void;
    updateViewport: () => void;
    destroy: () => void;
  };
  export let registerViewportEvents: (target: unknown) => void;
  export let bindGraphEditorLifecycle: (editor: unknown) => void;
  export let updateSize: () => void;
  export let scheduleMeasure: () => void;
  export let minimapWidth = 220;
  export let minimapHeight = 150;

  let graphRuntimeToken = 0;
  let resizeObserver: ResizeObserver | null = null;
  let runtimeOperation: ViewRuntimeOperation | null = null;

  function cleanupRuntime() {
    resizeObserver?.disconnect();
    resizeObserver = null;
    minimapRuntimeController.destroy();
    leafer?.destroy();
    leafer = null;
    LeaferCtor = undefined;
    PlainLeaferCtor = undefined;
    BoxCtor = undefined;
    TextCtor = undefined;
    PenCtor = undefined;
    MoveEventCtor = undefined;
    ZoomEventCtor = undefined;
    DragEventCtor = undefined;
    LeaferEventCtor = undefined;
    PointerEventCtor = undefined;
  }

  onMount(() => {
    const runtimeToken = ++graphRuntimeToken;
    const operation = createViewRuntimeOperation({
      captured: { token: runtimeToken },
      getCurrent: () => ({ token: graphRuntimeToken }),
      onStale: cleanupRuntime,
    });
    runtimeOperation = operation;
    graphRuntimeReady = false;
    errorMessage = '';

    void operation.run({
      execute: async ({ step }) => {
        scheduleMeasure();
        void getSharedWasmWorkerClient().catch(() => {});
        const runtimeModules = await step(async () => {
          await import('@leafer-in/viewport');
          await import('@leafer-in/editor');
          await import('@leafer-in/state');
          await import('@leafer-in/text-editor');
          await import('@leafer-in/export');
          return import('leafer-ui');
        });
        const runtimeContainer = container;
        if (!runtimeContainer) return false;
        const mod = runtimeModules;
        await step(async () => {
          cleanupRuntime();
          LeaferCtor = (mod.App ?? mod.Leafer) as typeof LeaferApp | typeof Leafer;
          PlainLeaferCtor = mod.Leafer as typeof Leafer | undefined;
          BoxCtor = mod.Box;
          TextCtor = mod.Text;
          PenCtor = mod.Pen;
          MoveEventCtor = mod.MoveEvent;
          ZoomEventCtor = mod.ZoomEvent;
          DragEventCtor = mod.DragEvent;
          LeaferEventCtor = mod.LeaferEvent;
          PointerEventCtor = mod.PointerEvent;
          if (!LeaferCtor || !BoxCtor || !TextCtor || !PenCtor) return;

          leafer = new LeaferCtor({
            view: runtimeContainer,
            type: 'viewport',
            editor: {
              visible: true,
              hittable: true,
              hover: false,
              moveable: false,
              resizeable: false,
              rotateable: false,
              skewable: false,
              flipable: false,
            },
            move: { drag: false, holdSpaceKey: true, holdRightKey: true, scroll: true },
            zoom: { disabled: false },
            wheel: { zoomMode: false },
            multiTouch: { disabled: false },
          });
          registerViewportEvents(leafer);
          const editor = (leafer as { editor?: unknown } | null)?.editor ?? null;
          bindGraphEditorLifecycle(editor);
          minimapRuntimeController.attach({
            app: leafer,
            PlainLeaferCtor,
            minimapHost,
            container: runtimeContainer,
            constructors: { BoxCtor, PenCtor, TextCtor },
            width: minimapWidth,
            height: minimapHeight,
            events: {
              move: (MoveEventCtor?.BEFORE_MOVE ?? MoveEventCtor?.MOVE) as string | undefined,
              zoom: (ZoomEventCtor?.BEFORE_ZOOM ?? ZoomEventCtor?.ZOOM) as string | undefined,
              dragStart: DragEventCtor?.START as string | undefined,
              drag: DragEventCtor?.DRAG as string | undefined,
              dragEnd: DragEventCtor?.END as string | undefined,
              pointerDown: PointerEventCtor?.DOWN as string | undefined,
            },
          });

          updateSize();
          resizeObserver = new ResizeObserver(() => {
            updateSize();
            minimapRuntimeController.updateLayout();
            minimapRuntimeController.updateViewport();
          });
          resizeObserver.observe(runtimeContainer);
        });
        return true;
      },
      land: (ready) => {
        if (ready) graphRuntimeReady = true;
      },
      handleError: () => {
        errorMessage = 'Graph view failed to load. Please refresh and try again.';
      },
    });

    return () => {};
  });

  onDestroy(() => {
    graphRuntimeToken += 1;
    if (runtimeOperation) {
      void runtimeOperation.cancel();
      return;
    }
    cleanupRuntime();
  });
</script>
