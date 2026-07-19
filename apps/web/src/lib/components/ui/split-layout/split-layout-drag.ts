// Responsibility: shared split-divider drag action reusing pointer-drag events without another component wrapper.
export type SplitLayoutDragDetail = { clientX: number; clientY: number };

type SplitLayoutDragEvents = {
  onDragStart?: (detail: SplitLayoutDragDetail) => void;
  onDragMove?: (detail: SplitLayoutDragDetail) => void;
  onDragEnd?: (detail: SplitLayoutDragDetail) => void;
};

function toDragDetail(event: PointerEvent): SplitLayoutDragDetail {
  return { clientX: event.clientX, clientY: event.clientY };
}

function attach(node: HTMLElement, params: SplitLayoutDragEvents): () => void {
  let cleanupDragListeners: (() => void) | null = null;

  function handlePointerDown(event: PointerEvent) {
    event.preventDefault();
    node.setPointerCapture?.(event.pointerId);
    params.onDragStart?.(toDragDetail(event));

    cleanupDragListeners?.();

    const handlePointerMove = (moveEvent: PointerEvent): void => {
      params.onDragMove?.(toDragDetail(moveEvent));
    };

    const stopDragging = (endEvent: PointerEvent): void => {
      cleanupDragListeners?.();
      cleanupDragListeners = null;
      if (node.hasPointerCapture?.(event.pointerId)) {
        node.releasePointerCapture?.(event.pointerId);
      }
      params.onDragEnd?.(toDragDetail(endEvent));
    };

    cleanupDragListeners = () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', stopDragging);
      window.removeEventListener('pointercancel', stopDragging);
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', stopDragging);
    window.addEventListener('pointercancel', stopDragging);
  }

  node.addEventListener('pointerdown', handlePointerDown);

  return () => {
    cleanupDragListeners?.();
    cleanupDragListeners = null;
    node.removeEventListener('pointerdown', handlePointerDown);
  };
}

export function splitLayoutDrag(node: HTMLElement, params: SplitLayoutDragEvents) {
  let cleanup = attach(node, params);

  return {
    update(nextParams: SplitLayoutDragEvents) {
      cleanup();
      cleanup = attach(node, nextParams);
    },
    destroy() {
      cleanup();
    },
  };
}
