function canReadRuntimeStartupDelay(): boolean {
  return typeof window !== 'undefined' && (import.meta.env.DEV || import.meta.env.MODE === 'test');
}

function readEditorRuntimeStartupDelay(): number {
  if (!canReadRuntimeStartupDelay()) return 0;
  const value = window.__treeaseEditorRuntimeStartupDelayMs;
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0;
}

export async function awaitEditorRuntimeStartupDelay(): Promise<void> {
  const delayMs = readEditorRuntimeStartupDelay();
  if (delayMs <= 0) return;
  await new Promise<void>((resolve) => {
    window.setTimeout(resolve, delayMs);
  });
}

declare global {
  interface Window {
    __treeaseEditorRuntimeStartupDelayMs?: number;
  }
}
