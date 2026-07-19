export type SubscriptionForPresentation = {
  tier: 'free' | 'pro';
  billingCadence: 'monthly' | 'yearly' | null;
  status: 'active' | 'inactive' | 'past_due' | 'canceled';
};

export type SubscriptionPresentation = {
  label: string;
  badge: string;
  cadence: string | null;
};

export function presentSubscription(subscription: SubscriptionForPresentation): SubscriptionPresentation {
  if (subscription.tier === 'pro') {
    switch (subscription.billingCadence) {
      case 'monthly':
      return { label: 'Pro', badge: 'PRO', cadence: 'Monthly' };
      case 'yearly':
      return { label: 'Pro', badge: 'PRO', cadence: 'Yearly' };
      default:
        return { label: 'Pro', badge: 'PRO', cadence: null };
    }
  }
  return { label: 'Free', badge: 'FREE', cadence: null };
}
