export type SubscriptionForPresentation = {
  plan: 'free' | 'monthly' | 'yearly';
  status: 'active' | 'inactive' | 'past_due' | 'canceled';
};

export type SubscriptionPresentation = {
  label: string;
  badge: string;
  cadence: string | null;
};

export function presentSubscription(subscription: SubscriptionForPresentation): SubscriptionPresentation {
  switch (subscription.plan) {
    case 'monthly':
      return { label: 'Pro', badge: 'PRO', cadence: '月付' };
    case 'yearly':
      return { label: 'Pro', badge: 'PRO', cadence: '年付' };
    case 'free':
    default:
      return { label: 'Free', badge: 'FREE', cadence: null };
  }
}
