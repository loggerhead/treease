const recoveryStorageKey = 'treease:asset-load-recovery';

const chunkLoadErrorPattern = /(?:failed to fetch dynamically imported module|importing a module script failed|loading chunk .* failed|chunkloaderror|expected a javascript module script)/i;

function isChunkLoadError(value: unknown): boolean {
  if (value instanceof Error) return chunkLoadErrorPattern.test(value.message);
  if (typeof value === 'string') return chunkLoadErrorPattern.test(value);
  if (!value || typeof value !== 'object') return false;

  const message = 'message' in value ? value.message : undefined;
  return typeof message === 'string' && chunkLoadErrorPattern.test(message);
}

function shouldRecover(storage: Storage): boolean {
  try {
    if (storage.getItem(recoveryStorageKey)) return false;
    storage.setItem(recoveryStorageKey, '1');
    return true;
  } catch {
    return false;
  }
}

export function installAssetLoadRecovery(browserWindow: Window): () => void {
  const recover = (value: unknown) => {
    if (!isChunkLoadError(value) || !shouldRecover(browserWindow.sessionStorage)) return;
    browserWindow.location.reload();
  };
  const handleError = (event: ErrorEvent) => recover(event.error ?? event.message);
  const handleRejection = (event: PromiseRejectionEvent) => recover(event.reason);

  browserWindow.addEventListener('error', handleError);
  browserWindow.addEventListener('unhandledrejection', handleRejection);
  return () => {
    browserWindow.removeEventListener('error', handleError);
    browserWindow.removeEventListener('unhandledrejection', handleRejection);
  };
}
