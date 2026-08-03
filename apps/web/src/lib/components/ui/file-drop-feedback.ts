// Responsibility: mark a drop target while a file payload is hovering it.
const activeClass = 'file-drop-feedback--active';

function carriesFiles(event: DragEvent): boolean {
  const transfer = event.dataTransfer;
  return (transfer?.files.length ?? 0) > 0 || Array.from(transfer?.types ?? []).includes('Files');
}

export function fileDropFeedback(node: HTMLElement) {
  let dragDepth = 0;

  const setActive = (active: boolean): void => {
    node.classList.toggle(activeClass, active);
  };

  const handleDragEnter = (event: DragEvent): void => {
    if (!carriesFiles(event)) return;
    dragDepth += 1;
    setActive(true);
  };

  const handleDragLeave = (event: DragEvent): void => {
    if (!carriesFiles(event)) return;
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) setActive(false);
  };

  const clear = (): void => {
    dragDepth = 0;
    setActive(false);
  };

  node.addEventListener('dragenter', handleDragEnter);
  node.addEventListener('dragleave', handleDragLeave);
  node.addEventListener('drop', clear);
  node.addEventListener('dragend', clear);

  return {
    destroy(): void {
      node.removeEventListener('dragenter', handleDragEnter);
      node.removeEventListener('dragleave', handleDragLeave);
      node.removeEventListener('drop', clear);
      node.removeEventListener('dragend', clear);
    },
  };
}
