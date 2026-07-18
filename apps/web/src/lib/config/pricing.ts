export type BillingPriceId = 'monthly' | 'yearly';

export type PricingFeature = {
  label: string;
  emphasis?: string;
  showCheck?: boolean;
};

export type PricingPlan = {
  id: string;
  name: string;
  eyebrow?: string;
  price?: string;
  cadence?: string;
  description: string;
  ctaLabel: string;
  ctaHref: string;
  billingPriceId?: BillingPriceId;
  featured?: boolean;
  features: PricingFeature[];
};

const proFeatures = [
  { label: 'Visualize structured documents' },
  { label: 'Trace fields from graph to source' },
  { label: 'Structural comparisons' },
  { label: 'Share link validity: up to 365 days', emphasis: 'up to 365 days' },
  { label: 'Bidirectional editing: unlimited', emphasis: 'unlimited' },
  { label: 'Large-file visualizations and processing: unlimited', emphasis: 'unlimited' },
  { label: 'AI features: included usage', emphasis: 'included usage' }
] satisfies PricingFeature[];

export const pricingConfig: { title: string; description: string; plans: PricingPlan[] } = {
  title: 'A clearer way to work with structured data.',
  description:
    'Start with the essential tools for understanding your files. Upgrade when your workflow needs more room to explore, compare, and ship.',
  plans: [
    {
      id: 'free',
      name: 'Free',
      eyebrow: 'ESSENTIALS',
      price: '$0',
      cadence: '/ month',
      description: 'Everything you need to make sense of a structured file.',
      ctaLabel: 'Start free',
      ctaHref: '/editor',
      features: [
        { label: 'Visualize structured documents' },
        { label: 'Trace fields from graph to source' },
        { label: 'Structural comparisons' },
        { label: 'Share link validity: up to 7 days', emphasis: 'up to 7 days' },
        { label: 'Bidirectional editing: up to 10 documents per month', emphasis: 'up to 10 documents per month' },
        {
          label: 'Large-file visualizations and processing: up to 3 runs per month',
          emphasis: 'up to 3 runs per month'
        },
        { label: 'AI features: not included', emphasis: 'not included', showCheck: false }
      ]
    },
    {
      id: 'pro-monthly',
      name: 'Pro',
      eyebrow: 'MOST POPULAR',
      description: 'More space for the documents and decisions in your day-to-day work.',
      ctaLabel: 'Get started',
      ctaHref: '/editor',
      billingPriceId: 'monthly',
      featured: true,
      features: proFeatures
    },
    {
      id: 'pro-yearly',
      name: 'Pro yearly',
      description: 'The full Pro workflow with a lower effective monthly price.',
      ctaLabel: 'Choose yearly',
      ctaHref: '/editor',
      billingPriceId: 'yearly',
      features: proFeatures
    }
  ] satisfies PricingPlan[]
};
