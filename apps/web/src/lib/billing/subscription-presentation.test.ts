import { describe, expect, it } from 'vitest';

import { presentSubscription } from './subscription-presentation';

describe('presentSubscription', () => {
  it.each([
    ['free', null, 'Free', 'FREE', null],
    ['pro', 'monthly', 'Pro', 'PRO', 'Monthly'],
    ['pro', 'yearly', 'Pro', 'PRO', 'Yearly'],
  ] as const)('presents %s with the customer-facing plan name', (tier, billingCadence, label, badge, cadence) => {
    expect(presentSubscription({ tier, billingCadence, status: 'active' })).toEqual({ label, badge, cadence });
  });
});
