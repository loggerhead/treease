import {
  createFreshnessScope,
  type FreshnessContext,
  type FreshnessScope,
} from './freshness-scope';

export type ViewRuntimeOperationStaleReason = 'contextChanged' | 'cancelled';

export type ViewRuntimeOperationResult<T> =
  | { status: 'completed'; value: T }
  | { status: 'stale'; reason: ViewRuntimeOperationStaleReason; cleanupError?: unknown }
  | { status: 'failed'; error: unknown };

export type ViewRuntimeOperation = {
  isCurrent: () => boolean;
  step: <T>(task: () => Promise<T>) => Promise<T>;
  run: <T>(options: {
    execute: (operation: ViewRuntimeOperation) => Promise<T>;
    land?: (value: T) => Promise<void> | void;
    handleError?: (error: unknown) => Promise<void> | void;
  }) => Promise<ViewRuntimeOperationResult<T>>;
  cancel: () => Promise<void>;
};

export type CreateViewRuntimeOperationOptions = {
  captured: FreshnessContext;
  getCurrent: () => FreshnessContext;
  onStale?: (reason: ViewRuntimeOperationStaleReason) => Promise<void> | void;
};

class StaleViewRuntimeOperationError extends Error {
  constructor() {
    super('View Runtime operation is stale');
  }
}

function isStaleOperationError(error: unknown): error is StaleViewRuntimeOperationError {
  return error instanceof StaleViewRuntimeOperationError;
}

/**
 * Owns the Web-visible lifecycle of one asynchronous View Runtime operation.
 *
 * It deliberately wraps FreshnessScope instead of replacing it: Core remains
 * authoritative for Document Runtime freshness and snapshots; this Module only
 * decides whether a UI result may still land and releases local resources once.
 */
export function createViewRuntimeOperation(options: CreateViewRuntimeOperationOptions): ViewRuntimeOperation {
  return createViewRuntimeOperationFromFreshnessScope(
    createFreshnessScope(options.captured, options.getCurrent),
    { onStale: options.onStale },
  );
}

/** Bridges an existing FreshnessScope into the operation lifecycle without
 * creating another freshness authority or token manager. */
export function createViewRuntimeOperationFromFreshnessScope(
  freshness: FreshnessScope,
  options: Pick<CreateViewRuntimeOperationOptions, 'onStale'> = {},
): ViewRuntimeOperation {
  let staleReason: ViewRuntimeOperationStaleReason | null = null;
  let cleanupPromise: Promise<void> | null = null;
  let cleanupError: unknown;

  function isCurrent(): boolean {
    const current = staleReason == null && freshness.isCurrent();
    if (!current && staleReason == null) {
      void markStale('contextChanged');
    }
    return current;
  }

  async function markStale(reason: ViewRuntimeOperationStaleReason): Promise<void> {
    if (staleReason == null) staleReason = reason;
    if (cleanupPromise) return cleanupPromise;
    cleanupPromise = Promise.resolve()
      .then(() => options.onStale?.(staleReason ?? reason))
      .catch((error) => {
        cleanupError = error;
      });
    return cleanupPromise;
  }

  function assertCurrent(): void {
    if (isCurrent()) return;
    void markStale(staleReason ?? 'contextChanged');
    throw new StaleViewRuntimeOperationError();
  }

  async function step<T>(task: () => Promise<T>): Promise<T> {
    assertCurrent();
    const value = await task();
    assertCurrent();
    return value;
  }

  async function staleResult<T>(): Promise<ViewRuntimeOperationResult<T>> {
    await markStale(staleReason ?? 'contextChanged');
    return {
      status: 'stale',
      reason: staleReason ?? 'contextChanged',
      ...(cleanupError === undefined ? {} : { cleanupError }),
    };
  }

  const operation: ViewRuntimeOperation = {
    isCurrent,
    step,
    async run<T>({ execute, land, handleError }): Promise<ViewRuntimeOperationResult<T>> {
      try {
        assertCurrent();
        const value = await execute(operation);
        assertCurrent();
        await land?.(value);
        assertCurrent();
        return { status: 'completed', value };
      } catch (error) {
        if (isStaleOperationError(error) || !isCurrent()) return staleResult<T>();
        await handleError?.(error);
        return { status: 'failed', error };
      }
    },
    async cancel(): Promise<void> {
      await markStale('cancelled');
    },
  };

  return operation;
}
