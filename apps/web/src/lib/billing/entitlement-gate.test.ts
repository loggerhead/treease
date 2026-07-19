import { describe, expect, it } from 'vitest';

import { usageBlockFor } from './entitlement-gate';
import type { UsageSummary } from '../services/treease-server';

const summary = (used: number): UsageSummary => ({
  tier: 'free',
  periodKey: '2026-07',
  limits: {
    bidirectionalEditDocumentsMonthly: { kind: 'limited', limit: 10 },
    largeFileProcessingRunsMonthly: { kind: 'limited', limit: 3 },
    aiSuggestionsMonthly: { kind: 'limited', limit: 0 },
    shareMaxAgeDays: 7,
  },
  usage: { bidirectional_edit: used },
});

describe('usage gate', () => {
  it('allows the action that reaches a monthly limit and blocks the next result', () => {
    expect(usageBlockFor(summary(9), 'bidirectional_edit')).toBeNull();
    expect(usageBlockFor(summary(10), 'bidirectional_edit')).toEqual({
      capability: 'bidirectional_edit',
      limit: 10,
      tier: 'free',
    });
  });

  it('never blocks an unlimited entitlement', () => {
    const pro = summary(1000);
    pro.tier = 'pro';
    pro.limits.bidirectionalEditDocumentsMonthly = { kind: 'unlimited' };
    expect(usageBlockFor(pro, 'bidirectional_edit')).toBeNull();
  });
});
