import { describe, expect, it } from 'vitest';

import { annualSavingsPercent } from './pricing-display';

const monthly = { priceId: 'monthly' as const, amount: 1000, currency: 'USD', interval: 'month' as const, intervalCount: 1 };
const yearly = { priceId: 'yearly' as const, amount: 9600, currency: 'USD', interval: 'year' as const, intervalCount: 1 };

describe('annualSavingsPercent', () => {
  it('calculates the yearly saving from the live billing periods', () => {
    expect(annualSavingsPercent(monthly, yearly)).toBe(20);
  });

  it('hides the saving label when the yearly plan is not cheaper', () => {
    expect(annualSavingsPercent(monthly, { ...yearly, amount: 12000 })).toBeNull();
  });
});
