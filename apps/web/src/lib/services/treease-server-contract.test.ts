import { describe, expect, it } from 'vitest';
import { usageSummarySchema } from '@treease/api-contracts';

describe('Treease API response contracts', () => {
  it('accepts additive usage fields while validating fields used by the web app', () => {
    const result = usageSummarySchema.safeParse({
      tier: 'free',
      periodKey: '2026-08',
      limits: {
        graphViewDocumentsMonthly: { kind: 'limited', limit: 10 },
        largeFileProcessingRunsMonthly: { kind: 'limited', limit: 3 },
        aiProcessingMonthly: { kind: 'limited', limit: 3 },
        shareMaxAgeDays: 7,
        futureLimit: { kind: 'unlimited' },
      },
      usage: {},
      futureField: 'ignored by this client',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data).not.toHaveProperty('futureField');
      expect(result.data.limits).not.toHaveProperty('futureLimit');
    }
  });

  it('still rejects invalid fields used by the web app', () => {
    const result = usageSummarySchema.safeParse({
      tier: 'free',
      periodKey: '2026-08',
      limits: {
        graphViewDocumentsMonthly: { kind: 'limited', limit: -1 },
        largeFileProcessingRunsMonthly: { kind: 'limited', limit: 3 },
        aiProcessingMonthly: { kind: 'limited', limit: 3 },
        shareMaxAgeDays: 7,
      },
      usage: {},
    });

    expect(result.success).toBe(false);
  });
});
