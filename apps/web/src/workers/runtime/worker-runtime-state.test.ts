// 职责：worker-runtime-state 的单元测试
import { describe, expect, it } from 'vitest';
import { clearWorkerRuntimeState, createWorkerRuntimeState } from './worker-runtime-state';

describe('worker-runtime-state', () => {
  it('creates runtime state and clears mutable worker state', () => {
    const state = createWorkerRuntimeState(new TextEncoder());
    state.searchIndexByDocumentKey.set('doc', { text: '', items: [], pathMap: undefined });

    clearWorkerRuntimeState(state);

    expect(state.searchIndexByDocumentKey.size).toBe(0);
  });
});
