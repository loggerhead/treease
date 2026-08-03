import { describe, expect, it } from 'vitest';

import { calculateUsageDelta } from './usage-queue';

describe('usage queue projection', () => {
  it('counts each pending logical event once', () => {
    expect(calculateUsageDelta([
      {
        eventId: 'a1', clientId: 'client', capability: 'graph_view',
        periodKey: '2026-07', idempotencyKey: 'document-a', quantity: 1,
        metadata: {}, createdAt: '', status: 'pending',
      },
      {
        eventId: 'a2', clientId: 'client', capability: 'graph_view',
        periodKey: '2026-07', idempotencyKey: 'document-a', quantity: 1,
        metadata: {}, createdAt: '', status: 'pending',
      },
      {
        eventId: 'b1', clientId: 'client', capability: 'large_file_processing',
        periodKey: '2026-07', idempotencyKey: 'operation-b', quantity: 1,
        metadata: {}, createdAt: '', status: 'pending',
      },
      {
        eventId: 'uploaded', clientId: 'client', capability: 'large_file_processing',
        periodKey: '2026-07', idempotencyKey: 'operation-c', quantity: 1,
        metadata: {}, createdAt: '', status: 'uploaded',
      },
    ], '2026-07')).toEqual({
      graph_view: 1,
      large_file_processing: 1,
    });
  });
});
