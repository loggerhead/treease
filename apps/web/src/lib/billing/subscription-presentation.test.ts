import { describe, expect, it } from 'vitest';

import { presentSubscription } from './subscription-presentation';

describe('presentSubscription', () => {
  it.each([
    ['free', 'Free', 'FREE', null],
    ['monthly', 'Pro', 'PRO', '月付'],
    ['yearly', 'Pro', 'PRO', '年付'],
  ] as const)('presents %s with the customer-facing plan name', (plan, label, badge, cadence) => {
    expect(presentSubscription({ plan, status: 'active' })).toEqual({ label, badge, cadence });
  });
});
