export function appendGraphProgressDebug(
  event: Record<string, unknown>,
  context?: Record<string, unknown>,
): void {
  if (!(typeof window !== 'undefined' && (import.meta.env.DEV || import.meta.env.MODE === 'test'))) return;
  const runtimeWindow = window as Window & {
    __treeaseGraphProgressDebug?: Array<Record<string, unknown>>;
  };
  const entries = runtimeWindow.__treeaseGraphProgressDebug ?? [];
  const now =
    typeof performance !== 'undefined' && typeof performance.now === 'function' ? performance.now() : Date.now();
  entries.push({
    ts: now,
    ...context,
    ...event,
  });
  if (entries.length > 400) entries.splice(0, entries.length - 400);
  runtimeWindow.__treeaseGraphProgressDebug = entries;
}
