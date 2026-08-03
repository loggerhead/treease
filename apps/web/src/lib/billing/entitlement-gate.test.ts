import { describe, expect, it } from 'vitest';

import { usageBlockFor } from './entitlement-gate';
import type { UsageSummary } from '../services/treease-server';

const summary = (used: number): UsageSummary => ({
  tier: 'free',
  periodKey: '2026-07',
  limits: {
    graphViewDocumentsMonthly: { kind: 'limited', limit: 10 },
    largeFileProcessingRunsMonthly: { kind: 'limited', limit: 3 },
    aiProcessingMonthly: { kind: 'limited', limit: 0 },
    shareMaxAgeDays: 7,
  },
  usage: { graph_view: used },
});

describe('usage gate', () => {
  it('allows the action that reaches a monthly limit and blocks the next result', () => {
    expect(usageBlockFor(summary(9), 'graph_view')).toBeNull();
    expect(usageBlockFor(summary(10), 'graph_view')).toEqual({
      capability: 'graph_view',
      used: 10,
      limit: 10,
      tier: 'free',
    });
  });

  it('never blocks an unlimited entitlement', () => {
    const pro = summary(1000);
    pro.tier = 'pro';
    pro.limits.graphViewDocumentsMonthly = { kind: 'unlimited' };
    expect(usageBlockFor(pro, 'graph_view')).toBeNull();
  });
});
