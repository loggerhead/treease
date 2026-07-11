import { describe, expect, it, vi } from 'vitest';
import { createViewRuntimeOperation } from './view-runtime-operation';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

describe('View Runtime operation lifecycle', () => {
  it('does not start an operation that is already stale', async () => {
    let token = 2;
    const cleanup = vi.fn();
    const execute = vi.fn();
    const operation = createViewRuntimeOperation({
      captured: { token: 1 },
      getCurrent: () => ({ token }),
      onStale: cleanup,
    });

    await expect(operation.run({ execute })).resolves.toMatchObject({ status: 'stale', reason: 'contextChanged' });
    expect(execute).not.toHaveBeenCalled();
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it('drops a result and releases resources when it becomes stale after the first await', async () => {
    let revision = 1;
    const job = deferred<string>();
    const cancelJob = vi.fn();
    const land = vi.fn();
    const operation = createViewRuntimeOperation({
      captured: { documentKey: 'doc', revision },
      getCurrent: () => ({ documentKey: 'doc', revision }),
      onStale: cancelJob,
    });

    const pending = operation.run({
      execute: ({ step }) => step(() => job.promise),
      land,
    });
    revision = 2;
    job.resolve('old result');

    await expect(pending).resolves.toMatchObject({ status: 'stale', reason: 'contextChanged' });
    expect(cancelJob).toHaveBeenCalledTimes(1);
    expect(land).not.toHaveBeenCalled();
  });

  it('keeps one freshness rule across multiple await stages', async () => {
    let sessionId = 'session-a';
    const first = deferred<string>();
    const second = vi.fn(async () => 'second');
    const operation = createViewRuntimeOperation({
      captured: { sessionId },
      getCurrent: () => ({ sessionId }),
    });

    const pending = operation.run({
      execute: async ({ step }) => {
        await step(() => first.promise);
        return step(second);
      },
    });
    sessionId = 'session-b';
    first.resolve('first');

    await expect(pending).resolves.toMatchObject({ status: 'stale' });
    expect(second).not.toHaveBeenCalled();
  });

  it('runs stale cleanup at most once when cancel and context change race', async () => {
    let token = 1;
    const cleanup = vi.fn();
    const operation = createViewRuntimeOperation({
      captured: { token },
      getCurrent: () => ({ token }),
      onStale: cleanup,
    });

    await operation.cancel();
    token = 2;
    await operation.cancel();
    await expect(operation.run({ execute: async () => 'ignored' })).resolves.toMatchObject({
      status: 'stale',
      reason: 'cancelled',
    });
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it('does not let a stale operation error land in a newer UI operation', async () => {
    let token = 1;
    const oldFailure = deferred<void>();
    const oldError = vi.fn();
    const oldOperation = createViewRuntimeOperation({
      captured: { token },
      getCurrent: () => ({ token }),
    });
    const oldPending = oldOperation.run({
      execute: () => oldFailure.promise,
      handleError: oldError,
    });

    token = 2;
    oldFailure.reject(new Error('old failure'));
    await expect(oldPending).resolves.toMatchObject({ status: 'stale' });
    expect(oldError).not.toHaveBeenCalled();

    const newError = vi.fn();
    const newOperation = createViewRuntimeOperation({
      captured: { token },
      getCurrent: () => ({ token }),
    });
    await expect(newOperation.run({ execute: async () => { throw new Error('new failure'); }, handleError: newError }))
      .resolves.toMatchObject({ status: 'failed' });
    expect(newError).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['documentKey', { documentKey: 'a' }, { documentKey: 'b' }],
    ['revision', { revision: 1 }, { revision: 2 }],
    ['languageId', { languageId: 'json' }, { languageId: 'yaml' }],
    ['model replacement', { model: { getVersionId: () => 1 } }, { model: { getVersionId: () => 1 } }],
    ['sessionId', { sessionId: 'one' }, { sessionId: 'two' }],
  ])('invalidates a previous operation when %s changes', async (_name, captured, current) => {
    const operation = createViewRuntimeOperation({ captured, getCurrent: () => current });
    await expect(operation.run({ execute: async () => 'old' })).resolves.toMatchObject({ status: 'stale' });
  });
});
