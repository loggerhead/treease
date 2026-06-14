import { describe, expect, it } from 'vitest';
import { createGraphStateService } from './graph-state-service';

describe('graph-state-service', () => {
  it('reuses the same state per cache key', () => {
    const service = createGraphStateService();

    const first = service.ensureGraphState('a');
    const second = service.ensureGraphState('a');

    expect(second).toBe(first);
    expect(service.getGraphState('a')).toBe(first);
  });

  it('clears per-cache and global state', () => {
    const service = createGraphStateService();
    const a = service.ensureGraphState('a');
    const b = service.ensureGraphState('b');

    a.nodes.set(1, { id: 1 } as any);
    b.edges.set('e', { fromRenderHandle: 1, toRenderHandle: 2 } as any);

    service.clearGraphState('a');
    expect(service.getGraphState('a')).toBeUndefined();
    expect(a.nodes.size).toBe(0);

    service.clearAllGraphStates();
    expect(service.graphStateByDocumentKey.size).toBe(0);
    expect(b.edges.size).toBe(0);
  });
});
