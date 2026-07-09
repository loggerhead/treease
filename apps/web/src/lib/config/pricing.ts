export type PricingPlan = {
  id: string;
  name: string;
  eyebrow?: string;
  price: string;
  cadence: string;
  description: string;
  ctaLabel: string;
  ctaHref: string;
  featured?: boolean;
  features: string[];
};

export const pricingConfig = {
  title: 'A clearer way to work with structured data.',
  description:
    'Start with the essential tools for understanding your files. Upgrade when your workflow needs more room to explore, compare, and ship.',
  plans: [
    {
      id: 'free',
      name: 'Free',
      price: '$0',
      cadence: '/ month',
      description: 'Everything you need to make sense of a structured file.',
      ctaLabel: 'Start free',
      ctaHref: '/editor',
      features: [
        'Visualize JSON, YAML, TOML, and CSV',
        'Trace fields from graph to source',
        'Format, sort, compare, and export',
        'Local-first editing in the browser'
      ]
    },
    {
      id: 'pro-monthly',
      name: 'Pro',
      eyebrow: 'MOST POPULAR',
      price: '$8',
      cadence: '/ month',
      description: 'More space for the documents and decisions in your day-to-day work.',
      ctaLabel: 'Get started',
      ctaHref: '/editor',
      featured: true,
      features: [
        'Everything in Free',
        'Unlimited graph and table views',
        'Unlimited structural comparisons',
        'Advanced CLI workflows'
      ]
    },
    {
      id: 'pro-yearly',
      name: 'Pro yearly',
      eyebrow: 'SAVE 20%',
      price: '$6.40',
      cadence: '/ month',
      description: 'The full Pro workflow with a lower effective monthly price.',
      ctaLabel: 'Choose yearly',
      ctaHref: '/editor',
      features: [
        'Everything in Pro',
        'Billed annually at $76.80',
        'Unlimited graph and table views',
        'Priority access to new workflows'
      ]
    }
  ] satisfies PricingPlan[]
};
