import type { BillingPlanPrice } from '../services/treease-server';

function annualAmount(price: BillingPlanPrice): number {
  switch (price.interval) {
    case 'day':
      return (price.amount * 365) / price.intervalCount;
    case 'week':
      return (price.amount * 52) / price.intervalCount;
    case 'month':
      return (price.amount * 12) / price.intervalCount;
    case 'year':
      return price.amount / price.intervalCount;
  }
}

export function annualSavingsPercent(
  monthly: BillingPlanPrice | undefined,
  yearly: BillingPlanPrice | undefined,
): number | null {
  if (!monthly || !yearly || monthly.currency !== yearly.currency) return null;
  const monthlyAnnualAmount = annualAmount(monthly);
  const yearlyAnnualAmount = annualAmount(yearly);
  if (yearlyAnnualAmount >= monthlyAnnualAmount) return null;
  return Math.round((1 - yearlyAnnualAmount / monthlyAnnualAmount) * 100);
}
