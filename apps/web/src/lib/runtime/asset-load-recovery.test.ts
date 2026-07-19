import { describe, expect, it, vi } from 'vitest';
import { installAssetLoadRecovery } from './asset-load-recovery';

function browserWindow() {
  const listeners = new Map<string, EventListener>();
  const storage = new Map<string, string>();
  return {
    sessionStorage: {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => {
        storage.set(key, value);
      },
    } as unknown as Storage,
    location: { reload: vi.fn() } as unknown as Location,
    addEventListener: (type: string, listener: EventListener) => listeners.set(type, listener),
    removeEventListener: (type: string, listener: EventListener) => {
      if (listeners.get(type) === listener) listeners.delete(type);
    },
    dispatch(type: string, event: unknown) {
      listeners.get(type)?.(event as Event);
    },
  } as unknown as Window & { dispatch(type: string, event: unknown): void };
}

describe('asset load recovery', () => {
  it('reloads once for a stale dynamic import and never loops', () => {
    const browser = browserWindow();
    const uninstall = installAssetLoadRecovery(browser);

    browser.dispatch('unhandledrejection', { reason: new Error('Failed to fetch dynamically imported module') });
    browser.dispatch('unhandledrejection', { reason: new Error('Failed to fetch dynamically imported module') });

    expect(browser.location.reload).toHaveBeenCalledTimes(1);
    uninstall();
    browser.dispatch('unhandledrejection', { reason: new Error('Failed to fetch dynamically imported module') });
    expect(browser.location.reload).toHaveBeenCalledTimes(1);
  });

  it('ignores ordinary application errors', () => {
    const browser = browserWindow();
    installAssetLoadRecovery(browser);

    browser.dispatch('error', { error: new Error('editor failed to render') });

    expect(browser.location.reload).not.toHaveBeenCalled();
  });
});
